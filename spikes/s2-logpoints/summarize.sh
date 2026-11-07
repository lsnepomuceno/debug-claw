#!/bin/sh
# Summarises a results file: per scenario, the median request time, the
# added time per hit against the --no-break run with the same n, and the
# listener's mean hold and gap per pause over all sessions.
#   ./summarize.sh results/t08-local.txt
awk '
/^scenario/ { sc = $0; sub(/^scenario /, "", sc); split($0, a, "n="); split(a[2], b, " "); n = b[1] }
/^median_s=/ { split($0, m, "="); med[sc] = m[2]; nn[sc] = n; order[++k] = sc
  if (sc ~ /--no-break/) base[n] = m[2] }
/^session id=.*pauses=/ {
  for (i = 1; i <= NF; i++) {
    split($i, kv, "=")
    if (kv[1] == "hold_avg_us") { hs[sc] += kv[2]; hc[sc]++ }
    if (kv[1] == "gap_avg_us") gs[sc] += kv[2]
  }
}
END {
  printf "%-60s %10s %12s %10s %10s\n", "scenario", "median_ms", "added_us/hit", "hold_us", "gap_us"
  for (i = 1; i <= k; i++) {
    s = order[i]; add = "-"
    if (s !~ /no-break|untrig/ && base[nn[s]] != "") add = sprintf("%.1f", (med[s] - base[nn[s]]) * 1e6 / nn[s])
    h = hc[s] ? sprintf("%.1f", hs[s] / hc[s]) : "-"; g = hc[s] ? sprintf("%.1f", gs[s] / hc[s]) : "-"
    printf "%-60s %10.2f %12s %10s %10s\n", s, med[s] * 1000, add, h, g
  }
}' "$1"
