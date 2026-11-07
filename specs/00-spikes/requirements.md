# 00: Spikes

State: active.

This milestone answers the questions that would change the design if the answer came out wrong. Each spike is a question with exit criteria. A spike ends when its exit criteria are met or when it shows the assumption does not hold. Both outcomes are results.

Spikes produce reports in [`reports/`](reports/), not product code. See [CONVENTIONS.md](../CONVENTIONS.md#spike-code).

Every spike that has rows in the [budget table](../../docs/performance.md#budgets) measures them and proposes a final number ([ADR-0008](../decisions/0008-resource-budgets.md)). Measurements give the command, the machine, and the PHP and Xdebug versions.

## S1 Path mapping without configuration

Can the daemon infer the mapping between paths inside a container and paths on the host from what Xdebug sends, with no configuration in the common Docker setups?

The assumption: the DBGp `init` packet carries the `fileuri` of the entry script (for example `file:///var/www/html/public/index.php`) before any code runs. Matching the longest suffix of that path against the local project tree gives the remote prefix (`/var/www/html`) and the local root. Breakpoints are translated and sent before the first `run`.

Exit criteria:

- A breakpoint set on a host path pauses the request inside the container, in at least two layouts: the project mounted at the container root of the web server, and a project whose entry script is in a subfolder (`public/`).
- The report lists the cases where inference fails or is ambiguous (the same relative path in two places, vendor mounted elsewhere, symlinks), and what the daemon should ask the user in each.
- The report says whether the mapping can be learned on the first session and reused, and what invalidates it.
- The report lists which DBGp features the Xdebug versions in the test bed support, from `feature_get` and by trying each one: the contexts from `context_names` (locals, superglobals, constants), class constants and statics in property trees, the asynchronous `break` command, `stdout` redirection, `property_set`, exception breakpoints on PHP warnings and notices, `call` and `return` breakpoints, and the `watch` breakpoint type. Rows of [docs/features.md](../../docs/features.md) depend on these answers.

Serves: [ADR-0003](../decisions/0003-framework-agnostic.md).

## S2 Logpoints over DBGp

Can a logpoint (capture the values of a few expressions at a line and continue) be built on DBGp without a noticeable cost for the request?

Xdebug has no native logpoint. The assumption: set a line breakpoint, and on each pause read the expressions with `property_get` (or `eval` when the expression is not a plain variable), store the values and send `run` at once.

Exit criteria:

- A logpoint inside a loop captures the expected values on every hit, and the request finishes normally.
- The report gives the added time per hit, measured for a local connection and for a connection into a container, with 1 and with 5 expressions, against the [logpoint budget](../../docs/performance.md#budgets).
- The report gives the daemon's CPU time and memory for 10 000 hits, and shows that memory does not grow with the number of hits once the capture buffer is full.
- The report says whether DBGp hit conditions (`hit_value` with `>=`, `==` or `%`) can cap the number of captures without pausing on every iteration.
- Two requests running at the same time each get their own captures, and neither waits for the other.
- The report says which expressions can be read without `eval`, since eval is off by default ([O-05](../OPEN.md#o-05-eval-policy)).
- The report gives the response sizes for the fixture's deepest object with and without `max_depth`, `max_children` and `max_data`, and the values to use by default.

## S3 Blocking MCP tools

Can an MCP tool block until something happens (a session pauses, a logpoint gets N hits) for up to 60 seconds, and return a normal result to the agent?

Exit criteria:

- A tool served by the daemon over Streamable HTTP waits up to 60 seconds and returns in at least two MCP clients: Claude Code and the MCP Inspector, the protocol's reference client, run in a container.
- The report gives the tool call timeout of each client tested, how to change it, and whether progress notifications keep the call alive.
- The same tool works through a stdio shim that forwards to the daemon.
- The report says whether the daemon can notify a client that a session paused without the client asking, and whether the tested clients surface that to the agent.
- With no client connected and no session open, the server's RSS and CPU use over 10 minutes fit the idle daemon budget in [docs/performance.md](../../docs/performance.md#budgets), and it has no wakeups faster than once per second.
- A blocked `wait` call costs no CPU while it waits.

Serves: [ADR-0002](../decisions/0002-mcp-first.md), [ADR-0004](../decisions/0004-daemon-and-clients.md).

## S4 Native UI for large debugging data

Can egui render the views the desktop app needs, at a usable frame rate on a mid-range laptop?

Exit criteria:

- A variable tree with 100 000 nodes, expanded lazily, scrolls and expands without visible stutter.
- A list of 10 000 dump entries with mixed content (text, tables, collapsed trees) scrolls smoothly.
- A flame graph with 50 000 frames renders, zooms and shows the frame under the cursor.
- A source view with syntax highlighting renders a 5 000-line PHP file and jumps to a line.
- The report covers text selection and copy, light and dark themes, and screen reader support through AccessKit.
- With the window open and no input or new data, the app uses 0% CPU, because it repaints only on events.
- The report gives the RSS with each dataset loaded, for the `glow` and `wgpu` backends, against the desktop app budgets in [docs/performance.md](../../docs/performance.md#budgets), and recommends a backend.
- The views hold only what is on screen plus a fixed margin: memory with 100 000 tree nodes is close to memory with 1 000.
- Styled mockups of the feed and the debugger from [docs/ui.md](../../docs/ui.md), with the candidate fonts and icons, tooltips on enabled and disabled buttons, and both themes. The report includes screenshots, and Lucas judges whether the result looks finished enough.
- If egui fails any of the above, the report says where, and whether iced or Slint would do better on the failing case.

Serves: [ADR-0001](../decisions/0001-rust-native-ui.md).

## S5 PHP extension in Rust

Can a PHP extension written in Rust observe calls in any application, at an acceptable cost, and be distributed?

Exit criteria:

- An extension built with `ext-php-rs` registers a PHP Observer API handler and reports each `PDOStatement::execute` call with its SQL and duration to a local socket.
- The same extension builds and runs on the oldest and the newest PHP version under consideration ([O-02](../OPEN.md#o-02-minimum-php-and-xdebug-versions)).
- The report gives the overhead on a request that runs about 100 queries, with the extension loaded and with it not loaded, against the extension budget in [docs/performance.md](../../docs/performance.md#budgets).
- After 10 000 requests on the same PHP-FPM worker, the worker's memory with the extension loaded matches its memory without it, within measurement noise.
- With the daemon down, the extension's added time per request stays within the same budget.
- Functions that are not observed get no observer handler, and the report shows how that was verified.
- The report says whether `ext-php-rs` exposes the Observer API directly or needs raw bindings, and what the build matrix looks like (PHP versions, ZTS and NTS, platforms).
- The report ends with a recommendation: build the extension, defer it, or drop it.

Serves: [ADR-0005](../decisions/0005-layered-php-integration.md).

## S6 Coexistence with PhpStorm

Can the daemon sit between Xdebug and PhpStorm, relay the session to the IDE, and still read the paused state for an agent?

Exit criteria:

- With the daemon on port 9003 and PhpStorm listening on another port, a PhpStorm debugging session works as it does without the daemon, including PhpStorm's own path mappings.
- While PhpStorm holds a paused session, the daemon reads the stack and a variable for an agent, and PhpStorm still shows correct values and continues normally.
- The report says how the daemon keeps its transaction IDs apart from the IDE's, and which reads are unsafe while the IDE drives (any command that changes execution state).
- The report gives the latency the proxy adds per DBGp round trip, against the IDE proxy budget in [docs/performance.md](../../docs/performance.md#budgets).

Serves: [vision non-goal on replacing the IDE](../../docs/vision.md#non-goals).
