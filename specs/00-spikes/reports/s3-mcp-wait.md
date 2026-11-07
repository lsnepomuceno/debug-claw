# S3 Blocking MCP tools: report

Spike code: [`spikes/s3-mcp-wait/`](../../../spikes/s3-mcp-wait/). Raw outputs of every run are in its `results/` folder, and the sections below quote them.

## Versions

| Component | Version |
|---|---|
| rmcp (server, HTTP client, stdio) | 3.9.0, Apache-2.0 |
| Rust and cargo, in the `toolchain` container | 1.99.1 |
| tokio, hyper, reqwest | 1.54.1, 1.12.0, 0.13.5 |
| Claude Code, on the host | 2.1.319 |
| MCP Inspector, in `node:22.23.3-alpine` | 2.14.0, which bundles `@modelcontextprotocol/client` 2.4.0 |

The build graph has 124 crates. Their licenses are MIT, MIT OR Apache-2.0, Unicode-3.0, Unlicense OR MIT, and Apache-2.0 alone for `rmcp`, `rmcp-macros` and `sync_wrapper`. The release binaries are 4.8 MB (`s3-daemon`) and 5.8 MB (`s3-shim`), stripped.

rmcp 3.9.0 speaks two protocol revisions. 2025-11-25 is the last one with `initialize`, sessions (`Mcp-Session-Id`) and a GET stream for server notifications. 2026-07-28 has no sessions: the client discovers the server per request and opens `subscriptions/listen` to receive list-changed and resource-updated notifications. The two clients picked different revisions:

| Client | Revision | How it was seen |
|---|---|---|
| Claude Code, HTTP and stdio | 2026-07-28 | debug log: `"protocolEra":"modern","negotiatedProtocolVersion":"2026-07-28"`. `MCP_PROTOCOL_NEGOTIATION=legacy` switches it to 2025-11-25. |
| Inspector CLI | 2025-11-25 | daemon log: `initialized: client Some("inspector-cli 2.14.0"), protocol Some("2025-11-25")`. The CLI's default `--protocol-era` is `legacy`. |

## How to rerun

From `spikes/s3-mcp-wait/`. `dc.sh` runs `docker compose` with the project name `testbed-s3`, the test bed's compose file and `compose.override.yaml`.

```sh
./dc.sh up -d --build toolchain
./dc.sh exec -T toolchain cargo build --release --manifest-path spikes/s3-mcp-wait/Cargo.toml
./dc.sh up -d daemon
./dc.sh --profile clients build inspector

./inspector.sh timeout-59      # criterion 1; also event-45
./inspector.sh timeout-90      # criterion 2; also progress-90 log-90 config-5
./shim-smoke.sh 3              # criterion 3, raw JSON-RPC through the shim
./claude.sh http-58 claude/http.json "Call the wait tool of the s3 MCP server once with timeout_s=58. Then reply with only the exact text the tool returned."
./measure.sh idle 600          # criterion 5
./measure.sh blocked 45 keepalive-15   # criterion 6
export S3_DAEMON_ARGS="--sse-keep-alive 0" && ./dc.sh up -d daemon
./measure.sh blocked 45 keepalive-0
./dc.sh --profile clients down
```

The prompts of the other Claude Code cases are in the table under criterion 2. Each `claude.sh` call runs `claude -p` with `--mcp-config` pointing at `claude/http.json` or `claude/stdio.json`, `--strict-mcp-config`, `--model haiku` and `--debug-file`. Nothing was added to the Claude Code configuration. Results keep the JSON summary and the debug lines of the `s3` server. The stdio entry runs `docker exec -i testbed-s3-daemon-1 /target/release/s3-shim http://127.0.0.1:8390/mcp`. The spike made 9 Claude Code calls, for US$ 0.03 in total as reported by `total_cost_usd`.

## The spike daemon

`s3-daemon` serves Streamable HTTP on `127.0.0.1:8390/mcp` with the rmcp defaults: SSE responses, a keep-alive comment every 15 s on open streams, sessions on for 2025-11-25 clients. It runs on a current-thread tokio runtime. Tools:

- `wait(timeout_s, progress_every_s, log_every_s)` blocks until an event or its own timeout, then returns a normal result: `paused: event #N from SOURCE after X s` or `timeout: no event after X s`. If the request carries a progress token and `progress_every_s` is set, it sends progress notifications. If `log_every_s` is set, it sends log messages.
- `fire(delay_s)` schedules an event. `POST /fire?delay_s=N` does the same from outside MCP.
- `sleep(seconds)` sleeps and ignores events.

Each event wakes every `wait` through a tokio broadcast channel. It is also pushed to every 2025-11-25 session as `notifications/message`, `notifications/resources/updated`, `notifications/resources/list_changed` and `notifications/tools/list_changed`, and to every open `subscriptions/listen` stream as the notifications that stream accepts.

