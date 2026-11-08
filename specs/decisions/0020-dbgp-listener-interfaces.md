# ADR-0020: Network interfaces of the DBGp listener

- Status: accepted
- Date: 2026-11-07
- Answers: O-22

## Context

PHP running on the host reaches the daemon on the loopback address. PHP in a container on Linux reaches the host through `host.docker.internal` mapped to `host-gateway` ([ADR-0012](0012-platforms.md)), which is the address of Docker's default bridge on the host, not loopback. Docker Desktop on Windows and macOS forwards `host.docker.internal` to the host's loopback. A listener on every interface would also accept connections from other machines on the network, and a DBGp session can run eval.

## Decision

1. The DBGp listener listens on the loopback address and, on Linux, on the host address of Docker's default bridge when that interface exists.
2. A setting makes it listen on every interface, for PHP on another machine or in a VM. The setup help says what that exposes.
3. The listener does not listen on other interfaces by default. `status` reports every address the listener has.
4. The MCP endpoint stays on loopback only, whatever this setting says ([R-02](../01-engine/requirements.md#r-02-mcp-over-streamable-http)).

## Alternatives rejected

- Every interface by default. Anyone on the same network could open a session, and with eval on, run code in it.
- Loopback only. PHP in a container on Linux would not reach the daemon, which is the main setup this tool targets.

## Consequences

- When the Docker bridge appears after the daemon starts (Docker started later), the listener has to pick it up. The design says how, without polling ([performance rule 1](../../docs/performance.md#rules)).
- No spike tested a daemon on the host with PHP in a container: the spikes ran everything inside the Compose network. Milestone 1 acceptance covers it.
- Containers on Compose networks reach the same bridge address through `host-gateway`. A setup that points Xdebug at another address of the host, such as a Compose network's own gateway, is not covered by rule 1 and needs the setting of rule 2. The setup help gives the `host.docker.internal` form so that this case stays rare.
