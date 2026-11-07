//! Spike S6: an IDE emulator. Listens like PhpStorm does, accepts a DBGp
//! session (from Xdebug directly or from the proxy) and sends roughly the
//! commands PhpStorm sends. The sequence is an approximation from public
//! Xdebug logs of PhpStorm sessions, not a capture.
//!
//!   ide_emu [--listen ADDR] [--sessions N] [--map LOCAL=REMOTE]
//!           [--break LOCAL_FILE:LINE] [--transcript FILE] [--hold-ms N]
//!           [--bench K]
//!
//! The breakpoint is given as a local path and mapped to the remote one
//! with the emulator's own mapping, as an IDE does. At every pause on the
//! breakpoint line it checks `$i`, `$sum` and `$item` against what `loop()`
//! computes. `--hold-ms N` keeps each pause N ms, then reads `$sum` and
//! the stack again and checks they did not change. `--bench K` sends K
//! rounds of three reads at the first pause and prints the round trip
//! times. The transcript has every command and every packet, with the
//! values that change from run to run blanked, so two runs can be diffed.

#[path = "../client.rs"]
mod client;
#[path = "../dbgp.rs"]
mod dbgp;
#[path = "../xml.rs"]
mod xml;

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::net::TcpListener;
use std::time::Duration;

use client::{Conn, Reply};

struct Args {
    listen: String,
    sessions: u32,
    local: String,
    remote: String,
    file: String,
    line: u32,
    transcript: Option<String>,
    hold_ms: u64,
    bench: usize,
}

fn die(msg: &str) -> ! {
    eprintln!("ide_emu: {msg}");
    std::process::exit(2);
}

fn parse_args() -> Args {
    let mut a = Args {
        listen: "0.0.0.0:9010".into(),
        sessions: 1,
        local: "/work/spikes/testbed/app".into(),
        remote: "/var/www/html".into(),
        file: "/work/spikes/testbed/app/public/index.php".into(),
        line: 44,
        transcript: None,
        hold_ms: 0,
        bench: 0,
    };
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let mut v = || it.next().unwrap_or_else(|| die(&format!("{k} needs a value")));
        match k.as_str() {
            "--listen" => a.listen = v(),
            "--sessions" => a.sessions = v().parse().unwrap_or_else(|_| die("bad --sessions")),
            "--map" => {
                let s = v();
                let (l, r) = s.split_once('=').unwrap_or_else(|| die("--map LOCAL=REMOTE"));
                a.local = l.into();
                a.remote = r.into();
            }
            "--break" => {
                let s = v();
                let (f, l) = s.rsplit_once(':').unwrap_or_else(|| die("--break FILE:LINE"));
                a.file = f.into();
                a.line = l.parse().unwrap_or_else(|_| die("bad line"));
            }
            "--transcript" => a.transcript = Some(v()),
            "--hold-ms" => a.hold_ms = v().parse().unwrap_or_else(|_| die("bad --hold-ms")),
            "--bench" => a.bench = v().parse().unwrap_or_else(|_| die("bad --bench")),
            _ => die(&format!("unknown argument {k}")),
        }
    }
    a
}

/// Blanks the attribute values that differ between two runs of the same
/// request: the process ID and control socket name in `init`, memory
/// addresses of values, and breakpoint IDs (Xdebug numbers them per
/// worker process).
fn normalize(x: &str) -> String {
    let mut out = String::with_capacity(x.len());
    for (n, part) in x.split('<').enumerate() {
        if n > 0 {
            out.push('<');
        }
        let (tag, rest) = part.split_at(part.find('>').unwrap_or(part.len()));
        let mut keys = vec![" appid=\"", " xdebug:ctrl_socket=\"", " address=\""];
        if tag.starts_with("breakpoint ") || tag.contains("command=\"breakpoint_") {
            keys.push(" id=\"");
        }
        out.push_str(&blank(tag, &keys));
        out.push_str(rest);
    }
    out
}

fn blank(tag: &str, keys: &[&str]) -> String {
    let mut out = String::with_capacity(tag.len());
    let mut rest = tag;
    'outer: while !rest.is_empty() {
        for key in keys {
            if rest.starts_with(key) {
                out.push_str(key);
                out.push('*');
                let after = &rest[key.len()..];
                rest = &after[after.find('"').unwrap_or(after.len())..];
                continue 'outer;
            }
        }
        let c = rest.chars().next().unwrap();
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

struct Emu<'a> {
    a: &'a Args,
    c: Conn,
    t: Option<BufWriter<File>>,
    pauses: u32,
    checks_ok: u32,
    checks_bad: u32,
    streams: u32,
    notifies: u32,
}

