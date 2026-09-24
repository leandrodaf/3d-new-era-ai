#!/usr/bin/env bash
# Packs the MCP Bundle (.mcpb): the one file Claude Desktop installs with a
# double click, and the package the MCP Registry, Smithery and Windows point
# at. One bundle for every platform — the registry has no field to choose a
# file per system, so the bundle chooses at run time (platform_overrides).
#
#   scripts/mcpb.sh --version 1.8.0 --out dist/newera-mcp.mcpb \
#     --linux target/x86_64-unknown-linux-gnu/release/newera \
#     --darwin path/to/universal/newera \
#     --windows target/x86_64-pc-windows-msvc/release/newera.exe
#
# Any subset of platforms packs; the manifest lists the ones given. What runs
# is `newera mcp`: the open window's plan when the editor is running, a
# project of its own when it is not.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
version="" out="" linux="" darwin="" windows=""
while [ $# -gt 0 ]; do
  case "$1" in
    --version) version="$2"; shift 2 ;;
    --out) out="$2"; shift 2 ;;
    --linux) linux="$2"; shift 2 ;;
    --darwin) darwin="$2"; shift 2 ;;
    --windows) windows="$2"; shift 2 ;;
    *) echo "unknown argument $1" >&2; exit 2 ;;
  esac
done
[ -n "$version" ] && [ -n "$out" ] || { echo "--version and --out are required" >&2; exit 2; }
[ -n "$linux$darwin$windows" ] || { echo "give at least one of --linux, --darwin, --windows" >&2; exit 2; }

stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/bin"
[ -z "$linux" ] || install -m 755 "$linux" "$stage/bin/newera-linux"
[ -z "$darwin" ] || install -m 755 "$darwin" "$stage/bin/newera-macos"
[ -z "$windows" ] || install -m 755 "$windows" "$stage/bin/newera-windows.exe"
cp "$root/assets/icon-512.png" "$stage/icon.png"
cp "$root/LICENSE-MIT" "$root/LICENSE-APACHE" "$stage/"

VERSION="$version" STAGE="$stage" ROOT="$root" python3 - <<'PY'
import json, os
stage, root = os.environ["STAGE"], os.environ["ROOT"]
have = {p: os.path.exists(os.path.join(stage, "bin", f))
        for p, f in [("linux", "newera-linux"), ("darwin", "newera-macos"),
                     ("win32", "newera-windows.exe")]}
files = {"linux": "newera-linux", "darwin": "newera-macos", "win32": "newera-windows.exe"}
platforms = [p for p in ("darwin", "win32", "linux") if have[p]]
first = platforms[0]
command = lambda p: "${__dirname}/bin/" + files[p]
# The tools as the server lists them, so the directory can show them before
# anything is installed. Titles are what people read.
surface = json.load(open(os.path.join(root, "crates/newera-mcp/tests/fixtures/tool-surface.json")))
tools = [{"name": name, "description": tool.get("title") or tool["description"].split(". ")[0]}
         for name, tool in sorted(surface.items())]
manifest = {
    "$schema": "https://raw.githubusercontent.com/modelcontextprotocol/mcpb/main/schemas/mcpb-manifest-v0.3.schema.json",
    "manifest_version": "0.3",
    "name": "3d-new-era-ai",
    "display_name": "3D New Era AI",
    "version": os.environ["VERSION"],
    "description": "Design homes with your AI: floor plans, furniture, joinery, lighting and photos in an open-source editor.",
    "long_description": (
        "3D New Era AI is an open-source home design, floor plan and interior design editor. "
        "Through this extension Claude draws walls and rooms, places furniture, doors and windows, "
        "builds joinery and kitchens, checks lighting and ergonomics against standards and renders "
        "the plan, a 3D view or a photo — every change one undo away.\n\n"
        "With the 3D New Era AI window open, Claude edits the plan on your screen, live. Without "
        "it, Claude works on a project of its own, which it can save as a .newera file for you to "
        "open later. Everything runs on your computer: no account, no cloud."
    ),
    "author": {"name": "Leandro Ferreira", "url": "https://3dneweraai.com"},
    "repository": {"type": "git", "url": "https://github.com/leandrodaf/3d-new-era-ai"},
    "homepage": "https://3dneweraai.com",
    "documentation": "https://github.com/leandrodaf/3d-new-era-ai#connect-your-ai",
    "support": "https://github.com/leandrodaf/3d-new-era-ai/issues",
    "icon": "icon.png",
    "server": {
        "type": "binary",
        "entry_point": "bin/" + files[first],
        "mcp_config": {
            "command": command(first),
            "args": ["mcp"],
            "platform_overrides": {p: {"command": command(p), "args": ["mcp"]}
                                   for p in platforms if p != first},
        },
    },
    "tools": tools,
    "tools_generated": False,
    "keywords": ["floor plan", "interior design", "architecture", "home design", "3d", "cad"],
    "license": "MIT OR Apache-2.0",
    "privacy_policies": ["https://3dneweraai.com/privacy/"],
    "compatibility": {"platforms": platforms},
}
if not manifest["server"]["mcp_config"]["platform_overrides"]:
    del manifest["server"]["mcp_config"]["platform_overrides"]
json.dump(manifest, open(os.path.join(stage, "manifest.json"), "w"), indent=2, ensure_ascii=False)
PY

mcpb=(npx --yes @anthropic-ai/mcpb@2.1.2)
"${mcpb[@]}" validate "$stage/manifest.json"
mkdir -p "$(dirname "$out")"
"${mcpb[@]}" pack "$stage" "$out"
printf 'packed %s (%s)\n' "$out" "$(du -h "$out" | cut -f1)"
