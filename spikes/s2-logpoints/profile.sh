#!/bin/sh
# Applies an Xdebug ini profile to the php85 container and reloads FPM.
#   ./profile.sh bridge    client_host=toolchain, Xdebug log off
#   ./profile.sh local     client_host=127.0.0.1, Xdebug log off
#   ./profile.sh default   back to the test bed ini (log on, level 7)
# The file lives only in the container's writable layer; nothing under
# spikes/testbed/ changes. Runs on the host; needs only docker and curl.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
export COMPOSE_PROJECT_NAME=testbed-s2 HTTP_PORT_85=8280 HTTP_PORT_83=8281
dc() { docker compose -f "$here/../testbed/compose.yaml" "$@"; }
target=/usr/local/etc/php/conf.d/zzz-s2.ini

case "${1:-}" in
  bridge|local) dc exec -T php85 sh -c "cat > $target" < "$here/ini/$1.ini" ;;
  default) dc exec -T php85 rm -f "$target" ;;
  *) echo "usage: $0 bridge|local|default" >&2; exit 2 ;;
esac
dc exec -T php85 sh -c "kill -USR2 1"
sleep 1
# Warm the workers and OPcache with untriggered requests.
for _ in 1 2 3 4 5; do curl -s -o /dev/null 'http://127.0.0.1:8280/loop?n=10'; done
dc exec -T php85 php -r 'echo "client_host=", ini_get("xdebug.client_host"), " log=", ini_get("xdebug.log"), "\n";'
