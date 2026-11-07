#!/bin/sh
# T-30 latency: the emulator times ROUNDS rounds of stack_get, context_get
# and property_get at the first pause, reached directly, through the proxy
# in the same container (loop) and through the proxy across the Compose
# bridge (bridge), REPS times each. The last runs add an agent that floods
# reads in the same pause. Writes results/bench-*.txt and a table of the
# "bench all" lines in results/bench-summary.txt.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
cd "$here"
rounds=${ROUNDS:-2000}
reps=${REPS:-3}
r=1
while [ "$r" -le "$reps" ]; do
    for mode in direct loop bridge; do
        EMU_ARGS="--bench $rounds" ./run.sh "$mode" "bench-$mode-$r" >/dev/null
    done
    EMU_ARGS="--bench $rounds" AGENT_ARGS="--pauses 2 --flood" ./run.sh bridge "bench-bridge-flood-$r" >/dev/null
    r=$((r + 1))
done
{
    echo "# rounds=$rounds reps=$reps, round trip in microseconds as the emulator sees it"
    for mode in direct loop bridge bridge-flood; do
        r=1
        while [ "$r" -le "$reps" ]; do
            f=results/bench-$mode-$r.txt
            echo "$mode $r: $(grep '^bench all' "$f")"
            grep -h 'xd commands\|xd->ide packets\|flood:' "$f" | sed 's/^/    /'
            r=$((r + 1))
        done
    done
} >results/bench-summary.txt
cat results/bench-summary.txt
