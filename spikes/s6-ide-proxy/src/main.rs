//! Spike S6: a DBGp proxy between Xdebug and an IDE. Throwaway code.
//!
//!   s6-ide-proxy --ide HOST:PORT [--listen ADDR] [--control ADDR]
//!                [--sessions N] [--log FILE]
//!
//! Xdebug connects to `--listen` (0.0.0.0:9003). For each session the
//! proxy connects to the IDE at `--ide`, forwards the `init` packet, and
//! then relays the IDE's commands to Xdebug and Xdebug's packets to the
//! IDE. Every command sent to Xdebug gets a transaction ID chosen by the
//! proxy; the IDE's own ID is restored on the way back. That keeps the
//! IDE's IDs and the proxy's apart whatever numbers the IDE uses.
//!
//! An agent connects to `--control` (127.0.0.1:9100) and sends DBGp
//! command lines, NUL-terminated, as an IDE would. Only read commands are
//! accepted, and only while the IDE holds the session paused. `x_wait
//! -i N [-t MS]` blocks until the next pause, and the agent's reads are
//! sent only while the session is still in the pause it waited for.
//! Responses come back framed like Xdebug's, with the agent's transaction
//! ID.
//!
//! With `--sessions N` the proxy exits after N Xdebug sessions. At the end
//! of each session it prints the time it spent per relayed packet.

