# Spike S4: egui/eframe

Throwaway code for spike S4 (T-19 to T-23 in
[`specs/00-spikes/tasks.md`](../../specs/00-spikes/tasks.md)). The
results are in [`specs/00-spikes/reports/s4-egui.md`](../../specs/00-spikes/reports/s4-egui.md).
Production code never imports anything from here.

One binary, `s4-egui`, with both eframe renderers (`--backend glow` or
`--backend wgpu`). It has:

- the Feed and Debugger mockups from `docs/ui.md`, in light and dark;
- a lazy variable tree (`--view tree`, 100 000 nodes by default);
- a virtualised list of mixed dumps (`--view feed`, 10 000 by default);
- a flame graph (`--view flame`, 50 000 frames);
- a PHP source view with tree-sitter highlighting and Ctrl+G
  (`--view source`, about 5 000 lines);
- a benchmark mode that injects input and prints frame time percentiles
  (`--bench tree-scroll|tree-expand|dumps-scroll|flame-zoom|source-scroll`);
- a headless AccessKit dump (`--a11y-dump`).

`s4-egui --help` lists every flag.

## Running it

Everything runs in Docker. Nothing is installed on the host.

```sh
cd spikes/s4-egui
docker compose up -d --build                      # ui: Xvfb at :99, software rendering
docker compose exec -T ui cargo build --release   # binary in the target volume
docker compose exec -T ui sh scripts/screenshots.sh
```

The `ui` service has its own virtual display, so nothing opens on the
desktop. Mesa renders on the CPU there (llvmpipe for GL, lavapipe for
Vulkan).

The `ui-host` service (profile `host`) shares the host X11 display and
`/dev/dri` for the GPU numbers. It needs `xhost +SI:localuser:$USER` on
the host, and `RENDER_GID` and `VIDEO_GID` when they differ from 110 and
44.

```sh
docker compose --profile host up -d ui-host
docker compose exec -T ui-host sh scripts/host.sh
docker compose --profile host down                # keeps the volumes
```

## Scripts

| Script | What it writes |
|---|---|
| `scripts/screenshots.sh [DIR]` | PNGs of both mockups in both themes, two tooltips and the stress views |
| `scripts/idle.sh BACKEND SECONDS ARGS` | CPU time and context switches of an idle window, from `/proc` |
| `scripts/idle-all.sh` | the idle runs in the report |
| `scripts/memory.sh` | heap (counting allocator) and RSS per dataset and backend |
| `scripts/bench.sh` | frame times per scenario and backend, vsync off |
| `scripts/select.sh` | text selection and Ctrl+C with xdotool, clipboard read with xclip |
| `scripts/atspi.sh VIEW` | the AT-SPI tree a screen reader sees |
| `scripts/goto.sh [LINE]` | Ctrl+G in the source view, then a screenshot |
| `scripts/flame.sh` | Ctrl+wheel zoom and hover in the flame graph, then a screenshot |
| `scripts/threads.sh BACKEND SECONDS ARGS` | context switches per thread of an idle window |
| `scripts/noak.sh [SECONDS]` | the same, for a build without eframe's `accesskit` feature |
| `scripts/versions.sh` | Rust, crates, Mesa, GL and Vulkan devices, fonts |
| `scripts/host.sh` | versions, memory, benchmarks and idle on the host GPU |

Raw outputs are in `results/`.

## Fonts and icons

The Dockerfile downloads Inter 4.1 and JetBrains Mono 2.304 from their
GitHub releases into `/opt/fonts`, with the SHA-256 of each archive in
`/opt/fonts/SOURCES.txt`. Both are under the SIL Open Font License 1.1.
The app reads them from `S4_FONT_DIR`. They are not committed.

Icons are Phosphor through the `egui-phosphor` 0.14 crate (MIT OR
Apache-2.0), which embeds the regular weight font (477 KB). The Phosphor
icon set itself is MIT; the crate does not ship a separate license file
for the font. Inter has glyphs in the private use area where Phosphor puts
its icons, so icons next to text go through `theme::icon_job`, which
gives the icon its own font section.
