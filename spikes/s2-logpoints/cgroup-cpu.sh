#!/bin/sh
# Runs one bridge scenario and prints the toolchain container's cgroup CPU
# counters (cpu.stat, in microseconds) before and after it, as a second
# source for the listener's CPU time next to /proc/self/stat.
#   ./cgroup-cpu.sh N [listener args...]
set -eu
here=$(cd "$(dirname "$0")" && pwd)
export COMPOSE_PROJECT_NAME=testbed-s2 HTTP_PORT_85=8280 HTTP_PORT_83=8281
dc() { docker compose -f "$here/../testbed/compose.yaml" "$@"; }
n=$1
shift
before=$(dc exec -T toolchain cat /sys/fs/cgroup/cpu.stat | head -n 3)
"$here/bench.sh" bridge 1 "$n" "$@"
after=$(dc exec -T toolchain cat /sys/fs/cgroup/cpu.stat | head -n 3)
printf '%s\n' "$before" | sed 's/^/cgroup before /'
printf '%s\n' "$after" | sed 's/^/cgroup after /'
printf '%s\n%s\n' "$before" "$after" | awk '
  NR <= 3 { b[$1] = $2; next } { printf "cgroup delta %s_ms=%.1f\n", $1, ($2 - b[$1]) / 1000 }'
