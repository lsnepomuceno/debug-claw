//! The minimum of DBGp the spike needs: framing, commands with
//! transaction IDs, a few XML attributes and base64.

use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Packets larger than this are refused. Xdebug's biggest replies in the
/// spike (source of a fixture file, a property tree) are a few KiB.
const MAX_PACKET: usize = 4 * 1024 * 1024;

pub struct Conn {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    next_id: u32,
    /// `<stream>` packets received while waiting for a response.
    pub streams: Vec<String>,
    /// Every packet sent and received, for the transcripts.
    pub log: Vec<String>,
}

impl Conn {
    pub fn new(stream: TcpStream) -> io::Result<Self> {
        let writer = stream.try_clone()?;
        Ok(Self {
            reader: BufReader::new(stream),
            writer,
            next_id: 1,
            streams: Vec::new(),
            log: Vec::new(),
        })
    }

    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.writer.set_read_timeout(timeout)
    }

    /// Reads one packet: ASCII length, NUL, XML, NUL.
    pub fn read_packet(&mut self) -> io::Result<String> {
        let mut len = Vec::new();
        self.reader.read_until(0, &mut len)?;
        if len.last() != Some(&0) {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "connection closed"));
        }
        len.pop();
        let len: usize = std::str::from_utf8(&len)
            .ok()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "bad length"))?;
        if len > MAX_PACKET {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "packet too large"));
        }
        let mut body = vec![0; len + 1];
        self.reader.read_exact(&mut body)?;
        body.pop();
        let xml = String::from_utf8_lossy(&body).into_owned();
        self.log.push(format!("<- {xml}"));
        Ok(xml)
    }

    /// Sends a command without waiting. Returns its transaction ID.
    pub fn send(&mut self, cmd: &str, args: &str, data: Option<&str>) -> io::Result<u32> {
        let id = self.next_id;
        self.next_id += 1;
        let mut line = format!("{cmd} -i {id}");
        if !args.is_empty() {
            line.push(' ');
            line.push_str(args);
        }
        if let Some(data) = data {
            line.push_str(" -- ");
            line.push_str(&base64_encode(data.as_bytes()));
        }
        self.log.push(format!("-> {line}"));
        line.push('\0');
        self.writer.write_all(line.as_bytes())?;
        Ok(id)
    }

    /// Waits for the response with this transaction ID. Stream packets
    /// that arrive first are kept in `streams`.
    pub fn wait(&mut self, id: u32) -> io::Result<String> {
        let want = id.to_string();
        loop {
            let xml = self.read_packet()?;
            if xml.contains("<stream") {
                self.streams.push(xml);
                continue;
            }
            if attrs(&xml, "response").and_then(|a| a.get("transaction_id").cloned()).as_deref()
                == Some(want.as_str())
            {
                return Ok(xml);
            }
        }
    }

    /// Sends a command and waits for its response.
    pub fn cmd(&mut self, cmd: &str, args: &str, data: Option<&str>) -> io::Result<String> {
        let id = self.send(cmd, args, data)?;
        self.wait(id)
    }
}

/// Attributes of the first `<tag ...>` element in `xml`.
pub fn attrs(xml: &str, tag: &str) -> Option<HashMap<String, String>> {
    all_attrs(xml, tag).into_iter().next()
}

/// Attributes of every `<tag ...>` element in `xml`, in order.
pub fn all_attrs(xml: &str, tag: &str) -> Vec<HashMap<String, String>> {
    let open = format!("<{tag}");
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(pos) = rest.find(&open) {
        let after = &rest[pos + open.len()..];
        // `<property` must not match `<propertyx`.
        if !after.starts_with(|c: char| c.is_whitespace() || c == '>' || c == '/') {
            rest = after;
            continue;
        }
        let end = after.find('>').unwrap_or(after.len());
        out.push(parse_attrs(&after[..end]));
        rest = &after[end..];
    }
    out
}

fn parse_attrs(s: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut rest = s;
    while let Some(eq) = rest.find("=\"") {
        let name = rest[..eq].trim().trim_start_matches('/').to_string();
        let after = &rest[eq + 2..];
        let Some(close) = after.find('"') else { break };
        map.insert(name, unescape(&after[..close]));
        rest = &after[close + 1..];
    }
    map
}

/// Text content of the first `<tag>` element, with CDATA unwrapped.
pub fn text(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let start = xml.find(&open)?;
    let after = &xml[start..];
    let gt = after.find('>')?;
    if after[..gt].ends_with('/') {
        return Some(String::new());
    }
    let body = &after[gt + 1..];
    let close = body.find(&format!("</{tag}>"))?;
    let inner = &body[..close];
    let inner = inner
        .strip_prefix("<![CDATA[")
        .and_then(|s| s.strip_suffix("]]>"))
        .unwrap_or(inner);
    Some(unescape(inner))
}

pub fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(B64[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub fn base64_decode(s: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut acc = 0u32;
    let mut bits = 0;
    for c in s.bytes() {
        let Some(v) = B64.iter().position(|&b| b == c) else { continue };
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trip() {
        for s in ["", "a", "ab", "abc", "$order->total", "file:///C:/x y.php"] {
            assert_eq!(base64_decode(&base64_encode(s.as_bytes())), s.as_bytes());
        }
        assert_eq!(base64_encode(b"ab"), "YWI=");
    }

    #[test]
    fn reads_attributes_and_text() {
        let xml = r#"<response xmlns="urn:debugger_protocol_v1" command="source" transaction_id="3" encoding="base64"><![CDATA[PD9waHA=]]></response>"#;
        let a = attrs(xml, "response").unwrap();
        assert_eq!(a["transaction_id"], "3");
        assert_eq!(base64_decode(&text(xml, "response").unwrap()), b"<?php");
    }
}
