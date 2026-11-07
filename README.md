# debug-claw

A local debugger for PHP applications, written in Rust, built so that an AI agent can drive it through MCP as fully as a person can through its desktop app.

It talks to Xdebug directly, so breakpoints, stack frames and live variables work in any PHP application without a framework package. Dumps, request timelines and profiling sit on top of that, all tied to the request that produced them.

## Status

Design phase. There is no code yet. The project is specified in [`specs/`](specs/README.md), and the first milestone is a set of spikes that test the riskiest technical assumptions before any design is fixed.

## Where to read

- [`docs/vision.md`](docs/vision.md): what the tool is for, its principles and its non-goals.
- [`docs/prior-art.md`](docs/prior-art.md): what LaraDumps and Buggregator do, and where this project differs.
- [`specs/README.md`](specs/README.md): how specs work here and which milestone is active.
- [`specs/decisions/`](specs/decisions/): architecture decisions, including the options that were rejected.
- [`specs/OPEN.md`](specs/OPEN.md): questions that are still unanswered.
