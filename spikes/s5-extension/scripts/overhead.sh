#!/bin/sh
# T-26: request time of GET /queries?n=100 in five configurations.
#
#   ./scripts/overhead.sh php85 [ROUNDS] [N] [WARMUP] [PATH TAG]
#
# PATH and TAG measure another endpoint of the fixture app, with TAG in the
# file names (results/t26-php85-TAG.txt); summarize.sh and paired.sh then
# take php85-TAG.
#
#   off      extension not loaded
#   on       loaded, listener up, one write per request
#   on-call  loaded, listener up, one write per observed call
#   closed   loaded, daemon port closed (connect is refused at once)
#   gone     loaded, listener container stopped (connect times out, 1 ms)
#
# Each round runs every configuration once, on a fresh FPM worker, with
# WARMUP untimed requests first. Raw timings go to results/raw/, one file
# per configuration and round; the summary lines to results/t26-SERVICE.txt.
# After each run, the worker's extension counters are appended too.
set -eu
. "$(dirname "$0")/lib.sh"

svc=${1:-php85} rounds=${2:-5} n=${3:-1000} warmup=${4:-200} path=${5:-/queries?n=100}
name=$svc${6:+-$6}
out=results/t26-$name.txt
mkdir -p results/raw
: > "$out"
port=$(port_of "$svc")
docker compose up -d >/dev/null 2>&1
echo "# $svc $path, $rounds rounds of $n requests (+$warmup warmup), $(date -u +%FT%TZ)" >> "$out"
for r in $(seq 1 "$rounds"); do
    # Rotate the order each round, so drift does not favour one configuration.
    for cfg in $(echo off on on-call closed gone off on on-call closed | cut -d' ' -f"$(( (r - 1) % 5 + 1 ))-$(( (r - 1) % 5 + 5 ))"); do
        case $cfg in
            gone) listener_stop; set_profile "$svc" on ;;
            *) listener_up; set_profile "$svc" "$cfg" ;;
        esac
        raw=raw/t26-$name-$cfg-$r.txt
        docker compose exec -T bench bench "nginx:$port" "$path" -n "$n" -w "$warmup" \
            --label "round=$r cfg=$cfg" --out "/results/$raw" | tee -a "$out"
        echo "  stats $(./scripts/mem.sh "$svc" | sed 's/.*"ext"://')" >> "$out"
    done
done
listener_up
./scripts/summarize.sh "$name" | tee -a "$out"
./scripts/paired.sh "$name" | tee -a "$out"
