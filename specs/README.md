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

Only one milestone is active at a time. The others are frozen: they exist so the scope is known, and they are not built ahead of time.

> Do not build beyond the active milestone. The scope covers a DBGp engine, an MCP server, a desktop app, a dump client, profiling and a PHP extension. Building the shared foundations for all of them before the first breakpoint works is the main risk of the project.

| Milestone | State | Goal |
|---|---|---|
| [00-spikes](00-spikes/requirements.md) | active | Test the riskiest assumptions with throwaway code before fixing any design |

## Candidate milestones

These come after the spikes. Their order and scope are tentative and will be revised with the spike reports. They get a folder only when they are about to start.

1. Daemon and DBGp engine, driven only through MCP: sessions, line and exception breakpoints, stack, variables, step, continue, stop.
2. Logpoints and request correlation: capture without pausing, request IDs, the request timeline tool.
3. Desktop app: sessions, variables, breakpoints and the request timeline.
4. Dump client: a PHP file loaded with `auto_prepend_file`, with labels, colors, screens and tables.
5. Profiling.
6. PHP extension in Rust: PDO, curl and exception observers.
7. IDE coexistence: proxy mode for PhpStorm.

## Review before implementation

Before a milestone moves from design to implementation, its `design.md` goes through the [implementation readiness reviewer](reviewers/implementation-readiness.md). Milestones that add or change MCP tools also go through the [MCP contract reviewer](reviewers/mcp-contract.md). Both run in a fresh context. See [ADR-0007](decisions/0007-spec-process.md).
