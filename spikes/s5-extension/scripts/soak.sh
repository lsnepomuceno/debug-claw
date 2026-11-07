#!/bin/sh
# T-27: memory of the single FPM worker over many requests.
#
#   ./scripts/soak.sh php85 PROFILE [TOTAL=10000] [STEP=1000] [PATH=/queries?n=100]
#
# Recreates the service with ini/PROFILE (off, on, on-call), so the run
# starts on a fresh worker, then sends TOTAL requests through nginx in steps
# of STEP. Before the first step and after each one, mem.php reports the
# worker's VmRSS, memory_get_usage and the extension counters. The output is
# one TSV row per sample in results/t27-SERVICE-PROFILE.tsv.
set -eu
. "$(dirname "$0")/lib.sh"

svc=${1:-php85} profile=${2:-on} total=${3:-10000} step=${4:-1000} path=${5:-/queries?n=100}
out=results/t27-$svc-$profile.tsv
port=$(port_of "$svc")
docker compose up -d >/dev/null 2>&1
listener_up
set_profile "$svc" "$profile"

sample() {
    ./scripts/mem.sh "$svc" | awk -v done="$1" '
        function num(k,   m) { if (match($0, "\"" k "\":[0-9]+")) { m = substr($0, RSTART, RLENGTH); sub(/.*:/, "", m); return m } return "-" }
        { printf "%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n", done, num("pid"), num("VmRSS_kb"), num("RssAnon_kb"), num("VmHWM_kb"),
          num("memory_get_usage"), num("memory_get_usage_real"), num("init_calls"), num("init_observed"), num("begin_calls"),
          num("events_sent"), num("requests") }'
}

printf 'requests\tpid\tvmrss_kb\trss_anon_kb\tvmhwm_kb\tmem_usage\tmem_usage_real\tinit_calls\tinit_observed\tbegin_calls\tevents_sent\text_requests\n' > "$out"
sample 0 | tee -a "$out"
done_n=0
while [ "$done_n" -lt "$total" ]; do
    docker compose exec -T bench bench "nginx:$port" "$path" -n "$step" -w 0 --label "t27 $svc $profile" >/dev/null
    done_n=$((done_n + step))
    sample "$done_n" | tee -a "$out"
done
