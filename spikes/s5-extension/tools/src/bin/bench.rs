//! A one-connection HTTP/1.1 load generator with per-request timing.
//!
//! Usage: `bench HOST:PORT PATH -n N [-w WARMUP] [--out FILE] [--label L]`
//!
//! Sends N requests one after another on a keep-alive connection, after
//! WARMUP untimed ones, and times each from the first byte written to the
//! last body byte read. Prints one summary line in microseconds and, with
//! `--out`, writes every timing (one per line) to FILE. Any status other
//! than 200 aborts the run.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let addr = args.first().expect("HOST:PORT").clone();
    let path = args.get(1).expect("PATH").clone();
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let n: usize = flag("-n").and_then(|v| v.parse().ok()).unwrap_or(1000);
    let warmup: usize = flag("-w").and_then(|v| v.parse().ok()).unwrap_or(0);
    let label = flag("--label").unwrap_or_default();
    let host = addr.split(':').next().unwrap_or("localhost").to_string();

    let stream = TcpStream::connect(&addr).expect("connect");
    stream.set_nodelay(true).expect("nodelay");
    let mut writer = stream.try_clone().expect("clone");
    let mut reader = BufReader::with_capacity(64 * 1024, stream);
    let request = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: keep-alive\r\n\r\n");

    let mut times = Vec::with_capacity(n);
    let mut body = Vec::with_capacity(4096);
    for i in 0..warmup + n {
        let start = Instant::now();
        writer.write_all(request.as_bytes()).expect("write");
        let status = read_response(&mut reader, &mut body);
        let us = start.elapsed().as_secs_f64() * 1e6;
        if status != 200 {
            panic!("request {i}: status {status}: {}", String::from_utf8_lossy(&body));
        }
        if i >= warmup {
            times.push(us);
        }
    }

    if let Some(out) = flag("--out") {
        let text: String = times.iter().map(|t| format!("{t:.1}\n")).collect();
        std::fs::write(out, text).expect("write --out");
    }
    let mut sorted = times.clone();
    sorted.sort_by(f64::total_cmp);
    let pct = |p: f64| sorted[((sorted.len() as f64 - 1.0) * p).round() as usize];
    let mean = sorted.iter().sum::<f64>() / sorted.len() as f64;
    println!(
        "{label} n={} median_us={:.1} p95_us={:.1} p99_us={:.1} mean_us={:.1} min_us={:.1} max_us={:.1}",
        sorted.len(),
        pct(0.5),
        pct(0.95),
        pct(0.99),
        mean,
        sorted[0],
        sorted[sorted.len() - 1]
    );
}

/// Reads one response (Content-Length or chunked) into `body` and returns
/// the status code.
fn read_response(reader: &mut BufReader<TcpStream>, body: &mut Vec<u8>) -> u16 {
    body.clear();
    let mut line = String::new();
    reader.read_line(&mut line).expect("status line");
    let status = line.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let mut length = None;
    let mut chunked = false;
    loop {
        line.clear();
        reader.read_line(&mut line).expect("header");
        let header = line.trim_end();
        if header.is_empty() {
            break;
        }
        let lower = header.to_ascii_lowercase();
        if let Some(v) = lower.strip_prefix("content-length:") {
            length = v.trim().parse::<usize>().ok();
        } else if lower.starts_with("transfer-encoding:") && lower.contains("chunked") {
            chunked = true;
        }
    }
    if chunked {
        loop {
            line.clear();
            reader.read_line(&mut line).expect("chunk size");
            let size = usize::from_str_radix(line.trim_end().split(';').next().unwrap_or("0"), 16).expect("chunk size");
            let start = body.len();
            body.resize(start + size, 0);
            reader.read_exact(&mut body[start..]).expect("chunk");
            line.clear();
            reader.read_line(&mut line).expect("chunk end");
            if size == 0 {
                break;
            }
        }
    } else if let Some(len) = length {
        body.resize(len, 0);
        reader.read_exact(body).expect("body");
    } else {
        panic!("response without length; keep-alive needs one");
    }
    status
}
