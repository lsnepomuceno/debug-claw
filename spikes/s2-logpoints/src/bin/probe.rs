//! Spike S2, T-11 and T-12: a scripted DBGp probe.
//!
//! Accepts one Xdebug session, sets one line breakpoint, and at the first
//! pause sends the commands of a script file one by one. For each command
//! it prints the response size in bytes, the time from write to the end of
//! the response, and either a decoded summary of the properties or the raw
//! XML. Then it removes the breakpoint and detaches, so the request ends
//! normally. Throwaway code, see NOTES.md.
//!
//!   probe --break FILE:LINE --script FILE [--hit N] [--lines N] [--listen ADDR] [--raw] [--save DIR]
//!
//! `--hit N` pauses only on the N-th visit of the line. `--lines N` caps
//! the property lines printed per response (40 by default).
//!
//! Script lines are DBGp commands without `-i`, which the probe adds.
//! `eval CODE` is sent as `eval -- base64(CODE)`. A line `@K command`
//! sends the command K times and prints the median, minimum and maximum
//! time. `#` starts a comment line.

#[path = "../xml.rs"]
mod xml;

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Instant;

use xml::Node;

/// Large enough for the unlimited reads of T-12.
const MAX_PACKET: usize = 512 << 20;

struct Args {
    listen: String,
    file: String,
    line: u32,
    script: String,
    raw: bool,
    save: Option<String>,
    hit: Option<u32>,
    lines: usize,
}

fn die(msg: &str) -> ! {
    eprintln!("probe: {msg}");
    std::process::exit(2);
}

fn parse_args() -> Args {
    let mut a = Args {
        listen: "0.0.0.0:9003".into(),
        file: String::new(),
        line: 0,
        script: String::new(),
        raw: false,
        save: None,
        hit: None,
        lines: 40,
    };
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let mut v = || it.next().unwrap_or_else(|| die(&format!("{k} needs a value")));
        match k.as_str() {
            "--listen" => a.listen = v(),
            "--break" => {
                let s = v();
                let (f, l) = s.rsplit_once(':').unwrap_or_else(|| die("--break FILE:LINE"));
                a.file = f.into();
                a.line = l.parse().unwrap_or_else(|_| die("bad line"));
            }
            "--script" => a.script = v(),
            "--raw" => a.raw = true,
            "--save" => a.save = Some(v()),
            "--lines" => a.lines = v().parse().unwrap_or_else(|_| die("bad --lines")),
            "--hit" => a.hit = Some(v().parse().unwrap_or_else(|_| die("bad --hit"))),
            _ => die(&format!("unknown argument {k}")),
        }
    }
    if a.file.is_empty() || a.script.is_empty() {
        die("--break and --script are required");
    }
    a
}

struct Conn {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    tid: u32,
}

impl Conn {
    /// Reads one `length NUL xml NUL` packet and returns it with its size.
    fn packet(&mut self) -> io::Result<String> {
        let mut len_buf = Vec::new();
        let n = self.reader.read_until(0, &mut len_buf)?;
        if n == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "closed"));
        }
        let len: usize = std::str::from_utf8(&len_buf[..n - 1])
            .ok()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| io::Error::other("bad length"))?;
        if len > MAX_PACKET {
            return Err(io::Error::other(format!("packet of {len} bytes over the cap")));
        }
        let mut buf = vec![0; len + 1];
        self.reader.read_exact(&mut buf)?;
        buf.truncate(len);
        Ok(String::from_utf8_lossy(&buf).into_owned())
    }

    /// Sends one command and reads packets up to its `<response>`.
    fn send(&mut self, line: &str) -> io::Result<(String, f64)> {
        self.tid += 1;
        let (cmd, rest) = line.split_once(' ').unwrap_or((line, ""));
        let mut out = if cmd == "eval" {
            format!("eval -i {} -- {}", self.tid, base64(rest.as_bytes()))
        } else if rest.is_empty() {
            format!("{cmd} -i {}", self.tid)
        } else {
            format!("{cmd} -i {} {rest}", self.tid)
        }
        .into_bytes();
        out.push(0);
        let t = Instant::now();
        self.writer.write_all(&out)?;
        loop {
            let text = self.packet()?;
            if text.contains("<response") {
                return Ok((text, t.elapsed().as_secs_f64() * 1e6));
            }
        }
    }
}

