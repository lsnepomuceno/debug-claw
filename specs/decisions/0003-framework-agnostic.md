# ADR-0003: Framework agnostic, Xdebug as the baseline

- Status: accepted
- Date: 2026-11-07

## Context

LaraDumps gets most of its value from Laravel observers, which do not exist in other frameworks. The goal is a tool that works in any PHP application with as little setup as the PhpStorm debugger needs.

## Decision

The core speaks Xdebug's DBGp protocol and plain HTTP, and knows nothing about any framework. With only Xdebug installed and pointed at the daemon, a user gets breakpoints, stack frames, variables, stepping and logpoints, with no code changes in the application.

Configuration is inferred where possible: the daemon listens on the Xdebug default port (9003), learns path mappings from the paths Xdebug reports, and asks the user only when inference fails. The mapping rules are in [ADR-0013](0013-path-mapping.md).

## Alternatives rejected

- Framework packages as the main integration, as LaraDumps does. Every framework would need its own package, and plain PHP applications would get the smallest feature set.
- A required PHP client library. It adds a Composer dependency to every project and still cannot see runtime values without Xdebug.

## Consequences

- Features that need data Xdebug does not provide (SQL queries, outgoing HTTP calls) come from the optional layers in [ADR-0005](0005-layered-php-integration.md), never from framework hooks in the core.
- Xdebug's limits become the tool's limits at the baseline: one debugging client per request, and a paused request holds a PHP worker.
- Xdebug 3.5.3 does not implement the DBGp `break` command while a request runs ([S1](../00-spikes/reports/s1-path-mapping.md)). The only way to pause a running request is Xdebug's control socket, which a daemon outside the PHP container cannot reach: an abstract Unix socket on Linux, a named pipe on Windows, nothing on macOS. Milestone 1 has no pause button. A later milestone adds a helper run inside the PHP container through `docker exec` ([docs/features.md](../../docs/features.md#execution-control)).
