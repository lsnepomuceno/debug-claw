# Performance

Low memory use and low CPU use are requirements, with the same weight as features. The tool runs next to the application being debugged, often on a laptop that also runs Docker, an IDE and a browser, and it must not compete with them. See [ADR-0008](../specs/decisions/0008-resource-budgets.md).

## Where the cost comes from

The cost that matters most is the one added to the PHP request, because it distorts what the developer is measuring. The daemon and the desktop app come next.

- PHP side: Xdebug itself, the time each logpoint hit holds the request, the dump client when the daemon is down, and the optional extension.
- Daemon: idle wakeups, memory held per session and per event, and the size of what it parses and stores.
- Desktop app: repainting when nothing changed, and keeping data in memory that is not on screen.
- MCP: large results cost the agent's context as well as the daemon's memory.

## Rules

1. Nothing polls. The daemon reacts to sockets and timers, and the desktop app repaints only on input or when the daemon sends a change.
2. Memory is bounded everywhere. Every buffer, queue, session store and event store has a cap, and the oldest data goes first when a cap is reached. No component grows with the number of requests it has seen.
3. Data stays where it is until someone asks for it. Variables are read from Xdebug on demand, one level at a time. The desktop app holds only what is on screen and fetches the rest from the daemon.
4. Events go to disk, not to memory. The daemon keeps indexes and recent items in memory and reads the rest from storage.
5. Payloads have limits at the source. DBGp reads use `max_depth`, `max_children` and `max_data`. Dumps are truncated in the PHP client before they are sent. MCP results have a size cap and pagination.
6. The PHP side fails fast. When the daemon is not running, the dump client and the extension stop trying for a while instead of paying a connection attempt on every call.
7. The extension observes only what it reports. Functions it does not care about get no observer, so they cost nothing.
8. Every budget below is measured, and a milestone is not done while it misses one.

## Budgets

State: draft. The numbers below are starting targets. The spikes measure the real costs, and the targets are confirmed or revised in their reports before milestone 1 is specified ([O-08](../specs/OPEN.md#o-08-confirm-the-resource-budgets)).

| Component | Situation | Draft budget | Measured in |
|---|---|---|---|
| Daemon | Idle, no sessions | RSS at most 20 MB, CPU 0% with no periodic wakeups faster than once per second | S3 |
| Daemon | 10 sessions open, 10 000 stored events | RSS at most 100 MB | milestone 1 |
| Desktop app | Window open, nothing changing | CPU 0% (no repaint) | S4 |
| Desktop app | Showing the large S4 datasets | RSS at most 200 MB | S4 |
| Logpoint | One hit with 5 expressions, daemon and PHP on the same host | at most 5 ms added to the request | S2 |
| Dump client | Daemon down, after the first failed call | at most 0.1 ms per dump call | dump client milestone |
| Extension | Request with about 100 PDO queries | at most 5% added request time, and no added memory per request after it ends | S5 |
| IDE proxy | Relaying a PhpStorm session | at most 1 ms added per DBGp round trip | S6 |
| MCP | Default tool result | at most 32 KB, with pagination for the rest | milestone 1 |

## How it is measured

Each spike report and each milestone gives the numbers for its rows, with the command, the machine and the PHP and Xdebug versions used. Once code exists, the daemon and desktop app budgets become automated checks, so a regression fails the build instead of being found later.
