//! Spike S5: a PHP extension in Rust that observes `PDOStatement::execute`.
//!
//! It registers one Observer API handler through ext-php-rs. The init
//! callback returns handlers only for `PDOStatement::execute`; every other
//! function gets no handler. Each observed call is timed with a monotonic
//! clock, and its SQL and duration are queued as one JSON line in a bounded
//! per-worker buffer. The buffer is written to a TCP socket at the end of
//! the request (or after each call with `debug_claw.flush=call`).
//!
//! When the daemon is down, the first failed connect marks it down for
//! `debug_claw.retry_ms`. Until then the observer skips the SQL read and
//! the formatting, and the request end does nothing but compare two
//! instants. Connects use a timeout of `debug_claw.connect_timeout_us`,
//! and writes are non-blocking, so a slow daemon drops events instead of
//! holding the request.
//!
//! Throwaway spike code: never imported by production code.

#![cfg_attr(windows, feature(abi_vectorcall))]

use std::cell::RefCell;
use std::io::{ErrorKind, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::{Duration, Instant};

use ext_php_rs::flags::IniEntryPermission;
use ext_php_rs::prelude::*;
use ext_php_rs::types::Zval;
use ext_php_rs::zend::{ExecuteData, ExecutorGlobals, IniEntryDef, IniEntryDefs};

static INI: IniEntryDefs<6> = IniEntryDefs::new([
    IniEntryDef::new(c"debug_claw.daemon", c"127.0.0.1:7781", IniEntryPermission::System),
    IniEntryDef::new(c"debug_claw.flush", c"request", IniEntryPermission::System),
    IniEntryDef::new(c"debug_claw.retry_ms", c"1000", IniEntryPermission::System),
    IniEntryDef::new(c"debug_claw.connect_timeout_us", c"1000", IniEntryPermission::System),
    IniEntryDef::new(c"debug_claw.buffer_bytes", c"65536", IniEntryPermission::System),
    IniEntryDef::end(),
]);

/// Settings read once at MINIT.
struct Config {
    daemon: String,
    flush_per_call: bool,
    retry: Duration,
    connect_timeout: Duration,
    buffer_bytes: usize,
}

static CONFIG: OnceLock<Config> = OnceLock::new();

/// Per-process counters, exposed by `debug_claw_s5_stats()`.
macro_rules! counters {
    ($($name:ident),* $(,)?) => {
        $(static $name: AtomicU64 = AtomicU64::new(0);)*
        fn stats_json() -> String {
            let mut out = String::from("{");
            $(out.push_str(&format!("\"{}\":{},", stringify!($name).to_lowercase(), $name.load(Relaxed)));)*
            out
        }
    };
}

counters!(
    INIT_CALLS,
    INIT_OBSERVED,
    BEGIN_CALLS,
    END_CALLS,
    EVENTS_QUEUED,
    EVENTS_SENT,
    EVENTS_DROPPED_DOWN,
    EVENTS_DROPPED_FULL,
    CONNECT_ATTEMPTS,
    CONNECT_FAILURES,
    WRITE_ERRORS,
    WOULD_BLOCK,
    BYTES_SENT,
    REQUESTS,
);

/// Deepest nesting of observed calls that is timed. Deeper calls are counted
/// but not reported, so the start stack stays bounded.
const MAX_DEPTH: usize = 64;

/// Per-worker state. Lazily created in the worker, never in the FPM master,
/// so nothing is shared across `fork`.
struct State {
    pid: u32,
    addr: Option<SocketAddr>,
    stream: Option<TcpStream>,
    down_until: Option<Instant>,
    /// Unsent bytes, never longer than `buffer_bytes`.
    pending: Vec<u8>,
    pending_events: u64,
    starts: Vec<Instant>,
    untimed_depth: usize,
}

impl State {
    fn new(cfg: &Config) -> Self {
        Self {
            pid: std::process::id(),
            addr: None,
            stream: None,
            down_until: None,
            pending: Vec::with_capacity(cfg.buffer_bytes),
            pending_events: 0,
            starts: Vec::with_capacity(MAX_DEPTH),
            untimed_depth: 0,
        }
    }

    fn is_down(&self, now: Instant) -> bool {
        self.stream.is_none() && self.down_until.is_some_and(|t| now < t)
    }

    fn mark_down(&mut self, cfg: &Config, now: Instant) {
        self.stream = None;
        self.down_until = Some(now + cfg.retry);
        self.drop_pending();
    }

    fn drop_pending(&mut self) {
        EVENTS_DROPPED_DOWN.fetch_add(self.pending_events, Relaxed);
        self.pending.clear();
        self.pending_events = 0;
    }

    fn connect(&mut self, cfg: &Config, now: Instant) -> bool {
        CONNECT_ATTEMPTS.fetch_add(1, Relaxed);
        if self.addr.is_none() {
            self.addr = cfg.daemon.to_socket_addrs().ok().and_then(|mut a| a.next());
        }
        let stream = self
            .addr
            .and_then(|addr| TcpStream::connect_timeout(&addr, cfg.connect_timeout).ok())
            .filter(|s| s.set_nonblocking(true).is_ok());
        match stream {
            Some(s) => {
                let _ = s.set_nodelay(true);
                self.stream = Some(s);
                self.down_until = None;
                true
            }
            None => {
                CONNECT_FAILURES.fetch_add(1, Relaxed);
                // Resolve again on the next attempt, in case the daemon moved.
                self.addr = None;
                self.mark_down(cfg, now);
                false
            }
        }
    }

    /// Writes what the socket takes now and keeps the rest for the next
    /// flush. Never blocks.
    fn flush(&mut self, cfg: &Config) {
        if self.pending.is_empty() {
            return;
        }
        let now = Instant::now();
        if self.is_down(now) {
            self.drop_pending();
            return;
        }
        if self.stream.is_none() && !self.connect(cfg, now) {
            return;
        }
        let Some(stream) = self.stream.as_mut() else { return };
        // std writes TCP with MSG_NOSIGNAL on Linux, so a closed peer gives
        // EPIPE instead of SIGPIPE.
        match stream.write(&self.pending) {
            Ok(n) => {
                BYTES_SENT.fetch_add(n as u64, Relaxed);
                if n == self.pending.len() {
                    EVENTS_SENT.fetch_add(self.pending_events, Relaxed);
                    self.pending.clear();
                    self.pending_events = 0;
                } else {
                    self.pending.drain(..n);
                }
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                WOULD_BLOCK.fetch_add(1, Relaxed);
            }
            Err(_) => {
                WRITE_ERRORS.fetch_add(1, Relaxed);
                self.mark_down(cfg, now);
            }
        }
    }

    fn queue(&mut self, cfg: &Config, sql: &str, nanos: u128) {
        let start = self.pending.len();
        let _ = write!(self.pending, "{{\"pid\":{},\"req\":{},\"ns\":{},\"sql\":\"", self.pid, REQUESTS.load(Relaxed), nanos);
        escape_json(&mut self.pending, sql);
        self.pending.extend_from_slice(b"\"}\n");
        if self.pending.len() > cfg.buffer_bytes {
            self.pending.truncate(start);
            EVENTS_DROPPED_FULL.fetch_add(1, Relaxed);
            return;
        }
        self.pending_events += 1;
        EVENTS_QUEUED.fetch_add(1, Relaxed);
    }
}

fn escape_json(out: &mut Vec<u8>, s: &str) {
    for &b in s.as_bytes() {
        match b {
            b'"' => out.extend_from_slice(b"\\\""),
            b'\\' => out.extend_from_slice(b"\\\\"),
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\r' => out.extend_from_slice(b"\\r"),
            b'\t' => out.extend_from_slice(b"\\t"),
            0..=0x1f => {
                let _ = write!(out, "\\u{b:04x}");
            }
            _ => out.push(b),
        }
    }
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn with_state<R>(f: impl FnOnce(&mut State, &Config) -> R) -> Option<R> {
    let cfg = CONFIG.get()?;
    STATE.with(|cell| {
        let mut slot = cell.try_borrow_mut().ok()?;
        let state = slot.get_or_insert_with(|| State::new(cfg));
        Some(f(state, cfg))
    })
}

struct PdoObserver;

impl FcallObserver for PdoObserver {
    fn should_observe(&self, info: &FcallInfo) -> bool {
        INIT_CALLS.fetch_add(1, Relaxed);
        let observe = info.is_internal
            && info.class_name == Some("PDOStatement")
            && info.function_name == Some("execute");
        if observe {
            INIT_OBSERVED.fetch_add(1, Relaxed);
        }
        observe
    }

    fn begin(&self, _execute_data: &ExecuteData) {
        BEGIN_CALLS.fetch_add(1, Relaxed);
        with_state(|s, _| {
            if s.starts.len() < MAX_DEPTH {
                s.starts.push(Instant::now());
            } else {
                s.untimed_depth += 1;
            }
        });
    }

    fn end(&self, execute_data: &ExecuteData, _retval: Option<&Zval>) {
        let end = Instant::now();
        END_CALLS.fetch_add(1, Relaxed);
        with_state(|s, cfg| {
            if s.untimed_depth > 0 {
                s.untimed_depth -= 1;
                return;
            }
            let Some(start) = s.starts.pop() else { return };
            if s.is_down(end) {
                EVENTS_DROPPED_DOWN.fetch_add(1, Relaxed);
                return;
            }
            let nanos = end.duration_since(start).as_nanos();
            match execute_data
                .This
                .object()
                .and_then(|obj| obj.get_property::<&str>("queryString").ok())
            {
                Some(sql) => s.queue(cfg, sql, nanos),
                None => s.queue(cfg, "", nanos),
            }
            if cfg.flush_per_call {
                s.flush(cfg);
            }
        });
    }
}

/// Returns this worker's counters as a JSON object.
#[php_function]
#[php(name = "debug_claw_s5_stats")]
pub fn debug_claw_s5_stats() -> String {
    let mut out = stats_json();
    let (pending_cap, pending_len, down) = with_state(|s, _| {
        (s.pending.capacity(), s.pending.len(), s.is_down(Instant::now()))
    })
    .unwrap_or((0, 0, false));
    out.push_str(&format!(
        "\"pending_capacity\":{pending_cap},\"pending_len\":{pending_len},\"down\":{down}}}"
    ));
    out
}

extern "C" fn startup(_ty: i32, module_number: i32) -> i32 {
    IniEntryDef::register(INI.as_slice(), module_number);
    let ini = ExecutorGlobals::get().ini_values();
    let get = |key: &str, default: &str| -> String {
        ini.get(key).cloned().flatten().unwrap_or_else(|| default.to_string())
    };
    let number = |key: &str, default: u64| get(key, "").parse().unwrap_or(default);
    let _ = CONFIG.set(Config {
        daemon: get("debug_claw.daemon", "127.0.0.1:7781"),
        flush_per_call: get("debug_claw.flush", "request") == "call",
        retry: Duration::from_millis(number("debug_claw.retry_ms", 1000)),
        connect_timeout: Duration::from_micros(number("debug_claw.connect_timeout_us", 1000).max(1)),
        buffer_bytes: number("debug_claw.buffer_bytes", 65536) as usize,
    });
    0
}

extern "C" fn request_shutdown(_ty: i32, _module_number: i32) -> i32 {
    with_state(|s, cfg| {
        // A bailout (fatal error, exit) skips the end handlers.
        s.starts.clear();
        s.untimed_depth = 0;
        s.flush(cfg);
    });
    REQUESTS.fetch_add(1, Relaxed);
    0
}

#[php_module]
pub fn get_module(module: ModuleBuilder) -> ModuleBuilder {
    module
        .startup_function(startup)
        .request_shutdown_function(request_shutdown)
        .fcall_observer(|| PdoObserver)
        .function(wrap_function!(debug_claw_s5_stats))
}
