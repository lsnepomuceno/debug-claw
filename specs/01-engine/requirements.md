# 01: Daemon and DBGp engine

State: requirements. Not active for implementation until its `design.md` passes review ([ADR-0007](../decisions/0007-spec-process.md)).

An agent debugs a PHP request through MCP alone: it fires a request, waits for a pause, reads the stack and the variables, steps, evaluates when allowed, and ends the session. The desktop app does not exist yet. Everything here is reached through MCP ([ADR-0002](../decisions/0002-mcp-first.md)).

The rules these requirements rely on are in the ADRs, not here: the daemon and its clients in [ADR-0004](../decisions/0004-daemon-and-clients.md), path mapping in [ADR-0013](../decisions/0013-path-mapping.md), reading values in [ADR-0014](../decisions/0014-reading-values.md), platforms in [ADR-0012](../decisions/0012-platforms.md).

## Terms

- Daemon: the long-running process that listens for Xdebug and serves MCP ([ADR-0004](../decisions/0004-daemon-and-clients.md)).
- MCP client: a program that calls the daemon's MCP tools. The agent is the main one.
- Stdio shim: the command an MCP client launches to reach the daemon over stdio.
- Session: one DBGp connection from Xdebug, from its `init` packet until the connection closes.
- Request: the PHP request or script that a session debugs.
- Paused: a session whose request is stopped at a line and accepts commands.
- Remote path and local path: a path as Xdebug reports it, and the matching path in the user's project tree ([ADR-0013](../decisions/0013-path-mapping.md)).
- Project: a local project root with its learned mappings and breakpoints.
- Blocking tool: an MCP tool that waits for an event before it returns.

## Test bed and versions

