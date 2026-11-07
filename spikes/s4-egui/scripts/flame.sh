#!/bin/sh
# Zoom the 50 000-frame flame graph with Ctrl+wheel and hover a frame,
# with xdotool under Xvfb. The app takes its screenshot after the input.
#   sh scripts/flame.sh [OUT]
set -eu
out=${1:-/work/specs/00-spikes/reports/s4-egui/flame-zoom-dark.png}
/target/release/s4-egui --backend wgpu --view flame --theme dark \
    --screenshot "$out" --screenshot-delay 6 &
pid=$!
sleep 2
wid=$(xdotool search --name "debug-claw S4 spike" | head -1)
# No window manager under Xvfb: focus the window and send input through
# XTEST, because synthetic events sent with --window are ignored.
xdotool windowfocus "$wid"
xdotool mousemove --window "$wid" 400 150 sleep 0.3 keydown ctrl
for _ in 1 2 3 4 5 6 7 8 9 10 11 12; do
    xdotool click 4 sleep 0.08
done
xdotool keyup ctrl sleep 0.5 mousemove --window "$wid" 420 190 sleep 0.2 mousemove --window "$wid" 421 191
wait "$pid"
