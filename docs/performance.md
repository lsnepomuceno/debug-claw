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
5. Payloads have limits at the source. DBGp reads use `max_depth`, `max_children` and `max_data`. These bound a read only at `max_depth` 1, so deeper levels are fetched one at a time and the daemon also caps what it returns ([ADR-0014](../specs/decisions/0014-reading-values.md)). Dumps are truncated in the PHP client before they are sent. MCP results have a size cap and pagination.
6. The PHP side fails fast. When the daemon is not running, the dump client and the extension stop trying for a while instead of paying a connection attempt on every call.
7. The extension observes only what it reports. Functions it does not care about get no observer, so they cost nothing.
8. Every budget below is measured, and a milestone is not done while it misses one.

## Budgets

State: draft. The numbers below are starting targets and stay in force until each row is decided ([O-08](../specs/OPEN.md#o-08-confirm-the-resource-budgets)). The spikes measured their rows, and their proposals are in the [next table](#measured-and-proposed).

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

### Measured and proposed

What the spike reports measured for each row, and the number each one proposes. A proposal is not a budget until O-08 decides it. Machines, commands and versions are in each report.

| Row | Measured | Proposed | Report |
|---|---|---|---|
| Daemon, idle | 4.3 MB RSS, no CPU and no context switches in 10 minutes, for a daemon with no DBGp listener, dump ingestion or storage. Each open MCP stream wakes it every 15 s for the SSE keep-alive. A blocked wait adds no CPU. | Keep the draft. The row stays open until milestone 1 measures the real daemon. | [S3](../specs/00-spikes/reports/s3-mcp-wait.md) |
| Daemon, 10 sessions and 10 000 events | Not measured. | None. | milestone 1 |
| Desktop app, idle | No frames and at most 2 CPU ticks in 60 s. With AccessKit on, the Linux adapter wakes the app about twice per second. | Keep CPU 0%. Whether the daemon's wakeup rule applies to the app is open. | [S4](../specs/00-spikes/reports/s4-egui.md) |
| Desktop app, large datasets | 127.2 MB RSS on the host GPU, 174.0 MB under software rendering, Linux only. | Keep 200 MB. 150 MB would hold on a GPU, to confirm on Windows and macOS first. | S4 |
| Logpoint, one hit with 5 expressions | 64 to 77 us one read at a time, 41 to 45 us with reads pipelined. | At most 0.2 ms. | [S2](../specs/00-spikes/reports/s2-logpoints.md) |
| Dump client | Not measured. | None. | dump client milestone |
| Extension, about 100 PDO queries | 0.79 us (PHP 8.5) and 0.95 us (PHP 8.3) added per observed call with the daemon up, 0.21 to 0.24 us and 0.46 to 0.48 us with it down. That is 34% and 42% of the fixture request, whose queries take about 0.5 us each. About 680 kB and 580 kB of RSS once per worker, and no growth per request. | At most 1 us per observed call with the daemon up and 0.5 us with it down, at most one write per request, worker RSS within 64 kB from request 1000 to 10 000 and `memory_get_usage` unchanged. Keep 5% only for an endpoint with realistic query times. | [S5](../specs/00-spikes/reports/s5-extension.md) |
| IDE proxy | 12 to 38 us added at the median and up to 173 us at p99 per round trip, with an emulator that replays PhpStorm 2026.3's captured commands. Slowest single round trip 726 us. Sessions with PhpStorm itself worked through the proxy. | At most 0.5 ms added per round trip at p99. | [S6](../specs/00-spikes/reports/s6-ide-proxy.md) |
| MCP, default tool result | With `max_depth` 1, `max_children` 32 and `max_data` 512, the worst single DBGp read S2 measured was 24 928 bytes of XML. The size after conversion to JSON was not measured. | Keep 32 KB. | S2 |

New rows the reports propose:

| Row | Measured | Proposed | Report |
|---|---|---|---|
| Daemon CPU per logpoint hit | About 40 us one read at a time, 24 us pipelined. | A row of its own. | S2 |
| Extension installed but switched off | With the observer registered and no handler running: 7 to 9% (PHP 8.5) and 15% (PHP 8.3) added on the call-heavy `/graph` endpoint. | Nothing added per call. | S5 |
| Desktop app frame time in the large views | At most 4.2 ms of CPU at p99 on the host GPU (4.83 ms of wall time). | For example p99 under 8 ms of CPU on a mid-range laptop GPU. | S4 |

## How it is measured

Each spike report and each milestone gives the numbers for its rows, with the command, the machine and the PHP and Xdebug versions used. Once code exists, the daemon and desktop app budgets become automated checks, so a regression fails the build instead of being found later.
