//! Prints what the S5 extension sends: one JSON line per observed call.
//!
//! Usage: `listener [ADDR] [--every N]`. With `--every N` it prints only
//! every Nth line and a running count, so a benchmark does not spend its
//! CPU on logging. A line that is not a JSON object counts as malformed.

use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::thread;

static LINES: AtomicU64 = AtomicU64::new(0);
static MALFORMED: AtomicU64 = AtomicU64::new(0);

fn main() {
    let mut args = std::env::args().skip(1);
    let mut addr = "0.0.0.0:7781".to_string();
    let mut every = 1u64;
    while let Some(a) = args.next() {
        if a == "--every" {
            every = args.next().and_then(|v| v.parse().ok()).unwrap_or(1).max(1);
        } else {
            addr = a;
        }
    }
    let listener = TcpListener::bind(&addr).expect("bind");
    eprintln!("listening on {addr}, printing every {every} line(s)");
    for conn in listener.incoming() {
        let Ok(conn) = conn else { continue };
        let peer = conn.peer_addr().map(|p| p.to_string()).unwrap_or_default();
        thread::spawn(move || {
            eprintln!("connection from {peer}");
            let mut reader = BufReader::new(conn);
            let mut line = String::new();
            let mut mine = 0u64;
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
                mine += 1;
                let total = LINES.fetch_add(1, Relaxed) + 1;
                let text = line.trim_end();
                if !(text.starts_with("{\"pid\":") && text.ends_with("\"}")) {
                    MALFORMED.fetch_add(1, Relaxed);
                    eprintln!("malformed: {text}");
                }
                if total % every == 0 {
                    println!("#{total} {text}");
                }
            }
            eprintln!(
                "closed {peer}: {mine} lines on this connection, {} total, {} malformed",
                LINES.load(Relaxed),
                MALFORMED.load(Relaxed)
            );
        });
    }
}
