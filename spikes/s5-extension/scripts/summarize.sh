#!/bin/sh
# Pools the raw T-26 timings per configuration: median, p95 and p99 of all
# requests, the spread of the per-round medians, and the difference to
# "off". Usage: ./scripts/summarize.sh php85
set -eu
cd "$(dirname "$0")/.."
svc=${1:-php85}

pct() { sort -n | awk -v p="$1" '{a[NR]=$1} END {i=int((NR-1)*p+0.5)+1; print a[i]}'; }

base=$(cat results/raw/t26-"$svc"-off-[0-9]*.txt | pct 0.5)
base95=$(cat results/raw/t26-"$svc"-off-[0-9]*.txt | pct 0.95)
printf '%-8s %7s %9s %9s %9s %-25s %9s %8s %9s\n' cfg n median_us p95_us p99_us round_medians_us d_med_us d_med_% d_p95_us
for cfg in off on on-call closed gone; do
    files=$(ls results/raw/t26-"$svc"-"$cfg"-[0-9]*.txt 2>/dev/null) || continue
    n=$(cat $files | wc -l)
    med=$(cat $files | pct 0.5)
    p95=$(cat $files | pct 0.95)
    p99=$(cat $files | pct 0.99)
    rounds=$(for f in $files; do pct 0.5 < "$f"; done | sort -n | awk 'NR==1{lo=$1} {hi=$1} END {print lo "-" hi}')
    awk -v c="$cfg" -v n="$n" -v m="$med" -v p="$p95" -v q="$p99" -v r="$rounds" -v b="$base" -v b95="$base95" \
        'BEGIN {printf "%-8s %7d %9.1f %9.1f %9.1f %-25s %9.1f %8.2f %9.1f\n", c, n, m, p, q, r, m-b, (m-b)/b*100, p-b95}'
done
