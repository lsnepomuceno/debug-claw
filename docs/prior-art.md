# Prior art

Two projects shaped the idea. Neither is a dependency, and no code is ported from them. The observations below were made on 2026-11-07, from the default branch of each repository.

## LaraDumps

Repositories: [laradumps/laradumps](https://github.com/laradumps/laradumps), [laradumps/laradumps-core](https://github.com/laradumps/laradumps-core), [laradumps/app](https://github.com/laradumps/app).

- The desktop app is Electron with Vue 3.
- The PHP client sends each dump as JSON in an HTTP `POST` to `127.0.0.1:9191/api/dumps`, through curl, with a 100 ms connect timeout and a 1 s total timeout (`src/Dispatcher/Curl.php` in laradumps-core). When the app is closed, the request fails fast and the application keeps running.
- `laradumps-core` is framework agnostic: `ds()`, labels, colors, tables, JSON, timers and screens (named tabs that group dumps). Configuration lives in a `laradumps.yaml` per project.
- The Laravel package carries most of the value: observers for queries, slow queries, jobs, mail, cache, gates, the HTTP client and logs, plus profile collectors. All of them hook into framework events, so they do not exist outside Laravel.
- `global-laradumps` installs the client once for every project on the machine.
- There is no Xdebug integration, no breakpoints and no MCP server.

What we take: the dump vocabulary (labels, colors, screens, tables), the fail-fast transport, the idea of a global install, and a desktop app that is separate from the browser.

## Buggregator v2

Repository: [buggregator/server](https://github.com/buggregator/server).

- Version 2 is a rewrite in Go: one binary with an embedded SQLite database, a web UI served by the binary and a WebSocket hub.
- It aggregates other tools' protocols: Sentry, Ray, Symfony VarDumper (TCP 9912), Monolog (TCP 9913), SMTP, Inspector, XHProf profiles and raw HTTP dumps.
- Its `xdebug` module speaks DBGp in three modes: `proxy` (relays the session to the IDE and only reads), `standalone` (drives the session itself) and `auto`. Breakpoints (line and exception) are saved in SQLite and sent to every session it drives.
- It has an MCP server with `xdebug_sessions`, `xdebug_location`, `xdebug_stack`, `xdebug_variables`, `xdebug_property`, `xdebug_eval`, `xdebug_step`, `xdebug_continue`, `xdebug_stop`, `xdebug_wait` and breakpoint tools, plus profiler and event tools. Eval and control tools are off by default.
- Events are stored per protocol. There is no timeline that joins a request's dumps, logs, pauses and profile.

What we take: proxy mode as the way to coexist with an IDE, eval and control off by default, and a wait tool that blocks until a session pauses.

## Where debug-claw differs

- Native desktop app in Rust instead of a web UI or Electron.
- Xdebug is the baseline integration, not one module among many.
- Every event is tied to a request, and the request timeline is a first-class MCP tool.
- Logpoints (capture and continue) are the default way for an agent to read values, so it rarely needs to hold a worker paused.
- An optional PHP extension written in Rust (see [ADR-0005](../specs/decisions/0005-layered-php-integration.md)) can observe PDO, curl and exceptions in any application, without framework hooks.

Buggregator overlaps with much of the debugging scope. The differences above are the reason for a separate project, and each one has to hold up in the spikes and milestones.
