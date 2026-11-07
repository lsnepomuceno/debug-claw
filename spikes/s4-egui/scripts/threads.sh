#!/bin/sh
# Which threads wake up while the window is idle: context switches per
# thread over SECONDS, with the thread names.
#   sh scripts/threads.sh BACKEND SECONDS [app args...]
set -eu
backend=$1
secs=$2
shift 2
/target/release/s4-egui --backend "$backend" "$@" >/tmp/threads-app.log 2>&1 &
pid=$!
sleep 5
snap() {
    for t in /proc/"$pid"/task/*; do
        printf '%s %s %s\n' "$(basename "$t")" "$(cat "$t/comm" | tr ' ' '_')" \
            "$(awk '/ctxt_switches/ {s += $2} END {print s}' "$t/status")"
    done | sort
}
snap >/tmp/threads-a.txt
sleep "$secs"
snap >/tmp/threads-b.txt
kill "$pid"
echo "threads backend=$backend args=[$*] seconds=$secs (tid name switches)"
join /tmp/threads-a.txt /tmp/threads-b.txt | awk '{d = $5 - $3; if (d > 0) printf "  %s %s %d\n", $1, $2, d}'
echo "  threads total: $(wc -l </tmp/threads-b.txt)"
