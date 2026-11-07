# Open questions

Questions without an answer yet. Each one says who decides and what it blocks. When a question is answered, the answer moves to an ADR or a spec, and the entry here is removed.

## O-02 Minimum PHP and Xdebug versions

Which PHP versions and which Xdebug versions are supported. This limits which DBGp features the engine can rely on and which PHP versions the extension has to build for.

- Decides: Lucas, informed by S5.
- Blocks: milestone 1 design.

## O-04 Storage

How the daemon stores sessions, events and breakpoints, and whether anything survives a restart. Candidates include SQLite and an embedded key-value store.

- Decides: milestone 1 design.
- Blocks: milestone 1 design.

## O-05 Eval policy

Evaluating PHP expressions runs code inside the application. It is off by default (see [vision](../docs/vision.md)), but the switch is still open: global, per project or per session, and whether logpoint expressions count as eval. The same answer sets the permission scopes an MCP client can be granted (read, control execution, eval, set variables) and whether the desktop app can grant them per client.

- Decides: Lucas, after S2.
- Blocks: milestone 1 design.

## O-06 Profiling source

Xdebug's profiler writes cachegrind files to disk and slows the request down a lot. Sampling profilers such as excimer have low overhead but are a separate extension. A profiler inside the debug-claw extension is a third option.

- Decides: profiling milestone design, informed by S5.
- Blocks: profiling milestone.

## O-07 Compatibility with existing dump protocols

Whether the daemon also accepts the Symfony VarDumper server protocol, Monolog over TCP or other formats that applications already emit. It is input compatibility, not a port, and would help projects that already call `dump()`.

- Decides: Lucas.
- Blocks: nothing yet.

## O-08 Confirm the resource budgets

The budgets in [docs/performance.md](../docs/performance.md#budgets) are drafts. Each spike measures its rows and proposes a final number.

- Decides: Lucas, from the spike reports.
- Blocks: milestone 1 requirements.

## O-09 Running PHP outside a request

The console with no paused request and running a CLI script under the debugger both need debug-claw to start a PHP process. That PHP lives where the application lives: on the host, in the application's container, or in a container debug-claw starts with the project mounted. Each option needs some configuration, which works against [principle 3](../docs/vision.md#principles), and the answer also decides how the project's autoloader is found.

- Decides: Lucas.
- Blocks: nothing yet. The console inside a paused request does not depend on it.

## O-10 Packaging and code signing

How the daemon and the desktop app are distributed on each system ([ADR-0012](decisions/0012-platforms.md)): archives, installers, package managers, and whether binaries are signed. Signing on macOS and Windows needs paid certificates, and unsigned binaries trigger warnings on both.

- Decides: Lucas.
- Blocks: the first public release.
