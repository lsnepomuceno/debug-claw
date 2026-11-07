# Vision

## The problem

Debugging a PHP application today means choosing between two kinds of tools. Step debuggers (Xdebug with an IDE) show real values at runtime but need path mappings, IDE setup and a person at the keyboard. Dump tools (LaraDumps, Ray, VarDumper) are easy to use but only show what someone remembered to dump, and the richer features depend on a specific framework.

AI coding agents get neither: an agent reads the code and guesses what a variable holds. When it can run the application, it adds `var_dump` calls, reads the output and removes them again.

## What debug-claw is

A local daemon and a native desktop app, written in Rust, that:

- accepts Xdebug sessions from any PHP application, with no framework package and as little configuration as the PhpStorm debugger needs;
- exposes every capability through MCP, so an agent can set a breakpoint, fire a request, wait for the pause and read the real values;
- shows dumps, logpoints, exceptions and profiles grouped by the request that produced them;
- lets a person do the same work in the desktop app.

## Principles

1. MCP first. A capability exists when an MCP client can use it. The desktop app never has a feature that MCP lacks. Tool descriptions are part of the product and are reviewed like code.
2. Framework agnostic. The core speaks Xdebug's DBGp protocol and plain HTTP. Nothing in the core knows about Laravel, Symfony or any other framework.
3. Minimal configuration. If Xdebug can reach the daemon, debugging works. Path mappings, ports and project roots are inferred where possible and only asked for when inference fails.
4. The request is the unit. Every event (pause, dump, logpoint hit, exception, profile) belongs to a request. An agent can ask for everything that happened during one request in a single call.
5. Non-blocking by default. Logpoints capture values and let the request continue. A paused request holds a PHP worker, so pauses have timeouts and are released when no one is driving them.
6. Local and explicit about risk. The daemon binds to the loopback interface. Evaluating PHP expressions runs code inside the application, so it is off until the user turns it on.
7. Light on resources. Memory and CPU use have budgets, and missing one is a bug. The cost added to the PHP request matters most, because it distorts what the developer measures. See [performance](performance.md).

## Non-goals

- Production monitoring or APM. The tool is for development machines and containers.
- A cloud service or shared server. Everything runs on the developer's machine.
- Framework packages in the core. Integrations, if they exist, live outside it.
- A web UI or a webview-based desktop app. See [ADR-0001](../specs/decisions/0001-rust-native-ui.md).
- Replacing the IDE debugger. When PhpStorm is attached, it can keep driving the session.
