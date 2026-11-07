//! Spike S3 stdio shim: an MCP server on stdin/stdout that forwards every
//! message to the daemon over Streamable HTTP, and every message from the
//! daemon back to stdout.
//!
//! It does not interpret MCP. It moves JSON-RPC messages between rmcp's
//! stdio transport and its Streamable HTTP client transport, which handles
//! the session header, SSE responses and the standalone GET stream.

use rmcp::{
    RoleServer,
    transport::{IntoTransport, StreamableHttpClientTransport, Transport},
};

fn main() -> std::io::Result<()> {
    let url = std::env::args().nth(1).unwrap_or_else(|| "http://127.0.0.1:8390/mcp".into());
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(run(url))
}

async fn run(url: String) -> std::io::Result<()> {
    let mut down = IntoTransport::<RoleServer, std::io::Error, _>::into_transport(rmcp::transport::stdio());
    let mut up = StreamableHttpClientTransport::from_uri(url.clone());
    eprintln!("s3-shim: forwarding stdio to {url}");
    loop {
        tokio::select! {
            msg = down.receive() => match msg {
                Some(msg) => {
                    // send() returns a 'static future: spawn it, so a long
                    // tools/call does not hold up the next message.
                    let sending = up.send(msg);
                    tokio::spawn(async move {
                        if let Err(e) = sending.await {
                            eprintln!("s3-shim: send to daemon failed: {e}");
                        }
                    });
                }
                None => break,
            },
            msg = up.receive() => match msg {
                Some(msg) => {
                    if let Err(e) = down.send(msg).await {
                        eprintln!("s3-shim: write to stdout failed: {e}");
                        break;
                    }
                }
                None => {
                    eprintln!("s3-shim: daemon transport closed");
                    break;
                }
            },
        }
    }
    let _ = up.close().await;
    Ok(())
}
