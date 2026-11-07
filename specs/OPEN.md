# Open questions

Questions without an answer yet. Each one says who decides and what it blocks. When a question is answered, the answer moves to an ADR or a spec, and the entry here is removed.

## O-01 License

The repository is public but has no license, so nobody can legally reuse the code. Common choices for Rust projects are MIT, Apache-2.0, or both. The reviewer prompts in [`reviewers/`](reviewers/README.md) are derived from Apache-2.0 material and keep that license whatever is chosen here.

- Decides: Lucas.
- Blocks: the first code that is not a spike.

## O-02 Minimum PHP and Xdebug versions

Which PHP versions and which Xdebug versions are supported. This limits which DBGp features the engine can rely on and which PHP versions the extension has to build for.

- Decides: Lucas, informed by S5.
- Blocks: milestone 1 design.

## O-03 Platforms

Linux first is assumed. Whether macOS and Windows are supported from the start affects the desktop app, packaging and the PHP extension builds.

- Decides: Lucas.
- Blocks: desktop app milestone and distribution.

## O-04 Storage

How the daemon stores sessions, events and breakpoints, and whether anything survives a restart. Candidates include SQLite and an embedded key-value store.

- Decides: milestone 1 design.
- Blocks: milestone 1 design.

## O-05 Eval policy

Evaluating PHP expressions runs code inside the application. It is off by default (see [vision](../docs/vision.md)), but the switch is still open: global, per project or per session, and whether logpoint expressions count as eval.

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
