# S4 Native UI for large debugging data: report

Spike code: [`spikes/s4-egui/`](../../../spikes/s4-egui/). Raw outputs of every run are in its `results/` folder, and the screenshots are in [`s4-egui/`](s4-egui/).

egui can do it. Version 0.37 through eframe renders all four views with the large datasets, and on the host GPU the 99th percentile frame of the heaviest view takes 4.83 ms. An idle window does not repaint, and on the host GPU RSS stays under 128 MB with every dataset loaded at once. No criterion failed, so iced and Slint were not tried.

## Versions

| Item | Version |
|---|---|
| Rust | rustc 1.99.1, cargo 1.99.1, image `rust:1.99-slim-trixie` |
| eframe, egui, egui-winit, egui-wgpu, egui_glow | 0.37.0 |
| wgpu, glow, glutin, winit | 30.0.1, 0.17.0, 0.32.3, 0.30.13 |
| AccessKit | accesskit 0.25.2 (0.24.1 also in the tree), accesskit_unix 0.24.1 over zbus 5.19 |
| tree-sitter-highlight, tree-sitter-php | 0.27.2, 0.25.1 (both MIT) |
| egui-phosphor | 0.14.0 (MIT OR Apache-2.0), Phosphor regular font, 477 KB |
| Fonts | Inter 4.1 and JetBrains Mono 2.304, SIL OFL 1.1, fetched at image build time |
| Mesa, LLVM | 25.0.7-2+deb13u1, 19.1.7 |

The egui crates are MIT OR Apache-2.0. The machine is a laptop with an Intel i7-1260P and Iris Xe graphics (ADL GT2), kernel 7.0.0-38, X.Org 21.1.11.

Two displays were used, and every number below says which one it comes from:

- Xvfb: a virtual display at `:99` inside the `ui` container. Mesa renders on the CPU there: llvmpipe for GL 4.5 and lavapipe for Vulkan 1.4.305. Screenshots, selection, AT-SPI and input scripts ran here.
- Host: the host X11 display `:0` with `/dev/dri`, through the `ui-host` container. GL 4.6 and Vulkan on the Iris Xe. Only short-lived windows ran there, for the GPU numbers.

`results/versions-xvfb.txt` and `results/versions-host.txt` hold the full version output.

## How to rerun

From `spikes/s4-egui/`:

```sh
docker compose up -d --build
docker compose exec -T ui cargo build --release
docker compose exec -T ui sh scripts/screenshots.sh        # mockups and views
docker compose exec -T ui sh scripts/goto.sh               # Ctrl+G, source-goto-light.png
docker compose exec -T ui sh scripts/flame.sh              # zoom and hover, flame-zoom-dark.png
docker compose exec -T ui sh scripts/select.sh             # results/select-xvfb.txt
docker compose exec -T ui sh scripts/atspi.sh debugger     # results/atspi-xvfb.txt
docker compose exec -T ui sh scripts/atspi.sh feed
docker compose exec -T ui sh scripts/idle-all.sh           # results/idle-xvfb.txt
docker compose exec -T ui sh scripts/memory.sh             # results/memory-xvfb.txt
docker compose exec -T ui sh scripts/bench.sh              # results/bench-xvfb.txt
docker compose exec -T ui sh scripts/threads.sh wgpu 20 --view debugger
docker compose exec -T ui sh scripts/noak.sh 20            # results/threads-noak-xvfb.txt

docker compose --profile host up -d ui-host                # needs xhost +SI:localuser:$USER
docker compose exec -T ui-host sh scripts/host.sh          # results/*-host.txt
docker compose --profile host down
```

Heap numbers come from a counting global allocator in the binary (`src/alloc.rs`). RSS, CPU ticks and context switches come from `/proc`. Frame times come from `Frame::info().cpu_usage` (ui and paint of one frame, without the wait for vsync) and from the wall clock between frames. The benchmarks run with vsync off and inject input through `raw_input_hook`, 600 frames after 30 warm-up frames.

## Criterion 1: a lazy variable tree with 100 000 nodes

