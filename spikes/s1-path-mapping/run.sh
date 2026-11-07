#!/bin/sh
# Runs one spike listener in the toolchain container, waits until it
# listens, fires the request(s) and waits for the listener to finish.
#
#   ./run.sh OUT 'LISTENER ARGS' 'REQUEST' ['REQUEST' ...]
#
# OUT is a file under results/. A REQUEST is a shell command run on the
# host; `dc` is docker compose for this test bed copy plus the override,
# so `dc exec -T php85 php ...` runs PHP in the container. Request output
# is appended to OUT with a "[request]" prefix. A request that must keep
# running while the next one starts redirects its own output and ends
# with "&".
set -eu

here=$(cd "$(dirname "$0")" && pwd)
out="$here/results/$1"
listener=$2
shift 2

export COMPOSE_PROJECT_NAME="${COMPOSE_PROJECT_NAME:-testbed-s1}"
export HTTP_PORT_85="${HTTP_PORT_85:-8180}" HTTP_PORT_83="${HTTP_PORT_83:-8181}"
compose="docker compose -f $here/../testbed/compose.yaml -f $here/compose.override.yaml"
dc() { $compose "$@"; }

mkdir -p "$here/results"
{
    echo "# listener: s1-path-mapping $listener"
    for request in "$@"; do echo "# request: $request"; done
} >"$out"
# shellcheck disable=SC2086
$compose exec -T toolchain /target/debug/s1-path-mapping $listener >>"$out" 2>&1 &
pid=$!
i=0
until grep -q '^listening' "$out" 2>/dev/null; do
    i=$((i + 1))
    [ "$i" -gt 100 ] && { echo "listener did not start" >&2; cat "$out" >&2; exit 1; }
    sleep 0.1
done
for request in "$@"; do
    { eval "$request" 2>&1 || true; } | sed 's/^/  [request] /' >>"$out"
done
status=0
wait "$pid" || status=$?
echo "# exit status $status" >>"$out"
cat "$out"
