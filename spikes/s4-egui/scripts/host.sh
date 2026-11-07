#!/bin/sh
# The hardware numbers, run in the ui-host service (host X11 display and
# /dev/dri). Every window closes by itself: memory runs draw 120 frames,
# benchmarks 600, idle runs last SECONDS.
#   docker compose --profile host up -d ui-host
#   docker compose exec -T ui-host sh scripts/host.sh [SECONDS]
set -eu
secs=${1:-60}
out=results
sh scripts/versions.sh >"$out/versions-host.txt" 2>&1
sh scripts/memory.sh >"$out/memory-host.txt" 2>&1
sh scripts/bench.sh >"$out/bench-host.txt" 2>&1
{
    sh scripts/idle.sh glow "$secs" --view debugger
    sh scripts/idle.sh wgpu "$secs" --view debugger
    sh scripts/idle.sh wgpu "$secs" --view feed --dumps 10000
} >"$out/idle-host.txt" 2>&1
