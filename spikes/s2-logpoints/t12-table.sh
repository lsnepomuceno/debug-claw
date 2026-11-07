#!/bin/sh
# Turns a probe transcript (results/t12-*.txt) into one line per read, with
# the limits in force at the time: depth, children, data, bytes, median us,
# number of <property> elements and the command.
#   ./t12-table.sh results/t12-depth.txt
awk '
/^cmd feature_set -n max_depth -v/    { d = $NF; next }
/^cmd feature_set -n max_children -v/ { c = $NF; next }
/^cmd feature_set -n max_data -v/     { m = $NF; next }
/^cmd / { cmd = substr($0, 5); want = 1; next }
want && /bytes=/ {
  split($1, b, "="); split($2, t, "="); bytes = b[2]; us = t[2]; next
}
want && /props=/ {
  split($1, p, "=")
  printf "depth=%-3s children=%-6s data=%-5s bytes=%-9s us=%-7s props=%-6s %s\n", d, c, m, bytes, us, p[2], cmd
  want = 0
}
' "$1"