`s3-shim` reads MCP from stdin and relays every message to the daemon with the rmcp Streamable HTTP client. It is 57 lines.

## Criterion 1: a 60 s `wait` returns in Claude Code and the Inspector

Met in both clients.

| Client | Call | Result | Wall time | File |
|---|---|---|---|---|
| Inspector CLI | `wait timeout_s=59` | `timeout: no event after 59.0 s` | 59.7 s | `results/inspector-timeout-59.txt` |
| Inspector CLI | `wait timeout_s=60`, event fired by `POST /fire?delay_s=45` | `paused: event #1 from http trigger after 44.5 s` | 45.2 s | `results/inspector-event-45.txt` |
| Claude Code, HTTP | `wait timeout_s=58` | `timeout: no event after 58.0 s`, the agent's reply quoting it | 63.8 s for the whole `claude -p` run, `Tool 'wait' completed successfully in 58s` | `results/claude-http-58.*` |

The Inspector command is:

```sh
./dc.sh --profile clients run --rm -T inspector --cli http://daemon:8390/mcp \
    --transport http --method tools/call --tool-name wait --tool-arg timeout_s=59
```

The first Inspector call failed with `{"error":{"code":"error","message":"logging/setLevel"}}`: the CLI sets the log level to `debug` right after connecting and stops if the server refuses. A server that declares the `logging` capability must answer `logging/setLevel`, so the daemon now implements it.

## Criterion 2: timeouts, how to change them, and progress

| Client and transport | Limit on one tool call | How to change it | Progress notifications |
|---|---|---|---|
| Inspector CLI 2.14.0, HTTP | 60 s, from the SDK's `DEFAULT_REQUEST_TIMEOUT_MSEC = 6e4` | Not from the CLI: no flag, and `requestTimeout` in a `--config` server entry is not applied | Do not keep the call alive |
| Claude Code 2.1.319, HTTP | 60 s, the HTTP transport's `timeoutMs` | `MCP_TOOL_TIMEOUT` (ms) raises the transport timer too | Keep the call alive past 60 s |
| Claude Code 2.1.319, stdio | none at 60 s: a 120 s wait returned | `MCP_TOOL_TIMEOUT` (ms) sets a hard limit | Do not extend `MCP_TOOL_TIMEOUT` |

Inspector runs, all with the default settings (`results/inspector-*.txt`):

| Case | Call | Result |
|---|---|---|
| `timeout-90` | `wait timeout_s=90` | `Request timed out after 1m00s (tools/call)`, wall 60.8 s |
| `progress-90` | the same plus `progress_every_s=20 --tool-metadata progressToken=p90` | the same error at 60.8 s. The daemon got the token and sent 4 progress notifications. |
| `log-90` | the same with `log_every_s=20` | the same error at 60.9 s. The error text counts 3 events delivered on the GET stream. |
| `config-5` | `--config inspector/short-timeout.json` with `"requestTimeout": 5000`, `wait timeout_s=8` | `timeout: no event after 8.0 s`: the setting was ignored |

The CLI source explains these results. `clients/cli/build/index.js` builds its `InspectorClient` with `progress: false` and no `timeout`, so the SDK default of 60 s applies and no progress handler is registered. `InspectorClient` itself has `resetTimeoutOnProgress` on by default and a `requestTimeout` setting, which the web UI exposes. The web UI was not tested.

Claude Code runs (`results/claude-*`). Prompts follow the pattern "Call the wait tool of the s3 MCP server once with timeout_s=N. Then reply with only the exact text the tool returned, or the exact error."

| Case | Transport | Environment | Call | Result |
|---|---|---|---|---|
| `http-58` | HTTP | default | `timeout_s=58` | returned after 58 s |
| `http-120` | HTTP | default | `timeout_s=120` | `The operation timed out.` after 60 s. Debug log: `code=tool_mcp_transport_error type=TimeoutError`. The daemon logged `wait cancelled by client after 60.0 s`. |
| `http-120-progress` | HTTP | default | `timeout_s=120 progress_every_s=20` | returned after 2m 0s, 5 progress notifications sent |
| `http-120-tooltimeout` | HTTP | `MCP_TOOL_TIMEOUT=150000` | `timeout_s=120` | returned after 2m 0s. The transport options in the debug log changed from `"timeoutMs":60000` to `"timeoutMs":150000`. |
| `stdio-120` | stdio shim | default | `timeout_s=120` | returned after 2m 0s |
| `stdio-tooltimeout-20` | stdio shim | `MCP_TOOL_TIMEOUT=20000` | `timeout_s=40 progress_every_s=5` | `MCP server "s3" tool "wait" timed out after 20s`. The daemon logged `wait cancelled by client after 20.0 s`. |

