#!/bin/sh
# T-10: concurrent triggered requests, each with its own DBGp session.
#   ./t10.sh
# 1. Two n=10000 requests fired at the same time, against one sequential
#    n=10000 request as the reference.
# 2. A long request (n=10000) and a short one (n=100) started 100 ms
#    later: the short one must finish while the long one is still paused.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
export COMPOSE_PROJECT_NAME=testbed-s2 HTTP_PORT_85=8280 HTTP_PORT_83=8281
dc() { docker compose -f "$here/../testbed/compose.yaml" "$@"; }
log="$here/results/t10.txt"
"$here/profile.sh" bridge > "$log"
dc exec -T php85 sh -c 'grep -E "^pm" /usr/local/etc/php-fpm.d/www.conf' >> "$log"

"$here/bench.sh" bridge 3 10000 --exprs 5 --quiet >> "$log"
PARALLEL=1 "$here/bench.sh" bridge 2 10000 --exprs 5 --quiet >> "$log"
PARALLEL=1 "$here/bench.sh" bridge 2 10000 --exprs 5 --quiet >> "$log"
PARALLEL=1 "$here/bench.sh" bridge 4 10000 --exprs 5 --quiet >> "$log"

# Long and short request; times are wall clock on the host, in ms.
out=$(mktemp)
trap 'rm -f "$out"' EXIT
dc exec -T toolchain /target/release/s2-logpoints --sessions 2 --exprs 5 --quiet > "$out" 2>&1 &
until grep -q '^listening' "$out"; do sleep 0.1; done
now() { date +%s%3N; }
t0=$(now)
( curl -s -o /dev/null -b XDEBUG_TRIGGER=1 -w 'long  http=%{http_code} time_s=%{time_total}' \
    'http://127.0.0.1:8280/loop?n=10000'; echo " end_ms=$(( $(now) - t0 ))" ) >> "$log" &
sleep 0.1
( curl -s -o /dev/null -b XDEBUG_TRIGGER=1 -w 'short http=%{http_code} time_s=%{time_total}' \
    'http://127.0.0.1:8280/loop?n=100'; echo " end_ms=$(( $(now) - t0 ))" ) >> "$log" &
wait
echo "scenario long-short" >> "$log"
cat "$out" >> "$log"
grep -E '^(scenario|request|median|long|short|session id=[0-9] peer|pm)' "$log" | cut -c1-230