The generated tree stands in for the daemon. `$orders` holds 1 999 orders of 50 nodes each (99 951 nodes in all), and a node's children are built on its first expand, the way the app will read Xdebug one level at a time. The view keeps a flat list of visible rows and paints only the rows inside the viewport (`ScrollArea::show_rows`). Expanding a node edits only the rows below it.

`--bench tree-scroll` expands the root (2 000 rows) and scrolls three lines per frame, down and back up. `--bench tree-expand` expands one order on even frames and collapses it on odd ones.

| Scenario | Display | Backend | CPU ms p50 / p95 / p99 | Wall ms p99 |
|---|---|---|---|---|
| tree-scroll | host | glow | 0.36 / 0.71 / 0.95 | 1.67 |
| tree-scroll | host | wgpu | 0.57 / 1.38 / 1.76 | 2.66 |
| tree-expand | host | glow | 0.57 / 1.00 / 1.38 | 1.66 |
| tree-expand | host | wgpu | 0.86 / 1.50 / 1.83 | 2.38 |
| tree-scroll | Xvfb | glow | 1.03 / 1.75 / 1.98 | 6.83 |
| tree-scroll | Xvfb | wgpu | 3.15 / 4.28 / 4.67 | 10.17 |
| tree-expand | Xvfb | glow | 0.99 / 1.61 / 1.80 | 6.60 |
| tree-expand | Xvfb | wgpu | 3.13 / 4.45 / 5.10 | 10.31 |

A 60 Hz display gives a frame 16.7 ms. The worst p99 on the host is 2.66 ms. Even on CPU rendering under Xvfb, p99 stays under 10.4 ms. Expanding all 99 951 nodes at once, the worst case for the row list, takes 19 to 22 ms on the host (`expand_all_ms` in `results/memory-host.txt`), about one dropped frame at 60 Hz.

Result: met, judged from the frame times. Nobody watched a long scroll on the host screen, so "no visible stutter" rests on the numbers. Screenshot: [tree-light.png](s4-egui/tree-light.png).

## Criterion 2: a list of 10 000 mixed dumps

The feed holds 10 000 cards of mixed height: text, SQL with bindings, tables and collapsed object trees. Card heights come from the content, so a prefix sum gives every card's offset without laying it out, and only the cards inside the viewport are built each frame. `--bench dumps-scroll` scrolls three lines per frame.

| Display | Backend | CPU ms p50 / p95 / p99 | Wall ms p99 |
|---|---|---|---|
| host | glow | 1.34 / 2.58 / 3.23 | 3.44 |
| host | wgpu | 1.93 / 3.27 / 4.20 | 4.83 |
| Xvfb | glow | 2.48 / 4.07 / 4.67 | 10.28 |
| Xvfb | wgpu | 4.34 / 5.93 / 6.74 | 14.12 |

This is the most expensive view. On the host its p99 is 4.83 ms. Under Xvfb with wgpu it comes closest to the 16.7 ms frame, at 14.12 ms. Loading the 10 000 dumps takes 11 to 17 ms.

Result: met. Screenshots: [feed-light.png](s4-egui/feed-light.png), [feed-dark.png](s4-egui/feed-dark.png).

## Criterion 3: a flame graph with 50 000 frames

The generated profile has 50 000 frames, depth up to 31 and 721 distinct names. It loads in 2.2 to 3.2 ms. Frames are stored in preorder with the index one past their subtree, so painting skips a whole subtree when it is off screen or narrower than a pixel. At full view 1 118 frames are drawn. The frame under the cursor is found through a per-depth list sorted by start time, shown in a tooltip (name, total, self, depth) and in a status line under the graph. Ctrl+wheel zooms around the cursor, dragging pans, and a double click zooms to a frame.

`scripts/flame.sh` zooms in with twelve Ctrl+wheel clicks and hovers a frame. The window then shows 0.823% of the time and draws 257 frames: [flame-zoom-dark.png](s4-egui/flame-zoom-dark.png). The full view is [flame-dark.png](s4-egui/flame-dark.png).

`--bench flame-zoom` zooms in for 120 frames and out for 120, around a pointer that drifts sideways.

