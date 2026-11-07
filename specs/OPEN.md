# Open questions

Questions without an answer yet. Each one says who decides and what it blocks. When a question is answered, the answer moves to an ADR or a spec, and the entry here is removed.

## O-02 Minimum PHP and Xdebug versions

Which PHP versions and which Xdebug versions are supported. This limits which DBGp features the engine can rely on and which PHP versions the extension has to build for.

What the spikes found:

- S1: Xdebug 3.5.3 on PHP 8.3.36 and 8.5.12 gave the same answer for every DBGp feature tried. Older Xdebug versions are untested. S2 ran its expression and size probes on PHP 8.5 only.
- S5: the extension does not set the minimum PHP version. `ext-php-rs` supports PHP 8.1 and later, and one source built for 8.3 and 8.5 with no version-specific code. Each supported minor version adds one binary per thread-safety mode and platform. On PHP 8.3 a loaded observer costs about twice what it costs on 8.5.

Until this is answered, milestone 1 acceptance runs on the test bed versions: Xdebug 3.5.3 on PHP 8.3 and 8.5.

- Decides: Lucas.
- Blocks: milestone 1 design.

## O-04 Storage

How the daemon stores sessions, events and breakpoints, and whether anything survives a restart. Candidates include SQLite and an embedded key-value store.

- Decides: milestone 1 design.
- Blocks: milestone 1 design.

## O-05 Eval policy

Evaluating PHP expressions runs code inside the application. It is off by default (see [vision](../docs/vision.md)), but the switch is still open: global, per project or per session. The same answer sets the permission scopes an MCP client can be granted (read, control execution, eval, set variables) and whether the desktop app can grant them per client.

What the spikes found:

