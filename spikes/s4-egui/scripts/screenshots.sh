#!/bin/sh
# Screenshots of the mockups under Xvfb (software rendering, no GPU).
# Run inside the `ui` service:
#   docker compose exec -T ui sh scripts/screenshots.sh [OUT_DIR]
set -eu
BIN=/target/release/s4-egui
OUT=${1:-/work/specs/00-spikes/reports/s4-egui}
mkdir -p "$OUT"

shot() {
    name=$1
    shift
    timeout 60 "$BIN" --backend wgpu --size 1280x800 --screenshot "$OUT/$name.png" "$@"
}

for theme in light dark; do
    shot "feed-$theme" --view feed --dumps 40 --theme "$theme"
    shot "debugger-$theme" --view debugger --theme "$theme"
done
# Tooltips: one enabled button and one disabled button.
shot tooltip-step-over-dark --view debugger --theme dark --hover step-over
shot tooltip-run-to-cursor-light --view debugger --theme light --hover run-to-cursor
# The stress views, for reference.
shot flame-dark --view flame --theme dark
shot tree-light --view tree --theme light --expand-sample 3
shot source-dark --view source --theme dark
ls -l "$OUT"
