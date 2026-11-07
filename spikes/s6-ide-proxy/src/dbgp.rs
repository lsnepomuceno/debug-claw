//! DBGp framing and the small parsing the proxy needs: the command line
//! the IDE sends (`name -i TID args -- data`, NUL-terminated) and the head
//! of the packets Xdebug sends (`length NUL xml NUL`). Throwaway code.

#![allow(dead_code)] // each binary uses part of this module

use std::io::{self, BufRead, Read, Write};
use std::ops::Range;

/// Cap on one Xdebug packet. The IDE's `max_*` features bound what Xdebug
/// sends; this only stops a broken peer from exhausting memory.
pub const MAX_PACKET: usize = 64 << 20;
/// Cap on one IDE command. Breakpoint conditions and eval code travel
/// base64-encoded after `--`, so commands can be long, but not this long.
pub const MAX_COMMAND: usize = 1 << 20;

fn bad(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

/// Reads one Xdebug packet and returns its XML, or None at a clean EOF.
pub fn read_packet<R: BufRead>(r: &mut R) -> io::Result<Option<Vec<u8>>> {
    let mut len_buf = Vec::with_capacity(16);
    let n = (&mut *r).take(24).read_until(0, &mut len_buf)?;
    if n == 0 {
        return Ok(None);
    }
    if len_buf.pop() != Some(0) {
        return Err(bad("bad length prefix"));
    }
    let len: usize = std::str::from_utf8(&len_buf)
        .ok()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| bad("bad length prefix"))?;
    if len > MAX_PACKET {
        return Err(bad(format!("packet of {len} bytes over the cap")));
    }
    let mut buf = vec![0; len + 1];
    r.read_exact(&mut buf)?;
    if buf.pop() != Some(0) {
        return Err(bad("packet not NUL-terminated"));
    }
    Ok(Some(buf))
}

/// Reads one IDE command (bytes up to NUL), or None at a clean EOF.
pub fn read_command<R: BufRead>(r: &mut R) -> io::Result<Option<Vec<u8>>> {
    let mut buf = Vec::new();
    let n = (&mut *r).take(MAX_COMMAND as u64 + 1).read_until(0, &mut buf)?;
    if n == 0 {
        return Ok(None);
    }
    if buf.pop() != Some(0) {
        return Err(bad(if n > MAX_COMMAND { "command over the cap" } else { "EOF inside a command" }));
    }
    Ok(Some(buf))
}

/// Writes one Xdebug-style packet in a single flush.
pub fn write_packet<W: Write>(w: &mut W, xml: &[u8]) -> io::Result<()> {
    write!(w, "{}\0", xml.len())?;
    w.write_all(xml)?;
    w.write_all(b"\0")?;
    w.flush()
}

/// Writes an XML packet assembled from three slices, so a rewritten
/// transaction ID costs no copy of the body.
pub fn write_packet_parts<W: Write>(w: &mut W, parts: [&[u8]; 3]) -> io::Result<()> {
    let len: usize = parts.iter().map(|p| p.len()).sum();
    write!(w, "{len}\0")?;
    for p in parts {
        w.write_all(p)?;
    }
    w.write_all(b"\0")?;
    w.flush()
}

/// A parsed command line: its name and where its `-i` value sits.
#[derive(Debug)]
pub struct Command {
    pub name: String,
    /// Byte range of the `-i` value, if the command has one.
    pub tid: Option<Range<usize>>,
}

/// Splits a command line into tokens up to `--`, honouring double quotes
/// with backslash escapes, and returns each token's byte range.
fn tokens(line: &[u8]) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < line.len() {
        while i < line.len() && line[i] == b' ' {
            i += 1;
        }
        if i >= line.len() {
            break;
        }
        let start = i;
        if line[i] == b'"' {
            i += 1;
            while i < line.len() && line[i] != b'"' {
                if line[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i = (i + 1).min(line.len());
        } else {
            while i < line.len() && line[i] != b' ' {
                i += 1;
            }
        }
        if &line[start..i] == b"--" {
            break;
        }
        out.push(start..i);
    }
    out
}

pub fn parse_command(line: &[u8]) -> Command {
    let toks = tokens(line);
    let name = toks
        .first()
        .map(|r| String::from_utf8_lossy(&line[r.clone()]).into_owned())
        .unwrap_or_default();
    // Options come in pairs after the name: `-x value`. Walk the pairs so
    // a value that happens to read "-i" is not taken for the option.
    let mut tid = None;
    let mut k = 1;
    while k < toks.len() {
        let t = &line[toks[k].clone()];
        if t == b"-i" {
            tid = toks.get(k + 1).cloned();
            break;
        }
        // Flags without a value do not exist in the commands Xdebug
        // knows, so every option takes the next token.
        k += 2;
    }
    Command { name, tid }
}

/// Returns the line with its `-i` value replaced (or added after the name).
pub fn with_tid(line: &[u8], cmd: &Command, tid: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(line.len() + 8);
    match &cmd.tid {
        Some(r) => {
            out.extend_from_slice(&line[..r.start]);
            out.extend_from_slice(tid.as_bytes());
            out.extend_from_slice(&line[r.end..]);
        }
        None => {
            let name_end = cmd.name.len().min(line.len());
            out.extend_from_slice(&line[..name_end]);
            out.extend_from_slice(b" -i ");
            out.extend_from_slice(tid.as_bytes());
            out.extend_from_slice(&line[name_end..]);
        }
    }
    out
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Kind {
    Init,
    Response,
    Notify,
    Stream,
    Other,
}

/// The first element of a packet, read without parsing the body.
#[derive(Debug)]
pub struct Head {
    pub kind: Kind,
    /// Byte range of the start tag of the first element.
    pub tag: Range<usize>,
}

pub fn head(xml: &[u8]) -> Head {
    let mut i = 0;
    // Skip the `<?xml ...?>` prolog and any whitespace.
    loop {
        while i < xml.len() && xml[i] != b'<' {
            i += 1;
        }
        if xml[i..].starts_with(b"<?") || xml[i..].starts_with(b"<!") {
            while i < xml.len() && xml[i] != b'>' {
                i += 1;
            }
            continue;
        }
        break;
    }
    let start = i;
    while i < xml.len() && xml[i] != b'>' {
        i += 1;
    }
    let tag = start..i.min(xml.len());
    let t = &xml[tag.clone()];
    let kind = if t.starts_with(b"<response") {
        Kind::Response
    } else if t.starts_with(b"<notify") {
        Kind::Notify
    } else if t.starts_with(b"<stream") {
        Kind::Stream
    } else if t.starts_with(b"<init") {
        Kind::Init
    } else {
        Kind::Other
    };
    Head { kind, tag }
}

/// Byte range (in `xml`) of the value of attribute `name` in the head tag.
pub fn attr_range(xml: &[u8], head: &Head, name: &str) -> Option<Range<usize>> {
    let t = &xml[head.tag.clone()];
    let pat = format!(" {name}=\"");
    let pos = t.windows(pat.len()).position(|w| w == pat.as_bytes())?;
    let vstart = pos + pat.len();
    let vlen = t[vstart..].iter().position(|&c| c == b'"')?;
    Some(head.tag.start + vstart..head.tag.start + vstart + vlen)
}

pub fn attr<'a>(xml: &'a [u8], head: &Head, name: &str) -> Option<&'a str> {
    attr_range(xml, head, name).and_then(|r| std::str::from_utf8(&xml[r]).ok())
}

pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in data.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= c.len() {
                s.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

/// Percentile of a sorted slice, nearest rank.
pub fn pct(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let rank = ((p / 100.0) * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_tid_and_rewrites_it() {
        let line = b"property_get -i 12 -n $x -d 0";
        let c = parse_command(line);
        assert_eq!(c.name, "property_get");
        assert_eq!(&line[c.tid.clone().unwrap()], b"12");
        assert_eq!(with_tid(line, &c, "900"), b"property_get -i 900 -n $x -d 0");
    }

    #[test]
    fn ignores_i_inside_values_and_data() {
        // `-n -i` names a property called "-i"; the data after -- is opaque.
        let line = b"property_get -n \"-i x\" -i 7 -- LWkgOQ==";
        let c = parse_command(line);
        assert_eq!(&line[c.tid.clone().unwrap()], b"7");
        let line = b"eval -i 3 -- LWkgOQ==";
        assert_eq!(&line[parse_command(line).tid.unwrap()], b"3");
        let line = b"property_get -n -i -i 5";
        assert_eq!(&line[parse_command(line).tid.unwrap()], b"5");
    }

    #[test]
    fn adds_tid_when_missing() {
        let line = b"stack_get";
        let c = parse_command(line);
        assert!(c.tid.is_none());
        assert_eq!(with_tid(line, &c, "4"), b"stack_get -i 4");
        let line = b"stack_get -d 0";
        assert_eq!(with_tid(line, &parse_command(line), "4"), b"stack_get -i 4 -d 0");
    }

    #[test]
    fn reads_head_attributes() {
        let xml = br#"<?xml version="1.0" encoding="iso-8859-1"?>
<response xmlns="urn:debugger_protocol_v1" xmlns:xdebug="https://xdebug.org/dbgp/xdebug" command="run" transaction_id="31" status="break" reason="ok"><xdebug:message filename="file:///a.php" lineno="4"></xdebug:message></response>"#;
        let h = head(xml);
        assert_eq!(h.kind, Kind::Response);
        assert_eq!(attr(xml, &h, "transaction_id"), Some("31"));
        assert_eq!(attr(xml, &h, "status"), Some("break"));
        assert_eq!(attr(xml, &h, "command"), Some("run"));
        assert_eq!(attr(xml, &h, "lineno"), None);
        let n = br#"<?xml version="1.0"?><notify xmlns="urn:debugger_protocol_v1" name="breakpoint_resolved"></notify>"#;
        let h = head(n);
        assert_eq!(h.kind, Kind::Notify);
        assert_eq!(attr(n, &h, "transaction_id"), None);
    }

    #[test]
    fn frames_round_trip() {
        let mut out = Vec::new();
        write_packet_parts(&mut out, [b"<a x=\"", b"5", b"\"/>"]).unwrap();
        assert_eq!(out, b"10\0<a x=\"5\"/>\0");
        let mut r = io::BufReader::new(&out[..]);
        assert_eq!(read_packet(&mut r).unwrap().unwrap(), b"<a x=\"5\"/>");
        assert!(read_packet(&mut r).unwrap().is_none());
        let mut r = io::BufReader::new(&b"run -i 1\0stop -i 2\0"[..]);
        assert_eq!(read_command(&mut r).unwrap().unwrap(), b"run -i 1");
        assert_eq!(read_command(&mut r).unwrap().unwrap(), b"stop -i 2");
        assert!(read_command(&mut r).unwrap().is_none());
    }

    #[test]
    fn percentiles() {
        let v: Vec<f64> = (1..=100).map(f64::from).collect();
        assert_eq!(pct(&v, 50.0), 50.0);
        assert_eq!(pct(&v, 99.0), 99.0);
        assert_eq!(pct(&v, 100.0), 100.0);
    }
}
