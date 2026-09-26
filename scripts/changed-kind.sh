#!/usr/bin/env bash
# Sorts a change into prose and code, so each workflow can skip the half that
# does not concern it.
#
# The filtering has to happen here rather than in a workflow's `paths:` filter:
# a workflow that never starts reports no status at all, and a required check
# that never reports blocks the pull request forever. A job that is skipped
# reports "skipped", which counts as a pass. So both workflows always run, and
# their jobs ask this script what to do.
#
# Prints `code=true|false` and `docs=true|false` for $GITHUB_OUTPUT. Reads the
# changed paths from the arguments, or works them out from the event. When it
# cannot tell what changed it says both are true: running everything wastes a
# few minutes, skipping wrongly ships something broken.
#
#   scripts/changed-kind.sh README.md        # docs=true  code=false
#   scripts/changed-kind.sh --self-test      # the table below, as a test
set -euo pipefail

# Prose: no compiler, linter or test in this repository reads any of it. The
# plugin's Markdown is deliberately absent — the Format job runs
# `plugin validate`, which reads every SKILL.md.
prose='^([^/]+\.md|docs/.+|LICENSE-[^/]+|\.github/ISSUE_TEMPLATE/.+|\.github/pull_request_template\.md|\.coderabbit\.yaml|\.github/workflows/docs\.yml|\.markdownlint-cli2\.jsonc|\.github/\.markdownlint-cli2\.jsonc|_typos\.toml|lychee\.toml)$'

# What the Docs workflow has something to say about: Markdown anywhere, and the
# configuration of the three checks that read it.
docs='^(.+\.md|\.markdownlint-cli2\.jsonc|\.github/\.markdownlint-cli2\.jsonc|_typos\.toml|lychee\.toml|\.github/workflows/docs\.yml)$'

classify() {
  local files=("$@") code=false has_docs=false f
  for f in "${files[@]}"; do
    [[ -n "$f" ]] || continue
    [[ "$f" =~ $prose ]] || code=true
    [[ "$f" =~ $docs ]] && has_docs=true
  done
  echo "code=$code"
  echo "docs=$has_docs"
}

# Cannot tell what changed: run everything rather than skip something that
# needed building.
unknown() {
  echo "code=true"
  echo "docs=true"
}

self_test() {
  local failed=0
  check() { # check <expected code=/docs=> <paths...>
    local want="$1"; shift
    local got
    got="$(classify "$@" | tr '\n' ' ')"
    got="${got% }"
    if [[ "$got" != "$want" ]]; then
      echo "FAIL  $*"
      echo "      want: $want"
      echo "      got:  $got"
      failed=1
    fi
  }

  # Prose only: the compiler stays home, the Docs workflow runs.
  check "code=false docs=true" README.md
  check "code=false docs=true" docs/ARCHITECTURE.md docs/README.md
  check "code=false docs=true" .github/ISSUE_TEMPLATE/bug_report.md
  check "code=false docs=true" _typos.toml
  # A licence reads as prose, but no docs check has anything to say about it.
  check "code=false docs=false" LICENSE-MIT
  check "code=false docs=false" .coderabbit.yaml
  # Code only.
  check "code=true docs=false" crates/newera-core/src/levels.rs
  check "code=true docs=false" Cargo.toml Cargo.lock
  check "code=true docs=false" .github/workflows/ci.yml
  check "code=true docs=false" Makefile
  check "code=true docs=false" scripts/changed-kind.sh
  # The plugin's Markdown is both: `plugin validate` reads it, and so does
  # markdownlint.
  check "code=true docs=true" plugin/skills/design-a-home/SKILL.md
  check "code=true docs=true" crates/newera-mcp/README.md
  # A mixed change runs both halves.
  check "code=true docs=true" README.md crates/newera-core/src/levels.rs
  # Both sides of a rename, which is what --no-renames reports: a source file
  # moved into docs/ still has to build.
  check "code=true docs=true" crates/newera-core/src/levels.rs docs/levels.md
  # An empty change touches neither half.
  check "code=false docs=false" ""

  if [[ $failed -eq 0 ]]; then
    echo "changed-kind: all cases pass"
  else
    exit 1
  fi
}

if [[ "${1:-}" == "--self-test" ]]; then
  self_test
  exit 0
fi

if [[ $# -gt 0 ]]; then
  classify "$@"
  exit 0
fi

# No arguments: work out what changed from the event.
case "${GITHUB_EVENT_NAME:-}" in
  pull_request | pull_request_target)
    base="origin/${GITHUB_BASE_REF:?}"
    ;;
  push)
    before="${GITHUB_EVENT_BEFORE:-}"
    # All zeroes on a branch's first push, and absent outside Actions.
    if [[ -z "$before" || "$before" =~ ^0+$ ]]; then
      unknown
      exit 0
    fi
    base="$before"
    ;;
  *)
    unknown   # workflow_dispatch and anything else
    exit 0
    ;;
esac

if ! git rev-parse --verify --quiet "$base" >/dev/null; then
  unknown           # shallow clone, or a base that was rewritten
  exit 0
fi

# --no-renames: with detection on, a rename reports only where the file
# landed, so moving a .rs into docs/ would read as prose and skip the
# build. Off, it reports both sides.
mapfile -t changed < <(git diff --no-renames --name-only "$base"...HEAD)
classify "${changed[@]:-}"
