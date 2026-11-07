#!/bin/sh
# Runs one Claude Code print-mode call against the spike daemon and keeps the
# JSON result plus the MCP lines of the debug log in results/claude-*.
#
#   ./claude.sh CASE CONFIG PROMPT [TOOLS]
#
# CONFIG is claude/http.json or claude/stdio.json. Extra environment for
# Claude Code (MCP_TOOL_TIMEOUT and so on) is passed through as is. The MCP
# server exists only for this call: nothing is written to the Claude Code
# configuration.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
case_name=$1 config=$2 prompt=$3
tools=${4:-mcp__s3__wait}
mkdir -p "$here/results"
scratch=${S3_SCRATCH:-$(mktemp -d)}
debug="$scratch/claude-$case_name.debug.log"
out="$scratch/claude-$case_name.json"

cd "$scratch"
start=$(date +%s.%N)
status=0
claude -p "$prompt" --mcp-config "$here/$config" --strict-mcp-config \
    --allowedTools "$tools" --model haiku --output-format json \
    --debug-file "$debug" >"$out" 2>"$scratch/claude-$case_name.stderr" || status=$?
end=$(date +%s.%N)
printf '# case %s, config %s, exit %s, wall %.1f s\n' "$case_name" "$config" "$status" \
    "$(echo "$end - $start" | bc)" >"$here/results/claude-$case_name.mcp.log"
grep -E '^# |MCP server "s3"|Server "s3"|mcp_tool tool error' "$debug" \
    | grep -viE 'oauth|token|authorization|hook' >>"$here/results/claude-$case_name.mcp.log" || true
jq '{is_error, result, duration_ms, num_turns, total_cost_usd}' "$out" >"$here/results/claude-$case_name.json" || cp "$out" "$here/results/claude-$case_name.json"
cat "$here/results/claude-$case_name.json"
head -1 "$here/results/claude-$case_name.mcp.log"
