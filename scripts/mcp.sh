#!/usr/bin/env bash
# Calls one MCP tool on a running editor/server and prints the text reply.
#
# Usage: scripts/mcp.sh <tool> ['{"json":"args"}']
#   scripts/mcp.sh get_home
#   scripts/mcp.sh create_walls '{"points":[[0,0],[400,0]]}'
# Env: NEWERA_ADDR (default 127.0.0.1:7878)
set -euo pipefail

TOOL="${1:?usage: scripts/mcp.sh <tool> [json-args]}"
ARGS="${2:-{\}}"
URL="http://${NEWERA_ADDR:-127.0.0.1:7878}/mcp"
PROTOCOL="2025-06-18"
HEADERS="$(mktemp)"
trap 'rm -f "$HEADERS"' EXIT

base=(-s -H "Content-Type: application/json" -H "Accept: application/json, text/event-stream")
curl "${base[@]}" -D "$HEADERS" "$URL" \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"'"$PROTOCOL"'","capabilities":{},"clientInfo":{"name":"mcp.sh","version":"1"}}}' >/dev/null
session=$(grep -i '^mcp-session-id:' "$HEADERS" | awk '{print $2}' | tr -d '\r')
base+=(-H "Mcp-Session-Id: $session" -H "MCP-Protocol-Version: $PROTOCOL")
curl "${base[@]}" "$URL" -d '{"jsonrpc":"2.0","method":"notifications/initialized"}' >/dev/null

reply=$(curl "${base[@]}" "$URL" \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"'"$TOOL"'","arguments":'"$ARGS"'}}' \
  | sed -n 's/^data: //p' | grep -v '^$' || true)

python3 - "$reply" <<'PY'
import json, sys
msg = json.loads(sys.argv[1] or "{}")
if "error" in msg:
    print("error:", msg["error"].get("message"), file=sys.stderr); sys.exit(1)
result = msg.get("result", {})
text = "".join(c.get("text", "") for c in result.get("content", []) if c.get("type") == "text")
print(text)
sys.exit(1 if result.get("isError") else 0)
PY
