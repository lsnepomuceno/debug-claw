# Spike S3: blocking MCP tools

Throwaway code for spike S3 (tasks T-14 to T-18). Nothing here is imported
by production code. The findings are in
[`specs/00-spikes/reports/s3-mcp-wait.md`](../../specs/00-spikes/reports/s3-mcp-wait.md).

## What is here

| Path | What it is |
|---|---|
| `src/bin/s3-daemon.rs` | Streamable HTTP MCP server (rmcp 3.9.0) with three tools: `wait` blocks until an event or its timeout, `fire` schedules an event, `sleep` just sleeps. `POST /fire?delay_s=N` schedules an event from outside MCP. Every event is also pushed as a log message, a resource update and two list-changed notifications. |
| `src/bin/s3-shim.rs` | stdio to HTTP forwarder: reads MCP on stdin and relays it to the daemon. |
| `compose.override.yaml` | Adds the `daemon` service (port `127.0.0.1:8390`) and the `inspector` service (profile `clients`) to the S3 copy of the test bed. |
| `inspector/` | Dockerfile for MCP Inspector 2.14.0 and two config files used to check the `requestTimeout` setting. |
| `claude/` | Temporary `--mcp-config` files for Claude Code, HTTP and stdio. They are passed per call and never registered. |
| `dc.sh` | `docker compose` with the S3 project name, ports and files. |
| `inspector.sh`, `claude.sh`, `shim-smoke.sh`, `measure.sh` | One script per kind of check. Each writes into `results/`. |
| `results/` | Raw outputs quoted by the report. |

## Run it

All commands start in this directory.

```sh
./dc.sh up -d --build toolchain
./dc.sh exec -T toolchain cargo build --release --manifest-path spikes/s3-mcp-wait/Cargo.toml
./dc.sh up -d daemon
./dc.sh --profile clients build inspector

./inspector.sh timeout-59        # also: event-45 timeout-90 progress-90 log-90 config-5
./shim-smoke.sh 3                # raw JSON-RPC through the shim
./claude.sh http-58 claude/http.json "Call the wait tool of the s3 MCP server once with timeout_s=58. Then reply with only the exact text the tool returned."
./measure.sh idle 600            # restarts the daemon, then samples it for 10 minutes
./measure.sh blocked 45 keepalive-15   # samples it during one wait call

./dc.sh --profile clients down   # keeps the volumes
```

`claude.sh` runs the Claude Code installed on the host in print mode with
`--strict-mcp-config` and the Haiku model. Each call is billed. Daemon flags
go through `S3_DAEMON_ARGS`, for example
`export S3_DAEMON_ARGS="--sse-keep-alive 0"` before `./dc.sh up -d daemon`.
Keep it exported for the scripts that follow: `./dc.sh run inspector`
starts the daemon it depends on, and compose recreates the daemon if the
variable differs.
