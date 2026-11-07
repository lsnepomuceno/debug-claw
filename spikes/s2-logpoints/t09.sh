#!/bin/sh
# T-09: hit conditions. Request time at n=10000 with 5 expressions for each
# condition, then a short run per condition that reads hit_count while
# paused (--bp-get), to check which hits Xdebug paused on.
#   ./t09.sh [REPS]
set -eu
here=$(cd "$(dirname "$0")" && pwd)
reps=${1:-3}
log="$here/results/t09.txt"
"$here/profile.sh" bridge > "$log"
b() { "$here/bench.sh" bridge "$@" >> "$log"; }
n=10000
b "$reps" $n --no-break --quiet
b "$reps" $n --exprs 5 --quiet
b "$reps" $n --exprs 5 --hit-cond '>=' --hit-value 9991 --quiet
b "$reps" $n --exprs 5 --hit-cond '==' --hit-value 5000 --quiet
b "$reps" $n --exprs 5 --hit-cond '%' --hit-value 1000 --quiet
b "$reps" $n --exprs 5 --hit-cond '==' --hit-value 20000 --quiet
b "$reps" $n --exprs 5 --cap 10 --quiet
# Hit counts seen while paused, on a short loop.
b 1 100 --exprs 5 --hit-cond '>=' --hit-value 95 --bp-get --quiet
b 1 100 --exprs 5 --hit-cond '==' --hit-value 50 --bp-get --quiet
b 1 100 --exprs 5 --hit-cond '%' --hit-value 10 --bp-get --quiet
"$here/summarize.sh" "$log"
grep -E '^session id=1 .*pauses=' "$log" | grep -oE 'pauses=[0-9]+ mismatches=[0-9]+|bp_hit_count=[^ ]+' | paste - -