| Display | Backend | CPU ms p50 / p95 / p99 | Wall ms p99 |
|---|---|---|---|
| host | glow | 0.69 / 1.35 / 1.83 | 2.28 |
| host | wgpu | 1.20 / 1.93 / 2.30 | 3.05 |
| Xvfb | glow | 2.79 / 4.58 / 6.01 | 11.46 |
| Xvfb | wgpu | 3.41 / 4.52 / 5.37 | 14.87 |

Result: met.

## Criterion 4: a 5 000-line PHP source view

The generated file has 4 987 lines: a class whose methods repeat a few shapes with different names and values. tree-sitter-php highlights it once at load, which takes 82 to 102 ms, and the spans are kept per line. Only the visible rows are laid out. The gutter shows line numbers and breakpoints. Ctrl+G opens a field that takes a line number and scrolls to it.

`scripts/goto.sh` presses Ctrl+G, types 4200 and Enter. Line 4200 ends up in view and marked: [source-goto-light.png](s4-egui/source-goto-light.png). The top of the file is [source-dark.png](s4-egui/source-dark.png).

| Display | Backend | CPU ms p50 / p95 / p99 | Wall ms p99 |
|---|---|---|---|
| host | glow | 0.38 / 0.78 / 0.99 | 1.57 |
| host | wgpu | 0.74 / 1.25 / 1.64 | 2.27 |
| Xvfb | glow | 1.12 / 1.85 / 2.29 | 7.63 |
| Xvfb | wgpu | 3.39 / 4.47 / 5.20 | 11.12 |

The highlighting cost at load grows with the file. For a file much larger than 5 000 lines, the app would need to highlight in a background thread or only near the viewport. That was not tried.

Licenses: tree-sitter-highlight and tree-sitter-php are MIT. The PHP grammar is C code compiled by the crate's build script, so the build needs a C compiler.

Result: met.

## Criterion 5: selection and copy, themes, AccessKit, Windows and macOS

### Selection and copy

`scripts/select.sh` drags across text with xdotool under Xvfb, presses Ctrl+C and reads the clipboard with xclip (`results/select-xvfb.txt`):

| Where | Clipboard |
|---|---|
| Debugger, one source line | `$total = $order->total` |
| Debugger, a drag across source lines | two lines, with their indentation |
| Debugger, tree context menu, "Copy value" | `"pending"` |
| Feed, SQL query | `SELECT * FROM ord` (the part dragged over) |
| Feed, table rows | `SKU-1042 2 19.90 39.80` and `SKU-2210 1 149.00 149.00` |

A selection can cross several labels: the table rows came out with their cells joined by spaces. Variable tree rows are painted click targets that expand and select, so a drag over them selects nothing. They copy through the context menu (Copy value, Copy name).

Under Xvfb there is no window manager. Keys sent with `xdotool --window` did not reach the app, so the scripts focus the window and send input through XTEST.

### Themes

Every mockup has a light and a dark palette, defined once in `src/theme.rs`. Screenshots:

- Feed: [light](s4-egui/feed-light.png), [dark](s4-egui/feed-dark.png)
- Debugger: [light](s4-egui/debugger-light.png), [dark](s4-egui/debugger-dark.png)
- Tooltip on an enabled button: [Step over, dark](s4-egui/tooltip-step-over-dark.png)
- Tooltip on a disabled button, with the reason: [Run to cursor, light](s4-egui/tooltip-run-to-cursor-light.png)

egui-winit passes the system theme from winit to egui, and egui follows it when the preference is `System`. The spike sets the theme with `--theme`, so following the system was not checked.

### AccessKit

AccessKit is on by default in eframe. Two checks:

- Headless (`--a11y-dump`, `results/a11y-dump.txt`): the debugger mockup produces 192 AccessKit nodes, 131 with a label. It has 17 buttons, 10 tree items, 10 list items and one text input. Button names are the action names, and disabled buttons are flagged (Run to cursor, Take over). The feed has 181 nodes with 8 links and 29 list items.
- AT-SPI under Xvfb (`scripts/atspi.sh`, `results/atspi-xvfb.txt`): with a D-Bus session and the accessibility bus started in the container, a walk through `Atspi` finds the application `s4-egui`. Its debugger tree has 107 nodes: 17 buttons ("Take over" and "Run to cursor" disabled), 10 tree items, 10 list items, one entry and 41 labels. The feed has 89 nodes, with 8 links and 30 list items. Tree items carry their state in the name, for example `status = string "pending"`, with "expanded" or "collapsed" on nodes that have children.

