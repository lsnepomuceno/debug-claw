# Features

This is the catalog of what debug-claw should do over its lifetime. It is product direction, not a requirement: a feature becomes a requirement when a milestone takes it, and that milestone's design fixes the details.

Each row says how an agent reaches the feature (MCP), how a person reaches it (desktop app, see [ui.md](ui.md)), where the data comes from, and which [candidate milestone](../specs/README.md#candidate-milestones) is expected to deliver it. The milestone column is as tentative as the candidate list.

Sources:

- Xdebug: DBGp only, no code added to the application.
- Dump client: the PHP file loaded with `auto_prepend_file` ([ADR-0005](../specs/decisions/0005-layered-php-integration.md)).
- Extension: the PHP extension in Rust (same ADR).
- Daemon: computed by the daemon from data it already has.

## MCP surface

The rows list capabilities, not one tool each. A long tool list costs every agent context on every call and makes the right tool harder to pick, so the milestone designs group capabilities into a small set of tools with parameters (one `breakpoint_set` with a type, not one tool per breakpoint type). The names below are working names. The [MCP contract reviewer](../specs/reviewers/mcp-contract.md) checks the final names, descriptions and tool count.

Every result follows the limits in [performance.md](performance.md): values are truncated at the source, results stay under the size cap, and long lists are paged.

## Connection and setup

| Feature | MCP | App | Source | Milestone |
|---|---|---|---|---|
| Accept any Xdebug session on port 9003, with no project configured | `status` | Listener state in the top bar | Xdebug | 1 |
| Setup snippets for `php.ini` and for Docker (`XDEBUG_MODE`, `XDEBUG_CONFIG`), with a live check that turns green on the first connection | `setup_help` | Empty state with copy buttons | Daemon | 1 (MCP), 3 (app) |
| Path mapping inferred from the `init` packet, asked for only when ambiguous ([ADR-0013](../specs/decisions/0013-path-mapping.md)) | `path_mappings` | Banner that asks, with the inferred answer filled in | Xdebug | 1 |
| Sessions grouped by project, from the inferred root | `projects` | Project switcher | Xdebug | 1 |
| Ignore sessions by URI or script pattern (health checks, assets) | `session_filter` | Settings | Xdebug | 2 |
| Fire an HTTP request with the trigger and get its request ID back, so the agent can wait on that request | `request_send` | "Send request" dialog, "Copy as curl" | Daemon | 1 |
| Replay a captured request with the trigger | `request_replay` | Button on the request | Dump client or extension | 2 |
| Optional pause on the first line of every session | parameter of `request_send` | Toggle | Xdebug | 1 |
| Run a CLI script under the debugger | `cli_run` | Run dialog | Needs a PHP runtime, see [O-09](../specs/OPEN.md#o-09-running-php-outside-a-request) | later |

## Breakpoints

| Feature | MCP | App | Source | Milestone |
|---|---|---|---|---|
| Line breakpoint | `breakpoint_set` | Click in the gutter | Xdebug | 1 |
| Condition | parameter | Edit popover | Xdebug | 1 |
| Hit count (`hit_value`, `hit_condition`) | parameter | Edit popover | Xdebug | 1 |
| Exception breakpoint, by class or for all | `breakpoint_set` type `exception` | Exceptions section of the breakpoints view | Xdebug | 1 |
| Break on PHP warnings, notices and deprecations, by the names `Warning`, `Notice`, `Deprecated` or `*`. Errors silenced with `@` are skipped by default, and an option pauses on them too | `breakpoint_set` type `exception` | Same section | Xdebug (confirmed in S1) | 1 |
| Break on function call or return, internal functions included, with the return value at a `return` pause | `breakpoint_set` type `call`, `return` | "Break on function" field | Xdebug (confirmed in S1) | 2 |
| One-shot breakpoint and run to line | `run_to` | "Run to cursor" | Xdebug | 1 |
| Enable, disable, group and keep breakpoints per project | `breakpoint_update` | Breakpoints view | Daemon, storage in [O-04](../specs/OPEN.md#o-04-storage) | 1 |
| Each breakpoint records who created it (person or agent) | field `origin` | Badge | Daemon | 1 |
| Logpoint: a message template with expressions; the worker pauses briefly at each hit and continues ([ADR-0015](../specs/decisions/0015-logpoints.md)) | `logpoint_set` | Diamond in the gutter | Xdebug | 2 |
| Snapshot point: capture every local of the frame to a small fixed depth, then continue (cycles in [O-15](../specs/OPEN.md#o-15-cycle-check-for-snapshot-points)) | `snapshot_set` | Camera mark in the gutter | Xdebug | 2 |
| Exception capture: on an uncaught exception, keep the stack and the locals of the throwing frame, then continue | `exception_capture` | Toggle, results in the timeline | Xdebug | 2 |
| Breakpoint from code | `xdebug_break()` works as is; the dump client adds `pause()` | | Xdebug, dump client | 1, 4 |

Snapshot points and exception capture give an agent the values without holding a PHP worker, which is why logpoints are the default ([vision](vision.md), principle 5).

Not planned: data watchpoints (break when a value changes), moving the next statement, and editing code during a pause. S1 found that Xdebug 3.5.3 answers a `watch` breakpoint with error 201. Moving the next statement and editing code were not tried, and as far as is known Xdebug implements neither.

## Execution control

| Feature | MCP | App | Source | Milestone |
|---|---|---|---|---|
| Continue, step over, step into, step out | `continue`, `step` | Toolbar, PhpStorm keys | Xdebug | 1 |
| Wait until a session pauses or a request ends, with a timeout ([S3](../specs/00-spikes/requirements.md#s3-blocking-mcp-tools)) | `wait` | | Daemon | 1 |
| Stop the request, or detach and let it finish | `stop`, `detach` | Toolbar | Xdebug | 1 |
| Pause a running request. Xdebug has no async `break` (S1); only its control socket, reachable from inside the PHP container, can pause. Milestone 1 has no pause; a later milestone adds a helper run inside the PHP container through `docker exec` | `break` | Pause button | Xdebug control socket, through a helper in the container | later |
| Release a paused worker with `detach` after 5 minutes when no one drives it ([ADR-0021](../specs/decisions/0021-paused-request-release.md)) | configuration | Countdown in the session header | Daemon | 1 |
| Several sessions at once | `sessions` | Session list | Xdebug | 1 |
| Step filters: stepping into skips `vendor/` and other listed paths | parameter | Toggle | Daemon over Xdebug | 3 |
| Shared control: the app shows who sent the last command, and the person can take over or lock the agent out of a session | `session_status` | "Agent is driving" banner with "Take over" | Daemon | 3 |

## Inspection

| Feature | MCP | App | Source | Milestone |
|---|---|---|---|---|
| Stack frames, with vendor frames collapsed | `stack` | Frames pane | Xdebug | 1 |
| Variables per frame: locals, superglobals and constants. A constant is read by name, built-in ones included, though the list shows only user constants | `variables` with a context | Variables pane, one tab per context | Xdebug (contexts 0, 1 and 2, confirmed in S1 and S2) | 1 |
| `$this` and class statics, private and protected included, with visibility shown. Statics are read through `$this`. Class constants only through eval | `variables` | Same tree | Xdebug (confirmed in S1 and S2); class constants need the eval scope ([ADR-0018](../specs/decisions/0018-eval-policy.md)) | 1 |
| Tree loaded on demand, one level at a time, with paging for large arrays ([ADR-0014](../specs/decisions/0014-reading-values.md)) | `property_get` with a path and a page | Lazy tree | Xdebug | 1 |
| Full value of a long string, binary shown as hex | `property_value` | Value viewer | Xdebug | 1 |
| PHP types rendered faithfully: object IDs, recursion, enums, closures with their file and line, resources, generators, uninitialized typed properties | in every value | In every tree | Xdebug | 1 |
| Watches: expressions read at every pause. Path expressions need no eval, anything else needs the eval scope | `watch_add` | Watches tab | Xdebug | 1 |
| Values that changed since the last step are marked | flag in `variables` | Highlight | Daemon | 1 (MCP), 3 (app) |
| Source of the current file, with the current line | `source` | Source pane | Xdebug (`source` command) or local file | 1 |
| Values shown inline next to the source lines, and on hover | | Source pane | Daemon over Xdebug | 3 |
| Set a variable. Xdebug evaluates the new value as PHP, so this has the power of eval | `variable_set` | Edit in the tree | Xdebug, eval scope ([ADR-0018](../specs/decisions/0018-eval-policy.md)) | 1 |
| Copy a value as a PHP path expression (`$order->items[3]->price`), JSON or `var_export`. Xdebug's `fullname` is the path, except its `->*Class*name` form for a parent's private property, which is not valid PHP | | Context menu | Daemon | 3 |
| Search the variable tree by name or loaded value | | Filter box | Daemon | 3 |
| Compare two pauses or two snapshots | `diff` | Diff view | Daemon | 2 |

## Console and evaluation

| Feature | MCP | App | Source | Milestone |
|---|---|---|---|---|
| Evaluate an expression in the paused frame | `eval` | Evaluate field | Xdebug, eval scope | 1 |
| Console: multi-line PHP in the paused frame, with history, completion from names in scope, and `echo` output captured | `eval` | Console panel | Xdebug, output through `stdout -c 1` (confirmed in S1; `stderr -c 1` fails) | 3 |
| Every evaluation is recorded in the request timeline with who ran it | in `request_timeline` | Timeline entry | Daemon | 1 |
| Console with no paused request, using the project's autoloader | `console_run` | Console panel | Needs a PHP runtime, see O-09 | later |

## Dumps

| Feature | MCP | App | Source | Milestone |
|---|---|---|---|---|
| `dc($value)` with label, color and screen | `dumps` with filters | Feed | Dump client | 4 |
| Tables, JSON and formatted SQL | in `dumps` | Card renderers | Dump client | 4 |
| Origin of each dump (file and line), opened in the editor | in `dumps` | Link on the card | Dump client | 4 |
| Timers, counters and memory readings | in `dumps` | Card renderers | Dump client | 4 |
| Backtrace at the call site | in `dumps` | Collapsible stack on the card | Dump client | 4 |
| Dump once, dump if, dump and stop | in `dumps` | | Dump client | 4 |
| `pause()`: a breakpoint when Xdebug is attached, a no-op otherwise | | | Dump client | 4 |
| Application logs through a PSR-3 handler shipped with the dump client | `logs` | Feed and timeline | Dump client | 4 |
| Filter the feed by color, label, screen, request and type; search; pin; clear; clear on each new request | parameters of `dumps` | Feed sidebar and toolbar | Daemon | 3 |
| Accept dumps that applications already send to other tools | | | [O-07](../specs/OPEN.md#o-07-compatibility-with-existing-dump-protocols) | later |

## Requests and timeline

| Feature | MCP | App | Source | Milestone |
|---|---|---|---|---|
| Request list: method, URI, status, duration, peak memory, web or CLI | `requests` | Requests view | URI from Xdebug; status, duration and memory from the dump client or the extension | 2 |
| Timeline of one request: start, pauses, logpoint hits, snapshots, dumps, evaluations, exceptions, queries, end | `request_timeline` | Timeline tab | Daemon | 2 |
| Request details: headers, query, body, cookies, session, response status and headers | `request_details` | Request and Response tabs | Dump client or extension; Xdebug only through a first-line capture | 2 |
| Exceptions and PHP errors, with the previous-exception chain and the stack | in `request_timeline` | Exceptions tab | Xdebug, dump client or extension | 2 |
| Database queries with bindings, duration and origin; duplicates (N+1) and slow queries marked. S5 captured SQL text and duration per `PDOStatement::execute`; bindings, origin, `PDO::query`, `PDO::exec` and mysqli were not tried | `queries` | Queries tab | Extension (PDO, mysqli) | 6 |
| Outgoing HTTP calls (curl) | `http_calls` | HTTP tab | Extension | 6 |
| Extension status: loaded, version, events sent and dropped, daemon reachable | in `status` | Status in the top bar | Extension | 6 |
| Compare two requests | `request_diff` | Diff view | Daemon | later |
| Export a request as JSON | `request_export` | Export button | Daemon | later |

## Profiling

| Feature | MCP | App | Source | Milestone |
|---|---|---|---|---|
| Profile of one request: top functions by self time, inclusive time and memory | `profile_top` with a sort and a limit | Table | [O-06](../specs/OPEN.md#o-06-profiling-source) | 5 |
| Flame graph and call tree, expanded on demand | `profile_frames` with a path | Flame graph | O-06 | 5 |
| Import a cachegrind file from Xdebug's profiler | `profile_import` | Drop the file on the window | Xdebug | 5 |
| Compare two profiles | `profile_diff` | Diff view | Daemon | later |
| Function trace with arguments and return values (Xdebug trace mode) | `trace` | Trace view | Xdebug | later |

## Agent workflow

These exist because the agent is a first-class user ([ADR-0002](../specs/decisions/0002-mcp-first.md)).

| Feature | MCP | App | Milestone |
|---|---|---|---|
| Notifications when a session pauses or a request ends. The daemon can send them, but neither client S3 tested shows them to the agent, so the tool descriptions tell the agent to keep a wait running instead | server notifications | | 1 |
| Sessions and request timelines as MCP resources | resources | | 2 |
| Guided workflows such as "investigate a failing request" and "find where this value changes" | prompts | | 2 |
| Shared focus: the request, frame or variable the person selected in the app, readable by the agent | `focus` | "Share with agent" action | 3 |
| Notes on a request or a frame, written by either side | `note_add` | Notes on the request | later |
| Permissions per project: read, control execution and eval, where setting a variable counts as eval ([ADR-0018](../specs/decisions/0018-eval-policy.md)) | enforced on every tool | Settings | 1 |
| Activity log of every MCP call that changed state, in plain words ("Agent set a breakpoint at OrderService.php:42") | | Activity panel | 3 |

## Editor and desktop

| Feature | App | Milestone |
|---|---|---|
| Open a file and line in the editor, from a configurable URL or command template (PhpStorm, VS Code) | Every file link | 3 |
| Proxy mode: PhpStorm keeps driving the session while debug-claw reads it, read only and during PhpStorm's pauses, verified with PhpStorm 2026.3 ([ADR-0016](../specs/decisions/0016-ide-proxy-mode.md)) | Session header shows "Attached to IDE" | 7 |
| OS notification when a breakpoint hits and the window is not focused | | 3 |
| Always on top and a compact window mode, for working beside the editor | Top bar | 3 |

## Outside the core

Candidates that are framework agnostic but not part of any milestone yet: an SMTP sink for outgoing mail, Redis and cache observers in the extension, and code coverage. Framework-specific collectors (queues, events, gates) stay outside the core ([ADR-0003](../specs/decisions/0003-framework-agnostic.md)).
