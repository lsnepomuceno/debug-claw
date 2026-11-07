#!/bin/sh
# Frame times under injected input, vsync off so the numbers show the
# cost of a frame and not the display rate.
#   sh scripts/bench.sh [BACKENDS] [FRAMES]
set -eu
BIN=/target/release/s4-egui
backends=${1:-"glow wgpu"}
frames=${2:-600}

for b in $backends; do
    for s in tree-scroll tree-expand dumps-scroll flame-zoom source-scroll; do
        timeout 300 "$BIN" --backend "$b" --vsync off --bench "$s" --frames "$frames" | /usr/bin/grep -E "^(bench|heap)"
    done
done
