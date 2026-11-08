# ADR-0019: Starting and finding the daemon

- Status: accepted
- Date: 2026-11-07
- Answers: O-19

## Context

[ADR-0004](0004-daemon-and-clients.md) has a long-running daemon with MCP clients and the desktop app as its clients. The daemon has to be running before Xdebug connects, and the clients have to find it, without making the user set up a system service ([principle 3](../../docs/vision.md#principles)).

## Decision

1. The stdio shim starts the daemon when it cannot reach it, then relays as usual. The daemon it starts keeps running after the shim exits, so sessions survive the end of an agent session.
2. The daemon can also be started by hand, and later by the desktop app. A second daemon that finds the ports taken by the first one exits and says so.
3. The daemon serves MCP on one fixed default port on the loopback address. The shim and the app connect to that port. `design.md` picks the number.
4. The port can be changed by a setting that the daemon, the shim and the app all read. When the MCP port is taken by another program, the daemon exits with an error that names the port and the setting, and the shim passes that error to the client.
5. An MCP client configured for Streamable HTTP reaches a daemon that is already running. It does not start one.

## Alternatives rejected

- A user service per system (systemd, launchd, a Windows service). Three setups to document and support, and a process running at all times for a tool used now and then.
- A port chosen at random and written to a file the clients read. Clients over HTTP are configured with a URL, so the port has to be stable.
- Manual start only. The first thing an agent would hit is an error.

## Consequences

- R-04 of milestone 1 changes: the shim starts the daemon, and returns an error only when the start fails.
- How the daemon stops is not decided here. It runs until it is stopped by hand or the user logs out.
- A daemon started by the shim inherits the shim's environment, so the design has to make it independent of the working directory and of the agent's process group.
