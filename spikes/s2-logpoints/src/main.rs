//! Spike S2: a logpoint built on a DBGp line breakpoint.
//!
//! Listens for Xdebug, sets one line breakpoint in the fixture's loop and,
//! on every pause, reads a few expressions with `property_get`, stores them
//! in a bounded capture buffer and sends `run` at once. Prints timing,
//! verification and memory figures. Throwaway code, see NOTES.md.

mod xml;

use std::collections::VecDeque;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};

use xml::Node;

/// Largest DBGp packet accepted; anything bigger is a protocol error here.
const MAX_PACKET: usize = 1 << 20;

static START: OnceLock<Instant> = OnceLock::new();
static TRACE: OnceLock<bool> = OnceLock::new();

/// S2_TRACE=1 prints every DBGp packet to stderr (off for measurements).
fn trace() -> bool {
    *TRACE.get_or_init(|| std::env::var_os("S2_TRACE").is_some())
}

fn since_start_ms() -> f64 {
    START.get().unwrap().elapsed().as_secs_f64() * 1000.0
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum HitCond {
    Ge,
    Eq,
    Mod,
}

#[derive(Clone, Debug)]
struct Config {
    listen: String,
    sessions: usize,
    file: String,
    line: u32,
    no_break: bool,
    exprs: usize,
    hit: Option<(HitCond, u32)>,
    cap: Option<u32>,
    pipeline: bool,
    buffer_cap: usize,
    max_depth: u32,
    max_children: u32,
    max_data: u32,
    mem_every: u32,
    bp_get: bool,
    quiet: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            listen: "0.0.0.0:9003".into(),
            sessions: 1,
            file: "file:///var/www/html/public/index.php".into(),
            line: 44,
            no_break: false,
            exprs: 1,
            hit: None,
            cap: None,
            pipeline: false,
            buffer_cap: 1000,
            max_depth: 1,
            max_children: 32,
            max_data: 1024,
            mem_every: 0,
            bp_get: false,
            quiet: false,
        }
    }
}

const USAGE: &str = "usage: s2-logpoints [--listen ADDR] [--sessions N] [--file URI] [--line L]
  [--no-break] [--exprs 0|1|5] [--hit-cond >=|==|% --hit-value V] [--cap K]
  [--pipeline] [--buffer-cap C] [--max-depth D] [--max-children C]
  [--max-data B] [--mem-every M] [--bp-get] [--quiet]";

fn parse_args() -> Config {
    let mut c = Config::default();
    let mut args = std::env::args().skip(1);
    let mut cond: Option<HitCond> = None;
    let mut value: Option<u32> = None;
    while let Some(a) = args.next() {
        let mut next = || args.next().unwrap_or_else(|| die(&format!("{a} needs a value")));
        match a.as_str() {
            "--listen" => c.listen = next(),
            "--sessions" => c.sessions = num(&next()),
            "--file" => c.file = next(),
            "--line" => c.line = num(&next()),
            "--no-break" => c.no_break = true,
            "--exprs" => c.exprs = num(&next()),
            "--hit-cond" => {
                cond = Some(match next().as_str() {
                    ">=" => HitCond::Ge,
                    "==" => HitCond::Eq,
                    "%" => HitCond::Mod,
                    other => die(&format!("unknown hit condition {other}")),
                })
            }
            "--hit-value" => value = Some(num(&next())),
            "--cap" => c.cap = Some(num(&next())),
            "--pipeline" => c.pipeline = true,
            "--buffer-cap" => c.buffer_cap = num(&next()),
            "--max-depth" => c.max_depth = num(&next()),
            "--max-children" => c.max_children = num(&next()),
            "--max-data" => c.max_data = num(&next()),
            "--mem-every" => c.mem_every = num(&next()),
            "--bp-get" => c.bp_get = true,
            "--quiet" => c.quiet = true,
            _ => die(USAGE),
        }
    }
    match (cond, value) {
        (Some(cond), Some(v)) => c.hit = Some((cond, v)),
        (None, None) => {}
        _ => die("--hit-cond and --hit-value go together"),
    }
    if ![0, 1, 5].contains(&c.exprs) {
        die("--exprs must be 0, 1 or 5");
    }
    c.buffer_cap = c.buffer_cap.max(1);
    c
}

