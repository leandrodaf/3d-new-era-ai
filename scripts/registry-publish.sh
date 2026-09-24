#!/usr/bin/env bash
# Publishes this release to the MCP Registry as com.3dneweraai/newera: the
# version, the bundle's URL and its checksum go into server.json, then
# mcp-publisher signs in by the domain and publishes.
#
#   MCP_REGISTRY_PRIVATE_KEY=<hex> scripts/registry-publish.sh 1.8.0 dist/newera-mcp.mcpb
#
# The domain proves ownership with the public half of the key, served at
# https://3dneweraai.com/.well-known/mcp-registry-auth (scripts/registry-key.sh
# makes both halves once). Every aggregator that reads the registry — VS Code,
# GitHub, Glama, PulseMCP, mcp.so, JetBrains, Zed — picks the release up.
set -euo pipefail

version="${1:?version, e.g. 1.8.0}"
bundle="${2:?path to newera-mcp.mcpb}"
: "${MCP_REGISTRY_PRIVATE_KEY:?the registry key (scripts/registry-key.sh)}"
publisher="${MCP_PUBLISHER:-mcp-publisher}"

root="$(cd "$(dirname "$0")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

sha="$(sha256sum "$bundle" | cut -d' ' -f1)"
url="https://github.com/leandrodaf/3d-new-era-ai/releases/download/v${version}/newera-mcp.mcpb"
VERSION="$version" URL="$url" SHA="$sha" python3 - "$root/server.json" "$work/server.json" <<'PY'
import json, os, sys
server = json.load(open(sys.argv[1]))
server["version"] = os.environ["VERSION"]
for package in server.get("packages", []):
    if package["registryType"] == "mcpb":
        package["identifier"] = os.environ["URL"]
        package["version"] = os.environ["VERSION"]
        package["fileSha256"] = os.environ["SHA"]
json.dump(server, open(sys.argv[2], "w"), indent=2)
PY

cd "$work"
"$publisher" validate
"$publisher" login http --domain 3dneweraai.com --private-key "$MCP_REGISTRY_PRIVATE_KEY"
"$publisher" publish