impl Emu<'_> {
    fn log(&mut self, s: &str) {
        if let Some(t) = &mut self.t {
            let _ = writeln!(t, "{}", normalize(s).replace('\n', " "));
        }
    }

    fn send(&mut self, cmd: &str) -> io::Result<Reply> {
        let (line, r) = self.c.send(cmd)?;
        self.log(&format!("> {line}"));
        for o in &r.other {
            if o.contains("<stream") {
                self.streams += 1;
            } else if o.contains("<notify") {
                self.notifies += 1;
            }
        }
        for o in r.other.clone() {
            self.log(&format!("< {o}"));
        }
        self.log(&format!("< {}", r.xml));
        Ok(r)
    }

    fn remote(&self, local: &str) -> String {
        match local.strip_prefix(&self.a.local) {
            Some(rest) => format!("file://{}{rest}", self.a.remote),
            None => format!("file://{local}"),
        }
    }

    fn local(&self, uri: &str) -> String {
        let p = uri.strip_prefix("file://").unwrap_or(uri);
        match p.strip_prefix(&self.a.remote) {
            Some(rest) => format!("{}{rest}", self.a.local),
            None => p.to_string(),
        }
    }

    /// The reads PhpStorm sends when a session pauses, plus value checks.
    fn on_pause(&mut self) -> io::Result<()> {
        self.pauses += 1;
        let st = self.send("stack_get")?;
        let (uri, line) = client::top_frame(&st.xml).unwrap_or_default();
        self.send("context_names -d 0")?;
        let ctx = self.send("context_get -d 0 -c 0")?;
        let item = self.send("property_get -d 0 -c 0 -n $item")?;
        let local = self.local(&uri);
        let mut note = String::new();
        if line == self.a.line && local == self.a.file {
            let ctx = xml::parse(&ctx.xml).map_err(io::Error::other)?;
            let item = xml::parse(&item.xml).map_err(io::Error::other)?;
            let i = client::int_of(&ctx, "$i");
            let sum = client::int_of(&ctx, "$sum");
            let sq = client::int_of(&item, "square");
            let idx = client::int_of(&item, "index");
            let ok = match (i, sum, sq, idx) {
                (Some(i), Some(s), Some(q), Some(x)) => s == client::sum_before(i) && q == i * i && x == i,
                _ => false,
            };
            if ok {
                self.checks_ok += 1;
            } else {
                self.checks_bad += 1;
            }
            note = format!(" i={i:?} sum={sum:?} square={sq:?} check={}", if ok { "ok" } else { "BAD" });
        }
        println!("pause {} at {local}:{line}{note}", self.pauses);
        if self.a.hold_ms > 0 {
            std::thread::sleep(Duration::from_millis(self.a.hold_ms));
            let again = self.send("stack_get")?;
            let same_stack = body(&again.xml) == body(&st.xml);
            let s1 = self.send("property_get -d 0 -c 0 -n $sum")?;
            let ctx = xml::parse(&s1.xml).map_err(io::Error::other)?;
            let sum = client::int_of(&ctx, "$sum");
            println!("  after hold: same top frames={same_stack} sum={sum:?}");
            if !same_stack {
                self.checks_bad += 1;
            }
        }
        if self.a.bench > 0 && self.pauses == 1 {
            self.bench()?;
        }
        Ok(())
    }

    fn bench(&mut self) -> io::Result<()> {
        let cmds = ["stack_get", "context_get -d 0 -c 0", "property_get -d 0 -c 0 -n $item"];
        let mut per: Vec<Vec<f64>> = vec![Vec::with_capacity(self.a.bench); cmds.len()];
        let mut all = Vec::with_capacity(self.a.bench * cmds.len());
        for _ in 0..self.a.bench {
            for (k, cmd) in cmds.iter().enumerate() {
                // Not in the transcript: only the times matter here.
                let (_, r) = self.c.send(cmd)?;
                per[k].push(r.us);
                all.push(r.us);
            }
        }
        for (k, cmd) in cmds.iter().enumerate() {
            println!("bench {}", client::summary(cmd, std::mem::take(&mut per[k])));
        }
        println!("bench {}", client::summary("all", all));
        Ok(())
    }

    fn run(&mut self) -> io::Result<()> {
        let init = self.c.packet()?;
        self.log(&format!("< {init}"));
        let fileuri = init.split("fileuri=\"").nth(1).and_then(|s| s.split('"').next()).unwrap_or("?").to_string();
        println!("init {} (local {})", fileuri, self.local(&fileuri));

        // PhpStorm finds its server configuration from these values.
        for v in ["PHP_IDE_CONFIG", "SERVER_NAME", "SERVER_PORT", "REQUEST_URI"] {
            self.send(&format!("eval isset($_SERVER['{v}']) ? $_SERVER['{v}'] : null"))?;
        }
        for (f, v) in [
            ("show_hidden", "1"),
            ("max_depth", "1"),
            ("max_children", "100"),
            ("max_data", "1024"),
            ("extended_properties", "1"),
            ("notify_ok", "1"),
            ("resolved_breakpoints", "1"),
            ("breakpoint_details", "1"),
        ] {
            self.send(&format!("feature_set -n {f} -v {v}"))?;
        }
        self.send("stdout -c 1")?;
        let target = self.remote(&self.a.file);
        let bp = self.send(&format!("breakpoint_set -t line -f {target} -n {}", self.a.line))?;
        println!("breakpoint {}:{} -> {target}: {}", self.a.file, self.a.line, short(&bp.xml));

        let mut r = self.send("run")?;
        let mut stepped = false;
        loop {
            let status = attr(&r.xml, "status").unwrap_or_default();
            match status.as_str() {
                "break" => {
                    self.on_pause()?;
                    r = if !stepped {
                        stepped = true;
                        self.send("step_over")?
                    } else {
                        self.send("run")?
                    };
                }
                "stopping" => {
                    let s = self.send("stop")?;
                    println!("stopping, stop -> {}", attr(&s.xml, "status").unwrap_or_default());
                    break;
                }
                other => {
                    println!("status {other}");
                    break;
                }
            }
        }
        Ok(())
    }
}

