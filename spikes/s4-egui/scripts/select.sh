#!/bin/sh
# Text selection and copy under Xvfb: drag across text with xdotool,
# press Ctrl+C, read the clipboard with xclip. Coordinates are for the
# default 1280x800 window (see the screenshots).
#   sh scripts/select.sh [BACKEND]
set -eu
BIN=/target/release/s4-egui
backend=${1:-wgpu}

open() {
    "$BIN" --backend "$backend" --size 1280x800 "$@" >/tmp/select-app.log 2>&1 &
    pid=$!
    sleep 4
    wid=$(xdotool search --name "debug-claw S4 spike" | head -1)
    # No window manager under Xvfb: give the window the keyboard focus,
    # and send keys through XTEST (synthetic key events are ignored).
    xdotool windowfocus "$wid"
}
close() {
    kill "$pid"
    wait "$pid" 2>/dev/null || true
}
drag() {
    xdotool mousemove --window "$wid" "$1" "$2" sleep 0.2 mousedown 1 sleep 0.1 \
        mousemove --window "$wid" "$(( ($1 + $3) / 2 ))" "$(( ($2 + $4) / 2 ))" sleep 0.1 \
        mousemove --window "$wid" "$3" "$4" sleep 0.2 mouseup 1 sleep 0.2 \
        key ctrl+c sleep 0.5
}
clip() {
    printf '%s: [' "$1"
    xclip -o -selection clipboard 2>/dev/null | head -c 300
    printf ']\n'
}

open --view debugger
drag 530 252 705 252
clip "debugger, one source line"
drag 530 233 790 271
clip "debugger, source lines 40 to 42"
drag 200 492 420 492
clip "debugger, a variable tree row (rows are painted click targets, so the clipboard keeps the previous text)"
xdotool mousemove --window "$wid" 250 558 sleep 0.2 click 3 sleep 0.4
xdotool mousemove --window "$wid" 290 573 sleep 0.2 click 1 sleep 0.5
clip "debugger, tree context menu, first item"
close

open --view feed --dumps 40
drag 400 212 660 212
clip "feed, query text"
drag 400 323 900 361
clip "feed, table rows"
close
