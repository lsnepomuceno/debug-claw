# ADR-0004: One daemon, with the desktop app and MCP as clients

- Status: proposed
- Date: 2026-11-07
- Confirmed by: spike S3

## Context

Xdebug connects to the debugger, not the other way around, so something has to listen on port 9003 for as long as the developer works. An MCP server launched over stdio lives only as long as the agent session that started it, and the desktop app may be closed while an agent debugs.

## Decision

A long-running daemon owns the listeners (DBGp, dump ingestion), the session state and storage. The desktop app and the MCP server are clients of the daemon:

- MCP is served by the daemon over Streamable HTTP.
- A small stdio shim forwards MCP traffic to the daemon, for clients that only launch MCP servers as commands.
- The desktop app talks to the daemon through a local API.

## Alternatives rejected

- The desktop app as the server, as LaraDumps does. Closing the app would stop debugging for the agent.
- An MCP server over stdio that also listens for Xdebug. Two agent sessions would compete for port 9003, and the state would vanish when the session ends.

## Consequences

- The daemon has to start without the desktop app (as a service or on first use) and has to be discoverable by both clients.
- The local API between the app and the daemon is a contract of its own, separate from MCP.
