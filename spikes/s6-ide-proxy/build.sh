#!/bin/sh
# Runs the unit tests and builds the three binaries in the toolchain
# container of the testbed-s6 copy. Runs on the host; needs only docker.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
export COMPOSE_PROJECT_NAME=testbed-s6 HTTP_PORT_85=8680 HTTP_PORT_83=8681
docker compose -f "$here/../testbed/compose.yaml" -f "$here/compose.override.yaml" \
    exec -T toolchain sh -c 'cd spikes/s6-ide-proxy && cargo test --release 2>&1 | grep -E "test result|FAILED|panicked|^(warning|error)" && cargo build --release 2>&1 | grep -E "^(warning|error)|-->|Finished"'
