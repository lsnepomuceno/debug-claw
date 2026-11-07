# ADR-0004: One daemon, with the desktop app and MCP as clients

- Status: accepted
- Date: 2026-11-07
- Evidence: [S3 report](../00-spikes/reports/s3-mcp-wait.md)

## Context

Xdebug connects to the debugger, not the other way around, so something has to listen on port 9003 for as long as the developer works. An MCP server launched over stdio lives only as long as the agent session that started it, and the desktop app may be closed while an agent debugs.

## Decision

A long-running daemon owns the listeners (DBGp, dump ingestion), the session state and storage. The desktop app and the MCP server are clients of the daemon:

- MCP is served by the daemon over Streamable HTTP.
- A small stdio shim forwards MCP traffic to the daemon, for clients that only launch MCP servers as commands.
- The desktop app talks to the daemon through a local API.

### Blocking tools

Spike S3 tested a blocking `wait` with the MCP Inspector CLI and Claude Code. These rules follow from it:

- A blocking tool takes a timeout whose default and maximum stay below 60 s, because the Inspector CLI and Claude Code over HTTP stop a call at 60 s with their defaults. When the timeout passes, the tool returns a normal result that says nothing happened, so the agent can call again.
- When a call carries a progress token, a blocking tool sends progress at least every 20 s.
- The daemon bounds every wait itself and caps how many run at once, because a client that gives up may close the connection without cancelling.
- The daemon serves both protocol revisions the clients use: 2025-11-25 with sessions and 2026-07-28 without.
- When the daemon declares the `logging` capability, it answers `logging/setLevel`. Nothing the agent must see is sent only as a notification.
- The stdio shim handles each request on its own, so a blocked call does not hold back a cancellation.

## Alternatives rejected

- The desktop app as the server, as LaraDumps does. Closing the app would stop debugging for the agent.
- An MCP server over stdio that also listens for Xdebug. Two agent sessions would compete for port 9003, and the state would vanish when the session ends.

## Consequences

- The daemon has to start without the desktop app and has to be discoverable by both clients. How it starts and how clients find it is [O-19](../OPEN.md#o-19-starting-and-finding-the-daemon).
- The S3 daemon was idle at 4.3 MB RSS, with no CPU and no context switches in 10 minutes. It had no DBGp listener, dump ingestion or storage, so milestone 1 measures the real daemon against the idle budget.
- Each open MCP stream wakes the daemon every 15 s for the SSE keep-alive. A blocked wait adds no CPU.
- The local API between the app and the daemon is a contract of its own, separate from MCP.
