#!/bin/sh
# Heap and RSS per dataset and backend. Each run draws 120 frames of one
# view and prints the counting allocator's numbers and VmRSS.
#   sh scripts/memory.sh [BACKENDS]
set -eu
BIN=/target/release/s4-egui
backends=${1:-"glow wgpu"}

run() {
    timeout 120 "$BIN" --backend "$b" --quit-after 120 "$@" | /usr/bin/grep -E "^(start|heap)"
}

for b in $backends; do
    run --view feed --dumps 40
    run --view feed --dumps 10000
    run --view debugger
    run --view tree --tree-nodes 1000
    run --view tree --tree-nodes 100000
    run --view tree --tree-nodes 1000 --expand-all
    run --view tree --tree-nodes 100000 --expand-all
    run --view flame --flame-frames 50000
    run --view source --source-lines 5000
    run --view feed --dumps 10000 --load-all --tree-nodes 100000 --flame-frames 50000 --source-lines 5000
done
