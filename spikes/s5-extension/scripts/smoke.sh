#!/bin/sh
# T-24 and T-25: the smoke test on both PHP versions, plus one FPM request
# to /queries?n=3, with the listener printing every line it receives.
# Output: results/smoke.txt.
set -eu
. "$(dirname "$0")/lib.sh"
export S5_LISTENER_EVERY=1
out=results/smoke.txt
: > "$out"
docker compose up -d >/dev/null 2>&1
docker compose up -d --force-recreate --no-deps listener >/dev/null 2>&1
sleep 1
for svc in php85 php83; do
    echo "## $svc CLI: app/smoke.php" >> "$out"
    docker compose exec -T "$svc" cat /usr/local/lib/php-s5/build-info.txt >> "$out"
    docker compose exec -T "$svc" env PHP_INI_SCAN_DIR=/usr/local/etc/php/conf.d:/s5/ini/on \
        php -r 'echo implode(",", array_filter(get_loaded_extensions(), fn ($e) => $e === "debug_claw_s5")), "\n";' >> "$out"
    docker compose exec -T "$svc" env PHP_INI_SCAN_DIR=/usr/local/etc/php/conf.d:/s5/ini/on php /s5/app/smoke.php >> "$out"
    echo "## $svc FPM: GET /queries?n=3" >> "$out"
    set_profile "$svc" on
    docker compose exec -T bench bench "nginx:$(port_of "$svc")" "/queries?n=3" -n 1 >> "$out"
    ./scripts/mem.sh "$svc" | sed 's/.*"ext"://' >> "$out"
done
sleep 1
echo "## listener output" >> "$out"
/usr/bin/docker logs "$(docker compose ps -q listener)" >> "$out" 2>&1
