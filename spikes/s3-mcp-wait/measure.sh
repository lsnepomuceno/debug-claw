#!/bin/sh
# Samples the daemon process from the host /proc: RSS, CPU time and context
# switches, per thread and in total, at the start and the end of a window.
#
#   ./measure.sh idle SECONDS          fresh daemon, no client at all
#   ./measure.sh blocked SECONDS [LABEL]  during one `wait` call (Inspector CLI)
#
# The daemon runs in the testbed-s3-daemon-1 container. Its host PID comes
# from `docker top`. Results go to results/measure-<mode>[-<label>].txt.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
mode=$1 secs=$2 label=${3:-}
out="$here/results/measure-$mode${label:+-$label}.txt"
mkdir -p "$here/results"
hz=$(getconf CLK_TCK)

daemon_pid() {
    docker top testbed-s3-daemon-1 -eo pid,args | awk '/s3-daemon/ && !/sh -c/ {print $1; exit}'
}

# One line per thread: tid utime stime voluntary nonvoluntary, then a
# process line with VmRSS and VmHWM in kB.
snap() {
    for t in /proc/"$pid"/task/*; do
        tid=${t##*/}
        set -- $(cut -d' ' -f14,15 "$t/stat")
        v=$(awk '/^voluntary_ctxt_switches/ {print $2}' "$t/status")
        n=$(awk '/^nonvoluntary_ctxt_switches/ {print $2}' "$t/status")
        name=$(cat "$t/comm")
        echo "thread $tid $name utime=$1 stime=$2 vol=$v nonvol=$n"
    done
    awk '/^(VmRSS|VmHWM|Threads)/ {printf "%s %s ", $1, $2} END {print ""}' /proc/"$pid"/status
}

total() { awk '/^thread/ {for (i = 4; i <= 7; i++) {split($i, kv, "="); s[kv[1]] += kv[2]}}
    END {printf "%d %d %d %d\n", s["utime"], s["stime"], s["vol"], s["nonvol"]}'; }

case "$mode" in
    idle)
        "$here/dc.sh" restart daemon >/dev/null 2>&1
        sleep 5
        pid=$(daemon_pid)
        ;;
    blocked)
        pid=$(daemon_pid)
        "$here/dc.sh" --profile clients run --rm -T -e MCP_INSPECTOR_SECRET_STORE=memory inspector \
            --cli http://daemon:8390/mcp --transport http --method tools/call \
            --tool-name wait --tool-arg timeout_s=$((secs + 15)) >/dev/null 2>&1 &
        sleep 10
        ;;
    *) echo "unknown mode $mode" >&2; exit 2 ;;
esac

{
    echo "# mode $mode, window $secs s, host pid $pid, CLK_TCK $hz, $(date -u +%FT%TZ)"
    echo "## start"
    a=$(snap); echo "$a"
    sleep "$secs"
    echo "## end"
    b=$(snap); echo "$b"
    set -- $(echo "$a" | total) $(echo "$b" | total)
    echo "## delta over $secs s (all threads)"
    echo "cpu ticks: utime $(($5 - $1)) stime $(($6 - $2)) ($(echo "scale=3; ($5 - $1 + $6 - $2) * 1000 / $hz" | bc) ms)"
    echo "context switches: voluntary $(($7 - $3)) nonvoluntary $(($8 - $4))"
    echo "wakeups per second: $(echo "scale=4; ($7 - $3 + $8 - $4) / $secs" | bc)"
} >"$out"
wait
cat "$out"
