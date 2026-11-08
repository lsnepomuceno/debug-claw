# Open questions

Questions without an answer yet. Each one says who decides and what it blocks. When a question is answered, the answer moves to an ADR or a spec, and the entry here is removed.

## O-04 Storage

How the daemon stores sessions, events and breakpoints, and whether anything survives a restart. Candidates include SQLite and an embedded key-value store.

- Decides: milestone 1 design.
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

## O-08 Budgets left open

The budgets were decided on 2026-11-07 from the spike reports ([docs/performance.md](../docs/performance.md#budgets)). Two points stay open:

- Whether the desktop app gets the daemon's rule of no wakeups faster than once per second. With AccessKit on, the Linux adapter wakes it about twice per second (S4), and turning AccessKit off goes against [docs/ui.md](../docs/ui.md#principles) principle 7. S4 did not find out whether the adapter can start only when an assistive technology asks for it.
- The number for the new row "Daemon CPU per logpoint hit". S2 measured about 40 us one read at a time and 24 us pipelined, and proposed the row without a number.

- Decides: Lucas.
- Blocks: the desktop app milestone (first point) and the logpoint milestone (second point).

## O-09 Running PHP outside a request

The console with no paused request and running a CLI script under the debugger both need debug-claw to start a PHP process. That PHP lives where the application lives: on the host, in the application's container, or in a container debug-claw starts with the project mounted. Each option needs some configuration, which works against [principle 3](../docs/vision.md#principles), and the answer also decides how the project's autoloader is found.

- Decides: Lucas.
- Blocks: nothing yet. The console inside a paused request does not depend on it.

## O-10 Packaging and code signing

How the daemon and the desktop app are distributed on each system ([ADR-0012](decisions/0012-platforms.md)): archives, installers, package managers, and whether binaries are signed. Signing on macOS and Windows needs paid certificates, and unsigned binaries trigger warnings on both.

- Decides: Lucas.
- Blocks: the first public release.

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

## O-20 The local project root

Path mapping walks the local project tree ([ADR-0013](decisions/0013-path-mapping.md)), so the daemon has to know where that tree is. The daemon runs on its own and is not started from the project folder. Options include the working directory or roots that the MCP client reports, a folder the user registers once, and the folders of the projects already learned.

- Decides: milestone 1 design.
- Blocks: milestone 1 design.

## O-23 Minimum PHP version for the extension

[ADR-0017](decisions/0017-supported-versions.md) supports PHP 7.4 and later, but the extension cannot reach that far: `ext-php-rs` supports PHP 8.1 and later, and the observer API it uses exists from PHP 8.0 (S5). The options are PHP 8.1 and later for the extension only, or a second way to observe calls on 7.4 and 8.0 (another binding, or C). Each supported minor version adds one binary per thread-safety mode and platform.

- Decides: Lucas.
- Blocks: the PHP extension milestone.
