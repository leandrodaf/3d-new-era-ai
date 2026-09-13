#!/usr/bin/env bash
# End-to-end smoke test: boots `newera serve`, talks MCP over Streamable HTTP
# like an AI client would, and checks the change through the REST API.
#
# Usage: scripts/mcp-smoke.sh [path/to/newera]
set -euo pipefail

BIN="${1:-target/debug/newera}"
PORT="${SMOKE_PORT:-7979}"
BASE="http://127.0.0.1:${PORT}"
PROTOCOL="2025-06-18"
LOG="$(mktemp)"

"$BIN" serve --demo --addr "127.0.0.1:${PORT}" >"$LOG" 2>&1 &
PID=$!
trap 'kill "$PID" 2>/dev/null || true; rm -f "$LOG"' EXIT

for _ in $(seq 1 50); do
  curl -sf "$BASE/health" >/dev/null && break
  sleep 0.2
done
curl -sf "$BASE/health" >/dev/null || { echo "server did not start:"; cat "$LOG"; exit 1; }

SESSION=""
# Sends a JSON-RPC message and prints the `data:` payload of the SSE reply.
rpc() {
  local headers=(-H "Content-Type: application/json" -H "Accept: application/json, text/event-stream")
  [[ -n "$SESSION" ]] && headers+=(-H "Mcp-Session-Id: $SESSION" -H "MCP-Protocol-Version: $PROTOCOL")
  curl -s "${headers[@]}" -D "$LOG.headers" "$BASE/mcp" -d "$1" | sed -n 's/^data: //p' | grep -v '^$' || true
}

check() {
  local label="$1" haystack="$2" needle="$3"
  if [[ "$haystack" == *"$needle"* ]]; then
    echo "  ✓ $label"
  else
    echo "  ✗ $label — expected to find: $needle"
    echo "    got: $haystack"
    exit 1
  fi
}

echo "MCP smoke test against $BASE"

reply=$(rpc '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"'"$PROTOCOL"'","capabilities":{},"clientInfo":{"name":"smoke","version":"1"}}}')
check "initialize" "$reply" '"name":"3d-new-era-ai"'
SESSION=$(grep -i '^mcp-session-id:' "$LOG.headers" | awk '{print $2}' | tr -d '\r')
rpc '{"jsonrpc":"2.0","method":"notifications/initialized"}' >/dev/null

reply=$(rpc '{"jsonrpc":"2.0","id":2,"method":"tools/list"}')
for tool in get_home create_walls create_room delete set_compass undo redo; do
  check "tool $tool listed" "$reply" "\"name\":\"$tool\""
done

count_walls() { curl -s "$BASE/api/home" | grep -o '"id":"w[0-9]*"' | wc -l | tr -d ' '; }

reply=$(rpc '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"create_walls","arguments":{"points":[[800,0],[1200,0],[1200,600]]}}}')
check "create_walls answers in one line" "$reply" 'ok rev=1 ids=w8,w9'
check "REST sees 7 walls (5 demo + 2 new)" "$(count_walls)" "7"

reply=$(rpc '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"get_home","arguments":{}}}')
check "get_home is compact" "$reply" '\"a\":[800,0]'

reply=$(rpc '{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"undo","arguments":{}}}')
check "undo" "$reply" 'ok rev=2'
check "undo reverts the whole batch" "$(count_walls)" "5"

reply=$(rpc '{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"delete","arguments":{"ids":["w1","nope"]}}}')
check "unknown id is rejected" "$reply" 'unknown id'
check "failed delete changes nothing" "$(count_walls)" "5"

rm -f "$LOG.headers"
echo "All good."
