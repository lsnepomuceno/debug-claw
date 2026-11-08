# ADR-0001: Rust from scratch, with a native UI

- Status: accepted
- Date: 2026-11-07
- Evidence: [S4 report](../00-spikes/reports/s4-egui.md)

## Context

LaraDumps and Buggregator already cover parts of the scope (see [prior art](../../docs/prior-art.md)). LaraDumps is Electron with Vue. Buggregator is Go with a web UI served by the binary. The goal is a single tool, with a desktop app, that does not depend on a browser engine.

## Decision

debug-claw is written in Rust from scratch. The desktop app uses a native Rust GUI toolkit, with no webview. No code is ported from LaraDumps or Buggregator.

The toolkit is egui through eframe 0.37, with the wgpu renderer and AccessKit on. Spike S4 rendered every large view the app needs with it inside the draft budgets: a lazy tree of 100 000 nodes, 10 000 mixed dumps, a flame graph of 50 000 frames and a 5 000-line source view. The heaviest view took at most 4.2 ms of CPU per frame at p99 on the host GPU, and an idle window drew no frames.

wgpu over glow: on Linux glow used up to 13 MB less RSS and 30% to 50% less CPU per frame at p50, and both stayed far inside the budgets. wgpu is eframe's default, uses the native graphics API on each system (Metal on macOS, DX12 or Vulkan on Windows), and carries a GL fallback. glow on macOS depends on OpenGL, which Apple deprecated.

### View rules

S4 met its budgets because of these rules, and every view follows them:

- Every large view is virtualised: rows have fixed or prefix-summed heights, and only visible rows are built.
- A tree fetches children when a node is expanded, never ahead.
- Nothing repaints without input or new data.

## Alternatives rejected

- Fork or extend Buggregator. It is Go, its UI is a web app, and its model is per protocol rather than per request. Changing those would mean rewriting most of it.
- iced or Slint. S4 did not try them, because egui met every criterion.
- Electron, as LaraDumps does. It ships a browser engine with every install.
- Tauri. It is Rust, but the UI runs in the system webview, which is what this decision rules out.

## Consequences

- One language across the daemon, the desktop app and the optional PHP extension.
- Rich views (syntax-highlighted source, large variable trees, flame graphs) are drawn with the toolkit instead of HTML and CSS, which costs more work.
- The visual polish of LaraDumps takes longer to reach. Whether the S4 mockups look finished is for Lucas to judge ([O-16](../OPEN.md#o-16-desktop-app-mockups)). If he rejects the look, this choice is reopened.
- S4 ran on Linux only. Frame times, idle behaviour and RSS on Windows and macOS are measured before the desktop app milestone.
- With AccessKit on, the Linux adapter wakes the app about twice per second ([O-08](../OPEN.md#o-08-budgets-left-open)).