fn num<T: std::str::FromStr>(s: &str) -> T {
    s.parse().unwrap_or_else(|_| die(&format!("not a number: {s}")))
}

fn die(msg: &str) -> ! {
    eprintln!("{msg}");
    std::process::exit(2)
}

/// The expressions read on each pause, for `--exprs 1` and `--exprs 5`.
fn expressions(n: usize) -> &'static [&'static str] {
    const ALL: [&str; 5] = ["$item", "$i", "$sum", "$n", "$item['square']"];
    &ALL[..n]
}

fn main() {
    START.set(Instant::now()).unwrap();
    let cfg = parse_args();
    let listener = TcpListener::bind(&cfg.listen).unwrap_or_else(|e| die(&format!("bind: {e}")));
    eprintln!("listening on {} for {} session(s)", cfg.listen, cfg.sessions);
    report_process("start");

    let mut workers = Vec::new();
    for id in 1..=cfg.sessions {
        let (stream, peer) = match listener.accept() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("accept: {e}");
                continue;
            }
        };
        let cfg = cfg.clone();
        workers.push(thread::spawn(move || {
            let accepted = since_start_ms();
            match run_session(id, stream, &cfg) {
                Ok(s) => s.print(id, &peer.to_string(), accepted),
                Err(e) => println!("session id={id} error={e}"),
            }
        }));
    }
    for w in workers {
        let _ = w.join();
    }
    report_process("end");
}

/// Prints the process RSS, peak RSS and CPU time, read from /proc/self.
fn report_process(label: &str) {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let field = |name: &str| {
        status
            .lines()
            .find(|l| l.starts_with(name))
            .and_then(|l| l.split_whitespace().nth(1))
            .unwrap_or("?")
            .to_string()
    };
    let (user_ms, sys_ms) = cpu_ms();
    println!(
        "process at={label} t_ms={:.1} rss_kb={} hwm_kb={} cpu_user_ms={user_ms} cpu_sys_ms={sys_ms}",
        since_start_ms(),
        field("VmRSS:"),
        field("VmHWM:"),
    );
}

/// utime and stime from /proc/self/stat, in ms (CLK_TCK is 100 here).
fn cpu_ms() -> (u64, u64) {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    // Fields after the parenthesised command name; utime is field 14.
    let after = stat.rsplit_once(')').map(|(_, r)| r).unwrap_or("");
    let f: Vec<&str> = after.split_whitespace().collect();
    let tick = |i: usize| f.get(i).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0) * 10;
    (tick(11), tick(12))
}

fn rss_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|v| v.parse().ok())
        })
        .unwrap_or(0)
}

struct Conn {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    next_tid: u32,
    out: Vec<u8>,
    packet: Vec<u8>,
    len_buf: Vec<u8>,
}

impl Conn {
    fn new(stream: TcpStream) -> io::Result<Conn> {
        stream.set_nodelay(true)?;
        Ok(Conn {
            writer: stream.try_clone()?,
            reader: BufReader::with_capacity(64 * 1024, stream),
            next_tid: 1,
            out: Vec::with_capacity(4096),
            packet: Vec::with_capacity(4096),
            len_buf: Vec::with_capacity(16),
        })
    }

    /// Reads one `length NUL xml NUL` packet.
    fn read_packet(&mut self) -> io::Result<String> {
        self.len_buf.clear();
        let n = self.reader.read_until(0, &mut self.len_buf)?;
        if n == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "connection closed"));
        }
        let len: usize = std::str::from_utf8(&self.len_buf[..n - 1])
            .ok()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| io::Error::other("bad packet length"))?;
        if len > MAX_PACKET {
            return Err(io::Error::other(format!("packet of {len} bytes over the cap")));
        }
        self.packet.resize(len + 1, 0);
        self.reader.read_exact(&mut self.packet)?;
        let text = String::from_utf8_lossy(&self.packet[..len]).into_owned();
        if trace() {
            eprintln!("<- {text}");
        }
        Ok(text)
    }

    /// Reads packets until a `<response>` (skipping `<stream>` and `<notify>`).
    fn read_response(&mut self) -> io::Result<Node> {
        loop {
            let text = self.read_packet()?;
            let node = xml::parse(&text).map_err(io::Error::other)?;
            if node.name == "response" {
                return Ok(node);
            }
        }
    }

    /// Queues a command; `flush` sends everything queued in one write.
    fn queue(&mut self, cmd: &str, args: &str) -> u32 {
        let tid = self.next_tid;
        self.next_tid += 1;
        let _ = write!(self.out, "{cmd} -i {tid}");
        if !args.is_empty() {
            let _ = write!(self.out, " {args}");
        }
        self.out.push(0);
        tid
    }

    fn flush(&mut self) -> io::Result<()> {
        if trace() {
            eprintln!("-> {}", String::from_utf8_lossy(&self.out).replace('\0', " | "));
        }
        self.writer.write_all(&self.out)?;
        self.out.clear();
        Ok(())
    }

    fn command(&mut self, cmd: &str, args: &str) -> io::Result<Node> {
        self.queue(cmd, args);
        self.flush()?;
        self.read_response()
    }
}

