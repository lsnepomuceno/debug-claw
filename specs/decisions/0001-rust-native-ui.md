# ADR-0001: Rust from scratch, with a native UI

- Status: accepted
- Date: 2026-11-07

## Context

LaraDumps and Buggregator already cover parts of the scope (see [prior art](../../docs/prior-art.md)). LaraDumps is Electron with Vue. Buggregator is Go with a web UI served by the binary. The goal is a single tool, with a desktop app, that does not depend on a browser engine.

## Decision

debug-claw is written in Rust from scratch. The desktop app uses a native Rust GUI toolkit, with no webview. No code is ported from LaraDumps or Buggregator.

The toolkit is not chosen yet. egui (through eframe) is the leading candidate, and spike S4 tests whether it can render the data the app needs.

## Alternatives rejected

- Fork or extend Buggregator. It is Go, its UI is a web app, and its model is per protocol rather than per request. Changing those would mean rewriting most of it.
- Electron, as LaraDumps does. It ships a browser engine with every install.
- Tauri. It is Rust, but the UI runs in the system webview, which is what this decision rules out.

## Consequences

- One language across the daemon, the desktop app and the optional PHP extension.
- Rich views (syntax-highlighted source, large variable trees, flame graphs) have to be drawn with the toolkit instead of HTML and CSS. This costs more work and is why S4 exists.
- The visual polish of LaraDumps takes longer to reach.
