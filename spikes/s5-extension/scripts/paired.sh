#!/bin/sh
# Paired view of a T-26 run: for each round, every configuration's median
# minus the "off" median of the same round, then the median and the range of
# those differences over the rounds. Rounds run the configurations back to
# back on the same machine state, so the paired difference removes most of
# the drift between rounds. Usage: ./scripts/paired.sh php85
set -eu
cd "$(dirname "$0")/.."
svc=${1:-php85}
awk '
    /^round=/ {
        split($1, a, "="); split($2, c, "="); split($4, m, "=")
        med[a[2], c[2]] = m[2]; rounds[a[2]] = 1; cfgs[c[2]] = 1
    }
    END {
        n = split("on on-call closed gone", order, " ")
        printf "%-8s %7s %9s %21s\n", "cfg", "rounds", "d_med_us", "d_range_us"
        for (k = 1; k <= n; k++) {
            cfg = order[k]; cnt = 0
            for (r in rounds) if ((r, cfg) in med && (r, "off") in med) d[++cnt] = med[r, cfg] - med[r, "off"]
            if (cnt == 0) continue
            for (i = 1; i <= cnt; i++) for (j = i + 1; j <= cnt; j++) if (d[j] < d[i]) { t = d[i]; d[i] = d[j]; d[j] = t }
            mid = (cnt % 2) ? d[(cnt + 1) / 2] : (d[cnt / 2] + d[cnt / 2 + 1]) / 2
            printf "%-8s %7d %9.1f %10.1f to %8.1f\n", cfg, cnt, mid, d[1], d[cnt]
        }
    }' "results/t26-$svc.txt"
