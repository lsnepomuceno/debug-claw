#!/bin/sh
# Drives the stdio shim by hand: initialize, then one `wait` call, with raw
# JSON-RPC lines on stdin. Prints what the shim writes to stdout.
#
#   ./shim-smoke.sh [TIMEOUT_S] [PROTOCOL_VERSION]
set -eu
t="${1:-3}"
v="${2:-2025-11-25}"
{
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"'"$v"'","capabilities":{},"clientInfo":{"name":"shim-smoke","version":"0"}}}'
    sleep 1
    echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"wait","arguments":{"timeout_s":'"$t"',"progress_every_s":1},"_meta":{"progressToken":"smoke"}}}'
    sleep $((t + 2))
} | docker exec -i testbed-s3-daemon-1 /target/release/s3-shim http://127.0.0.1:8390/mcp