Claude Code sends a progress token with every tool call (the daemon logged `progress token Some(ProgressToken(Number(2)))` in calls where the prompt did not ask for one). It also cancels the request when it gives up, and the cancellation reaches the daemon over HTTP and through the shim.

The `http-120-progress` case does not tell apart the two readings of the HTTP limit: a timer to the first byte of the response, which the first progress notification at 20 s satisfies, or a timer that each progress notification resets. The Claude Code MCP documentation describes the HTTP limit as a timer to the first response byte, set to the largest of 60 s, the tool timeout and `MCP_TIMEOUT`. It also documents what was not tested here: a per-server `timeout` field in milliseconds, and an idle timeout (no response and no progress) of 5 minutes for HTTP and 30 minutes for stdio, set with `CLAUDE_CODE_MCP_TOOL_IDLE_TIMEOUT` and reset by progress.

When the Inspector CLI timed out it exited without sending `notifications/cancelled` and without `DELETE`. rmcp did not cancel the handler when the connection closed: the three 90 s waits ran to their own deadline (`wait end: timeout: no event after 90.0 s`) and the daemon kept sending progress and log notifications to a gone client without an error.

## Criterion 3: the same tool through a stdio shim

Met. Claude Code ran the shim as a stdio server with `claude/stdio.json`:

| Case | Call | Result |
|---|---|---|
| `stdio-event` | `wait timeout_s=60`, event fired by `POST /fire?delay_s=40` before the run | `paused: event #2 from http trigger after 32.1 s` |
| `stdio-120` | `wait timeout_s=120` | `timeout: no event after 120.0 s` |
| `stdio-tooltimeout-20` | see criterion 2 | cancelled at 20 s, and the cancel reached the daemon |

Claude Code negotiated 2026-07-28 through the shim, and the daemon saw the shim's requests with `version=2026-07-28` and no session. `./shim-smoke.sh 3` checks the 2025-11-25 path by hand: the shim relayed the `initialize` answer, three progress notifications and the result.

The shim has no state of its own. It spawns each outgoing request so a blocked `wait` does not hold back other messages, which is how cancellations got through during a wait.

## Criterion 4: notifying a client without a request

The daemon can push a notification when an event happens, in both protocol revisions. Neither client passes it to the agent.

| Revision | What the daemon pushed | Result in the daemon log |
|---|---|---|
| 2025-11-25 (Claude Code with `MCP_PROTOCOL_NEGOTIATION=legacy`) | `notifications/message`, `resources/updated`, `resources/list_changed`, `tools/list_changed` on the session's GET stream | all four `pushed ... (legacy)` |
| 2026-07-28 (Claude Code default) | the same three resource and tool notifications on the `subscriptions/listen` stream | `tools/list_changed` and `resources/list_changed` accepted. `resources/updated` refused with `NotificationNotAccepted`, because Claude Code's filter asks for `tools_list_changed` and `resources_list_changed` and no resource subscriptions. |

In 2026-07-28 a log message cannot be sent outside a request: `subscriptions/listen` carries no log notifications, and rmcp 3.9.0 marks logging deprecated (SEP-2577).

The two `notify-*` runs asked Claude Code to call `fire delay_s=5`, then `sleep seconds=15`, then say whether it saw anything from the server besides the two results. The event arrived during `sleep`. In both runs the debug log shows only `Received tools/list_changed notification, refreshing tools`, and the agent answered that it saw nothing else. Claude Code did not send `logging/setLevel` or `resources/subscribe`. The log message did not appear in the debug log at all.

The Inspector CLI makes one request and exits. Its GET stream received the pushed events (the `log-90` error counts them), but the CLI prints only the response.

Not tested: the Inspector web UI, which lists server notifications, and Claude Code channels. Channels are a research preview in which a stdio server declares the `claude/channel` capability and sends `notifications/claude/channel` messages that reach the agent. The Claude Code documentation says they need `--channels` or `--dangerously-load-development-channels`, are ignored in `-p` mode, and are not delivered when the server negotiates 2026-07-28, so they could not be tested here.

The conclusion for the design is that a pause reaches the agent only as the result of a call, such as a blocked `wait`.

## Criterion 5: the idle daemon over 10 minutes

Met. `./measure.sh idle 600` restarts the daemon container, waits 5 s, then reads `/proc/<pid>/status` and `/proc/<pid>/task/*/stat` from the host at the start and the end of a 600 s window. No client connected during the window (`results/measure-idle.txt`):

