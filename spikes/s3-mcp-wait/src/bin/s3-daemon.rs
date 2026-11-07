//! Spike S3 daemon: an MCP server over Streamable HTTP whose `wait` tool
//! blocks until an event fires or a timeout passes.
//!
//! Events stand in for "a debug session paused". They come from a timer
//! scheduled with the `fire` tool or with `POST /fire?delay_s=N`. When an
//! event fires, the daemon also pushes unsolicited notifications to every
//! connected client, to see which ones reach the agent.
//!
//! The runtime is single-threaded and nothing polls: with no client and no
//! timer scheduled, the process only sits in `epoll_wait`.

// MCP logging is deprecated by SEP-2577 in rmcp 3.9, but it is one of the
// notification kinds this spike tests.
#![allow(deprecated)]

use std::{
    convert::Infallible,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use bytes::Bytes;
use http_body_util::{BodyExt, Full, combinators::BoxBody};
use hyper::{Request, Response, body::Incoming, server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;
use rmcp::{
    ErrorData, Peer, RoleServer, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolResult, ContentBlock, ListResourcesResult, LoggingLevel,
        LoggingMessageNotificationParam, PaginatedRequestParams, ProgressNotificationParam,
        ReadResourceRequestParams, ReadResourceResponse, ReadResourceResult, Resource, ResourceContents,
        ResourceUpdatedNotificationParam, ServerCapabilities, ServerConfig, SetLevelRequestParams, SubscribeRequestParams,
        SubscriptionFilter,
    },
    service::{NotificationContext, RequestContext, SubscriptionContext},
    tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use serde::Deserialize;
use tokio::sync::broadcast;

const SESSIONS_URI: &str = "debug-claw://sessions";
/// Cap on remembered legacy peers, so the list cannot grow without bound.
const MAX_PEERS: usize = 64;
/// Cap on queued events per subscriber. A slow waiter sees `Lagged`.
const EVENT_QUEUE: usize = 16;

#[derive(Clone, Debug)]
struct Event {
    seq: u64,
    source: &'static str,
    at: Instant,
}

/// State shared by every MCP session and the HTTP trigger.
struct Hub {
    events: broadcast::Sender<Event>,
    seq: AtomicU64,
    started: Instant,
    /// Peers of legacy (session-based) clients, captured on `initialized`.
    peers: Mutex<Vec<Peer<RoleServer>>>,
}

impl Hub {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            events: broadcast::channel(EVENT_QUEUE).0,
            seq: AtomicU64::new(0),
            started: Instant::now(),
            peers: Mutex::new(Vec::new()),
        })
    }

    fn uptime(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    /// Fires one event now: wakes every `wait`, then pushes unsolicited
    /// notifications to the legacy peers.
    fn fire(self: &Arc<Self>, source: &'static str) -> u64 {
        let seq = self.seq.fetch_add(1, Ordering::Relaxed) + 1;
        let waiters = self.events.send(Event { seq, source, at: Instant::now() }).unwrap_or(0);
        let peers: Vec<_> = {
            let mut peers = self.peers.lock().unwrap();
            peers.retain(|p| !p.is_transport_closed());
            peers.clone()
        };
        eprintln!(
            "[{:9.3}] event #{seq} from {source}: {waiters} receivers, {} legacy peers",
            self.uptime(),
            peers.len()
        );
        for peer in peers {
            let hub = self.clone();
            tokio::spawn(async move { hub.push_legacy(&peer, seq).await });
        }
        seq
    }

    async fn push_legacy(&self, peer: &Peer<RoleServer>, seq: u64) {
        let data = serde_json::json!({ "event": "session_paused", "seq": seq });
        let log = LoggingMessageNotificationParam::new(LoggingLevel::Warning, data)
            .with_logger("debug-claw");
        let results = [
            ("message", peer.notify_logging_message(log).await.err()),
            (
                "resources/updated",
                peer.notify_resource_updated(ResourceUpdatedNotificationParam::new(SESSIONS_URI))
                    .await
                    .err(),
            ),
            ("resources/list_changed", peer.notify_resource_list_changed().await.err()),
            ("tools/list_changed", peer.notify_tool_list_changed().await.err()),
        ];
        for (name, err) in results {
            match err {
                None => eprintln!("[{:9.3}]   pushed notifications/{name} (legacy)", self.uptime()),
                Some(e) => eprintln!("[{:9.3}]   notifications/{name} failed: {e}", self.uptime()),
            }
        }
    }

    fn schedule(self: &Arc<Self>, delay_s: u64, source: &'static str) {
        let hub = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(delay_s)).await;
            hub.fire(source);
        });
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct WaitParams {
    /// Seconds to wait before giving up. Default 60.
    timeout_s: Option<u64>,
    /// Send a progress notification every N seconds while waiting, if the
    /// client sent a progress token.
    progress_every_s: Option<u64>,
    /// Send a logging notification every N seconds while waiting.
    log_every_s: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct FireParams {
    /// Seconds from now until the event fires. Default 0.
    delay_s: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SleepParams {
    /// Seconds to sleep. Default 10.
    seconds: Option<u64>,
}

#[derive(Clone)]
struct Daemon {
    hub: Arc<Hub>,
    #[allow(dead_code, reason = "read by the tool_handler macro")]
    tool_router: ToolRouter<Self>,
}

fn every(n: Option<u64>) -> Option<tokio::time::Interval> {
    n.filter(|n| *n > 0).map(|n| {
        let period = Duration::from_secs(n);
        tokio::time::interval_at(tokio::time::Instant::now() + period, period)
    })
}

async fn tick(i: &mut Option<tokio::time::Interval>) {
    match i {
        Some(i) => {
            i.tick().await;
        }
        None => std::future::pending().await,
    }
}

#[tool_router]
impl Daemon {
    fn new(hub: Arc<Hub>) -> Self {
        Self { hub, tool_router: Self::tool_router() }
    }

    #[tool(description = "Block until a debug session pauses (an event fires) or the timeout \
        passes. Returns which happened and how long it waited.")]
    async fn wait(
        &self,
        Parameters(p): Parameters<WaitParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let timeout = Duration::from_secs(p.timeout_s.unwrap_or(60));
        let token = ctx.meta.get_progress_token();
        let started = Instant::now();
        let mut rx = self.hub.events.subscribe();
        let mut progress = if token.is_some() { every(p.progress_every_s) } else { None };
        let mut log = every(p.log_every_s);
        let deadline = tokio::time::sleep(timeout);
        tokio::pin!(deadline);
        eprintln!(
            "[{:9.3}] wait start: timeout {}s, progress token {:?}, progress_every {:?}, log_every {:?}",
            self.hub.uptime(),
            timeout.as_secs(),
            token,
            p.progress_every_s,
            p.log_every_s
        );
        let mut sent = 0u32;
        let outcome = loop {
            tokio::select! {
                ev = rx.recv() => match ev {
                    Ok(ev) => break format!(
                        "paused: event #{} from {} after {:.1} s",
                        ev.seq, ev.source, ev.at.duration_since(started).as_secs_f64()
                    ),
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break "daemon stopping".into(),
                },
                () = &mut deadline => break format!(
                    "timeout: no event after {:.1} s", started.elapsed().as_secs_f64()
                ),
                () = ctx.ct.cancelled() => {
                    eprintln!("[{:9.3}] wait cancelled by client after {:.1} s",
                        self.hub.uptime(), started.elapsed().as_secs_f64());
                    return Err(ErrorData::internal_error("cancelled", None));
                }
                () = tick(&mut progress) => {
                    sent += 1;
                    let elapsed = started.elapsed().as_secs_f64();
                    let param = ProgressNotificationParam::new(token.clone().unwrap(), elapsed)
                        .with_total(timeout.as_secs_f64())
                        .with_message(format!("waiting for a pause, {elapsed:.0} s"));
                    if let Err(e) = ctx.peer.notify_progress(param).await {
                        eprintln!("[{:9.3}] progress failed: {e}", self.hub.uptime());
                    }
                }
                () = tick(&mut log) => {
                    sent += 1;
                    let data = serde_json::json!({
                        "waiting_s": started.elapsed().as_secs(),
                    });
                    let param = LoggingMessageNotificationParam::new(LoggingLevel::Info, data)
                        .with_logger("debug-claw");
                    if let Err(e) = ctx.peer.notify_logging_message(param).await {
                        eprintln!("[{:9.3}] log failed: {e}", self.hub.uptime());
                    }
                }
            }
        };
        eprintln!(
            "[{:9.3}] wait end: {outcome} ({sent} notifications sent)",
            self.hub.uptime()
        );
        Ok(CallToolResult::success(vec![ContentBlock::text(outcome)]))
    }

    #[tool(description = "Schedule a debug session pause (an event) after delay_s seconds.")]
    async fn fire(&self, Parameters(p): Parameters<FireParams>) -> String {
        let delay = p.delay_s.unwrap_or(0);
        self.hub.schedule(delay, "fire tool");
        format!("event scheduled in {delay} s")
    }

    #[tool(description = "Sleep for some seconds without watching for events, then return.")]
    async fn sleep(&self, Parameters(p): Parameters<SleepParams>) -> String {
        let s = p.seconds.unwrap_or(10);
        tokio::time::sleep(Duration::from_secs(s)).await;
        format!("slept {s} s")
    }
}

#[tool_handler]
impl ServerHandler for Daemon {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_tool_list_changed()
                .enable_resources()
                .enable_resources_subscribe()
                .enable_resources_list_changed()
                .enable_logging()
                .build(),
        )
        .with_instructions(
            "Spike S3. Call `wait` to block until a debug session pauses. \
             Call `fire` to schedule a pause.",
        )
    }

    async fn on_initialized(&self, ctx: NotificationContext<RoleServer>) {
        let info = ctx.peer.peer_info();
        eprintln!(
            "[{:9.3}] initialized: client {:?}, protocol {:?}",
            self.hub.uptime(),
            info.as_ref().map(|i| format!("{} {}", i.client_info.name, i.client_info.version)),
            info.as_ref().map(|i| i.protocol_version.to_string()),
        );
        let mut peers = self.hub.peers.lock().unwrap();
        peers.retain(|p| !p.is_transport_closed());
        if peers.len() >= MAX_PEERS {
            peers.remove(0);
        }
        peers.push(ctx.peer);
    }

    async fn set_level(
        &self,
        req: SetLevelRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        eprintln!("[{:9.3}] logging/setLevel {:?}", self.hub.uptime(), req.level);
        Ok(())
    }

    async fn list_resources(
        &self,
        _req: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        let mut r = Resource::new(SESSIONS_URI, "sessions");
        r.description = Some("Debug sessions and the last pause".into());
        Ok(ListResourcesResult::with_all_items(vec![r]))
    }

    async fn read_resource(
        &self,
        req: ReadResourceRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        let n = self.hub.seq.load(Ordering::Relaxed);
        Ok(ReadResourceResult::new(vec![ResourceContents::text(
            format!("{n} pauses so far"),
            req.uri,
        )]).into())
    }

    #[allow(deprecated)]
    async fn subscribe(
        &self,
        req: SubscribeRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        eprintln!("[{:9.3}] resources/subscribe {}", self.hub.uptime(), req.uri);
        Ok(())
    }

    fn accepted_subscription_filter(&self, requested: &SubscriptionFilter) -> Option<SubscriptionFilter> {
        eprintln!("[{:9.3}] subscriptions/listen requested {requested:?}", self.hub.uptime());
        Some(requested.clone())
    }

    /// Pushes events to clients on the 2026-07-28 revision, which have no
    /// session and receive server notifications only through this stream.
    async fn listen(&self, ctx: SubscriptionContext) -> Result<(), ErrorData> {
        let mut rx = self.hub.events.subscribe();
        loop {
            tokio::select! {
                () = ctx.cancelled() => return Ok(()),
                ev = rx.recv() => match ev {
                    Ok(ev) => {
                        let sink = ctx.sink();
                        let r1 = sink.notify_resource_updated(SESSIONS_URI).await;
                        let r2 = sink.notify_resource_list_changed().await;
                        let r3 = sink.notify_tool_list_changed().await;
                        eprintln!("[{:9.3}]   listen push for event #{}: {r1:?} {r2:?} {r3:?}",
                            self.hub.uptime(), ev.seq);
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => return Ok(()),
                },
            }
        }
    }
}

type Body = BoxBody<Bytes, Infallible>;

fn text(status: u16, s: String) -> Response<Body> {
    Response::builder().status(status).body(Full::new(Bytes::from(s)).boxed()).unwrap()
}

fn query_u64(q: Option<&str>, key: &str) -> Option<u64> {
    q?.split('&').find_map(|kv| kv.strip_prefix(key)?.strip_prefix('=')?.parse().ok())
}

fn main() -> std::io::Result<()> {
    let mut listen = "127.0.0.1:8390".to_string();
    let mut hosts: Vec<String> = vec!["localhost".into(), "127.0.0.1".into(), "::1".into()];
    let mut json_response = false;
    let mut keep_alive: Option<u64> = Some(15);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--listen" => listen = args.next().expect("--listen ADDR"),
            "--allow-host" => hosts.push(args.next().expect("--allow-host NAME")),
            "--json-response" => json_response = true,
            "--sse-keep-alive" => {
                keep_alive = Some(args.next().expect("--sse-keep-alive SECS").parse().unwrap())
                    .filter(|s| *s > 0)
            }
            other => panic!("unknown argument {other}"),
        }
    }
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(serve(listen, hosts, json_response, keep_alive))
}