struct Capture {
    pause: u32,
    values: Vec<Box<str>>,
}

/// Fixed 10 us buckets up to 20 ms, plus an overflow bucket.
struct Histogram {
    buckets: Box<[u32; 2001]>,
    count: u64,
    sum_us: f64,
    max_us: f64,
}

impl Histogram {
    fn new() -> Self {
        Histogram { buckets: Box::new([0; 2001]), count: 0, sum_us: 0.0, max_us: 0.0 }
    }

    fn add(&mut self, d: Duration) {
        let us = d.as_secs_f64() * 1e6;
        let i = ((us / 10.0) as usize).min(2000);
        self.buckets[i] += 1;
        self.count += 1;
        self.sum_us += us;
        self.max_us = self.max_us.max(us);
    }

    fn pct(&self, p: f64) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        let target = ((self.count as f64) * p).ceil() as u64;
        let mut seen = 0u64;
        for (i, b) in self.buckets.iter().enumerate() {
            seen += *b as u64;
            if seen >= target {
                return (i as f64 + 1.0) * 10.0;
            }
        }
        self.max_us
    }

    fn fmt(&self, name: &str) -> String {
        let avg = if self.count == 0 { 0.0 } else { self.sum_us / self.count as f64 };
        format!(
            "{name}_avg_us={avg:.1} {name}_p50_us<={:.0} {name}_p99_us<={:.0} {name}_max_us={:.0}",
            self.pct(0.5),
            self.pct(0.99),
            self.max_us
        )
    }
}

struct Summary {
    appid: String,
    pauses: u32,
    mismatches: u32,
    first_mismatch: Option<String>,
    evicted: u64,
    buffer: VecDeque<Capture>,
    hold: Histogram,
    gap: Histogram,
    setup: Duration,
    total: Duration,
    breakpoint_hits: Option<String>,
}

impl Summary {
    fn print(&self, id: usize, peer: &str, accepted_ms: f64) {
        let fmt_cap = |c: &Capture| {
            let vals: Vec<&str> = c.values.iter().map(|v| v.as_ref()).collect();
            format!("#{}[{}]", c.pause, vals.join(" | "))
        };
        let first = self.buffer.front().map(fmt_cap).unwrap_or_default();
        let last = self.buffer.back().map(fmt_cap).unwrap_or_default();
        println!(
            "session id={id} peer={peer} appid={} start_ms={accepted_ms:.1} end_ms={:.1} \
             pauses={} mismatches={} buffer_len={} evicted={} setup_ms={:.2} total_ms={:.1} \
             {} {} bp_hit_count={}",
            self.appid,
            since_start_ms(),
            self.pauses,
            self.mismatches,
            self.buffer.len(),
            self.evicted,
            self.setup.as_secs_f64() * 1000.0,
            self.total.as_secs_f64() * 1000.0,
            self.hold.fmt("hold"),
            self.gap.fmt("gap"),
            self.breakpoint_hits.as_deref().unwrap_or("-"),
        );
        println!("session id={id} oldest_capture={first}");
        println!("session id={id} newest_capture={last}");
        if let Some(m) = &self.first_mismatch {
            println!("session id={id} first_mismatch={m}");
        }
    }
}

