#!/bin/sh
# The idle runs in the report: each mockup and stress view on both
# backends, 60 s each, plus the debugger with its session countdown on
# (one repaint per second, as docs/ui.md asks for the timer).
#   sh scripts/idle-all.sh [BACKENDS] [SECONDS]
set -eu
backends=${1:-"glow wgpu"}
secs=${2:-60}
for b in $backends; do
    sh scripts/idle.sh "$b" "$secs" --view feed --dumps 10000
    sh scripts/idle.sh "$b" "$secs" --view debugger
    sh scripts/idle.sh "$b" "$secs" --view tree --tree-nodes 100000 --expand-sample 3
    sh scripts/idle.sh "$b" "$secs" --view flame
done
sh scripts/idle.sh wgpu "$secs" --view debugger --live-countdown
