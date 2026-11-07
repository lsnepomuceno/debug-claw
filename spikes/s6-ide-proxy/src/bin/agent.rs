//! Spike S6: the agent side. Connects to the proxy's control port, waits
//! for each pause the IDE holds, and injects reads: the stack and three
//! variables of `loop()`, checked against what the loop computes.
//!
//!   agent [--connect ADDR] [--pauses N] [--unsafe] [--flood]
//!
//! `--unsafe` sends, at the first pause, every command the proxy must
//! refuse, and checks that each one is refused. `--flood` keeps reading
//! `$i` at every pause until the proxy says the session is no longer
//! paused, so the agent's reads interleave with the IDE's commands.

#[path = "../client.rs"]
mod client;
#[path = "../dbgp.rs"]
mod dbgp;
#[path = "../xml.rs"]
mod xml;

use std::net::TcpStream;

use client::Conn;

fn die(msg: &str) -> ! {
    eprintln!("agent: {msg}");
    std::process::exit(2);
}

fn error_code(x: &str) -> Option<u32> {
    let n = xml::parse(x).ok()?;
    n.child("error")?.attr("code")?.parse().ok()
}

fn main() {
    let mut addr = "127.0.0.1:9100".to_string();
    let mut pauses = 1u32;
    let mut unsafe_ = false;
    let mut flood = false;
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        match k.as_str() {
            "--connect" => addr = it.next().unwrap_or_else(|| die("--connect ADDR")),
            "--pauses" => pauses = it.next().and_then(|v| v.parse().ok()).unwrap_or_else(|| die("bad --pauses")),
            "--unsafe" => unsafe_ = true,
            "--flood" => flood = true,
            _ => die(&format!("unknown argument {k}")),
        }
    }
    let s = TcpStream::connect(&addr).unwrap_or_else(|e| die(&format!("connect {addr}: {e}")));
    let mut c = Conn::new(s);
    println!("agent connected to {addr}");
    let mut times = Vec::new();
    let (mut ok, mut bad, mut refused_ok, mut refused_bad) = (0u32, 0u32, 0u32, 0u32);
    let mut flood_reads = 0u32;

    for p in 1..=pauses {
        let w = match c.send("x_wait -t 20000") {
            Ok((_, w)) => w,
            Err(e) => {
                // The proxy exits after its last session.
                println!("pause {p}: x_wait ended ({e})");
                break;
            }
        };
        if !w.xml.contains("status=\"break\"") {
            println!("pause {p}: no pause ({})", w.xml.lines().last().unwrap_or(""));
            break;
        }
        let mut read = |c: &mut Conn, cmd: &str| -> Option<String> {
            match c.send(cmd) {
                Ok((_, r)) if error_code(&r.xml).is_none() => {
                    times.push(r.us);
                    Some(r.xml)
                }
                Ok((_, r)) => {
                    let code = error_code(&r.xml);
                    // 901 to 903: the IDE moved on before this read, which
                    // is the proxy doing its job. Anything else is a failure.
                    if !matches!(code, Some(901..=903)) {
                        bad += 1;
                    }
                    println!("  {cmd}: error {code:?}");
                    None
                }
                Err(e) => die(&format!("{cmd}: {e}")),
            }
        };
        let st = read(&mut c, "stack_get");
        let (uri, line) = st.as_deref().and_then(client::top_frame).unwrap_or_default();
        let int = |x: Option<String>, name: &str| x.and_then(|x| xml::parse(&x).ok()).and_then(|n| client::int_of(&n, name));
        let i = int(read(&mut c, "property_get -n $i"), "$i");
        let sum = int(read(&mut c, "property_get -n $sum"), "$sum");
        let sq = int(read(&mut c, "property_get -n $item"), "square");
        // `-m 5` must apply to this read only, not to the IDE's later reads.
        let uri_req = read(&mut c, "property_get -c 1 -n $_SERVER['REQUEST_URI'] -m 5")
            .and_then(|x| xml::parse(&x).ok())
            .and_then(|n| n.child("property").map(client::value));
        // The request is /loop?n=3, so the first 5 bytes are "/loop".
        let uri_ok = uri_req.as_deref().is_none_or(|u| u == "/loop");
        let check = if line == 44 {
            match (i, sum, sq) {
                (Some(i), Some(s), Some(q)) if s == client::sum_before(i) && q == i * i && uri_ok => "ok",
                (Some(_), Some(_), Some(_)) => "BAD",
                _ if !uri_ok => "BAD",
                _ => "incomplete (the IDE moved on)",
            }
        } else if !uri_ok {
            "BAD"
        } else {
            "n/a (not the breakpoint line)"
        };
        match check {
            "ok" => ok += 1,
            "BAD" => bad += 1,
            _ => {}
        }
        println!("pause {p} at {uri}:{line}: i={i:?} sum={sum:?} square={sq:?} REQUEST_URI(-m 5)={uri_req:?} check={check}");

        if unsafe_ && p == 1 {
            for cmd in [
                "run",
                "step_into",
                "step_over",
                "step_out",
                "stop",
                "detach",
                "break",
                "feature_set -n max_depth -v 5",
                "breakpoint_set -t line -f file:///var/www/html/public/index.php -n 43",
                "breakpoint_update -d 1 -s disabled",
                "breakpoint_remove -d 1",
                "stdout -c 0",
                "stderr -c 1",
                "property_set -n $sum -- MA==",
                "eval 1",
                "expr 1",
                "exec 1",
                "interact",
            ] {
                let (_, r) = c.send(cmd).unwrap_or_else(|e| die(&format!("{cmd}: {e}")));
                if error_code(&r.xml) == Some(900) {
                    refused_ok += 1;
                } else {
                    refused_bad += 1;
                    println!("  NOT REFUSED: {cmd}: {}", r.xml);
                }
            }
            println!("  unsafe commands refused: {refused_ok}, not refused: {refused_bad}");
        }

        if flood {
            let mut n = 0;
            loop {
                let (_, r) = c.send("property_get -n $i").unwrap_or_else(|e| die(&format!("flood: {e}")));
                match error_code(&r.xml) {
                    None => {
                        let v = xml::parse(&r.xml).ok().and_then(|x| client::int_of(&x, "$i"));
                        if v != i {
                            bad += 1;
                            println!("  flood read {n}: $i={v:?}, expected {i:?}");
                        }
                        times.push(r.us);
                        n += 1;
                    }
                    // Not paused, session gone, or paused again since x_wait.
                    Some(901..=903) => break,
                    Some(code) => {
                        bad += 1;
                        println!("  flood read {n}: error {code}");
                        break;
                    }
                }
            }
            flood_reads += n;
            println!("  flood: {n} reads of $i while paused");
        }
    }
    println!(
        "agent: checks ok={ok} bad={bad}, tid mismatches={}, unsafe refused={refused_ok} not refused={refused_bad}, flood reads={flood_reads}",
        c.mismatches
    );
    println!("agent {}", client::summary("injected reads", times));
    if bad > 0 || refused_bad > 0 || c.mismatches > 0 {
        std::process::exit(1);
    }
}