fn run_session(id: usize, stream: TcpStream, cfg: &Config) -> io::Result<Summary> {
    let t_accept = Instant::now();
    let mut c = Conn::new(stream)?;
    let init = xml::parse(&c.read_packet()?).map_err(io::Error::other)?;
    let appid = init.attr("appid").unwrap_or("?").to_string();
    if !cfg.quiet {
        eprintln!(
            "session {id}: init fileuri={} appid={appid}",
            init.attr("fileuri").unwrap_or("?")
        );
    }

    for (name, v) in [
        ("max_depth", cfg.max_depth),
        ("max_children", cfg.max_children),
        ("max_data", cfg.max_data),
    ] {
        let r = c.command("feature_set", &format!("-n {name} -v {v}"))?;
        if r.attr("success") != Some("1") {
            eprintln!("session {id}: feature_set {name} failed");
        }
    }

    let mut bp_id = String::new();
    if !cfg.no_break {
        let mut args = format!("-t line -f {} -n {}", cfg.file, cfg.line);
        if let Some((cond, v)) = cfg.hit {
            let op = match cond {
                HitCond::Ge => ">=",
                HitCond::Eq => "==",
                HitCond::Mod => "%",
            };
            args.push_str(&format!(" -h {v} -o {op}"));
        }
        let r = c.command("breakpoint_set", &args)?;
        bp_id = r.attr("id").unwrap_or_default().to_string();
        if bp_id.is_empty() {
            return Err(io::Error::other(format!("breakpoint_set failed: {}", error_text(&r))));
        }
    }
    let setup = t_accept.elapsed();

    let exprs = expressions(cfg.exprs);
    let max_data = cfg.max_data as usize;
    let mut s = Summary {
        appid,
        pauses: 0,
        mismatches: 0,
        first_mismatch: None,
        evicted: 0,
        buffer: VecDeque::with_capacity(cfg.buffer_cap),
        hold: Histogram::new(),
        gap: Histogram::new(),
        setup,
        total: Duration::ZERO,
        breakpoint_hits: None,
    };
    let mut first_n: Option<Box<str>> = None;
    let mut bp_removed = false;

    c.queue("run", "");
    c.flush()?;
    let mut t_run = Instant::now();
    loop {
        let r = c.read_response()?;
        let t_break = Instant::now();
        match r.attr("status") {
            Some("break") => {}
            Some("stopping") | Some("stopped") => break,
            other => return Err(io::Error::other(format!("unexpected status {other:?}"))),
        }
        s.gap.add(t_break - t_run);
        s.pauses += 1;
        let pause = s.pauses;

        // Read the expressions, sequentially or pipelined with the run.
        let mut values: Vec<Box<str>> = Vec::with_capacity(exprs.len());
        let cap_reached = cfg.cap.is_some_and(|k| pause >= k);
        // breakpoint_get is refused once the script is stopping, so the
        // hit count is read while paused (only for the T-09 checks).
        if cfg.bp_get && !bp_removed {
            let r = c.command("breakpoint_get", &format!("-d {bp_id}"))?;
            s.breakpoint_hits = r
                .child("breakpoint")
                .and_then(|b| b.attr("hit_count"))
                .map(str::to_string);
        }
        if cfg.pipeline {
            for e in exprs {
                c.queue("property_get", &format!("-n {e} -m {max_data}"));
            }
            if cap_reached && !bp_removed {
                c.queue("breakpoint_remove", &format!("-d {bp_id}"));
            }
            c.queue("run", "");
            c.flush()?;
            t_run = Instant::now();
            for _ in exprs {
                let r = c.read_response()?;
                values.push(render_response(&r, max_data));
            }
            if cap_reached && !bp_removed {
                c.read_response()?;
                bp_removed = true;
            }
        } else {
            for e in exprs {
                let r = c.command("property_get", &format!("-n {e} -m {max_data}"))?;
                values.push(render_response(&r, max_data));
            }
            if cap_reached && !bp_removed {
                c.command("breakpoint_remove", &format!("-d {bp_id}"))?;
                bp_removed = true;
            }
            c.queue("run", "");
            c.flush()?;
            t_run = Instant::now();
        }
        s.hold.add(t_run - t_break);

        if let Some(problem) = verify(cfg, pause, &values, &mut first_n) {
            s.mismatches += 1;
            if s.first_mismatch.is_none() {
                s.first_mismatch = Some(format!("pause {pause}: {problem}"));
            }
        }
        if s.buffer.len() == cfg.buffer_cap {
            s.buffer.pop_front();
            s.evicted += 1;
        }
        s.buffer.push_back(Capture { pause, values });

        if cfg.mem_every > 0 && pause % cfg.mem_every == 0 {
            let (u, sy) = cpu_ms();
            println!(
                "mem session={id} pause={pause} rss_kb={} buffer_len={} cpu_ms={}",
                rss_kb(),
                s.buffer.len(),
                u + sy
            );
        }
    }

    c.queue("stop", "");
    c.flush()?;
    let _ = c.read_response();
    s.total = t_accept.elapsed();
    Ok(s)
}

