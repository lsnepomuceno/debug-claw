#!/bin/sh
# Runs one S6 scenario against the IDE emulator and writes everything to
# results/NAME.txt, plus the emulator transcript in results/NAME.transcript.
# Runs on the host; needs only docker and curl.
#
#   ./run.sh MODE NAME [REQUESTS]
#
# MODE is how the emulator is reached:
#   direct   the emulator listens on toolchain:9003, Xdebug connects to it
#   loop     the proxy on toolchain:9003 relays to the emulator on
#            127.0.0.1:9010 in the same container
#   bridge   the proxy on toolchain:9003 relays to the emulator in the
#            ide container (ide:9010), across the Compose bridge
# REQUESTS triggered requests are fired one after the other (default 1).
# Environment:
#   URL         request URL (default http://127.0.0.1:8680/loop?n=3)
#   EMU_ARGS    extra ide_emu arguments (--hold-ms, --bench)
#   AGENT_ARGS  if set, runs the agent in toolchain with these arguments
#   PROXY_LOG   if set, the proxy logs every packet to results/PROXY_LOG
set -eu
here=$(cd "$(dirname "$0")" && pwd)
export COMPOSE_PROJECT_NAME=testbed-s6 HTTP_PORT_85=8680 HTTP_PORT_83=8681
dc() { docker compose -f "$here/../testbed/compose.yaml" -f "$here/compose.override.yaml" "$@"; }

mode=$1 name=$2 reqs=${3:-1}
url=${URL:-http://127.0.0.1:8680/loop?n=3}
bin=/target/release
res=/work/spikes/s6-ide-proxy/results
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$here/results"

wait_for() {
    i=0
    until grep -q "$2" "$1" 2>/dev/null; do
        i=$((i + 1))
        [ "$i" -gt 100 ] && { echo "$1 did not start" >&2; cat "$1" >&2; exit 1; }
        sleep 0.1
    done
}

emu="$bin/ide_emu --sessions $reqs --transcript $res/$name.transcript ${EMU_ARGS:-}"
proxy="$bin/s6-ide-proxy --sessions $reqs ${PROXY_LOG:+--log $res/$PROXY_LOG}"
pids=""
case "$mode" in
    direct)
        # shellcheck disable=SC2086
        dc exec -T toolchain $emu --listen 0.0.0.0:9003 >"$tmp/emu" 2>&1 &
        pids="$pids $!"
        wait_for "$tmp/emu" listening
        ;;
    loop | bridge)
        if [ "$mode" = loop ]; then svc=toolchain ide=127.0.0.1:9010; else svc=ide ide=ide:9010; fi
        # shellcheck disable=SC2086
        dc exec -T "$svc" $emu --listen 0.0.0.0:9010 >"$tmp/emu" 2>&1 &
        pids="$pids $!"
        wait_for "$tmp/emu" listening
        # shellcheck disable=SC2086
        dc exec -T toolchain $proxy --ide "$ide" >"$tmp/proxy" 2>&1 &
        pids="$pids $!"
        wait_for "$tmp/proxy" listening
        ;;
    *) echo "unknown mode $mode" >&2; exit 2 ;;
esac
if [ -n "${AGENT_ARGS:-}" ]; then
    # shellcheck disable=SC2086
    dc exec -T toolchain $bin/agent $AGENT_ARGS >"$tmp/agent" 2>&1 &
    pids="$pids $!"
    wait_for "$tmp/agent" connected
fi

# One untriggered request first, so OPcache is warm in every run: Xdebug
# reports a breakpoint as resolved only for a file it sees compiled.
curl -s -o /dev/null "$url"
n=0
while [ "$n" -lt "$reqs" ]; do
    n=$((n + 1))
    curl -s -o "$tmp/body" -w "%{http_code} %{time_total}s" -b XDEBUG_TRIGGER=1 "$url" >"$tmp/curl.$n"
    echo " $(tr -d '\n ' <"$tmp/body")" >>"$tmp/curl.$n"
done
status=0
for p in $pids; do wait "$p" || status=1; done

{
    echo "# $(date -u +%FT%TZ) mode=$mode requests=$reqs url=$url"
    echo "# EMU_ARGS=${EMU_ARGS:-} AGENT_ARGS=${AGENT_ARGS:-} PROXY_LOG=${PROXY_LOG:-}"
    for f in emu proxy agent; do
        [ -f "$tmp/$f" ] || continue
        echo "## $f"
        cat "$tmp/$f"
    done
    echo "## requests"
    n=0
    while [ "$n" -lt "$reqs" ]; do n=$((n + 1)); echo "request $n: $(cat "$tmp/curl.$n")"; done
    echo "## exit status $status"
} >"$here/results/$name.txt"
cat "$here/results/$name.txt"
exit "$status"