async fn serve(
    listen: String,
    hosts: Vec<String>,
    json_response: bool,
    keep_alive: Option<u64>,
) -> std::io::Result<()> {
    let hub = Hub::new();
    let config = StreamableHttpServerConfig::default()
        .with_allowed_hosts(hosts.clone())
        .with_json_response(json_response)
        .with_sse_keep_alive(keep_alive.map(Duration::from_secs));
    let mcp: StreamableHttpService<Daemon, LocalSessionManager> = StreamableHttpService::new(
        {
            let hub = hub.clone();
            move || Ok(Daemon::new(hub.clone()))
        },
        Default::default(),
        config,
    );
    let listener = tokio::net::TcpListener::bind(&listen).await?;
    eprintln!(
        "listening on {listen}, allowed hosts {hosts:?}, json_response {json_response}, \
         sse keep-alive {keep_alive:?}"
    );
    loop {
        let (stream, peer) = listener.accept().await?;
        let mcp = mcp.clone();
        let hub = hub.clone();
        tokio::spawn(async move {
            let svc = service_fn(move |req: Request<Incoming>| {
                let mcp = mcp.clone();
                let hub = hub.clone();
                async move {
                    let h = req.headers();
                    let hv = |k: &str| h.get(k).and_then(|v| v.to_str().ok()).unwrap_or("-").to_owned();
                    eprintln!(
                        "[{:9.3}] http {peer} {} {} session={} version={} accept={}",
                        hub.uptime(),
                        req.method(),
                        req.uri().path(),
                        hv("mcp-session-id"),
                        hv("mcp-protocol-version"),
                        hv("accept"),
                    );
                    if req.uri().path() == "/fire" {
                        let delay = query_u64(req.uri().query(), "delay_s").unwrap_or(0);
                        hub.schedule(delay, "http trigger");
                        return Ok::<_, Infallible>(text(200, format!("event scheduled in {delay} s\n")));
                    }
                    if req.uri().path() != "/mcp" {
                        return Ok(text(404, "not found\n".into()));
                    }
                    Ok(mcp.handle(req).await)
                }
            });
            if let Err(e) = http1::Builder::new().serve_connection(TokioIo::new(stream), svc).await {
                eprintln!("connection {peer}: {e}");
            }
        });
    }
}
