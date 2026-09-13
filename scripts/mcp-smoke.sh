#!/usr/bin/env bash
# End-to-end smoke test: boots `newera serve`, talks MCP over Streamable HTTP
# like an AI client would, and checks the change through the REST API.
#
# Usage: scripts/mcp-smoke.sh [path/to/newera]
set -euo pipefail

BIN="${1:-target/debug/newera}"
PORT="${SMOKE_PORT:-7989}"
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
for tool in get_home create update delete move split_wall set_home set_background render_plan export_plan save_home open_home new_home undo redo catalog place check_layout; do
  check "tool $tool listed" "$reply" "\"name\":\"$tool\""
done

call() { rpc '{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"'"$1"'","arguments":'"$2"'}}'; }
count() { curl -s "$BASE/api/home" | grep -o "\"id\":\"$1[0-9]*\"" | wc -l | tr -d ' '; }

reply=$(call create '{"walls":[{"pts":[[900,0],[1300,0],[1300,600],[900,600]],"closed":true}],"rooms":[{"name":"Cozinha","at":[1100,300]}],"dims":[{"wall":"w1"}],"labels":[{"text":"Entrada","at":[1100,-60]}]}')
check "create answers in one line" "$reply" 'ok rev=1 ids=w22,w23,w24,w25,r26,d27,t28'
check "REST sees 9 walls (5 demo + 4 new)" "$(count w)" "9"
check "room detected from walls" "$(count r)" "3"

reply=$(call get_home '{"detail":"summary"}')
check "summary lists the kitchen" "$reply" 'Cozinha'

reply=$(call update '{"items":[{"id":"w22","t":25},{"id":"t28","text":"Porta"}]}')
check "update several kinds" "$reply" 'ok rev=2'
reply=$(call update '{"items":[{"id":"w22","text":"x"}]}')
check "update rejects fields of other kinds" "$reply" 'does not apply'

reply=$(call split_wall '{"id":"w23"}')
check "split_wall returns the new wall" "$reply" 'ids=w29'

reply=$(call render_plan '{"w":320,"h":240}')
check "render_plan returns a PNG image" "$reply" '"mimeType":"image/png"'

reply=$(call undo '{}')
check "undo" "$reply" 'ok rev=4'
check "undo reverted the split" "$(count w)" "9"

reply=$(call delete '{"ids":["w1","nope"]}')
check "unknown id is rejected" "$reply" 'invalid id'
check "failed delete changes nothing" "$(count w)" "9"

reply=$(call catalog '{"q":"cama casal"}')
check "catalog search finds the double bed" "$reply" 'bed-double'
reply=$(call place '{"items":[{"cat":"window","wall":"w25"},{"cat":"bed-single","at":[1100,300]}]}')
check "place answers with furniture ids" "$reply" 'ids=f30,f31'
reply=$(call get_home '{}')
check "doors and windows report their wall" "$reply" '\"wall\":\"w25\"'
reply=$(call check_layout '{}')
check "check_layout reports issues as JSON" "$reply" '{'
reply=$(call render_plan '{"w":320,"h":240}')
check "render_plan with furniture still works" "$reply" '"mimeType":"image/png"'

TMP_PROJECT="$(mktemp -d)/casa"
reply=$(call save_home '{"path":"'"$TMP_PROJECT"'"}')
check "save_home adds the extension" "$reply" 'casa.newera'
call new_home '{}' >/dev/null
check "new_home clears" "$(count w)" "0"
reply=$(call open_home '{"path":"'"$TMP_PROJECT"'.newera"}')
check "open_home restores" "$(count w)" "9"
rm -rf "$(dirname "$TMP_PROJECT")"

rm -f "$LOG.headers"
echo "All good."