Acceptance runs in the Docker test bed ([ADR-0011](../decisions/0011-docker-development.md)) against Xdebug 3.5.3 on PHP 8.3 and 8.5, until [O-02](../OPEN.md#o-02-minimum-php-and-xdebug-versions) sets the supported versions. The MCP clients are the ones S3 used: the MCP Inspector CLI (protocol revision 2025-11-25) and Claude Code (2026-07-28), each with its default settings unless a criterion says otherwise.

## Out of scope

- Pausing a running request, until [O-11](../OPEN.md#o-11-pausing-a-running-request) is answered.
- Logpoints, snapshot points, exception capture and the request timeline (candidate milestone 2, [ADR-0015](../decisions/0015-logpoints.md)).
- The desktop app and its local API.
- Proxy mode for an IDE ([ADR-0016](../decisions/0016-ide-proxy-mode.md)).
- Running PHP outside a request ([O-09](../OPEN.md#o-09-running-php-outside-a-request)).

## Daemon and transport

### R-01 Accept Xdebug sessions with no setup

When Xdebug connects to the daemon on port 9003, the daemon shall open a session, with no project or configuration file set up beforehand.

Serves: [features, connection and setup](../../docs/features.md#connection-and-setup), [ADR-0003](../decisions/0003-framework-agnostic.md). Depends on: [O-22](../OPEN.md#o-22-network-interfaces-of-the-dbgp-listener) (which interfaces).

Acceptance criteria:

- On a fresh install with no configuration, a request with the Xdebug trigger from the PHP 8.3 and the PHP 8.5 containers each opens a session that `status` lists.
- A PHP container on Linux configured as the setup help says (R-07) reaches the daemon.

### R-02 MCP over Streamable HTTP

The daemon shall serve MCP over Streamable HTTP to clients on the same machine, in protocol revisions 2025-11-25 (with sessions) and 2026-07-28 (without sessions).

Serves: [ADR-0004](../decisions/0004-daemon-and-clients.md). Depends on: [O-19](../OPEN.md#o-19-starting-and-finding-the-daemon) (port and discovery), [O-22](../OPEN.md#o-22-network-interfaces-of-the-dbgp-listener).

Acceptance criteria:

- The Inspector CLI connects, lists the tools and calls `status`.
- Claude Code over HTTP connects, lists the tools and calls `status`.
- When the daemon declares the `logging` capability, it answers `logging/setLevel`, and the Inspector CLI connects.
- A connection to the MCP endpoint from another machine on the network is refused.

### R-03 Stdio shim

Where an MCP client starts MCP servers only as commands, the system shall provide a stdio shim that relays MCP traffic to the daemon.

Serves: [ADR-0004](../decisions/0004-daemon-and-clients.md).

Acceptance criteria:

- Claude Code over stdio, through the shim, lists the tools and completes a blocking call that lasts longer than 60 s.
- While one call through the shim blocks, a cancellation sent for it reaches the daemon and ends that call, and a second call through the same shim gets its answer.

### R-04 Shim without a daemon

If the stdio shim cannot reach the daemon, then the shim shall answer the client with an error that says the daemon is not reachable and how to start it.

Serves: [ADR-0004](../decisions/0004-daemon-and-clients.md). Depends on: [O-19](../OPEN.md#o-19-starting-and-finding-the-daemon) (whether the shim starts the daemon instead).

Acceptance criteria:

- With the daemon stopped, the first request through the shim gets an MCP error with that message, and the shim does not hang.

### R-05 Daemon independent of its clients

The daemon shall keep listening for Xdebug and keep its sessions while no MCP client is connected.

Serves: [ADR-0004](../decisions/0004-daemon-and-clients.md).

Acceptance criteria:

- An MCP client disconnects while a session is paused. A new client connects and finds the session, still paused, in `status`.

### R-06 Status

When an MCP client asks for the status, the daemon shall return the state of the DBGp listener (address, port, listening or the error that stopped it), the open sessions with their state, and the learned projects.

Serves: [features, connection and setup](../../docs/features.md#connection-and-setup).

Acceptance criteria:

- With port 9003 already taken by another process, `status` reports the listener error, and the MCP tools keep answering.
- With two sessions open, one paused and one running, `status` lists both with those states.

### R-07 Setup help

When an MCP client asks for setup help, the daemon shall return the `php.ini` settings and the Docker settings (`XDEBUG_MODE`, `XDEBUG_CONFIG`) for the system the daemon runs on, and whether a first session has connected.

Serves: [features, connection and setup](../../docs/features.md#connection-and-setup), [ADR-0012](../decisions/0012-platforms.md).

Acceptance criteria:

- On Linux the Docker settings include `extra_hosts: host.docker.internal:host-gateway`. On Windows and macOS they do not.
- Before any session the answer says none has connected. After the first session it says one has.

## Path mapping

The rules are [ADR-0013](../decisions/0013-path-mapping.md). The cases below are the ones S1 recorded in its [report](../00-spikes/reports/s1-path-mapping.md), criterion 2, rebuilt as milestone 1 fixtures.

### R-08 Mapping inferred before the request runs

When a session starts, the daemon shall infer the mapping between remote and local paths and place the project's breakpoints before the request runs its first line.

Serves: ADR-0013 rules 1 to 4 and 6. Depends on: [O-20](../OPEN.md#o-20-the-local-project-root) (where the local tree is).

Acceptance criteria:

- With the project mounted at the container root (`/srv/app`), where a suffix tie is broken by content, a breakpoint set on a local path pauses the request at that line.
- With the entry script in `public/` (`/var/www/html`), the same holds.
- The pause is reported with the local path and line.

### R-09 Ambiguous mapping

If two or more local folders match the entry script equally and their content does not tell them apart, then the daemon shall ask the MCP client which folder is the project, offer every candidate, and remember the answer.

Serves: ADR-0013 rule 4. Depends on: [O-21](../OPEN.md#o-21-how-long-a-paused-request-may-wait) (how long the request waits for the answer).

Acceptance criteria:

- With two identical copies of the project, the question lists both folders.
- After the answer, the next session of the same project does not ask again.

### R-10 Stale local tree

If the only matching local file differs in content from the entry script in the container, then the daemon shall use the mapping and warn that the container runs other code than the local tree, without asking.

Serves: ADR-0013 rule 5.

Acceptance criteria:

- With an older copy in the local tree, the session pauses at the breakpoint, and the session state carries the warning.

### R-11 Breakpoints checked in the container

The daemon shall report a breakpoint as placed only when its file exists at the translated remote path, and otherwise as not placed, with the reason.

Serves: ADR-0013 rule 6.

Acceptance criteria:

- With `vendor/` mounted at another remote path, a breakpoint in a vendor file is reported as not placed, with the remote path that was tried.
- On Linux, a breakpoint on `src/graph.php` when the file is `src/Graph.php` is reported as not placed, with the closest local name.

### R-12 Second mapping from a frame

When a frame's remote path is outside every known remote prefix, the daemon shall infer a second mapping from that path and try the breakpoints that were not placed again.

Serves: ADR-0013 rule 7.

Acceptance criteria:

- With `vendor/` mounted elsewhere, a step into a vendor file adds a mapping, and the vendor breakpoint pauses later in the same request.

### R-13 Mapping reused and invalidated

The daemon shall reuse a learned mapping on later sessions of the same project, and shall infer it again when the entry script content differs, when a remote breakpoint check fails, or when the local root no longer exists.

Serves: ADR-0013 rule 8.

Acceptance criteria:

- A second session of the same project reuses the mapping and makes one content check of the entry script.
- Each of the three invalidations, set up on purpose, leads to a new inference on the next session.
- Two projects that both use `/var/www/html` in their containers keep separate mappings.

### R-14 Symlinks, case and Windows paths

The daemon shall resolve local symlinks, compare names with or without case as the local file system does, keep the spelling found on disk in what it sends to Xdebug, and accept Windows drive letters, backslashes and UNC paths.

Serves: ADR-0013 rules 9 to 11, [ADR-0012](../decisions/0012-platforms.md).

Acceptance criteria:

- A breakpoint set through a symlinked local folder pauses and is reported at the path Xdebug reports, translated to the local real path.
- Unit tests on recorded and constructed `init` packets cover drive letters in either case, backslashes, UNC paths, WSL paths, a local path on another drive (no match), and macOS case folding, as S1 did.

### R-15 Bounded walk of the local tree

The daemon shall walk the local tree without following symlinks, skip version control folders, `node_modules` and `.idea`, stop at a fixed cap of entries, and say so when it stops at the cap.

Serves: ADR-0013 rule 2, [performance rule 2](../../docs/performance.md#rules).

Acceptance criteria:

- On a tree larger than the cap, inference stops at the cap and the session state says the walk was cut.
- The cap value is set in `design.md`, and the walk's time and memory on a project with a full `vendor/` folder are measured and reported.

### R-16 Inspect and correct mappings

When an MCP client asks for the path mappings, the daemon shall list each project's mappings, and it shall let the client replace a mapping.

Serves: [features, connection and setup](../../docs/features.md#connection-and-setup) (`path_mappings`).

Acceptance criteria:

- After R-12, the list shows both mappings of the project.
- A mapping set by the client is used by the next session and is not replaced by inference while it passes the checks of R-13.

## Sessions and requests

### R-17 Fire a triggered request

When an MCP client asks to send an HTTP request, the daemon shall send it with the Xdebug trigger and return an ID that the blocking tools accept to wait on the session of that request.

Serves: [features, connection and setup](../../docs/features.md#connection-and-setup) (`request_send`).

Acceptance criteria:

- A request sent to the test bed returns an ID. A wait on that ID returns the pause of that request's session, and not the pause of another session open at the same time.
- With the option to pause on the first line, the session pauses on the first line of the entry script.
- The tool result includes the HTTP status of the response once the request ends.

### R-18 Several sessions at once

While several sessions are open, the daemon shall drive each one independently.

Serves: [features, execution control](../../docs/features.md#execution-control).

Acceptance criteria:

- Two requests paused at the same time each answer stack and variable reads, and stepping one does not move the other.
- A paused session does not delay a second request's session from opening.

### R-19 Sessions grouped by project

The daemon shall group sessions by the project their mapping belongs to.

Serves: [features, connection and setup](../../docs/features.md#connection-and-setup) (`projects`).

Acceptance criteria:

- Sessions from two projects are listed under their own project, each with its local root.

### R-20 Release a paused worker

If a paused session receives no command for the release timeout, then the daemon shall release the request and record that it did.

Serves: [features, execution control](../../docs/features.md#execution-control), [ADR-0003](../decisions/0003-framework-agnostic.md). Depends on: [O-21](../OPEN.md#o-21-how-long-a-paused-request-may-wait) (the timeout, how the request is released, what counts as a command).

Acceptance criteria:

- With the timeout set to a short value for the test, a paused session that gets no command is released, its request completes or stops as O-21 decides, and `status` shows why the session ended.
- A session that keeps getting commands is not released.

## Waiting

The timeout and progress rules are in [ADR-0004](../decisions/0004-daemon-and-clients.md#blocking-tools).

### R-21 Wait for a pause or an end

When an MCP client calls the wait tool, the daemon shall return when a matching session pauses or ends, or when the wait's timeout passes, with a result that says which happened.

Serves: [features, execution control](../../docs/features.md#execution-control) (`wait`).

Acceptance criteria:

- A wait on a request ID returns the pause with the local file and line once the breakpoint hits.
- A wait with no event returns a normal result that says nothing happened, before the Inspector CLI and Claude Code over HTTP stop the call with their default settings.
- The default and the maximum timeout are below 60 s. A larger timeout asked for is cut to the maximum, and the result says so.

### R-22 Progress while waiting

While a wait blocks and its call carries a progress token, the daemon shall send progress at least every 20 s.

Serves: ADR-0004.

Acceptance criteria:

- With Claude Code over HTTP and `MCP_TOOL_TIMEOUT` raised above 60 s, a wait that blocks past 60 s with progress returns its result and is not cut by the client.

### R-23 Waits bounded by the daemon

The daemon shall end every wait at its own timeout, whether or not the client is still connected, and shall cap how many waits run at once.

Serves: ADR-0004, [performance rule 2](../../docs/performance.md#rules).

Acceptance criteria:

- When the client cancels a wait, the wait ends at once.
- When the Inspector CLI exits during a wait without cancelling, the daemon ends the wait at its timeout and stops sending progress for it.
- With the cap reached, one more wait is refused with an error that says how many waits are running. The cap value is set in `design.md`.

### R-24 Events reach the agent through results

The daemon shall make every pause and every session end available through a tool result, and shall not rely on server notifications to inform the agent.

Serves: [features, agent workflow](../../docs/features.md#agent-workflow). Depends on: [O-13](../OPEN.md#o-13-pauses-the-agent-is-not-waiting-for) (how the agent learns of a pause it is not waiting for).

Acceptance criteria:

- With the client ignoring all notifications, an agent completes the flow: send a request, wait, read the stack and a variable, continue, see the end.
- A session that paused while no wait was running is reported by the next wait or `status` call.

## Breakpoints

### R-25 Line breakpoints

When an MCP client sets a line breakpoint on a local path, the daemon shall pause matching sessions at that line, with an optional condition and an optional hit count with the conditions "at or after hit N", "only hit N" and "every N-th hit".

Serves: [features, breakpoints](../../docs/features.md#breakpoints).

Acceptance criteria:

- In a loop of 100 iterations, a breakpoint with the condition `$i == 50` pauses once.
- In the same loop, "at or after hit 95" pauses 6 times, "only hit 50" once, and "every 10th hit" 10 times.
- A breakpoint on a line with no code is reported as Xdebug resolves it, with the line it moved to if it moved.

### R-26 Exception and PHP error breakpoints

When an MCP client sets an exception breakpoint by class name or for all exceptions, or on PHP warnings, notices or deprecations, the daemon shall pause sessions where one is raised, and report the name, the code and the message.

Serves: [features, breakpoints](../../docs/features.md#breakpoints). Depends on: [O-12](../OPEN.md#o-12-silenced-php-errors) (errors silenced with `@`).

Acceptance criteria:

- A caught `RuntimeException` pauses on a breakpoint for that class.
- Breakpoints on `Warning`, `Notice` and `Deprecated` each pause on a fixture that raises one, with the PHP message.
- A breakpoint for all of them pauses on each of the four.
- Errors silenced with `@` behave as O-12 decides.

### R-27 Run to a line

When an MCP client asks a paused session to run to a line, the daemon shall continue and pause at that line once, then remove that breakpoint.

Serves: [features, breakpoints](../../docs/features.md#breakpoints) (`run_to`).

Acceptance criteria:

- The session pauses at the line, and a later pass over the same line does not pause.

### R-28 Breakpoints kept per project

The daemon shall keep each project's breakpoints, place them in every session of that project, and let an MCP client list, enable, disable, group and remove them.

Serves: [features, breakpoints](../../docs/features.md#breakpoints). Depends on: [O-04](../OPEN.md#o-04-storage) (whether they survive a daemon restart).

Acceptance criteria:

- A breakpoint set during one session pauses the next session of the same project.
- A disabled breakpoint does not pause, and it pauses again once enabled.
- A breakpoint of one project is not placed in another project's sessions.

### R-29 Breakpoint origin

The daemon shall record with each breakpoint which client created it.

Serves: [features, breakpoints](../../docs/features.md#breakpoints) (`origin`).

Acceptance criteria:

- A breakpoint created through MCP lists the MCP client as its origin.

### R-30 Breakpoints from code

When PHP code calls `xdebug_break()`, the daemon shall report the pause as it reports a breakpoint pause.

Serves: [features, breakpoints](../../docs/features.md#breakpoints).

Acceptance criteria:

- A fixture that calls `xdebug_break()` pauses at the next line, and a wait returns that pause.

## Execution control

### R-31 Continue and step

When an MCP client asks a paused session to continue, step over, step into or step out, the daemon shall run that command and return where the session paused next, or that the request ended.

Serves: [features, execution control](../../docs/features.md#execution-control).

Acceptance criteria:

- On a fixture with nested calls, each step lands on the line the fixture expects, reported with the local path.
- A continue with no further breakpoint returns that the request ended.

### R-32 Stop and detach

When an MCP client asks to stop a session, the daemon shall end the request. When it asks to detach, the daemon shall end the session and let the request finish.

Serves: [features, execution control](../../docs/features.md#execution-control).

Acceptance criteria:

- After stop, the HTTP request ends without running the rest of the script.
- After detach, the HTTP request completes with its normal response.

## Inspection

### R-33 Stack

When an MCP client asks a paused session for its stack, the daemon shall return the frames with local paths and lines, with frames under the project's `vendor/` folder collapsed.

Serves: [features, inspection](../../docs/features.md#inspection).

Acceptance criteria:

- On a fixture that calls into a vendor package, the stack shows the project frames and one collapsed entry per run of vendor frames, which can be expanded.
- A frame whose path has no mapping is shown with its remote path and marked as not mapped.

### R-34 Variables read on demand

When an MCP client asks for the variables of a frame, the daemon shall return locals, superglobals or constants as asked, one level deep, with the limits of [ADR-0014](../decisions/0014-reading-values.md), and shall return deeper levels and further pages only when asked.

Serves: [features, inspection](../../docs/features.md#inspection), [performance rule 3](../../docs/performance.md#rules).

Acceptance criteria:

- `$this` of a fixture object graph is returned one level deep, and each child that has children says how many.
- A child is read by the path the result gives for it.
- Of an array of 50 entries, the first page has 32 and the second has 18.
- A built-in constant (`PHP_VERSION`) and a superglobal entry (`$_GET['n']`) are read by name.

### R-35 PHP types as Xdebug reports them

The daemon shall show for each value its type, its class when it has one, its visibility and modifiers, and its full size or number of children.

Serves: [features, inspection](../../docs/features.md#inspection).

Acceptance criteria:

- Private, protected and readonly properties are shown and marked as such.
- A parent class's private property is shown and readable.
- Statics are shown with their visibility.

S1 and S2 did not check closures, resources, generators, uninitialized typed properties or object identity. `design.md` lists what Xdebug reports for each, and the tests cover them.

### R-36 Long strings and binary values

When a string is longer than the read limit, the daemon shall mark it as cut, with its full length, and shall return the full value in parts on request. Binary content shall be shown as hex.

Serves: [features, inspection](../../docs/features.md#inspection) (`property_value`).

Acceptance criteria:

- A string of 100 000 characters comes back cut, with its length of 100 000.
- The full value is returned in parts, each within the result cap of R-45, and the parts joined equal the string.
- A string with bytes that are not valid UTF-8 is shown as hex.

### R-37 Changed values marked

When a session pauses after a step, the daemon shall mark the values that changed since the previous pause, among the values the client read at that pause.

Serves: [features, inspection](../../docs/features.md#inspection).

Acceptance criteria:

- After a step that changes `$sum` and not `$n`, `$sum` is marked and `$n` is not.

### R-38 Source

When an MCP client asks for the source around the current line, the daemon shall return it with the current line marked, and say whether it came from the local file or from the container.

Serves: [features, inspection](../../docs/features.md#inspection).

Acceptance criteria:

- With a mapped local file, the lines come from it.
- With a stale local tree (R-10), the lines come from the container and the result says so.

### R-39 Watches

When a session pauses, the daemon shall read every watch expression of the session and include the values in the pause result.

Serves: [features, inspection](../../docs/features.md#inspection). Depends on: [O-05](../OPEN.md#o-05-eval-policy) (watches outside the path grammar).

Acceptance criteria:

- A watch on `$item['square']` shows its value at every pause in a loop.
- While eval is off, adding a watch that does not match the path grammar of ADR-0014 (for example `$i + 1`) is refused when it is added, with the reason.

## Evaluation and permissions

### R-40 Eval off by default

While eval is off, the daemon shall refuse to evaluate expressions and to set variables, with an error that says eval is off and how it is turned on.

Serves: [vision](../../docs/vision.md), [features, console and evaluation](../../docs/features.md#console-and-evaluation), [ADR-0014](../decisions/0014-reading-values.md) (setting a value evaluates PHP). Depends on: [O-05](../OPEN.md#o-05-eval-policy) (how eval is turned on).

Acceptance criteria:

- On a fresh install, eval and set variable are refused with that error.
- Reads that match the path grammar still work.

### R-41 Evaluate in the paused frame

Where eval is on, when an MCP client sends an expression to a paused session, the daemon shall evaluate it in the selected frame and return the value with the limits of ADR-0014.

Serves: [features, console and evaluation](../../docs/features.md#console-and-evaluation). Depends on: [O-05](../OPEN.md#o-05-eval-policy).

Acceptance criteria:

- `"i=$i sum=$sum"` returns the interpolated string, and a class constant (`self::KIND`) returns its value.
- An expression with a PHP error returns the error message, and the session stays paused.

### R-42 Set a variable

Where eval is on, when an MCP client sets a variable in a paused frame, the daemon shall set it and return the new value.

Serves: [features, inspection](../../docs/features.md#inspection). Depends on: [O-05](../OPEN.md#o-05-eval-policy).

Acceptance criteria:

- After setting `$n` to `3`, a read of `$n` returns `3` and the request uses the new value.

### R-43 Record of evaluations

The daemon shall record every evaluation and every variable set, with the time, the session, the frame, the expression, the client that sent it and the outcome, and shall let an MCP client read that record.

Serves: [features, console and evaluation](../../docs/features.md#console-and-evaluation).

Acceptance criteria:

- After two evaluations and one set, the record lists the three in order with those fields.
- Refused evaluations are recorded as refused.

### R-44 Permission scopes

The daemon shall assign every tool to one scope (read, control execution, eval, set variables) and refuse a call whose scope the client has not been granted, with an error that names the scope.

Serves: [features, agent workflow](../../docs/features.md#agent-workflow). Depends on: [O-05](../OPEN.md#o-05-eval-policy) (how and where scopes are granted).

Acceptance criteria:

- `design.md` lists the scope of every tool.
- A client without the control execution scope gets that error on a step and can still read variables.

## MCP results

### R-45 Result size cap

The daemon shall keep every tool result within the [MCP default tool result budget](../../docs/performance.md#budgets), mark where a result was cut, and say how to get the rest.

Serves: [ADR-0014](../decisions/0014-reading-values.md), [performance rule 5](../../docs/performance.md#rules). Depends on: [O-08](../OPEN.md#o-08-confirm-the-resource-budgets).

Acceptance criteria:

- A read of a fixture frame whose superglobals pass the budget comes back cut at the budget, with the next page or the paths to read.
- Each tool is tested with an input that would pass the budget, and none of its results does.

### R-46 Errors that say what to do next

If a tool cannot do what it was asked, then the daemon shall return an error that says what is wrong and what the client can do next.

Serves: [ADR-0002](../decisions/0002-mcp-first.md).

Acceptance criteria:

- Each of these returns its own error with a next step: no such session, session not paused, session ended, unknown request ID, breakpoint not placed, eval off, scope not granted, wait cap reached.

## Resource budgets

Each requirement below points to a row of the [budget table](../../docs/performance.md#budgets). The drafts are in force until [O-08](../OPEN.md#o-08-confirm-the-resource-budgets) decides each row, and the measurement follows [How it is measured](../../docs/performance.md#how-it-is-measured).

### R-47 Idle daemon

While the daemon has no sessions, it shall stay within the "Daemon, idle, no sessions" budget.

Serves: [ADR-0008](../decisions/0008-resource-budgets.md).

Acceptance criteria:

- Over 10 minutes with the DBGp listener and the MCP endpoint up and no client, RSS, CPU and wakeups are measured and within the row.
- The same is measured with one MCP client connected. Its stream's keep-alive is the only periodic wakeup.

### R-48 Sessions and events

While 10 sessions are open and 10 000 events are stored, the daemon shall stay within the "Daemon, 10 sessions open, 10 000 stored events" budget.

Serves: [ADR-0008](../decisions/0008-resource-budgets.md). Depends on: [O-04](../OPEN.md#o-04-storage) (what an event is stored as).

Acceptance criteria:

- `design.md` defines what counts as an event in this milestone. A test opens 10 sessions, produces 10 000 events and measures RSS within the row.

### R-49 A blocked wait costs nothing

While a wait blocks, the daemon shall use no more CPU than when idle.

Serves: [ADR-0008](../decisions/0008-resource-budgets.md), [performance rule 1](../../docs/performance.md#rules).

Acceptance criteria:

- CPU time sampled over 45 s of a blocked wait with no progress equals the idle figure of R-47.

### R-50 Memory does not grow with sessions

The daemon's memory shall not grow with the number of sessions it has seen.

Serves: [performance rule 2](../../docs/performance.md#rules).

Acceptance criteria:

- After 1000 sequential sessions, each paused, read and continued, RSS stays within the row of R-48.
- Every per-session store, queue and buffer has a cap listed in `design.md`.