The walk found 15 nodes with the role `unknown` in the debugger and one in the feed. A screen reader was not run, so how Orca reads those nodes is not known.

Two things were added by hand with `widget_info`: buttons with an icon and a text label take the action name as their AccessKit name, and painted rows such as the tree rows get a role and a name.

### Fonts and icons

Inter is the interface font, JetBrains Mono the code font, both under the SIL OFL 1.1. The Dockerfile downloads them from their GitHub releases and checks their SHA-256 (see the [README](../../../spikes/s4-egui/README.md)). Icons are Phosphor through `egui-phosphor` 0.14.

Three findings:

- Inter has 745 glyphs in the private use area, where Phosphor puts its icons. With Phosphor as a fallback font behind Inter, those codepoints render as Inter's glyphs. `theme::icon_job` gives each icon its own section in an `icons` font family, and every icon next to text goes through it.
- `egui-phosphor` 0.14 depends on egui 0.36.2, so a second egui and epaint compile next to 0.37. The spike uses only its font bytes and codepoint constants and registers the font with 0.37 types. The app should embed the Phosphor font and constants directly, or wait for a release built on its egui version.
- egui rasterises fonts itself (skrifa and vello_cpu), with hinting and subpixel positioning but grayscale antialiasing, without LCD subpixel rendering. `system_font_fallback` (on by default) loads system fonts for glyphs the bundled fonts lack. The spike turns it off so the screenshots depend on the bundled fonts only.

### Windows and macOS

Not run. From the eframe, egui-winit and egui-wgpu 0.37 sources:

- Rendering: wgpu takes `Backends::PRIMARY | GL` unless `WGPU_BACKEND` says otherwise, which is DX12 or Vulkan on Windows, Metal on macOS and Vulkan on Linux, with GL as a fallback. glow goes through glutin: WGL or EGL on Windows and OpenGL on macOS. Apple deprecated OpenGL in macOS 10.14.
- Fonts: text looks the same on every OS, because egui does not use the system text renderer. The system font fallback reads each OS's font folders, so glyphs outside the bundled fonts (CJK, emoji) can differ.
- High DPI: `pixels_per_point` comes from winit's `scale_factor()`, per monitor, and follows `ScaleFactorChanged` when the window moves to another monitor. On Windows that is the per-monitor DPI, and on macOS the backing scale factor (2 on Retina).
- Platform code in eframe: on Windows, a repaint path that avoids flicker while resizing, a cloaked maximize for hidden windows, IME key filtering, and the Shift+Delete, Ctrl+Insert and Shift+Insert clipboard keys. On macOS, a scroll momentum workaround, focus handling when the window moves, and placement of the traffic light buttons.
- Accessibility: AccessKit uses UI Automation on Windows, NSAccessibility on macOS and AT-SPI over D-Bus on Linux. The idle wakeup described under criterion 6 comes from the Linux adapter.

Result: met for Linux. Windows and macOS are covered from the docs and sources only.

## Criterion 6: 0% CPU when idle

`scripts/idle.sh` starts a view, waits 5 s, then counts for 60 s the CPU ticks (`utime + stime`, 100 per second) and the context switches of all threads from `/proc`, and the frames the app painted (`S4_LOG_FRAMES=1` prints one line per frame).

| Display | Backend | View | CPU ticks in 60 s | Frames | Context switches |
|---|---|---|---|---|---|
| Xvfb | glow | feed, 10 000 dumps | 0 | 0 | 120 |
| Xvfb | glow | debugger | 0 | 0 | 120 |
| Xvfb | glow | tree, 100 000 nodes | 1 | 0 | 120 |
| Xvfb | glow | flame | 1 | 0 | 120 |
| Xvfb | wgpu | feed, 10 000 dumps | 1 | 0 | 121 |
| Xvfb | wgpu | debugger | 2 | 0 | 120 |
| Xvfb | wgpu | tree, 100 000 nodes | 1 | 0 | 121 |
| Xvfb | wgpu | flame | 2 | 0 | 120 |
| host | glow | debugger | 0 | 0 | 120 |
| host | wgpu | debugger | 1 | 0 | 120 |
| host | wgpu | feed, 10 000 dumps | 2 | 0 | 120 |

