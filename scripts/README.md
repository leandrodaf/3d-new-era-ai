# Scripts

Helpers for building, testing, installing and releasing. Most of them are
called by the [Makefile](../Makefile) or by CI; each file starts with a comment
saying what it does and how to run it.

## Development

| Script | What it does |
|---|---|
| `mcp.sh` | Calls one MCP tool on a running editor or server and prints the reply (`make mcp TOOL=get_home`). |
| `make-icons.py` | Renders every icon the project ships (macOS, Windows, Linux) from one mark (`make icons`). |

## Tests

| Script | What it does | Run by |
|---|---|---|
| `mcp-smoke.sh` | Boots `newera serve`, talks MCP like an AI client and checks the result through REST. | `make check`, CI |
| `web-editor-e2e.mjs` | Draws a wall in the browser editor in headless Chrome (WebGPU and WebGL) and loads the viewer. | CI, `publish.yml` |
| `web-mcp-e2e.mjs` | The browser tab as an MCP server: an AI client reaches it through the relay and edits its project. Imports `web-diagnostics-test.mjs`. | CI, `publish.yml` |
| `web-diagnostics-test.mjs` | Unit test for the diagnostics the browser editor records when it fails to start or crashes. | `web-mcp-e2e.mjs` |
| `mobile-audit.mjs` | Loads a page at phone sizes and reports sideways scrolling, overflow, small text and small tap targets. | CI |
| `chrome-session.mjs` | Launches a disposable headless Chrome for the browser tests. Tested by `chrome-session.test.mjs`. | the tests above |
| `fixtures/render-assets.py` | Builds a demo bundle with imported model and texture assets for the browser tests. | `web-mcp-e2e.mjs` |

## Installers

| Script | What it does |
|---|---|
| `install-macos.sh` | Installs on a Mac for the current user, from a release or built from source. |
| `install-windows.ps1` | Installs on Windows for the current user. |
| `install-desktop.sh` | Adds the Linux menu entry, icons and the `.newera` file type. |
| `macos-app.sh` | Wraps a built binary as `3D New Era AI.app`, signed ad hoc. |

The installers speak the user's language (English, Portuguese, Spanish or
French), so their messages are in all four.

## Release and distribution

See [docs/DISTRIBUTION.md](../docs/DISTRIBUTION.md) for how these fit together.

| Script | What it does | Run by |
|---|---|---|
| `sync-version.sh` | Copies the `Cargo.toml` version into `server.json` and the plugin manifests; `--check` fails on drift. | by hand, CI |
| `release-notes.sh` | Prints a version's `CHANGELOG.md` section for the GitHub release. | `release.yml` |
| `mcpb.sh` | Packs the MCP Bundle (`.mcpb`) for Claude Desktop and the MCP Registry. | `release.yml` |
| `registry-publish.sh` | Publishes the release to the Official MCP Registry. | `release.yml` |
| `registry-key.sh` | Once per domain: creates the key that proves ownership of `3dneweraai.com` to the registry. | by hand |
| `winget.sh` | Writes the winget manifests of a release into `target/packaging/`, for a manual submission; releases go through winget-releaser. | by hand |
| `homebrew.sh` | Writes the Homebrew cask of a release (to `target/packaging/` unless given a path). | `release.yml` |
| `site-build.sh` | Builds the website, the browser editor and the viewer into `_site/`. | `publish.yml` |