fn base64(data: &[u8]) -> String {
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

/// Counts the `<property` elements of a response (for the size tables).
fn count_props(node: &Node) -> usize {
    node.children
        .iter()
        .map(|c| usize::from(c.name == "property") + count_props(c))
        .sum()
}

fn value_of(p: &Node) -> String {
    let text = p.text.trim();
    if p.attr("encoding") == Some("base64") {
        match xml::base64_decode(text) {
            Some(b) => format!("{:?}", String::from_utf8_lossy(&b)),
            None => "<bad base64>".into(),
        }
    } else {
        text.to_string()
    }
}

/// Prints the property tree, at most `limit` lines.
fn summary(node: &Node, depth: usize, lines: &mut usize, limit: usize) {
    for p in node.children.iter().filter(|c| c.name == "property") {
        *lines += 1;
        if *lines > limit {
            return;
        }
        let mut meta = Vec::new();
        for k in ["classname", "facet", "numchildren", "page", "pagesize", "size"] {
            if let Some(v) = p.attr(k) {
                meta.push(format!("{k}={v}"));
            }
        }
        let name = p.attr("fullname").or(p.attr("name")).unwrap_or("?");
        let mut v = value_of(p);
        if v.len() > 120 {
            let total = v.len();
            v.truncate(v.floor_char_boundary(120));
            v.push_str(&format!("... ({total} chars received, quotes included)"));
        }
        println!(
            "  {:indent$}{name} type={} [{}]{}",
            "",
            p.attr("type").unwrap_or("?"),
            meta.join(" "),
            if v.is_empty() { String::new() } else { format!(" = {v}") },
            indent = depth * 2
        );
        summary(p, depth + 1, lines, limit);
    }
}

fn describe(text: &str, raw: bool, limit: usize) {
    if raw {
        println!("{text}");
        return;
    }
    let node = match xml::parse(text) {
        Ok(n) => n,
        Err(e) => {
            println!("  parse error: {e}");
            return;
        }
    };
    let ok = node.attr("success").map(|s| format!(" success={s}")).unwrap_or_default();
    println!("  props={}{ok}", count_props(&node));
    if let Some(err) = node.child("error") {
        let msg = err.child("message").map(|m| m.text.trim()).unwrap_or("");
        println!("  error code={} {msg}", err.attr("code").unwrap_or("?"));
    }
    let mut lines = 0;
    summary(&node, 0, &mut lines, limit);
    if lines > limit {
        println!("  ... more property lines");
    }
}

fn main() {
    let a = parse_args();
    let script = std::fs::read_to_string(&a.script).unwrap_or_else(|e| die(&format!("script: {e}")));
    let listener = TcpListener::bind(&a.listen).unwrap_or_else(|e| die(&format!("bind: {e}")));
    eprintln!("listening on {}", a.listen);
    let (stream, _) = listener.accept().unwrap_or_else(|e| die(&format!("accept: {e}")));
    stream.set_nodelay(true).ok();
    let mut c = Conn {
        writer: stream.try_clone().unwrap(),
        reader: BufReader::with_capacity(1 << 16, stream),
        tid: 0,
    };
    let init = c.packet().unwrap_or_else(|e| die(&format!("init: {e}")));
    let lang = init
        .split("language_version=\"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .unwrap_or("?");
    println!("init php={lang}");

    let mut bp = format!("breakpoint_set -t line -f file://{} -n {}", a.file, a.line);
    if let Some(h) = a.hit {
        bp.push_str(&format!(" -h {h} -o =="));
    }
    let (r, _) = c.send(&bp).unwrap();
    let bp_id = r
        .split(" id=\"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .unwrap_or_else(|| die(&format!("breakpoint_set: {r}")))
        .to_string();
    let (r, _) = c.send("run").unwrap();
    if !r.contains("status=\"break\"") {
        die(&format!("no break: {r}"));
    }
    println!("paused at {}:{}", a.file, a.line);

    for (n, line) in script.lines().map(str::trim).enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (reps, cmd) = match line.strip_prefix('@').and_then(|s| s.split_once(' ')) {
            Some((k, rest)) => (k.parse::<usize>().unwrap_or(1).max(1), rest),
            None => (1, line),
        };
        let mut times = Vec::with_capacity(reps);
        let mut last = String::new();
        for _ in 0..reps {
            match c.send(cmd) {
                Ok((text, us)) => {
                    times.push(us);
                    last = text;
                }
                Err(e) => die(&format!("{cmd}: {e}")),
            }
        }
        times.sort_by(f64::total_cmp);
        println!(
            "cmd {cmd}\n  bytes={} us_median={:.0} us_min={:.0} us_max={:.0} reps={reps}",
            last.len(),
            times[times.len() / 2],
            times[0],
            times[times.len() - 1]
        );
        describe(&last, a.raw, a.lines);
        if let Some(dir) = &a.save {
            let _ = std::fs::write(format!("{dir}/{:03}.xml", n + 1), &last);
        }
        let _ = io::stdout().flush();
    }

    let _ = c.send(&format!("breakpoint_remove -d {bp_id}"));
    let _ = c.send("detach");
    println!("detached");
}

#[cfg(test)]
mod tests {
    use super::base64;

    #[test]
    fn base64_pads() {
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"$i + 1"), "JGkgKyAx");
    }
}
