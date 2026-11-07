#!/bin/sh
# Idle CPU with no input, read from /proc.
#   sh scripts/idle.sh BACKEND SECONDS [app args...]
# Starts the app, waits 5 s for the first frames, then reads utime+stime
# of the process and the context switches of all its threads at the
# start and the end of the window. It also counts the frames drawn in
# the window: with S4_LOG_FRAMES set the app prints one line per frame.
set -eu
BIN=/target/release/s4-egui
backend=$1
secs=$2
shift 2

S4_LOG_FRAMES=1 "$BIN" --backend "$backend" "$@" >/tmp/idle-app.log 2>/tmp/idle-frames.log &
pid=$!
sleep 5

ticks() { awk '{print $14 + $15}' "/proc/$pid/stat"; }
switches() { cat /proc/"$pid"/task/*/status | awk '/ctxt_switches/ {s += $2} END {print s}'; }
rss() { awk '/VmRSS/ {print $2}' "/proc/$pid/status"; }

t0=$(ticks)
f0=$(/usr/bin/grep -c '^frame' /tmp/idle-frames.log || true)
c0=$(switches)
sleep "$secs"
t1=$(ticks)
f1=$(/usr/bin/grep -c '^frame' /tmp/idle-frames.log || true)
c1=$(switches)
r=$(rss)
hz=$(getconf CLK_TCK)
kill "$pid"
wait "$pid" 2>/dev/null || true

awk -v t="$((t1 - t0))" -v hz="$hz" -v s="$secs" -v c="$((c1 - c0))" -v r="$r" -v f="$((f1 - f0))" -v b="$backend" -v a="$*" \
    'BEGIN {printf "idle backend=%s args=[%s] seconds=%d cpu_ticks=%d cpu_pct=%.3f frames=%d ctx_switches=%d rss_mb=%.1f\n", b, a, s, t, 100 * t / hz / s, f, c, r / 1024}'
head -1 /tmp/idle-app.log
