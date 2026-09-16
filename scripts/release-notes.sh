#!/usr/bin/env bash
# Prints the CHANGELOG.md section of a version, for the GitHub release notes.
#   scripts/release-notes.sh v1.3.0 > notes.md
# Fails when the version has no section, so a release is never published
# with notes nobody wrote.
set -euo pipefail
version="${1#v}"
changelog="${2:-CHANGELOG.md}"
notes="$(awk -v v="$version" '
  /^## \[/ { if (on) exit; on = index($0, "## [" v "]") == 1; next }
  /^\[[^]]+\]: / { if (on) exit }
  on { print }
' "$changelog" | sed -e '/./,$!d' | sed -e ':a' -e '/^\n*$/{$d;N;ba' -e '}')"
if [ -z "$notes" ]; then
  echo "CHANGELOG.md has no section for $version" >&2
  exit 1
fi
printf '%s\n' "$notes"
