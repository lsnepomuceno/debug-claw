#!/bin/sh
# S6 with a real PhpStorm, for the owner to run by hand. Runs on the host;
# needs only docker and curl. See "Pending: run with PhpStorm" in
# specs/00-spikes/reports/s6-ide-proxy.md for the PhpStorm settings and
# what to observe.
#
#   ./phpstorm-run.sh check         can the toolchain container reach PhpStorm?
#   ./phpstorm-run.sh proxy         PhpStorm through the proxy, no agent
#   ./phpstorm-run.sh proxy-agent   the same, plus the agent reading at each pause
#
# PhpStorm listens on the host on IDE_PORT (default 9010), never on 9003:
# Xdebug connects to toolchain:9003, where the proxy listens. Xdebug only
# knows toolchain:9003, so PhpStorm cannot be reached without the proxy in
# this testbed; "proxy" is the baseline that "proxy-agent" is compared to.
# The agent's checks assume URL /loop?n=3 and a breakpoint on line 44 of
# public/index.php.
#
# Environment:
#   IDE_PORT   PhpStorm's Xdebug port on the host (default 9010)
#   URL        request URL (default http://127.0.0.1:8680/loop?n=3)
#   PAUSES     pauses the agent reads in proxy-agent mode (default 4)
#
# Writes results/phpstorm-MODE-proxy.log (every packet the proxy relays),
# results/phpstorm-MODE-xdebug.log (Xdebug's own log, level 7) and
# results/phpstorm-MODE.txt (proxy, agent and request output).
set -eu
here=$(cd "$(dirname "$0")" && pwd)
export COMPOSE_PROJECT_NAME=testbed-s6 HTTP_PORT_85=8680 HTTP_PORT_83=8681
dc() { docker compose -f "$here/../testbed/compose.yaml" -f "$here/compose.override.yaml" "$@"; }

mode=${1:-}
port=${IDE_PORT:-9010}
url=${URL:-http://127.0.0.1:8680/loop?n=3}
bin=/target/release
res=/work/spikes/s6-ide-proxy/results
mkdir -p "$here/results"

case "$mode" in
    check)
        if dc exec -T toolchain timeout 3 bash -c "exec 3<>/dev/tcp/host.docker.internal/$port"; then
            echo "ok: toolchain reaches host.docker.internal:$port"
        else
            echo "FAILED: toolchain cannot reach host.docker.internal:$port." >&2
            echo "Is PhpStorm listening on $port, and does the host firewall accept the Docker bridge?" >&2
            exit 1
        fi
        exit 0
        ;;
    proxy | proxy-agent) ;;
    *) sed -n '2,24p' "$0"; exit 2 ;;
esac

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
name=phpstorm-$mode

dc exec -T -u root php85 sh -c ': > /var/log/xdebug/xdebug.log'
dc exec -T toolchain $bin/s6-ide-proxy --sessions 1 --ide "host.docker.internal:$port" \
    --log "$res/$name-proxy.log" >"$tmp/proxy" 2>&1 &
proxy_pid=$!
until grep -q listening "$tmp/proxy" 2>/dev/null; do sleep 0.1; done
agent_pid=""
if [ "$mode" = proxy-agent ]; then
    dc exec -T toolchain $bin/agent --pauses "${PAUSES:-4}" --unsafe >"$tmp/agent" 2>&1 &
    agent_pid=$!
    until grep -q connected "$tmp/agent" 2>/dev/null; do sleep 0.1; done
fi

curl -s -o /dev/null "$url"
echo "Sending the triggered request. Step through it in PhpStorm (see the report), then let it finish."
curl -s -o "$tmp/body" -w "%{http_code} %{time_total}s" -b XDEBUG_TRIGGER=1 "$url" >"$tmp/curl" || true
status=0
wait "$proxy_pid" || status=1
if [ -n "$agent_pid" ]; then wait "$agent_pid" || status=1; fi
dc exec -T php85 cat /var/log/xdebug/xdebug.log >"$here/results/$name-xdebug.log"

{
    echo "# $(date -u +%FT%TZ) mode=$mode ide=host.docker.internal:$port url=$url"
    echo "## proxy"
    cat "$tmp/proxy"
    if [ -f "$tmp/agent" ]; then echo "## agent"; cat "$tmp/agent"; fi
    echo "## request"
    echo "$(cat "$tmp/curl") $(tr -d '\n ' <"$tmp/body")"
    echo "## exit status $status"
} >"$here/results/$name.txt"
cat "$here/results/$name.txt"
echo "Logs: results/$name-proxy.log and results/$name-xdebug.log"
exit "$status"