| Measure | Start | End | Change over 600 s |
|---|---|---|---|
| VmRSS | 4,360 kB | 4,360 kB | 0 |
| Threads | 1 | 1 | 0 |
| utime + stime (ticks of 10 ms) | 0 | 0 | 0 |
| Voluntary context switches | 1 | 1 | 0 |
| Involuntary context switches | 0 | 0 | 0 |

The budget in [docs/performance.md](../../../docs/performance.md#budgets) is RSS at most 20 MB, CPU 0%, and no periodic wakeups faster than once per second. The daemon used 4.3 MB and was not scheduled once in 10 minutes. The tick count only shows that CPU stayed under 10 ms, but zero context switches mean the thread never ran. With no client there is no open stream, so the SSE keep-alive of criterion 6 does not run either.

The daemon ran in a container on a host where other spikes ran their own containers at the same time. The numbers are the daemon's own counters in `/proc`, so the other containers' work is not in them.

## Criterion 6: CPU of a blocked `wait`

Met. `./measure.sh blocked 45 LABEL` starts an Inspector CLI call of `wait timeout_s=60`, waits 10 s for it to block, then samples the daemon for 45 s while it waits with no progress and no log notifications:

| Daemon setting | CPU ticks | Context switches | Wakeups per second | VmRSS | File |
|---|---|---|---|---|---|
| default, SSE keep-alive every 15 s | 0 | 15 voluntary, 1 involuntary | 0.36 | 5,796 kB | `results/measure-blocked-keepalive-15.txt` |
| `--sse-keep-alive 0` | 0 | 0 | 0 | 5,868 kB | `results/measure-blocked-keepalive-0.txt` |

The wait itself costs nothing: a pending `tokio::select!` over a broadcast receiver, a timer and the cancellation token. The wakeups in the default run come from rmcp's SSE keep-alive. The Inspector holds two streams open (the GET stream and the POST response), and each gets a comment every 15 s. That is well under one wakeup per second, and it stops when the client leaves. RSS was about 1.5 MB higher than in the idle run once clients had connected.

## Not verified

- The Inspector web UI: its `requestTimeout` setting, `resetTimeoutOnProgress` and how it shows notifications.
- Claude Code channels, which need an interactive session.
- Claude Code's per-server `timeout` field and the idle timeout. The documentation describes both. No call ran longer than 2 minutes.
- Whether progress resets the HTTP timer in Claude Code or only satisfies a first-byte timer. A first progress notification after 60 s would tell them apart.
- Claude Code on Windows and macOS, and other MCP clients (Cursor, Codex, Gemini CLI).
- The idle numbers come from a daemon that only serves MCP. The real daemon also listens for DBGp and dumps and keeps storage.

## Spec changes

ADRs:

- [ADR-0004](../../decisions/0004-daemon-and-clients.md): accept. The daemon served a 60 s `wait` over Streamable HTTP to both clients, the stdio shim worked for Claude Code with cancellations, and the idle daemon fits the budget (criterion 5).
- [ADR-0002](../../decisions/0002-mcp-first.md) stays as it is. Its consequence that long operations need blocking tools holds, with the timeout rules below.

[OPEN.md](../../OPEN.md):

- O-08, "Daemon idle" row: keep RSS at most 20 MB, CPU 0% and no wakeups faster than once per second. The spike daemon measured 4.3 MB, no CPU and no wakeups in 10 minutes, but it has no DBGp listener, dump ingestion or storage, so the row stays open until milestone 1 measures the real daemon. Add to the row that with an MCP client connected each open stream wakes the daemon every 15 s for the SSE keep-alive, and that a blocked `wait` adds nothing to that.
- Add a question: how the agent learns of a pause it is not waiting for. Server notifications do not reach the agent in the clients tested. The options are to tell the agent to keep a `wait` running, Claude Code channels (research preview, stdio only, legacy protocol only), or nothing until clients surface notifications.

MCP design notes for milestone 1:

- Blocking tools take a timeout whose default and maximum stay below 60 s, because the Inspector CLI and Claude Code over HTTP both stop at 60 s with their defaults. When the timeout passes they return a normal result that says nothing happened, so the agent can call again.
- When the request has a progress token, a blocking tool sends progress at least every 20 s. Claude Code sends a token with every call, and progress kept a 120 s HTTP call alive. It does not help the Inspector CLI.
- The daemon bounds every wait itself and caps how many run at once. A client that gives up may close the connection without cancelling, and rmcp keeps the handler running until it returns.
- Serve both protocol revisions. Claude Code uses 2026-07-28 without sessions over HTTP and stdio, and the Inspector CLI uses 2025-11-25 with sessions.
- Answer `logging/setLevel` if the `logging` capability is declared, or the Inspector CLI fails to connect. Do not rely on log notifications for anything the agent must see.
- The stdio shim spawns each request so a blocked call does not hold back a cancellation.
