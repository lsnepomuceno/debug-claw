# ADR-0010: Workspace layout and architecture patterns

- Status: proposed
- Date: 2026-11-07
- Confirmed by: milestone 1 design

## Context

The daemon, the MCP server, the stdio shim and the desktop app share one core: DBGp sessions, breakpoints, logpoints and path mapping. [ADR-0002](0002-mcp-first.md) requires that the desktop app never has a capability MCP lacks, and [ADR-0008](0008-resource-budgets.md) requires bounded memory and no polling. The layout should make both hold by construction.

Large Rust projects (rust-analyzer, Zed, uv, ruff, tokio) converge on a few conventions rather than one named architecture. This ADR adopts the ones that fit.

## Decision

### Layout

One Cargo workspace with a virtual manifest at the root and a flat `crates/` folder. Each crate's name matches its folder.

```
crates/
  dbgp/        sans-IO DBGp: parser, serializer, session state machine
  engine/      sessions as actors, breakpoints, logpoints, path mapping
  store/       storage (backend open in O-04)
  mcp/         MCP tools over the engine
  api/         local API for the desktop app
  types/       types shared by the daemon and the desktop app
  daemon/      binary: configuration, listeners, wiring
  mcp-stdio/   binary: stdio shim that forwards to the daemon
  app/         binary: the desktop app
xtask/         project automation in Rust (benchmarks, budget checks, release)
```

The PHP extension ([ADR-0005](0005-layered-php-integration.md)) gets its own crate when its milestone starts.

Versions and lints are declared once, in `[workspace.dependencies]` and `[workspace.lints]`.

### Dependency rules

- `dbgp` has no async runtime and no I/O. It turns bytes into events and commands into bytes.
- `engine` depends on `dbgp`, `store` and `types`. It knows nothing about MCP, HTTP or the desktop app.
- `mcp` and `api` are adapters over `engine`. Neither depends on the other.
- `app` depends on `types` and talks to the daemon through `api`. It never links `engine`.
- Only the binaries (`daemon`, `mcp-stdio`, `app`) read configuration and pick concrete implementations.

### Patterns

- Sans-IO protocols, as in `quinn-proto` and `str0m`. DBGp is tested from recorded byte streams, with no Xdebug and no sockets, and benchmarked without network noise.
- One actor per DBGp session: a tokio task owns the socket and the session state, and the rest of the daemon sends it messages through a bounded channel. Backpressure and the memory cap come from the channel bound, not from shared locks.
- Typestate and newtypes where a mistake is easy and costly: commands that need a paused session take a paused-session type, and remote and local paths are different types.
- Traits only at real boundaries (storage, clock, transport). Everything else uses concrete types. No trait per service and no `dyn` where a generic or a concrete type works.
- In the desktop app, state is kept apart from drawing code, and each view is a function of that state.

### Tooling

- Errors: `thiserror` in library crates, `anyhow` in binaries.
- Logs: `tracing`.
- CLI: `clap`.
- Tests: `cargo-nextest`, `insta` snapshots for DBGp responses and MCP results, `proptest` for the parser.
- Budgets: `divan` benchmarks and `dhat` heap measurements, run from `xtask` so they can fail the build.
- CI: `clippy`, `rustfmt` and `cargo-deny`, which also rejects dependencies whose license is incompatible with MIT ([ADR-0009](0009-mit-license.md)).

## Alternatives rejected

- A single crate with modules. Simpler at first, but nothing stops the desktop app from calling engine internals that MCP cannot reach, and every change recompiles everything.
- Layered DDD or clean architecture, with an interface for every service and repository. Each `dyn Trait` adds indirection and boilerplate without a second implementation to justify it.
- `Arc<Mutex<...>>` around shared session state. Lock contention and unbounded queues are harder to measure and cap than one task per session with a bounded channel.
- Nested crate folders (`crates/core/dbgp`, `crates/adapters/mcp`). The dependency rules above already express the grouping, and a flat folder is easier to navigate.

## Consequences

- MCP and the desktop app reach the same capabilities through the same engine, which enforces ADR-0002 structurally.
- The DBGp crate can be published on its own later, if that proves useful.
- More `Cargo.toml` files to maintain. Workspace inheritance keeps that small.
- Spike code does not follow this layout ([CONVENTIONS.md](../CONVENTIONS.md#spike-code)).