- S2: an expression that matches the path grammar of [ADR-0014](decisions/0014-reading-values.md) is read with `property_get` and needs no eval. That covers variables, properties of any visibility, quoted array keys, statics through `$this`, superglobal entries and constants. Calls, operators, casts, interpolation and class constants need eval. Logpoint and watch expressions follow the same rule, so the open part is whether expressions outside the grammar are allowed in them.
- S1: `property_set` evaluates its value as PHP, so the "set variables" scope gives the same power as "eval".
- S1: class constants are shown only through eval, so with eval off they are not visible.
- Skipping errors silenced with `@` ([O-12](#o-12-silenced-php-errors)) would cost one `eval` of `error_reporting()` per error pause.
- S6: in proxy mode PhpStorm sends three `eval` calls per session of its own (Xdebug settings and `PHP_IDE_CONFIG`). They pass through unchanged, so the policy applies to the daemon and the agent, not to the IDE.

- Decides: Lucas.
- Blocks: milestone 1 design.

## O-06 Profiling source

Xdebug's profiler writes cachegrind files to disk and slows the request down a lot. Sampling profilers such as excimer have low overhead but are a separate extension. A profiler inside the debug-claw extension is a third option.

S5 measured a Rust observer handler that only reads the clock and checks a flag at 67 to 86 ns per call, on top of 2.6 to 8.2 ns for the observer check. A PHP function call costs 14 to 25 ns on the same machine. That argues for a sampling profiler (excimer, or sampling inside the extension, which S5 did not try) and for observing only chosen functions.

- Decides: profiling milestone design.
- Blocks: profiling milestone.

## O-07 Compatibility with existing dump protocols

Whether the daemon also accepts the Symfony VarDumper server protocol, Monolog over TCP or other formats that applications already emit. It is input compatibility, not a port, and would help projects that already call `dump()`.

- Decides: Lucas.
- Blocks: nothing yet.

## O-08 Confirm the resource budgets

The budgets in [docs/performance.md](../docs/performance.md#budgets) are drafts. The spikes measured their rows and proposed numbers, listed next to each draft in the [measured and proposed table](../docs/performance.md#measured-and-proposed). The drafts stay in force until each row is decided.

Decisions to take:

- For each draft row, keep the draft or take the proposed number.
- Whether to add the new rows the reports propose: daemon CPU per logpoint hit (S2), extension switched off (S5), desktop app frame time (S4).
- Whether the desktop app gets the daemon's rule of no wakeups faster than once per second. With AccessKit on, the Linux adapter wakes it about twice per second (S4), and turning AccessKit off goes against [docs/ui.md](../docs/ui.md#principles) principle 7. S4 did not find out whether the adapter can start only when an assistive technology asks for it.
- The idle daemon row stays open until milestone 1 measures the real daemon (S3).

- Decides: Lucas, from the spike reports.
- Blocks: the budget values milestone 1 is accepted against. Its requirements point to the budget rows without numbers.

## O-09 Running PHP outside a request

The console with no paused request and running a CLI script under the debugger both need debug-claw to start a PHP process. That PHP lives where the application lives: on the host, in the application's container, or in a container debug-claw starts with the project mounted. Each option needs some configuration, which works against [principle 3](../docs/vision.md#principles), and the answer also decides how the project's autoloader is found.

- Decides: Lucas.
- Blocks: nothing yet. The console inside a paused request does not depend on it.

## O-10 Packaging and code signing

How the daemon and the desktop app are distributed on each system ([ADR-0012](decisions/0012-platforms.md)): archives, installers, package managers, and whether binaries are signed. Signing on macOS and Windows needs paid certificates, and unsigned binaries trigger warnings on both.

- Decides: Lucas.
- Blocks: the first public release.

## O-11 Pausing a running request

Xdebug 3.5.3 does not implement the DBGp `break` command while a request runs. S1 sent it during a running script and got no answer until the script stopped, then error 4 (`unimplemented command`). The only way to pause is Xdebug's control socket, which S1 reached only from inside the PHP container: an abstract Unix socket on Linux (tied to the container's network namespace), a named pipe on Windows (not tested), and nothing on macOS native PHP. The options are a helper run inside the PHP container, which needs access to Docker (for example `docker exec`), or no pause button. The same socket also attaches to a request started without a trigger, so the answer decides that too.

- Decides: Lucas.
- Blocks: the "Pause a running request" row of [docs/features.md](../docs/features.md#execution-control). Milestone 1 leaves it out until this is answered.

## O-12 Silenced PHP errors

Exception breakpoints on PHP errors (`Warning`, `Notice`, `Deprecated`) also pause on errors silenced with `@` (S1). The daemon can tell them apart with one `eval` of `error_reporting()` at the pause, which falls under [O-05](#o-05-eval-policy). The choice is whether to skip them, to show them marked as silenced, or to leave it to a setting.

- Decides: Lucas.
- Blocks: milestone 1 design of exception breakpoints on PHP errors.

## O-13 Pauses the agent is not waiting for

Server notifications did not reach the agent in the MCP clients S3 tested (Claude Code and the Inspector CLI), in either protocol revision. A pause reaches the agent only as the result of a call. The options S3 lists are to tell the agent, in the tool descriptions, to keep a wait running; Claude Code channels (research preview, stdio only, legacy protocol revision only, not tested); or nothing until clients show notifications.

- Decides: Lucas.
- Blocks: milestone 1 design of the MCP tool descriptions.

## O-14 Logpoint capture buffer cap

Logpoint captures go to a buffer with a fixed cap ([ADR-0015](decisions/0015-logpoints.md)). The cap can count captures or bytes. With the per-value limits of ADR-0015 a capture of 5 values can reach about 18.5 KB, so 1000 captures can reach about 18.5 MB. With 1000 small captures RSS grew by about 0.5 MB (S2).

- Decides: Lucas.
- Blocks: the logpoint milestone design.

## O-15 Cycle check for snapshot points

A snapshot point captures every local of the frame to a fixed depth. Xdebug follows parent links with no cycle check, so each level about doubles the size: `$this` of the fixture graph was 26.8 KB at depth 6 and 13.8 MB at depth 20 (S2). The question is whether the daemon detects cycles itself or keeps the depth small.

- Decides: Lucas.
- Blocks: the logpoint milestone design (snapshot points).

## O-16 Desktop app mockups

S4 built styled mockups of the feed and the debugger with egui ([ADR-0001](decisions/0001-rust-native-ui.md)). Whether they look finished enough is a judgment the report leaves to Lucas. If they do not, the toolkit choice is reopened.

- Decides: Lucas.
- Blocks: the desktop app milestone.

## O-17 System font fallback

egui's `system_font_fallback`, on by default, loads system fonts for glyphs that Inter and JetBrains Mono lack (CJK, emoji), so those glyphs can look different on each system. S4 turned it off so its screenshots depend on the bundled fonts only.

- Decides: Lucas.
- Blocks: the desktop app milestone design.

## O-18 Proxy mode details

[ADR-0016](decisions/0016-ide-proxy-mode.md) leaves five questions from S6:

1. How the daemon finds the IDE (a configured host and port, and which default), and what it does when the IDE is not listening. The spike drops the session. Prior art's `auto` mode has the daemon drive the session itself instead.
2. Whether an agent may set its own breakpoints or logpoints while the IDE drives. The spike refuses them, because the IDE would get pauses for breakpoints it does not know. Allowing them means the daemon hides those pauses and notifications from the IDE.
3. Whether the agent sees the `stream` packets (stdout) of an IDE-driven session, which the daemon could copy.
4. Which session the agent reads when several requests are debugged through the IDE at once.
5. Whether the agent's wait skips the pauses an IDE makes for itself. PhpStorm sends `step_into` right after `init`, so the script stops on its first line while PhpStorm sets up; the user never sees that pause, but the agent does.

- Decides: Lucas.
- Blocks: the IDE coexistence milestone design.

## O-19 Starting and finding the daemon

[ADR-0004](decisions/0004-daemon-and-clients.md) says the daemon starts without the desktop app and is discoverable by its clients, but not how. The options include a user service per system, a start on first use by the stdio shim or the app, and a manual start. Discovery covers which MCP port the daemon uses by default, what happens when the port is taken, and how the shim and the app learn it.

- Decides: Lucas.
- Blocks: milestone 1 design.

## O-20 The local project root

Path mapping walks the local project tree ([ADR-0013](decisions/0013-path-mapping.md)), so the daemon has to know where that tree is. The daemon runs on its own and is not started from the project folder. Options include the working directory or roots that the MCP client reports, a folder the user registers once, and the folders of the projects already learned.

- Decides: milestone 1 design.
- Blocks: milestone 1 design.

## O-21 How long a paused request may wait

A paused request holds a PHP worker ([ADR-0003](decisions/0003-framework-agnostic.md)). When no client drives the session (the agent stopped calling, or gave up without cancelling, as S3 saw the Inspector CLI do), the worker stays held. The question is whether the daemon releases it after some time without commands, by `detach` or `stop`, and which actions count as driving the session.

S6 found a related limit outside the daemon: behind nginx, a request paused for longer than `fastcgi_read_timeout` (60 s by default) gets HTTP 504 while PHP stays paused. Whatever the answer, the setup help should say how to raise that timeout.

- Decides: Lucas.
- Blocks: milestone 1 design.

## O-22 Network interfaces of the DBGp listener

PHP in a container on Linux reaches the host through `host.docker.internal` mapped to `host-gateway` ([ADR-0012](decisions/0012-platforms.md)), which is not the host's loopback. A listener on loopback only would not be reached there. A listener on every interface also accepts connections from other machines on the network. The spikes ran the listener inside the Compose network and did not test a daemon on the host.

- Decides: Lucas.
- Blocks: milestone 1 design.