fn error_text(r: &Node) -> String {
    match r.child("error") {
        Some(e) => format!(
            "code {} {}",
            e.attr("code").unwrap_or("?"),
            e.child("message").map(|m| m.text.as_str()).unwrap_or("")
        ),
        None => "no error element".into(),
    }
}

/// Turns a property_get response into a short string, capped at max_data.
fn render_response(r: &Node, max_data: usize) -> Box<str> {
    let mut out = String::new();
    match r.child("property") {
        Some(p) => render_property(p, &mut out),
        None => out.push_str(&format!("error({})", error_text(r))),
    }
    if out.len() > max_data {
        let mut cut = max_data;
        while !out.is_char_boundary(cut) {
            cut -= 1;
        }
        out.truncate(cut);
    }
    out.into_boxed_str()
}

fn render_property(p: &Node, out: &mut String) {
    let props: Vec<&Node> = p.children.iter().filter(|c| c.name == "property").collect();
    if !props.is_empty() {
        out.push('{');
        for (i, child) in props.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(child.attr("name").unwrap_or("?"));
            out.push('=');
            render_property(child, out);
        }
        out.push('}');
        return;
    }
    match p.attr("type") {
        Some("array") | Some("object") => {
            out.push_str(&format!("{}({})", p.attr("type").unwrap(), p.attr("numchildren").unwrap_or("0")));
        }
        Some("null") => out.push_str("null"),
        Some("uninitialized") => out.push_str("uninitialized"),
        _ if p.attr("encoding") == Some("base64") => match xml::base64_decode(&p.text) {
            Some(bytes) => out.push_str(&String::from_utf8_lossy(&bytes)),
            None => out.push_str("bad-base64"),
        },
        _ => out.push_str(&p.text),
    }
}

/// The loop index Xdebug should be paused on for the given pause number.
fn expected_index(cfg: &Config, pause: u32) -> i64 {
    let p = pause as i64;
    match cfg.hit {
        None => p - 1,
        Some((HitCond::Ge, v)) => v as i64 - 1 + p - 1,
        Some((HitCond::Eq, v)) => v as i64 - 1 + (p - 1) * 1_000_000_000,
        Some((HitCond::Mod, v)) => v as i64 * p - 1,
    }
}

/// Checks the captured values against what the fixture's loop computes.
fn verify(cfg: &Config, pause: u32, values: &[Box<str>], first_n: &mut Option<Box<str>>) -> Option<String> {
    if cfg.exprs == 0 {
        return None;
    }
    let i = expected_index(cfg, pause);
    let want_item = format!("{{index={i},square={}}}", i * i);
    if *values[0] != *want_item {
        return Some(format!("$item={} want {want_item}", values[0]));
    }
    if cfg.exprs == 5 {
        let want_sum = if i == 0 { 0 } else { (i - 1) * i * (2 * i - 1) / 6 };
        let checks = [
            ("$i", values[1].as_ref(), i.to_string()),
            ("$sum", values[2].as_ref(), want_sum.to_string()),
            ("$item['square']", values[4].as_ref(), (i * i).to_string()),
        ];
        for (name, got, want) in checks {
            if got != want {
                return Some(format!("{name}={got} want {want}"));
            }
        }
        match first_n {
            None => *first_n = Some(values[3].clone()),
            Some(n) if **n != *values[3] => return Some(format!("$n changed to {}", values[3])),
            _ => {}
        }
    }
    None
}
