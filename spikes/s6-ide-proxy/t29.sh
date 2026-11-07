#!/bin/sh
# T-29 and the transcript half of T-30: the same emulator session direct,
# through the proxy (same container and across the bridge), and through
# the proxy with the agent injecting reads during every pause. Diffs each
# transcript against the direct run with the same emulator arguments and
# writes results/t29-diff.txt.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
cd "$here"
./run.sh direct t29-direct-a >/dev/null
./run.sh direct t29-direct-b >/dev/null
./run.sh loop t29-loop >/dev/null
./run.sh bridge t29-bridge >/dev/null
EMU_ARGS="--hold-ms 200" ./run.sh direct t29-direct-hold >/dev/null
EMU_ARGS="--hold-ms 200" AGENT_ARGS="--pauses 4 --unsafe" ./run.sh bridge t30-inject >/dev/null
AGENT_ARGS="--pauses 4 --flood" ./run.sh bridge t30-flood >/dev/null
{
    for pair in t29-direct-a:t29-direct-b t29-direct-a:t29-loop t29-direct-a:t29-bridge \
        t29-direct-hold:t30-inject t29-direct-a:t30-flood; do
        a=${pair%%:*} b=${pair#*:}
        if cmp -s "results/$a.transcript" "results/$b.transcript"; then
            echo "$b: transcript identical to $a ($(wc -l <"results/$b.transcript") lines)"
        else
            echo "$b: transcript DIFFERS from $a"
            diff "results/$a.transcript" "results/$b.transcript" | cut -c1-300 || true
        fi
    done
} >results/t29-diff.txt
cat results/t29-diff.txt
