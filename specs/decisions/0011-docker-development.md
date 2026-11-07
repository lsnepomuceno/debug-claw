# ADR-0011: Development and tests run in Docker

- Status: accepted
- Date: 2026-11-07

## Context

Development needs a Rust toolchain, several PHP versions with Xdebug, a web server, and later the build tools for a PHP extension. The developer's machine should not get any of these installed for this project.

## Decision

Everything needed to build, test and measure the project runs in Docker containers, defined in the repository and started with Docker Compose. Nothing is installed on the host for this project, and the host's own toolchains (a local `cargo` or `php`, if present) are not used.

- A `toolchain` image has the Rust toolchain and the cargo tools from [ADR-0010](0010-workspace-layout.md). Builds, tests, benchmarks and `xtask` run in it.
- The cargo registry, the git cache and `target/` live in named volumes, so builds stay incremental across container runs.
- PHP services (PHP-FPM with Xdebug, nginx) run as separate containers on the same Compose network as the daemon.
- The desktop app runs in a container that gets the host's X11 socket (`/tmp/.X11-unix` and `DISPLAY`) and the GPU render node (`/dev/dri`).
- Tools that already exist on the host and are part of the workflow, namely Docker itself, the IDE (PhpStorm) and the MCP clients (Claude Code), stay on the host. They reach containers through published ports bound to `127.0.0.1`, and containers reach them through `host.docker.internal` mapped to the host gateway.

## Alternatives rejected

- Installing the Rust toolchain and PHP on the host. Ruled out by the developer.
- Dev containers tied to one IDE. They depend on editor support, and Compose already covers the services.
- Running the desktop app only on the host. It would require a host Rust build, which this decision rules out.

## Consequences

- Resource budgets ([ADR-0008](0008-resource-budgets.md)) are measured inside containers, from cgroup counters or `docker stats`. Reports say so, because the numbers include no host-side processes.
- The first build in a fresh volume is slow. Later builds reuse the volumes.
- GUI forwarding depends on an X11 session on the host. A Wayland-only host would need a different socket mount.
- Files created by containers in the working tree must belong to the host user, so containers run with the host UID and GID.
