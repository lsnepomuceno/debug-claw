#!/bin/sh
# Jump to a line in the 5 000-line source view through the keyboard:
# Ctrl+G, type the line, Enter, sent with xdotool under Xvfb. The app
# takes its screenshot after the keys.
#   sh scripts/goto.sh [LINE] [OUT]
set -eu
line=${1:-4200}
out=${2:-/work/specs/00-spikes/reports/s4-egui/source-goto-light.png}
/target/release/s4-egui --backend wgpu --view source --theme light \
    --screenshot "$out" --screenshot-delay 5 &
pid=$!
sleep 2
wid=$(xdotool search --name "debug-claw S4 spike" | head -1)
# No window manager under Xvfb: focus the window and send input through
# XTEST, because synthetic events sent with --window are ignored.
xdotool windowfocus "$wid"
xdotool mousemove --window "$wid" 700 400 sleep 0.2 key ctrl+g sleep 0.4 \
    type --delay 80 "$line"
xdotool sleep 0.3 key Return
wait "$pid"
