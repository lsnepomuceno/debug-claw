#!/bin/sh
# CLI micro-benchmarks: per-call cost on PDOStatement::execute
# (app/percall.php) and the cost for unobserved functions (app/calls.php).
#
#   ./scripts/cli.sh php85 [ROUNDS=3]
#
# Runs inside the PHP service container, on the same CPU as the FPM worker,
# so nothing else should be measuring at the same time. Output goes to
# results/cli-SERVICE.txt.
#
#   off          extension not loaded
#   on           loaded, flush=request (the buffer fills, then events drop)
#   on-call      loaded, listener up, one write per execute
#   closed-call  loaded, flush=call, daemon port closed
#   gone-call    loaded, flush=call, listener container stopped
set -eu
. "$(dirname "$0")/lib.sh"

svc=${1:-php85} rounds=${2:-3}
out=results/cli-$svc.txt
: > "$out"
docker compose up -d >/dev/null 2>&1
echo "# $svc CLI, $rounds rounds, $(date -u +%FT%TZ)" >> "$out"

php_with() {
    profile=$1
    shift
    docker compose exec -T "$svc" env PHP_INI_SCAN_DIR="/usr/local/etc/php/conf.d:/s5/ini/$profile" php "$@"
}

for r in $(seq 1 "$rounds"); do
    listener_up
    php_with off /s5/app/percall.php | sed "s/^/round=$r cfg=off /" | tee -a "$out"
    php_with on /s5/app/percall.php | sed "s/^/round=$r cfg=on /" | tee -a "$out"
    php_with on-call /s5/app/percall.php | sed "s/^/round=$r cfg=on-call /" | tee -a "$out"
    php_with closed -d debug_claw.flush=call /s5/app/percall.php | sed "s/^/round=$r cfg=closed-call /" | tee -a "$out"
    listener_stop
    php_with on-call /s5/app/percall.php | sed "s/^/round=$r cfg=gone-call /" | tee -a "$out"
    listener_up
    php_with off /s5/app/calls.php | sed "s/^/round=$r cfg=off /" | tee -a "$out"
    php_with on /s5/app/calls.php | sed "s/^/round=$r cfg=on /" | tee -a "$out"
done
