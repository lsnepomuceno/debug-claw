//! Spike S1: path mapping without configuration, and a DBGp feature probe.
//!
//! Subcommands:
//!
//! - `map`: accept sessions, infer the mapping from `init`, set one
//!   breakpoint given as a local path, check that the request pauses there.
//! - `vendor`: the "vendor mounted elsewhere" case, which learns a second
//!   mapping from a stack frame.
//! - `script`: accept one session and run a list of DBGp commands, for
//!   the feature probes.
//! - `ctrl`: send one command to Xdebug's control socket (run it inside
//!   the PHP container, which owns the abstract socket).

mod dbgp;
mod mapping;
mod tree;

use std::fs;
use std::io::{self, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use dbgp::{Conn, attrs, base64_decode, text};
use mapping::{AbsPath, Inference, Learned, Mapping, Style, infer, parse_file_uri, parse_native};

const WALK_LIMIT: usize = 200_000;

struct Args(Vec<String>);

impl Args {
    fn value(&self, name: &str) -> Option<String> {
        self.0.iter().position(|a| a == name).and_then(|i| self.0.get(i + 1).cloned())
    }
    fn values(&self, name: &str) -> Vec<String> {
        self.0.windows(2).filter(|w| w[0] == name).map(|w| w[1].clone()).collect()
    }
    fn flag(&self, name: &str) -> bool {
        self.0.iter().any(|a| a == name)
    }
}

fn main() -> ExitCode {
    let all: Vec<String> = std::env::args().skip(1).collect();
    let Some(sub) = all.first().cloned() else {
        eprintln!("usage: s1-path-mapping map|vendor|script|ctrl ...");
        return ExitCode::from(2);
    };
    let args = Args(all[1..].to_vec());
    let result = match sub.as_str() {
        "map" => cmd_map(&args),
        "vendor" => cmd_vendor(&args),
        "script" => cmd_script(&args),
        "ctrl" => cmd_ctrl(&args),
        other => Err(io::Error::other(format!("unknown subcommand {other}"))),
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}

fn listen() -> io::Result<TcpListener> {
    let listener = TcpListener::bind("0.0.0.0:9003")?;
    println!("listening on 0.0.0.0:9003");
    io::stdout().flush()?;
    Ok(listener)
}

/// Accepts a connection and reads `init`.
fn accept(listener: &TcpListener) -> io::Result<(Conn, String)> {
    let (stream, peer) = listener.accept()?;
    stream.set_nodelay(true)?;
    let mut conn = Conn::new(stream)?;
    let init = conn.read_packet()?;
    let a = attrs(&init, "init").unwrap_or_default();
    let engine = attrs(&init, "engine").map(|e| e.get("version").cloned().unwrap_or_default());
    println!("session from {peer}");
    for key in ["fileuri", "language", "xdebug:language_version", "protocol_version", "appid", "idekey", "xdebug:ctrl_socket"] {
        println!("  init {key}={}", a.get(key).map(String::as_str).unwrap_or("(absent)"));
    }
    println!("  init engine={}", engine.unwrap_or_default());
    Ok((conn, init))
}

fn local_style(args: &Args) -> (Style, bool) {
    // The spike runs on Linux; --case-insensitive simulates macOS.
    (Style::Posix, args.flag("--case-insensitive"))
}

fn read_local(path: &AbsPath) -> Option<Vec<u8>> {
    fs::read(path.native()).ok()
}

/// Reads a remote file through the DBGp `source` command.
fn remote_source(conn: &mut Conn, uri: &str, line: Option<u32>) -> io::Result<Result<Vec<u8>, String>> {
    let range = line.map(|l| format!(" -b {l} -e {l}")).unwrap_or_default();
    let xml = conn.cmd("source", &format!("-f {uri}{range}"), None)?;
    if xml.contains("<error") {
        let msg = text(&xml, "message").unwrap_or_else(|| "error".into());
        return Ok(Err(msg));
    }
    Ok(Ok(base64_decode(&text(&xml, "response").unwrap_or_default())))
}

/// Infers a mapping for a remote file: walk the local tree for files with
/// its name, take the longest suffix, break ties by content.
fn infer_for(
    conn: &mut Conn,
    remote: &AbsPath,
    root: &AbsPath,
    local_ci: bool,
) -> io::Result<Result<Mapping, String>> {
    let name = remote.parts.last().cloned().unwrap_or_default();
    let t = Instant::now();
    let walk = tree::find_named(Path::new(&root.native()), &name, local_ci, WALK_LIMIT);
    println!(
        "  walk: {} entries visited in {:?}, {} named {name}{}",
        walk.visited,
        t.elapsed(),
        walk.found.len(),
        if walk.truncated { " (truncated)" } else { "" }
    );
    for (link, target) in &walk.symlinks {
        println!("  walk: symlink {link} -> {} (not followed)", target.display());
    }
    match infer(remote, root, &walk.found, local_ci) {
        Inference::NoMatch => Ok(Err(format!("no local file named {name} under {}", root.native()))),
        Inference::Unique { mapping, matched } => {
            println!("  suffix: unique, {matched} trailing components match");
            // One round trip confirms the match, and catches an image
            // that holds an older copy of the code.
            let local = mapping.to_local(remote).and_then(|p| read_local(&p));
            match remote_source(conn, &remote.uri(), None)? {
                Ok(bytes) if Some(&bytes) == local.as_ref() => println!("  source: entry script content matches"),
                Ok(_) => println!(
                    "  source: entry script content DIFFERS from {}. WARN: the container runs other code than the local tree (stale image or another project?)",
                    mapping.to_local(remote).map(|p| p.native()).unwrap_or_default()
                ),
                Err(e) => println!("  source: failed ({e})"),
            }
            Ok(Ok(mapping))
        }
        Inference::Ambiguous { candidates, matched } => {
            println!("  suffix: tie between {} candidates, {matched} trailing components each", candidates.len());
            let t = Instant::now();
            let remote_bytes = match remote_source(conn, &remote.uri(), None)? {
                Ok(b) => b,
                Err(e) => return Ok(Err(format!("source of the entry script failed: {e}"))),
            };
            println!("  source: entry script read through DBGp in {:?} ({} bytes)", t.elapsed(), remote_bytes.len());
            let same: Vec<Mapping> = candidates
                .iter()
                .filter(|m| {
                    let local = m.to_local(remote).and_then(|p| read_local(&p));
                    let hit = local.as_deref() == Some(remote_bytes.as_slice());
                    println!("    candidate {} -> {}: content {}", m.remote.native(), m.local.native(), if hit { "matches" } else { "differs" });
                    hit
                })
                .cloned()
                .collect();
            match same.len() {
                1 => {
                    println!("  tie broken by content");
                    Ok(Ok(same.into_iter().next().unwrap()))
                }
                0 => Ok(Err(format!(
                    "ASK: the entry script {} matches {} local files by name, but none has the same content. Which local folder holds this project?",
                    remote.native(),
                    candidates.len()
                ))),
                n => Ok(Err(format!(
                    "ASK: {n} local folders hold an identical copy of {}: {}. Which one is the project?",
                    remote.native(),
                    same.iter().map(|m| m.local.native()).collect::<Vec<_>>().join(", ")
                ))),
            }
        }
    }
}

fn parse_break(spec: &str, root: &AbsPath) -> io::Result<(AbsPath, u32)> {
    let (path, line) = spec.rsplit_once(':').ok_or_else(|| io::Error::other("--break needs path:line"))?;
    let line: u32 = line.parse().map_err(|_| io::Error::other("bad line"))?;
    let local = if path.starts_with('/') {
        parse_native(path, Style::Posix).map_err(io::Error::other)?
    } else {
        root.join(&path.split('/').map(String::from).collect::<Vec<_>>())
    };
    Ok((local, line))
}

/// Sends `run` and returns the status and the break location, if any.
fn run(conn: &mut Conn) -> io::Result<(String, Option<(String, String)>, String)> {
    let xml = conn.cmd("run", "", None)?;
    let status = attrs(&xml, "response").and_then(|a| a.get("status").cloned()).unwrap_or_default();
    let loc = attrs(&xml, "xdebug:message").map(|m| {
        (m.get("filename").cloned().unwrap_or_default(), m.get("lineno").cloned().unwrap_or_default())
    });
    Ok((status, loc, xml))
}

/// Lets the request finish. A connection that Xdebug already closed (the
/// script ended) is not an error.
fn finish(conn: &mut Conn) -> io::Result<()> {
    loop {
        let Ok((status, loc, _)) = run(conn) else {
            println!("  run -> connection closed by Xdebug");
            return Ok(());
        };
        if status != "break" {
            println!("  run -> {status}");
            break;
        }
        if let Some((f, l)) = loc {
            println!("  run -> break again at {f}:{l}");
        }
    }
    let _ = conn.cmd("stop", "", None);
    Ok(())
}

fn cmd_map(args: &Args) -> io::Result<bool> {
    let (style, local_ci) = local_style(args);
    let root = parse_native(&args.value("--local").ok_or_else(|| io::Error::other("--local DIR"))?, style)
        .map_err(io::Error::other)?;
    let breaks = args.values("--break");
    let sessions: usize = args.value("--sessions").and_then(|s| s.parse().ok()).unwrap_or(1);
    let listener = listen()?;
    let mut learned = Learned::default();
    let mut all_ok = true;
    for n in 1..=sessions {
        let (mut conn, init) = accept(&listener)?;
        let started = Instant::now();
        let uri = attrs(&init, "init").and_then(|a| a.get("fileuri").cloned()).unwrap_or_default();
        let remote = match parse_file_uri(&uri) {
            Ok(r) => r,
            Err(e) => {
                println!("  session {n}: FAIL {e}");
                all_ok = false;
                finish(&mut conn)?;
                continue;
            }
        };

        // Reuse a learned mapping while the entry script is still under
        // its prefix, exists locally and has the same content.
        let mut mapping = None;
        if let Some(m) = learned.lookup(&remote).cloned() {
            let local = m.to_local(&remote).and_then(|p| read_local(&p));
            let remote_bytes = remote_source(&mut conn, &remote.uri(), None)?.ok();
            if local.is_some() && local == remote_bytes {
                println!("  mapping: reused {} -> {} (entry script checked by content, no walk)", m.remote.native(), m.local.native());
                mapping = Some(m);
            } else {
                println!("  mapping: learned {} -> {} no longer explains the entry script, forgotten", m.remote.native(), m.local.native());
                learned.forget(&m);
            }
        }
        if mapping.is_none() {
            match infer_for(&mut conn, &remote, &root, local_ci)? {
                Ok(m) => {
                    println!("  mapping: inferred {} -> {}", m.remote.native(), m.local.native());
                    learned.learn(m.clone());
                    mapping = Some(m);
                }
                Err(question) => println!("  mapping: none. {question}"),
            }
        }
        let Some(mapping) = mapping else {
            all_ok = false;
            finish(&mut conn)?;
            continue;
        };

        let mut expected = Vec::new();
        for spec in &breaks {
            let (local, line) = parse_break(spec, &root)?;
            let Some(remote_bp) = mapping.to_remote(&local) else {
                println!("  breakpoint {}:{line}: outside the mapped folder {}, ASK: where is this file in the container?", local.native(), mapping.local.native());
                continue;
            };
            let bp_uri = remote_bp.uri();
            match remote_source(&mut conn, &bp_uri, Some(line))? {
                Ok(src) => println!("  remote check {bp_uri}:{line}: exists, line is {:?}", String::from_utf8_lossy(&src).trim()),
                Err(e) => println!("  remote check {bp_uri}:{line}: MISSING ({e})"),
            }
            let xml = conn.cmd("breakpoint_set", &format!("-t line -f {bp_uri} -n {line}"), None)?;
            let a = attrs(&xml, "response").unwrap_or_default();
            println!(
                "  breakpoint_set {bp_uri}:{line} -> id={} resolved={}",
                a.get("id").map(String::as_str).unwrap_or("?"),
                a.get("resolved").map(String::as_str).unwrap_or("(absent)")
            );
            expected.push((bp_uri, line.to_string(), local.native()));
        }
        println!("  time from init to first run: {:?}", started.elapsed());
        let (status, loc, _) = run(&mut conn)?;
        let ok = match (&status[..], &loc) {
            ("break", Some((f, l))) => {
                let exact = expected.iter().any(|(eu, el, _)| eu == f && el == l);
                // Xdebug stores a breakpoint under its realpath and frames
                // report realpaths, so a breakpoint set through a symlink
                // pauses at another path. Compare the local files too.
                let reported = parse_file_uri(f).ok().and_then(|r| mapping.to_local(&r)).and_then(|p| fs::canonicalize(p.native()).ok());
                let via_link = !exact && expected.iter().any(|(_, el, lp)| el == l && fs::canonicalize(lp).ok() == reported && reported.is_some());
                let hit = exact || via_link;
                println!(
                    "  run -> break at {f}:{l} {}",
                    if exact { "(PASS: the breakpoint set by local path)" } else if via_link { "(PASS: set through a symlink, reported at the resolved path)" } else { "(unexpected location)" }
                );
                if let Some(local) = parse_file_uri(f).ok().and_then(|r| mapping.to_local(&r)) {
                    println!("  shown to the user as {}:{l}", local.native());
                }
                hit
            }
            _ => {
                println!("  run -> {status} without a break (FAIL)");
                false
            }
        };
        all_ok &= ok || expected.is_empty();
        finish(&mut conn)?;
        println!("  session {n}: {}", if ok { "PASS" } else { "no pause" });
    }
    Ok(all_ok)
}

/// Vendor mounted elsewhere: the entry script gives the app mapping, the
/// vendor breakpoint translates into a path that does not exist, and a
/// stack frame that enters the vendor code teaches the second mapping.
fn cmd_vendor(args: &Args) -> io::Result<bool> {
    let (style, local_ci) = local_style(args);
    let root = parse_native(&args.value("--local").ok_or_else(|| io::Error::other("--local DIR"))?, style)
        .map_err(io::Error::other)?;
    let app_break = args.value("--app-break").ok_or_else(|| io::Error::other("--app-break rel:line"))?;
    let vendor_break = args.value("--vendor-break").ok_or_else(|| io::Error::other("--vendor-break rel:line"))?;
    let listener = listen()?;
    let (mut conn, init) = accept(&listener)?;
    let uri = attrs(&init, "init").and_then(|a| a.get("fileuri").cloned()).unwrap_or_default();
    let remote = parse_file_uri(&uri).map_err(io::Error::other)?;
    let app = match infer_for(&mut conn, &remote, &root, local_ci)? {
        Ok(m) => m,
        Err(q) => return Err(io::Error::other(q)),
    };
    println!("  mapping: inferred {} -> {}", app.remote.native(), app.local.native());

    let (vlocal, vline) = parse_break(&vendor_break, &root)?;
    let vremote = app.to_remote(&vlocal).expect("vendor/ is inside the project");
    match remote_source(&mut conn, &vremote.uri(), Some(vline))? {
        Ok(_) => println!("  remote check {}: exists", vremote.uri()),
        Err(e) => println!("  remote check {}: MISSING ({e}). The vendor breakpoint cannot be placed yet.", vremote.uri()),
    }

    let (alocal, aline) = parse_break(&app_break, &root)?;
    let auri = app.to_remote(&alocal).unwrap().uri();
    let xml = conn.cmd("breakpoint_set", &format!("-t line -f {auri} -n {aline}"), None)?;
    let app_id = attrs(&xml, "response").and_then(|a| a.get("id").cloned()).unwrap_or_default();
    let (status, loc, _) = run(&mut conn)?;
    println!("  run -> {status} at {loc:?}");
    let xml = conn.cmd("step_into", "", None)?;
    let frame = attrs(&xml, "xdebug:message").and_then(|m| m.get("filename").cloned()).unwrap_or_default();
    println!("  step_into -> now in {frame}");
    let fremote = parse_file_uri(&frame).map_err(io::Error::other)?;
    let mut ok = false;
    if app.to_local(&fremote).is_some() {
        println!("  frame is under the app mapping; nothing to learn");
    } else {
        println!("  frame {} is outside every known prefix, inferring a second mapping from it", fremote.native());
        match infer_for(&mut conn, &fremote, &root, local_ci)? {
            Ok(vendor) => {
                println!("  mapping: inferred {} -> {}", vendor.remote.native(), vendor.local.native());
                let vuri = vendor.to_remote(&vlocal).unwrap().uri();
                // The loop would hit the app line again first.
                conn.cmd("breakpoint_remove", &format!("-d {app_id}"), None)?;
                conn.cmd("breakpoint_set", &format!("-t line -f {vuri} -n {vline}"), None)?;
                println!("  breakpoint_set {vuri}:{vline}");
                let (status, loc, _) = run(&mut conn)?;
                ok = status == "break" && loc.as_ref().is_some_and(|(f, l)| *f == vuri && *l == vline.to_string());
                println!("  run -> {status} at {loc:?} {}", if ok { "(PASS: vendor breakpoint hit)" } else { "(FAIL)" });
            }
            Err(q) => println!("  {q}"),
        }
    }
    finish(&mut conn)?;
    Ok(ok)
}

/// Runs a list of commands on one session. Each `--cmd` is a DBGp command
/// line (`name args`, with an optional ` -- data` sent as base64), or one
/// of: `RUN`, `FINISH`, `SLEEP ms`, `ASYNC cmd` (send without waiting),
/// `WAIT ms` (print any packet that arrives within ms).
fn cmd_script(args: &Args) -> io::Result<bool> {
    let mut cmds = args.values("--cmd");
    if let Some(file) = args.value("--cmd-file") {
        let body = fs::read_to_string(&file)?;
        cmds.extend(
            body.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .map(String::from),
        );
    }
    let max: usize = args.value("--max").and_then(|s| s.parse().ok()).unwrap_or(1500);
    let listener = listen()?;
    let (mut conn, init) = accept(&listener)?;
    println!("  init raw: {}", clip(&init, max));
    for line in &cmds {
        let (head, data) = match line.split_once(" -- ") {
            Some((h, d)) => (h, Some(d)),
            None => (line.as_str(), None),
        };
        let (name, rest) = head.split_once(' ').unwrap_or((head, ""));
        let t = Instant::now();
        match name {
            "RUN" => {
                let (status, loc, xml) = run(&mut conn)?;
                println!("> run\n  status={status} at={loc:?} ({:?})\n  {}", t.elapsed(), clip(&xml, max));
            }
            "FINISH" => {
                println!("> finish");
                finish(&mut conn)?;
            }
            "SLEEP" => std::thread::sleep(Duration::from_millis(rest.parse().unwrap_or(0))),
            "ASYNC" => {
                let (n, r) = rest.split_once(' ').unwrap_or((rest, ""));
                let id = conn.send(n, r, data)?;
                println!("> {rest} (async, id {id}, not waiting)");
            }
            "WAIT" => {
                let ms: u64 = rest.parse().unwrap_or(1000);
                conn.set_read_timeout(Some(Duration::from_millis(ms)))?;
                match conn.read_packet() {
                    Ok(xml) => println!("> wait {ms} ms\n  got after {:?}: {}", t.elapsed(), clip(&xml, max)),
                    Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => {
                        println!("> wait {ms} ms\n  nothing arrived")
                    }
                    Err(e) => return Err(e),
                }
                conn.set_read_timeout(None)?;
            }
            _ => {
                let xml = conn.cmd(name, rest, data)?;
                let shown = if data.is_some() { format!("{head} -- {}", data.unwrap()) } else { head.to_string() };
                println!("> {shown}\n  ({:?}) {}", t.elapsed(), clip(&decode_values(&xml), max));
            }
        }
        for s in conn.streams.drain(..) {
            let a = attrs(&s, "stream").unwrap_or_default();
            let body = String::from_utf8_lossy(&base64_decode(&text(&s, "stream").unwrap_or_default())).into_owned();
            println!("  stream type={} data={body:?}", a.get("type").map(String::as_str).unwrap_or("?"));
        }
        io::stdout().flush()?;
    }
    Ok(true)
}

/// Appends the decoded value after each base64 property, so transcripts
/// are readable.
fn decode_values(xml: &str) -> String {
    let mut out = String::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<![CDATA[") {
        let (before, after) = rest.split_at(start);
        out.push_str(before);
        let end = after.find("]]>").map(|e| e + 3).unwrap_or(after.len());
        let cdata = &after[..end];
        out.push_str(cdata);
        let inner = cdata.trim_start_matches("<![CDATA[").trim_end_matches("]]>");
        if before.rfind("<property").is_some_and(|p| before[p..].contains("encoding=\"base64\"")) {
            out.push_str(&format!("{{={:?}}}", String::from_utf8_lossy(&base64_decode(inner))));
        }
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

fn clip(s: &str, max: usize) -> String {
    let s = s.trim_start_matches("<?xml version=\"1.0\" encoding=\"iso-8859-1\"?>\n");
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}... ({} bytes)", &s[..end], s.len())
}

/// Sends one command to Xdebug's control socket, an abstract Unix socket
/// named in the init packet (`xdebug:ctrl_socket`). Linux only.
fn cmd_ctrl(args: &Args) -> io::Result<bool> {
    #[cfg(target_os = "linux")]
    {
        use std::os::linux::net::SocketAddrExt;
        use std::os::unix::net::{SocketAddr, UnixStream};
        let name = args.0.first().ok_or_else(|| io::Error::other("ctrl SOCKET COMMAND"))?;
        let command = args.0.get(1).cloned().unwrap_or_else(|| "ps".into());
        let addr = SocketAddr::from_abstract_name(name.as_bytes())?;
        let t = Instant::now();
        let mut s = UnixStream::connect_addr(&addr)?;
        s.write_all(command.as_bytes())?;
        s.write_all(b"\0")?;
        let mut out = Vec::new();
        s.read_to_end(&mut out)?;
        println!("ctrl {name} {command:?} -> ({:?}) {}", t.elapsed(), String::from_utf8_lossy(&out).replace('\0', ""));
        Ok(true)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = args;
        Err(io::Error::other("the control socket client is Linux only"))
    }
}
