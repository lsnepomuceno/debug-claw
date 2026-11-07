#!/bin/sh
# Calls the daemon's `wait` tool from the MCP Inspector CLI, one case at a
# time, and records the result and the wall time in results/inspector-*.txt.
#
#   ./inspector.sh CASE
#
# The daemon must be up (./dc.sh up -d daemon). Cases are listed below.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$here/results"
url=http://daemon:8390/mcp

insp() { "$here/dc.sh" --profile clients run --rm -T -e MCP_INSPECTOR_SECRET_STORE=memory -v "$here/inspector:/cfg:ro" inspector --cli "$@"; }
call() { insp "$url" --transport http --method tools/call --tool-name wait "$@"; }

case "$1" in
    # A wait that times out on its own just under the 60 s client default.
    timeout-59) run() { call --tool-arg timeout_s=59; } ;;
    # A wait ended by an event fired by the HTTP trigger after 45 s.
    event-45) run() {
        curl -s -X POST 'http://127.0.0.1:8390/fire?delay_s=45'
        call --tool-arg timeout_s=60
    } ;;
    # Longer than the client default, no progress: expect a client timeout.
    timeout-90) run() { call --tool-arg timeout_s=90; } ;;
    # Same, with a progress token and progress every 20 s.
    progress-90) run() {
        call --tool-arg timeout_s=90 progress_every_s=20 --tool-metadata progressToken=p90
    } ;;
    # Same, with logging notifications instead of progress.
    log-90) run() { call --tool-arg timeout_s=90 log_every_s=20; } ;;
    # The request timeout raised to 120 s in a config file, no progress.
    config-90) run() {
        insp --config /cfg/long-timeout.json --server s3 --method tools/call \
            --tool-name wait --tool-arg timeout_s=90
    } ;;
    # Config file check: a 5 s request timeout must cut an 8 s wait.
    config-5) run() {
        insp --config /cfg/short-timeout.json --server s3 --method tools/call \
            --tool-name wait --tool-arg timeout_s=8
    } ;;
    *) echo "unknown case $1" >&2; exit 2 ;;
esac

out="$here/results/inspector-$1.txt"
start=$(date +%s.%N)
status=0
{ echo "# case $1, $(date -u +%FT%TZ)"; run 2>&1 | grep -v ' Container ' || status=$?; } >"$out"
end=$(date +%s.%N)
printf '# wall %.1f s, exit %s\n' "$(echo "$end - $start" | bc)" "$status" >>"$out"
cat "$out"
