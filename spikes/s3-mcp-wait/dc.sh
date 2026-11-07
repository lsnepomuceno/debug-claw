#!/bin/sh
# docker compose for the S3 copy of the test bed plus the S3 services.
#
#   ./dc.sh up -d --build toolchain daemon
#   ./dc.sh exec -T toolchain cargo build --release --manifest-path spikes/s3-mcp-wait/Cargo.toml
#   ./dc.sh run --rm inspector --cli http://daemon:8390/mcp --transport http --method tools/list
set -eu
here=$(cd "$(dirname "$0")" && pwd)
export COMPOSE_PROJECT_NAME="${COMPOSE_PROJECT_NAME:-testbed-s3}"
export HTTP_PORT_85="${HTTP_PORT_85:-8380}" HTTP_PORT_83="${HTTP_PORT_83:-8381}"
exec docker compose -f "$here/../testbed/compose.yaml" -f "$here/compose.override.yaml" "$@"