At most 2 ticks in 60 s is 0.033%, and no run painted a frame. egui repaints only on input or on an explicit request.

The context switches come from one thread. `scripts/threads.sh` names it: `blocking-1`, with 41 switches in 20 s, about twice per second, and no CPU ticks. It is the thread pool of the `blocking` crate, which zbus uses for the AT-SPI connection. `scripts/noak.sh` builds the same app with eframe's `accesskit` feature off: `accesskit_unix` drops out of the build, and in 20 s idle no thread of the 71 switched context once (`results/threads-noak-xvfb.txt`). The wakeups come from the Linux AccessKit adapter, and they happen with no screen reader running.

A live element costs a full repaint. `--live-countdown` adds a countdown that repaints once per second:

| Display | Backend | Frames | CPU |
|---|---|---|---|
| host | wgpu | 30 in 30 s | 0.333% |
| Xvfb | wgpu | 60 in 60 s | 9.333% |

egui has no partial redraw, so every repaint lays out and draws the whole window. On llvmpipe that is about 93 ms of CPU per frame.

Result: met. The window does not repaint when idle.

## Criterion 7: RSS per dataset on glow and wgpu

`scripts/memory.sh` loads each dataset, renders 120 frames and prints the live heap and the RSS. Heap is in MB from the counting allocator, RSS in MB from `/proc`.

| Dataset | Host glow heap / RSS | Host wgpu heap / RSS | Xvfb glow RSS | Xvfb wgpu RSS |
|---|---|---|---|---|
| Feed, 40 dumps | 5.4 / 98.4 | 4.8 / 111.2 | 128.3 | 148.2 |
| Feed, 10 000 dumps | 14.8 / 114.2 | 14.1 / 120.2 | 139.7 | 159.5 |
| Debugger mockup | 5.5 / 101.1 | 4.8 / 108.9 | 128.5 | 148.8 |
| Tree, 951 nodes, lazy | 5.0 / 96.3 | 4.4 / 108.2 | 125.5 | 145.6 |
| Tree, 99 951 nodes, lazy | 5.0 / 99.9 | 4.4 / 101.8 | 126.0 | 145.6 |
| Tree, 951 nodes, all expanded | 5.3 / 100.5 | 4.7 / 103.8 | 125.9 | 146.4 |
| Tree, 99 951 nodes, all expanded | 17.7 / 116.4 | 17.0 / 119.5 | 143.9 | 163.8 |
| Flame, 50 000 frames | 7.6 / 99.9 | 6.9 / 107.8 | 130.5 | 152.5 |
| Source, 4 987 lines | 6.3 / 109.7 | 5.6 / 109.6 | 129.8 | 153.9 |
| All datasets at once | 17.9 / 124.9 | 17.3 / 127.2 | 147.0 | 174.0 |

