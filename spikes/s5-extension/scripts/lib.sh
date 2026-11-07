# Shared helpers for the S5 scripts. Sourced, not run.
# shellcheck shell=sh

cd "$(dirname "$0")/.." || exit 1
export COMPOSE_PROJECT_NAME="${COMPOSE_PROJECT_NAME:-testbed-s5}"
# The listener prints only every 10 000th line during measurements.
export S5_LISTENER_EVERY="${S5_LISTENER_EVERY:-10000}"

# nginx port inside the network for a PHP service.
port_of() { [ "$1" = php85 ] && echo 80 || echo 81; }

# set_profile SERVICE PROFILE: recreate the PHP service with ini/PROFILE and
# wait until it answers. A fresh container also means a fresh FPM worker.
set_profile() {
    svc=$1 profile=$2
    if [ "$svc" = php85 ]; then export S5_PROFILE_85="$profile"; else export S5_PROFILE_83="$profile"; fi
    docker compose up -d --force-recreate --no-deps "$svc" >/dev/null 2>&1
    i=0
    until docker compose exec -T bench bench "nginx:$(port_of "$svc")" "/" -n 1 >/dev/null 2>&1; do
        i=$((i + 1))
        [ "$i" -gt 50 ] && { echo "$svc did not come up" >&2; exit 1; }
        sleep 0.2
    done
}

listener_up() { docker compose up -d --no-deps listener >/dev/null 2>&1; }
listener_stop() { docker compose stop listener >/dev/null 2>&1; }

# worker_rss SERVICE: VmRSS of the single FPM worker, in kB.
worker_rss() {
    docker compose exec -T "$1" sh -c 'grep VmRSS /proc/$(pgrep -f "php-fpm: pool www")/status' | awk '{print $2}'
}
