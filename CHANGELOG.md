# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org).

## [Unreleased]

### Added

- Workspace with `newera-core`, `newera-mcp`, `newera-server`, `newera-app` and the `newera` binary.
- Reversible commands with undo/redo, atomic batches and document revisions.
- Walls, rooms and compass; short typed ids (`w12`, `r3`) and compact `[x, y]` points.
- Desktop editor in the Sweet Home 3D layout: catalog and home tree, 2D floor plan, native 3D view.
- MCP server (stdio and Streamable HTTP) with token-efficient tools; REST `GET /api/home`.
- CI for formatting, clippy, tests on Linux/macOS/Windows, MCP smoke test, MSRV and cargo-deny; release builds.