mod dbgp;

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Write};
use std::net::{Shutdown, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use dbgp::Kind;

/// Commands that resume the script. Their response marks the next state.
const CONTINUATION: &[&str] = &["run", "step_into", "step_over", "step_out", "stop", "detach"];

/// Commands an agent may send while the IDE drives. Everything else is
/// refused: it changes execution state, changes what the IDE will see
/// (features, breakpoints, output redirection, values), or runs PHP code.
const SAFE: &[&str] = &[
    "stack_depth",
    "stack_get",
    "context_names",
    "context_get",
    "property_get",
    "property_value",
    "typemap_get",
    "source",
    "status",
    "feature_get",
    "breakpoint_get",
    "breakpoint_list",
];

/// Outstanding commands per session. The IDE and the agent wait for each
/// response, so a handful is normal; the cap only bounds a broken peer.
const MAX_PENDING: usize = 1024;
/// Samples kept for the per-packet timing summary.
const MAX_SAMPLES: usize = 1 << 20;
/// How much of each packet goes to the log.
const LOG_BYTES: usize = 2048;

const ERR_UNSAFE: u32 = 900;
const ERR_NOT_PAUSED: u32 = 901;
const ERR_NO_SESSION: u32 = 902;
const ERR_STALE: u32 = 903;

struct Args {
    listen: String,
    ide: String,
    control: String,
    sessions: u32,
    log: Option<String>,
}

fn die(msg: &str) -> ! {
    eprintln!("s6-ide-proxy: {msg}");
    std::process::exit(2);
}

fn parse_args() -> Args {
    let mut a = Args {
        listen: "0.0.0.0:9003".into(),
        ide: String::new(),
        control: "127.0.0.1:9100".into(),
        sessions: 0,
        log: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let mut v = || it.next().unwrap_or_else(|| die(&format!("{k} needs a value")));
        match k.as_str() {
            "--listen" => a.listen = v(),
            "--ide" => a.ide = v(),
            "--control" => a.control = v(),
            "--sessions" => a.sessions = v().parse().unwrap_or_else(|_| die("bad --sessions")),
            "--log" => a.log = Some(v()),
            _ => die(&format!("unknown argument {k}")),
        }
    }
    if a.ide.is_empty() {
        die("--ide HOST:PORT is required");
    }
    a
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Starting,
    Running,
    Break,
    Stopping,
    Closed,
}

enum Route {
    Ide { tid: Vec<u8>, cont: bool },
    Agent { tid: Vec<u8>, reply: mpsc::SyncSender<Vec<u8>> },
    /// The proxy's own `detach` after the IDE went away.
    Internal,
}

struct Inner {
    xd: BufWriter<TcpStream>,
    next_tid: u64,
    pending: HashMap<u64, Route>,
    state: State,
    /// Counts the pauses of this session. An agent read carries the pause
    /// it waited for, and is refused once the session has moved on.
    pause_no: u64,
}

impl Inner {
    /// Sends a command to Xdebug under a fresh transaction ID.
    fn send(&mut self, line: &[u8], route: Route) -> io::Result<u64> {
        if self.pending.len() >= MAX_PENDING {
            return Err(io::Error::other("too many outstanding commands"));
        }
        let cmd = dbgp::parse_command(line);
        let tid = self.next_tid;
        self.next_tid += 1;
        let out = dbgp::with_tid(line, &cmd, &tid.to_string());
        self.pending.insert(tid, route);
        self.xd.write_all(&out)?;
        self.xd.write_all(b"\0")?;
        self.xd.flush()?;
        Ok(tid)
    }
}

struct Session {
    id: u64,
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct HubState {
    session: Option<Arc<Session>>,
    paused: bool,
    /// The current session's pause number (a copy of `Inner::pause_no`).
    pause_no: u64,
}

#[derive(Default)]
struct Hub {
    st: Mutex<HubState>,
    cv: Condvar,
}

impl Hub {
    fn set_paused(&self, sid: u64, paused: bool, pause_no: u64) {
        let mut g = self.st.lock().unwrap();
        if g.session.as_ref().map(|s| s.id) != Some(sid) {
            return;
        }
        g.paused = paused;
        g.pause_no = pause_no;
        drop(g);
        self.cv.notify_all();
    }
}

struct Log {
    f: Mutex<BufWriter<File>>,
    t0: Instant,
}

impl Log {
    fn line(log: &Option<Arc<Log>>, sid: u64, dir: &str, bytes: &[u8]) {
        let Some(l) = log else { return };
        let ms = l.t0.elapsed().as_secs_f64() * 1e3;
        let cut = &bytes[..bytes.len().min(LOG_BYTES)];
        let text = String::from_utf8_lossy(cut).replace('\n', " ");
        let more = if bytes.len() > LOG_BYTES { format!(" ... ({} bytes)", bytes.len()) } else { String::new() };
        let mut f = l.f.lock().unwrap();
        let _ = writeln!(f, "{ms:10.3} s{sid} {dir} {text}{more}");
        let _ = f.flush();
    }
}

struct Samples(Vec<f64>);

impl Samples {
    fn push(&mut self, t: Instant) {
        if self.0.len() < MAX_SAMPLES {
            self.0.push(t.elapsed().as_secs_f64() * 1e6);
        }
    }
    fn summary(mut self, what: &str) -> String {
        self.0.sort_by(f64::total_cmp);
        let v = &self.0;
        format!(
            "{what}: n={} proxy_us median={:.1} p99={:.1} max={:.1}",
            v.len(),
            dbgp::pct(v, 50.0),
            dbgp::pct(v, 99.0),
            v.last().copied().unwrap_or(f64::NAN)
        )
    }
}

fn error_xml(command: &str, tid: &[u8], code: u32, msg: &str) -> Vec<u8> {
    let mut x = format!(
        "<?xml version=\"1.0\" encoding=\"iso-8859-1\"?>\n<response xmlns=\"urn:debugger_protocol_v1\" command=\"{command}\" transaction_id=\""
    )
    .into_bytes();
    x.extend_from_slice(tid);
    x.extend_from_slice(format!("\"><error code=\"{code}\"><message><![CDATA[{msg}]]></message></error></response>").as_bytes());
    x
}

fn connect(addr: &str) -> io::Result<TcpStream> {
    let mut last = io::Error::other("no address");
    for sa in addr.to_socket_addrs()? {
        match TcpStream::connect_timeout(&sa, Duration::from_secs(2)) {
            Ok(s) => return Ok(s),
            Err(e) => last = e,
        }
    }
    Err(last)
}

/// One Xdebug session: connects to the IDE and relays both ways.
fn session(id: u64, xd: TcpStream, ide_addr: &str, hub: Arc<Hub>, log: Option<Arc<Log>>) {
    let _ = xd.set_nodelay(true);
    let ide = match connect(ide_addr) {
        Ok(s) => s,
        Err(e) => {
            // Closing the socket lets Xdebug run the request undebugged.
            eprintln!("s{id}: IDE at {ide_addr} unreachable ({e}), dropping the session");
            return;
        }
    };
    let _ = ide.set_nodelay(true);
    eprintln!("s{id}: relaying to {ide_addr}");
    let sess = Arc::new(Session {
        id,
        inner: Mutex::new(Inner {
            xd: BufWriter::with_capacity(1 << 16, xd.try_clone().expect("clone")),
            next_tid: 1,
            pending: HashMap::new(),
            state: State::Starting,
            pause_no: 0,
        }),
    });
    {
        let mut g = hub.st.lock().unwrap();
        g.session = Some(sess.clone());
        g.paused = false;
    }
    hub.cv.notify_all();

    let ide_reader = {
        let sess = sess.clone();
        let hub = hub.clone();
        let log = log.clone();
        let r = ide.try_clone().expect("clone");
        thread::spawn(move || ide_loop(sess, r, hub, log))
    };
    let xd_stats = xd_loop(&sess, xd, ide.try_clone().expect("clone"), &hub, &log);

    // Xdebug closed: end the IDE side and fail the agent's pending reads.
    {
        let mut g = sess.inner.lock().unwrap();
        g.state = State::Closed;
        g.pending.clear();
        let _ = g.xd.get_ref().shutdown(Shutdown::Both);
    }
    let _ = ide.shutdown(Shutdown::Both);
    let ide_stats = ide_reader.join().unwrap_or(None);
    {
        let mut g = hub.st.lock().unwrap();
        if g.session.as_ref().map(|s| s.id) == Some(id) {
            g.session = None;
            g.paused = false;
        }
    }
    hub.cv.notify_all();
    if let Some(s) = ide_stats {
        eprintln!("s{id}: {s}");
    }
    eprintln!("s{id}: {xd_stats}");
    eprintln!("s{id}: closed");
}

/// Reads Xdebug's packets and routes them. Returns the timing summary.
fn xd_loop(sess: &Arc<Session>, xd: TcpStream, ide: TcpStream, hub: &Hub, log: &Option<Arc<Log>>) -> String {
    let mut r = BufReader::with_capacity(1 << 16, xd);
    let mut ide_w = BufWriter::with_capacity(1 << 16, ide);
    let mut ide_ok = true;
    let mut times = Samples(Vec::new());
    loop {
        let pkt = match dbgp::read_packet(&mut r) {
            Ok(Some(p)) => p,
            Ok(None) => break,
            Err(e) => {
                eprintln!("s{}: Xdebug read: {e}", sess.id);
                break;
            }
        };
        let t0 = Instant::now();
        let h = dbgp::head(&pkt);
        if h.kind == Kind::Response {
            let tr = dbgp::attr_range(&pkt, &h, "transaction_id");
            let ptid = tr.clone().and_then(|r| std::str::from_utf8(&pkt[r]).ok()?.parse::<u64>().ok());
            let (route, new_state, pause_no) = {
                let mut g = sess.inner.lock().unwrap();
                let route = ptid.and_then(|t| g.pending.remove(&t));
                if let Some(Route::Ide { cont: true, .. }) = &route {
                    g.state = match dbgp::attr(&pkt, &h, "status") {
                        Some("break") => State::Break,
                        Some("stopping") => State::Stopping,
                        Some("stopped") => State::Closed,
                        _ => State::Running,
                    };
                    if g.state == State::Break {
                        g.pause_no += 1;
                    }
                }
                (route, g.state, g.pause_no)
            };
            match route {
                Some(Route::Ide { tid, cont }) => {
                    let r = tr.expect("routed by tid");
                    if ide_ok {
                        let res = dbgp::write_packet_parts(&mut ide_w, [&pkt[..r.start], &tid, &pkt[r.end..]]);
                        if let Err(e) = res {
                            eprintln!("s{}: IDE write: {e}", sess.id);
                            ide_ok = false;
                        }
                        times.push(t0);
                    }
                    if cont {
                        hub.set_paused(sess.id, new_state == State::Break, pause_no);
                    }
                    Log::line(log, sess.id, "xd>ide", &pkt);
                    continue;
                }
                Some(Route::Agent { tid, reply }) => {
                    let r = tr.expect("routed by tid");
                    let mut v = Vec::with_capacity(pkt.len() + tid.len());
                    v.extend_from_slice(&pkt[..r.start]);
                    v.extend_from_slice(&tid);
                    v.extend_from_slice(&pkt[r.end..]);
                    let _ = reply.try_send(v);
                    Log::line(log, sess.id, "xd>agent", &pkt);
                    continue;
                }
                Some(Route::Internal) => {
                    Log::line(log, sess.id, "xd>proxy", &pkt);
                    continue;
                }
                // Not ours: pass it on untouched, like init, notify and stream.
                None => {}
            }
        }
        if ide_ok {
            if let Err(e) = dbgp::write_packet(&mut ide_w, &pkt) {
                eprintln!("s{}: IDE write: {e}", sess.id);
                ide_ok = false;
            }
            times.push(t0);
        }
        Log::line(log, sess.id, "xd>ide", &pkt);
    }
    times.summary("xd->ide packets")
}

/// Reads the IDE's commands and sends them to Xdebug under proxy IDs.
fn ide_loop(sess: Arc<Session>, ide: TcpStream, hub: Arc<Hub>, log: Option<Arc<Log>>) -> Option<String> {
    let mut r = BufReader::with_capacity(1 << 16, ide);
    let mut times = Samples(Vec::new());
    let mut ended = false;
    loop {
        let line = match dbgp::read_command(&mut r) {
            Ok(Some(l)) => l,
            Ok(None) => break,
            Err(e) => {
                eprintln!("s{}: IDE read: {e}", sess.id);
                break;
            }
        };
        let t0 = Instant::now();
        let cmd = dbgp::parse_command(&line);
        let cont = CONTINUATION.contains(&cmd.name.as_str());
        let tid = cmd.tid.clone().map(|r| line[r].to_vec()).unwrap_or_default();
        let (res, pause_no) = {
            let mut g = sess.inner.lock().unwrap();
            if g.state == State::Closed {
                break;
            }
            if cont {
                g.state = State::Running;
            }
            (g.send(&line, Route::Ide { tid, cont }), g.pause_no)
        };
        times.push(t0);
        if cont {
            hub.set_paused(sess.id, false, pause_no);
        }
        Log::line(&log, sess.id, "ide>xd", &line);
        if let Err(e) = res {
            eprintln!("s{}: Xdebug write: {e}", sess.id);
            break;
        }
        if cmd.name == "stop" || cmd.name == "detach" {
            ended = true;
        }
    }
    if !ended {
        // The IDE went away mid-session: let the script run to the end.
        let mut g = sess.inner.lock().unwrap();
        if g.state != State::Closed {
            g.state = State::Running;
            let _ = g.send(b"detach", Route::Internal);
            eprintln!("s{}: IDE closed, sent detach", sess.id);
        }
    }
    Some(times.summary("ide->xd commands"))
}

/// One agent connection on the control port.
fn agent_conn(stream: TcpStream, hub: Arc<Hub>, log: Option<Arc<Log>>) {
    let _ = stream.set_nodelay(true);
    let mut w = BufWriter::new(stream.try_clone().expect("clone"));
    let mut r = BufReader::new(stream);
    // The (session, pause) the agent last waited for. Its reads are only
    // sent while the session is still in that pause.
    let mut seen: Option<(u64, u64)> = None;
    while let Ok(Some(line)) = dbgp::read_command(&mut r) {
        let cmd = dbgp::parse_command(&line);
        let atid = cmd.tid.clone().map(|r| line[r].to_vec()).unwrap_or_default();
        let reply = if cmd.name == "x_wait" {
            let ms = timeout_arg(&line).unwrap_or(30_000);
            let g = hub.st.lock().unwrap();
            let current = |s: &HubState| s.session.as_ref().map(|x| (x.id, s.pause_no));
            let (g, res) = hub
                .cv
                .wait_timeout_while(g, Duration::from_millis(ms), |s| !(s.paused && current(s) != seen))
                .unwrap();
            let status = if res.timed_out() {
                "timeout"
            } else {
                seen = current(&g);
                "break"
            };
            let (sid, pause) = current(&g).unwrap_or((0, 0));
            let mut x = format!(
                "<?xml version=\"1.0\" encoding=\"iso-8859-1\"?>\n<response xmlns=\"urn:debugger_protocol_v1\" command=\"x_wait\" transaction_id=\""
            )
            .into_bytes();
            x.extend_from_slice(&atid);
            x.extend_from_slice(format!("\" status=\"{status}\" session=\"{sid}\" pause=\"{pause}\"/>").as_bytes());
            x
        } else if !SAFE.contains(&cmd.name.as_str()) {
            let msg = format!("refused by the proxy: {} is unsafe while the IDE drives the session", cmd.name);
            error_xml(&cmd.name, &atid, ERR_UNSAFE, &msg)
        } else {
            agent_read(&hub, seen, &line, &cmd.name, &atid, &log)
        };
        if dbgp::write_packet(&mut w, &reply).is_err() {
            return;
        }
    }
}

fn timeout_arg(line: &[u8]) -> Option<u64> {
    let s = std::str::from_utf8(line).ok()?;
    let mut it = s.split(' ');
    while let Some(t) = it.next() {
        if t == "-t" {
            return it.next()?.parse().ok();
        }
    }
    None
}

/// Injects one read into the session the IDE holds paused.
fn agent_read(hub: &Hub, seen: Option<(u64, u64)>, line: &[u8], name: &str, atid: &[u8], log: &Option<Arc<Log>>) -> Vec<u8> {
    let Some(sess) = hub.st.lock().unwrap().session.clone() else {
        return error_xml(name, atid, ERR_NO_SESSION, "no session");
    };
    let (tx, rx) = mpsc::sync_channel(1);
    {
        let mut g = sess.inner.lock().unwrap();
        // Checked under the lock that the IDE's commands also take, so a
        // continuation the IDE sends either goes first (and the read is
        // refused) or after the read (and Xdebug answers it paused).
        if g.state != State::Break {
            return error_xml(name, atid, ERR_NOT_PAUSED, &format!("not paused (state {:?})", g.state));
        }
        // A read meant for an earlier pause would read another moment.
        if seen != Some((sess.id, g.pause_no)) {
            let msg = format!("stale: the session is at pause {} of session {}, x_wait first", g.pause_no, sess.id);
            return error_xml(name, atid, ERR_STALE, &msg);
        }
        if let Err(e) = g.send(line, Route::Agent { tid: atid.to_vec(), reply: tx }) {
            return error_xml(name, atid, ERR_NO_SESSION, &format!("write: {e}"));
        }
    }
    Log::line(log, sess.id, "agent>xd", line);
    match rx.recv_timeout(Duration::from_secs(10)) {
        Ok(x) => x,
        Err(_) => error_xml(name, atid, ERR_NO_SESSION, "session ended before the response"),
    }
}

fn main() {
    let a = parse_args();
    let log = a.log.as_ref().map(|p| {
        let f = File::create(p).unwrap_or_else(|e| die(&format!("log {p}: {e}")));
        Arc::new(Log { f: Mutex::new(BufWriter::new(f)), t0: Instant::now() })
    });
    let hub = Arc::new(Hub::default());
    let control = TcpListener::bind(&a.control).unwrap_or_else(|e| die(&format!("bind {}: {e}", a.control)));
    {
        let hub = hub.clone();
        let log = log.clone();
        thread::spawn(move || {
            for s in control.incoming().flatten() {
                let hub = hub.clone();
                let log = log.clone();
                thread::spawn(move || agent_conn(s, hub, log));
            }
        });
    }
    let listener = TcpListener::bind(&a.listen).unwrap_or_else(|e| die(&format!("bind {}: {e}", a.listen)));
    eprintln!("listening on {} for Xdebug, control on {}, IDE at {}", a.listen, a.control, a.ide);
    let mut handles = Vec::new();
    let mut n = 0u64;
    for s in listener.incoming() {
        let Ok(s) = s else { continue };
        n += 1;
        let hub = hub.clone();
        let log = log.clone();
        let ide = a.ide.clone();
        handles.push(thread::spawn(move || session(n, s, &ide, hub, log)));
        if a.sessions > 0 && n >= u64::from(a.sessions) {
            break;
        }
    }
    for h in handles {
        let _ = h.join();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_response_carries_the_agent_tid() {
        let x = error_xml("run", b"17", ERR_UNSAFE, "no");
        let h = dbgp::head(&x);
        assert_eq!(dbgp::attr(&x, &h, "transaction_id"), Some("17"));
        assert_eq!(dbgp::attr(&x, &h, "command"), Some("run"));
    }

    #[test]
    fn unsafe_commands_are_not_in_the_safe_list() {
        for c in [
            "run", "step_into", "step_over", "step_out", "stop", "detach", "break", "feature_set", "breakpoint_set",
            "breakpoint_update", "breakpoint_remove", "stdout", "stderr", "property_set", "eval", "expr", "exec",
            "interact",
        ] {
            assert!(!SAFE.contains(&c), "{c}");
        }
    }
}