fn attr(x: &str, k: &str) -> Option<String> {
    let h = dbgp::head(x.as_bytes());
    dbgp::attr(x.as_bytes(), &h, k).map(str::to_string)
}

/// The packet after the head tag, which holds the transaction ID.
fn body(x: &str) -> String {
    let h = dbgp::head(x.as_bytes());
    normalize(&x[h.tag.end..])
}

fn short(x: &str) -> String {
    let h = dbgp::head(x.as_bytes());
    x[h.tag.clone()].chars().take(200).collect()
}

fn main() {
    let a = parse_args();
    let mut t = a.transcript.as_ref().map(|p| BufWriter::new(File::create(p).unwrap_or_else(|e| die(&format!("{p}: {e}")))));
    let listener = TcpListener::bind(&a.listen).unwrap_or_else(|e| die(&format!("bind {}: {e}", a.listen)));
    eprintln!("listening on {}", a.listen);
    let mut total_bad = 0;
    for n in 1..=a.sessions {
        let (s, peer) = listener.accept().unwrap_or_else(|e| die(&format!("accept: {e}")));
        println!("session {n} from {peer}");
        if let Some(t) = &mut t {
            let _ = writeln!(t, "# session {n}");
        }
        let mut e = Emu { a: &a, c: Conn::new(s), t: t.take(), pauses: 0, checks_ok: 0, checks_bad: 0, streams: 0, notifies: 0 };
        let res = e.run();
        println!(
            "session {n}: {} pauses, value checks ok={} bad={}, tid mismatches={}, stream packets={}, notify packets={}, result={:?}",
            e.pauses, e.checks_ok, e.checks_bad, e.c.mismatches, e.streams, e.notifies, res.as_ref().err()
        );
        total_bad += e.checks_bad + e.c.mismatches + u32::from(res.is_err());
        t = e.t.take();
        if let Some(t) = &mut t {
            let _ = t.flush();
        }
    }
    if total_bad > 0 {
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::normalize;

    #[test]
    fn blanks_volatile_attributes() {
        assert_eq!(normalize(r#"<init appid="123" x="1"><p address="140" a="2"/>"#), r#"<init appid="*" x="1"><p address="*" a="2"/>"#);
        assert_eq!(
            normalize(r#"<response command="breakpoint_set" id="70001"/><context id="0"/><breakpoint type="line" id="7"/>"#),
            r#"<response command="breakpoint_set" id="*"/><context id="0"/><breakpoint type="line" id="*"/>"#
        );
    }
}
