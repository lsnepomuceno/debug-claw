#!/bin/sh
# T-08 time matrix: for one connection kind, runs every scenario REPS times
# at n=1000 and n=10000 and keeps the raw output in results/.
#   ./matrix.sh local|bridge [REPS]
set -eu
here=$(cd "$(dirname "$0")" && pwd)
kind=$1 reps=${2:-5}
mkdir -p "$here/results"
log="$here/results/t08-$kind.txt"
"$here/profile.sh" "$kind" > "$log"
for n in 1000 10000; do
  {
    "$here/bench.sh" untrig "$reps" "$n"
    "$here/bench.sh" "$kind" "$reps" "$n" --no-break --quiet
    "$here/bench.sh" "$kind" "$reps" "$n" --exprs 0 --quiet
    "$here/bench.sh" "$kind" "$reps" "$n" --exprs 1 --quiet
    "$here/bench.sh" "$kind" "$reps" "$n" --exprs 5 --quiet
    "$here/bench.sh" "$kind" "$reps" "$n" --exprs 5 --pipeline --quiet
  } >> "$log"
done
grep -E '^(scenario|median_s)' "$log" | paste - -