The draft budget is RSS at most 200 MB with the large datasets ([docs/performance.md](../../../docs/performance.md#budgets)). On the host GPU the worst case is 127.2 MB. Under Xvfb it is 174.0 MB, because llvmpipe and lavapipe sit in the process.

Most of the RSS belongs to the renderer and the drivers: the heap with everything loaded is under 18 MB. RSS moves by a few MB between runs (the lazy trees on wgpu: 108.2 MB for the small one, 101.8 MB for the large one), so heap is the better measure for the views.

Startup to the first frame on the host: glow 43 to 60 ms, wgpu 60 to 119 ms.

### Backend recommendation

wgpu. On Linux glow is lighter: up to 13 MB less RSS on the host (2 MB with everything loaded), 30% to 50% less CPU per frame at p50, and a faster start. Both backends stay far inside the budgets, and the difference does not change any result. wgpu is eframe's default renderer and uses the native API on each OS (Metal on macOS, DX12 or Vulkan on Windows). glow on macOS depends on OpenGL, which Apple deprecated. wgpu also carries its own GL backend, so one renderer covers the machines without Vulkan.

The macOS and Windows numbers can change this, and they need to be measured before the app milestone.

## Criterion 8: tree memory at 1 000 and 100 000 nodes

The view holds only the visible rows: the live heap is 5.0 MB with the lazy tree at 951 nodes and at 99 951 nodes (glow, host). With the root expanded and 2 000 rows scrolled through (the `tree-scroll` run), the heap is 5.5 MB.

Expanding every node of the large tree raises the heap to 17.7 MB. The extra 12.4 MB is node data, about 125 bytes per node in the spike's model, and the view still paints only the rows on screen. In the app the daemon holds the variables, and the tree holds only the nodes fetched so far.

Result: met.

## Criterion 9: styled mockups of the feed and the debugger

The mockups follow the layouts in [docs/ui.md](../../../docs/ui.md), with Inter, JetBrains Mono and Phosphor, an icon and a label on main actions, icon-only buttons in the stepping toolbar, and tooltips that give the action name, its shortcut, what it does and, for a disabled button, why it is disabled. Actions come from one registry (`src/actions.rs`) that the buttons and the tooltips both read.

- [feed-light.png](s4-egui/feed-light.png), [feed-dark.png](s4-egui/feed-dark.png)
- [debugger-light.png](s4-egui/debugger-light.png), [debugger-dark.png](s4-egui/debugger-dark.png)
- [tooltip-step-over-dark.png](s4-egui/tooltip-step-over-dark.png), [tooltip-run-to-cursor-light.png](s4-egui/tooltip-run-to-cursor-light.png)

All screenshots are 1280x800 under Xvfb with wgpu. Whether they look finished enough is for Lucas to judge.

## Criterion 10: iced or Slint

No view failed, so neither was tried.

## Not verified

- Windows and macOS: rendering, fonts, high DPI and AccessKit there come from the sources, not from runs.
- High DPI: the host display's scale factor was not recorded, and no run set a scale above 1 on purpose. Text scaling with the system setting was not tried either.
- Wayland: every run used X11.
- A real screen reader: the AT-SPI tree was read by a script, not by Orca.
- Following the system theme: the spike picks the theme with a flag.
- Smoothness as seen by a person: the scroll and zoom results rest on frame times, not on someone watching the host screen.
- Highlighting cost for files much larger than 5 000 lines.

## Spec changes

ADRs:

- [ADR-0001](../../decisions/0001-rust-native-ui.md): the toolkit can be chosen. Record egui through eframe 0.37, with the wgpu renderer and AccessKit on.
- A new ADR, or a section of ADR-0001, for the rules S4 relied on: every large view is virtualised (fixed or prefix-summed row heights, only visible rows built), trees fetch children on expand, and nothing repaints without input or new data.

[OPEN.md](../../OPEN.md), O-08:

- Desktop app, window open and nothing changing: CPU 0% holds (at most 2 ticks in 60 s, no frames). Decide whether the app gets the daemon's rule of no wakeups faster than once per second. With AccessKit on, Linux breaks it at about two per second, and turning AccessKit off is not an option under docs/ui.md principle 7. Whether the adapter can start only when an assistive technology asks for it is not known yet.
- Desktop app with the large S4 datasets: 200 MB holds, with 127 MB measured on the host GPU and 174 MB on software rendering. 150 MB would also hold on a GPU. Confirm on Windows and macOS before lowering it.
- Add a frame time budget for the large views: for example p99 under 8 ms of CPU on a mid-range laptop GPU, where this spike measured at most 4.2 ms.

[docs/ui.md](../../../docs/ui.md), visual system:

- Icons: Phosphor, embedded directly rather than through `egui-phosphor` 0.14, which pulls in a second egui.
- Every icon next to text sits in its own font section, because Inter's private use glyphs collide with Phosphor's codepoints.
- Live elements (countdowns, elapsed time, a spinner) repaint at most once per second and only while they are visible. Each one costs a full window repaint.
- Decide whether to keep `system_font_fallback` on, which makes glyphs outside Inter and JetBrains Mono come from the system.
- Tree rows copy through their context menu. Text in dumps, source and tables copies by selection.
