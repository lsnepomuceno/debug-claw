#!/bin/sh
# Is the idle wakeup thread (blocking-1) part of the AccessKit adapter?
# Builds a copy of the spike in /tmp with eframe's `accesskit` feature
# off, and counts the context switches per thread of an idle window.
#   sh scripts/noak.sh [SECONDS]
set -eu
secs=${1:-20}
rm -rf /tmp/noak
mkdir -p /tmp/noak
cp -r /work/spikes/s4-egui/src /work/spikes/s4-egui/Cargo.lock /tmp/noak/
sed 's/^eframe = .*/eframe = { version = "0.37", default-features = false, features = ["glow", "wgpu", "clipboard", "default_fonts", "links", "system_fonts", "wayland", "x11"] }\nwinit = "0.30"/' \
    /work/spikes/s4-egui/Cargo.toml >/tmp/noak/Cargo.toml
CARGO_TARGET_DIR=/target/noak cargo build --release --quiet --manifest-path /tmp/noak/Cargo.toml
echo "accesskit_unix in the build: $(cargo tree --manifest-path /tmp/noak/Cargo.toml -i accesskit_unix 2>&1 | head -1)"
/target/noak/release/s4-egui --backend wgpu --view debugger >/tmp/noak-app.log 2>&1 &
pid=$!
sleep 5
snap() {
    for t in /proc/"$pid"/task/*; do
        printf '%s %s %s\n' "$(basename "$t")" "$(cat "$t/comm" | tr ' ' '_')" \
            "$(awk '/ctxt_switches/ {s += $2} END {print s}' "$t/status")"
    done | sort
}
snap >/tmp/noak-a.txt
sleep "$secs"
snap >/tmp/noak-b.txt
kill "$pid"
echo "threads without accesskit backend=wgpu args=[--view debugger] seconds=$secs (tid name switches)"
join /tmp/noak-a.txt /tmp/noak-b.txt | awk '{d = $5 - $3; if (d > 0) printf "  %s %s %d\n", $1, $2, d}'
echo "  threads total: $(wc -l </tmp/noak-b.txt)"
