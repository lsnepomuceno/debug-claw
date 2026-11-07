# ADR-0005: Layered PHP integration

- Status: proposed
- Date: 2026-11-07
- Confirmed by: spike S5 (layer 3 only)

## Context

Xdebug provides breakpoints, stack frames, variables, eval, traces and profiles. It does not provide labelled dumps, and it cannot report SQL queries or outgoing HTTP calls as events without a breakpoint on them. Every extra component the user has to install works against [ADR-0003](0003-framework-agnostic.md).

## Decision

PHP integration comes in three layers. Each one is optional on top of the one before it.

1. Xdebug only. No code in the application. Breakpoints, logpoints, stack, variables, eval, and the trace and profile files Xdebug writes.
2. A single PHP file loaded through `auto_prepend_file`, installed once per machine. It defines the dump functions (labels, colors, screens, tables) and sends dumps to the daemon over HTTP with a short timeout, so the application keeps running when the daemon is down.
3. A PHP extension written in Rust (with `ext-php-rs`) that uses the PHP Observer API to report PDO queries, curl calls, exceptions, timing and memory for any application.

## Alternatives rejected

- A Composer package for the dump functions. It has to be added to every project, and it would be the only layer that touches the project's dependencies.
- Making the extension the baseline. Prebuilt extension binaries are needed for every PHP version, thread-safety mode and platform, which is too much friction for the first contact with the tool.

## Consequences

- Layer 2 is PHP code by necessity. It stays small and has no dependencies.
- Layer 3 has the highest cost in the project (FFI with the Zend engine, a build matrix, distribution). S5 decides whether it is feasible, and its milestone comes last among the candidates.
