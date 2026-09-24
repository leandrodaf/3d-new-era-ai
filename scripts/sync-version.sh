#!/usr/bin/env bash
# The version lives in Cargo.toml; the plugin manifests and server.json repeat
# it for the stores that read them straight from the repository. This copies
# it over (no argument), or checks that nothing drifted (--check, in CI).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
files=(plugin/plugin.json plugin/.claude-plugin/plugin.json plugin/.cursor-plugin/plugin.json
       gemini-extension.json server.json)

MODE="${1:-write}" VERSION="$version" python3 - "${files[@]}" <<'PY'
import json, os, sys
mode, version, stale = os.environ["MODE"], os.environ["VERSION"], []
for path in sys.argv[1:]:
    if not os.path.exists(path):
        continue
    data = json.load(open(path))
    wanted = dict(data, version=version)
    if path == "server.json":
        for package in wanted.get("packages", []):
            package["version"] = version
            package["identifier"] = package["identifier"].rsplit("/download/", 1)[0] + \
                f"/download/v{version}/" + package["identifier"].rsplit("/", 1)[1]
    if wanted != data:
        stale.append(path)
        if mode == "write":
            json.dump(wanted, open(path, "w"), indent=2, ensure_ascii=False)
            open(path, "a").write("\n")
if stale and mode == "--check":
    sys.exit(f"version {version} (Cargo.toml) not in: {', '.join(stale)} — run scripts/sync-version.sh")
print(f"{version}: " + ("updated " + ", ".join(stale) if stale and mode == "write" else "in sync"))
PY
