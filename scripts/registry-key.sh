#!/usr/bin/env bash
# Once: makes the key that proves to the MCP Registry that this project owns
# 3dneweraai.com, so it may publish as com.3dneweraai/*.
#
#   scripts/registry-key.sh
#
# Writes the public half to site/.well-known/mcp-registry-auth (commit it:
# the site serves it) and stores the private half as the repository secret
# MCP_REGISTRY_PRIVATE_KEY, which the release job signs in with. The key
# never touches the disk outside a temporary directory.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

openssl genpkey -algorithm Ed25519 -out "$work/key.pem"
public="$(openssl pkey -in "$work/key.pem" -pubout -outform DER | tail -c 32 | base64)"
private="$(openssl pkey -in "$work/key.pem" -noout -text | grep -A3 'priv:' | tail -n +2 | tr -d ' :\n')"

mkdir -p "$root/site/.well-known"
printf 'v=MCPv1; k=ed25519; p=%s\n' "$public" > "$root/site/.well-known/mcp-registry-auth"
printf '%s' "$private" | gh secret set MCP_REGISTRY_PRIVATE_KEY --repo leandrodaf/3d-new-era-ai

echo "public key written to site/.well-known/mcp-registry-auth — commit and deploy the site"
echo "private key stored as the MCP_REGISTRY_PRIVATE_KEY secret"
