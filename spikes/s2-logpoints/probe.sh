#!/bin/sh
# Runs the scripted DBGp probe (src/bin/probe.rs) for one request.
#
#   ./probe.sh FILE:LINE SCRIPT TRIGGER [probe args...]
#
# FILE:LINE is the remote breakpoint, SCRIPT a file under scripts/, and
# TRIGGER the request that starts the session:
#   url:/graph?depth=6&width=3    triggered GET to the php85 site
#   post:/orders:customer_id=3    triggered POST with one form field
#   cli:/probe/visibility.php     PHP CLI inside php85, as www-data
# The test bed must be up with compose.override.yaml (for /probe).
# Runs on the host; needs only docker and curl.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
export COMPOSE_PROJECT_NAME=testbed-s2 HTTP_PORT_85=8280 HTTP_PORT_83=8281
dc() { docker compose -f "$here/../testbed/compose.yaml" -f "$here/compose.override.yaml" "$@"; }

brk=$1 script=$2 trigger=$3
shift 3
out=$(mktemp)
trap 'rm -f "$out"' EXIT

dc exec -T toolchain /target/release/probe --break "$brk" \
  --script "/work/spikes/s2-logpoints/scripts/$script" "$@" > "$out" 2>&1 &
i=0
until grep -q '^listening' "$out" 2>/dev/null; do
  i=$((i + 1)); [ $i -gt 100 ] && { cat "$out"; exit 1; }
  sleep 0.1
done

case "$trigger" in
  url:*) body=$(curl -s -b XDEBUG_TRIGGER=1 -w '\nhttp=%{http_code} time_s=%{time_total}' "http://127.0.0.1:8280${trigger#url:}") ;;
  post:*)
    rest=${trigger#post:}
    body=$(curl -s -b XDEBUG_TRIGGER=1 -d "${rest#*:}" -w '\nhttp=%{http_code} time_s=%{time_total}' "http://127.0.0.1:8280${rest%%:*}") ;;
  cli:*) body=$(dc exec -T -u www-data -e XDEBUG_TRIGGER=1 php85 php "${trigger#cli:}") ;;
  *) echo "unknown trigger $trigger" >&2; exit 2 ;;
esac
wait

echo "trigger $trigger break $brk script $script args $*"
cat "$out"
echo "request: $(printf '%s\n' "$body" | tail -n 1)"
