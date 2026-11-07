#!/bin/sh
# Prints mem.php's JSON for one PHP service: ./scripts/mem.sh php85
# Talks FastCGI to the single FPM worker from inside its container.
set -eu
cd "$(dirname "$0")/.."
docker compose exec -T "$1" env SCRIPT_FILENAME=/s5/app/mem.php SCRIPT_NAME=/mem.php \
    REQUEST_METHOD=GET QUERY_STRING= cgi-fcgi -bind -connect 127.0.0.1:9000 | sed -n '/^{/p'
