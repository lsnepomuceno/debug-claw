#!/bin/sh
# Runs one S2 scenario: starts the listener, fires REPS triggered requests
# one after the other, prints each request's time and the listener output.
#
#   ./bench.sh MODE REPS N [listener args...]
#
# MODE is where the listener runs:
#   bridge     toolchain container, Xdebug connects across the bridge
#   local      a container in php85's network namespace (loopback)
#   untrig     no listener and no trigger cookie (no debug session)
# Set the matching ini first with ./profile.sh. Set PARALLEL=1 to fire the
# REPS requests at the same time instead of one after the other.
# Runs on the host; needs only docker and curl.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
export COMPOSE_PROJECT_NAME=testbed-s2 HTTP_PORT_85=8280 HTTP_PORT_83=8281
dc() { docker compose -f "$here/../testbed/compose.yaml" "$@"; }

mode=$1 reps=$2 n=$3
shift 3
out=$(mktemp)
times=$(mktemp)
trap 'rm -f "$out" "$times"' EXIT
url="http://127.0.0.1:8280/loop?n=$n"
want_sum=$(( (n - 1) * n * (2 * n - 1) / 6 ))
bin=/target/release/s2-logpoints

case "$mode" in
  bridge)
    dc exec -T ${S2_TRACE:+-e S2_TRACE=1} toolchain "$bin" --sessions "$reps" "$@" > "$out" 2>&1 &
    cookie="-b XDEBUG_TRIGGER=1" ;;
  local)
    docker run --rm --network container:testbed-s2-php85-1 \
      --user "${HOST_UID:-1000}:${HOST_GID:-1000}" \
      -v testbed-s2_target:/target:ro ${S2_TRACE:+-e S2_TRACE=1} testbed-s2-toolchain \
      "$bin" --listen 127.0.0.1:9003 --sessions "$reps" "$@" > "$out" 2>&1 &
    cookie="-b XDEBUG_TRIGGER=1" ;;
  untrig) cookie="" ;;
  *) echo "unknown mode $mode" >&2; exit 2 ;;
esac

if [ "$mode" != untrig ]; then
  i=0
  until grep -q '^listening' "$out" 2>/dev/null; do
    i=$((i + 1)); [ $i -gt 100 ] && { cat "$out"; exit 1; }
    sleep 0.1
  done
fi

fire() {
  body=$(curl -s $cookie -w '\n%{http_code} %{time_total}' "$url")
  code_time=$(printf '%s\n' "$body" | tail -n 1)
  sum_ok=no
  printf '%s\n' "$body" | grep -q "\"sum\": $want_sum\$" && sum_ok=yes
  echo "$code_time body_sum_ok=$sum_ok" >> "$times"
}

if [ "${PARALLEL:-0}" = 1 ]; then
  for _ in $(seq "$reps"); do fire & done
else
  for _ in $(seq "$reps"); do fire; done
fi
wait

echo "scenario mode=$mode reps=$reps n=$n args=$*"
awk '{print "request http=" $1 " time_s=" $2 " " $3}' "$times"
median=$(awk '{print $2}' "$times" | sort -n | awk '{a[NR]=$1} END {print a[int((NR+1)/2)]}')
echo "median_s=$median"
cat "$out"
