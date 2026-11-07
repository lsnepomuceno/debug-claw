//! A minimal DBGp client connection, shared by the IDE emulator and the
//! agent: sends `name -i N args`, reads packets up to the matching
//! response, checks that its transaction ID and command match.

#![allow(dead_code)] // each binary uses part of this module

use std::io::{self, BufReader, BufWriter, Write};
use std::net::TcpStream;
use std::time::Instant;

use crate::dbgp;
use crate::xml::{self, Node};

pub struct Conn {
    pub r: BufReader<TcpStream>,
    pub w: BufWriter<TcpStream>,
    pub tid: u32,
    /// Responses whose transaction ID or command did not match the request.
    pub mismatches: u32,
}

pub struct Reply {
    pub xml: String,
    /// Packets that arrived before the response (stream, notify).
    pub other: Vec<String>,
    /// Microseconds from the write to the end of the response.
    pub us: f64,
}

impl Conn {
    pub fn new(s: TcpStream) -> Conn {
        let _ = s.set_nodelay(true);
        Conn {
            w: BufWriter::with_capacity(1 << 16, s.try_clone().expect("clone")),
            r: BufReader::with_capacity(1 << 16, s),
            tid: 0,
            mismatches: 0,
        }
    }

    pub fn packet(&mut self) -> io::Result<String> {
        match dbgp::read_packet(&mut self.r)? {
            Some(p) => Ok(String::from_utf8_lossy(&p).into_owned()),
            None => Err(io::Error::new(io::ErrorKind::UnexpectedEof, "closed")),
        }
    }

    /// Builds `name -i N rest`; `eval CODE` becomes `eval -i N -- base64`.
    pub fn line(&mut self, cmd: &str) -> (String, String) {
        self.tid += 1;
        let (name, rest) = cmd.split_once(' ').unwrap_or((cmd, ""));
        let line = if name == "eval" {
            format!("eval -i {} -- {}", self.tid, dbgp::base64(rest.as_bytes()))
        } else if rest.is_empty() {
            format!("{name} -i {}", self.tid)
        } else {
            format!("{name} -i {} {rest}", self.tid)
        };
        (name.to_string(), line)
    }

    pub fn send(&mut self, cmd: &str) -> io::Result<(String, Reply)> {
        let (name, line) = self.line(cmd);
        let t = Instant::now();
        self.w.write_all(line.as_bytes())?;
        self.w.write_all(b"\0")?;
        self.w.flush()?;
        let mut other = Vec::new();
        loop {
            let text = self.packet()?;
            let h = dbgp::head(text.as_bytes());
            if h.kind != dbgp::Kind::Response {
                other.push(text);
                continue;
            }
            let us = t.elapsed().as_secs_f64() * 1e6;
            let tid = dbgp::attr(text.as_bytes(), &h, "transaction_id").unwrap_or("");
            let command = dbgp::attr(text.as_bytes(), &h, "command").unwrap_or("");
            if tid != self.tid.to_string() || command != name {
                self.mismatches += 1;
                eprintln!("MISMATCH: sent {line:?}, got transaction_id={tid} command={command}");
            }
            return Ok((line, Reply { xml: text, other, us }));
        }
    }
}

/// A property's field, from the attribute or from the child element that
/// `extended_properties` uses, decoded if base64.
pub fn field(p: &Node, key: &str) -> Option<String> {
    if let Some(v) = p.attr(key) {
        return Some(v.to_string());
    }
    let c = p.child(key)?;
    Some(decode(c.attr("encoding"), &c.text))
}

fn decode(enc: Option<&str>, text: &str) -> String {
    if enc == Some("base64") {
        xml::base64_decode(text.trim())
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_else(|| "<bad base64>".into())
    } else {
        text.trim().to_string()
    }
}

pub fn value(p: &Node) -> String {
    match p.child("value") {
        Some(v) => decode(v.attr("encoding"), &v.text),
        None => decode(p.attr("encoding"), &p.text),
    }
}

/// The first property (depth first) whose name is `name`.
pub fn find<'a>(n: &'a Node, name: &str) -> Option<&'a Node> {
    for c in n.children.iter().filter(|c| c.name == "property") {
        if field(c, "name").as_deref() == Some(name) {
            return Some(c);
        }
        if let Some(f) = find(c, name) {
            return Some(f);
        }
    }
    None
}

pub fn int_of(n: &Node, name: &str) -> Option<i64> {
    find(n, name).and_then(|p| value(p).parse().ok())
}

/// The top frame of a `stack_get` response: (file URI, line).
pub fn top_frame(resp: &str) -> Option<(String, u32)> {
    let n = xml::parse(resp).ok()?;
    let s = n.child("stack")?;
    Some((s.attr("filename")?.to_string(), s.attr("lineno")?.parse().ok()?))
}

/// What `loop()` holds at `$sum += $item['square'];` with `$i` = i.
pub fn sum_before(i: i64) -> i64 {
    (0..i).map(|k| k * k).sum()
}

pub fn summary(name: &str, mut v: Vec<f64>) -> String {
    v.sort_by(f64::total_cmp);
    format!(
        "{name}: n={} us median={:.1} p99={:.1} min={:.1} max={:.1}",
        v.len(),
        dbgp::pct(&v, 50.0),
        dbgp::pct(&v, 99.0),
        v.first().copied().unwrap_or(f64::NAN),
        v.last().copied().unwrap_or(f64::NAN)
    )
}
