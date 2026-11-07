# Specs

Each deliverable milestone has a folder named `NN-name/` with up to three files, read in this order:

| File | Answers | Changes when |
|---|---|---|
| `requirements.md` | What must be true, in EARS, with acceptance criteria | the required behavior changes |
| `design.md` | How: crates, protocols, data, MCP tools, technical decisions | the chosen implementation changes |
| `tasks.md` | In what order, as verifiable steps | the work moves forward |

A small milestone can skip `design.md` when the requirements leave no design choice open. Writing rules are in [CONVENTIONS.md](CONVENTIONS.md).

Cross-cutting material lives elsewhere and is linked, never copied: product direction in [`docs/`](../docs/), decisions in [`decisions/`](decisions/), unanswered questions in [OPEN.md](OPEN.md), reviewer prompts in [`reviewers/`](reviewers/README.md).

## Milestone state

Only one milestone is active at a time. The others are frozen: they exist so the scope is known, and they are not built ahead of time. A milestone in the requirements state is being specified, and it becomes active for implementation only after its `design.md` passes review.

> Do not build beyond the active milestone. The scope covers a DBGp engine, an MCP server, a desktop app, a dump client, profiling and a PHP extension. Building the shared foundations for all of them before the first breakpoint works is the main risk of the project.

| Milestone | State | Goal |
|---|---|---|
| [00-spikes](00-spikes/requirements.md) | done | Test the riskiest assumptions with throwaway code before fixing any design |
| [01-engine](01-engine/requirements.md) | requirements | Daemon and DBGp engine, driven only through MCP: sessions, line and exception breakpoints, stack, variables, watches, eval in the paused frame, step, continue, stop, and firing a triggered request |

## Candidate milestones

These come after milestone 1. Their order and scope are tentative. The spike reports gave no reason to change the order. They get a folder only when they are about to start. [docs/features.md](../docs/features.md) maps each planned feature to one of them.

2. Logpoints and request correlation: capture without pausing, snapshot points, exception capture, request IDs, the request timeline tool.
3. Desktop app: feed, debugger with console, requests, breakpoints and agent activity, as described in [docs/ui.md](../docs/ui.md).
4. Dump client: a PHP file loaded with `auto_prepend_file`, with labels, colors, screens and tables.
5. Profiling.
6. PHP extension in Rust: PDO, curl and exception observers.
7. IDE coexistence: proxy mode for PhpStorm, read only while PhpStorm drives the session ([ADR-0016](decisions/0016-ide-proxy-mode.md)).

## Review before implementation

Before a milestone moves from design to implementation, its `design.md` goes through the [implementation readiness reviewer](reviewers/implementation-readiness.md). Milestones that add or change MCP tools also go through the [MCP contract reviewer](reviewers/mcp-contract.md). Both run in a fresh context. See [ADR-0007](decisions/0007-spec-process.md).
