# 3D New Era AI

[![CI](https://github.com/leandrodaf/3d-new-era-ai/actions/workflows/ci.yml/badge.svg)](https://github.com/leandrodaf/3d-new-era-ai/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

An open-source home design editor in Rust, inspired by Sweet Home 3D, that is
**AI-native from day one**: the editor ships with a built-in
[Model Context Protocol](https://modelcontextprotocol.io) server, so an AI agent
can design alongside you and every change shows up live on screen.

![Editor: catalog on the left, floor plan on top, native 3D view below](docs/images/editor.png)

## Why

- **Fast and light.** Native Rust, GPU rendering through `wgpu` (Vulkan, Metal, DirectX 12).
- **One command, everything running.** The window, the REST API and the MCP server
  start together and share the same document.
- **AI as a first-class client.** People and agents go through the same undoable
  commands. An agent's edit is one Ctrl+Z away.
- **Cheap for agents.** Short ids (`w12`), `[x, y]` points, compact reads and
  one-line write replies keep token usage low.
- **Real-world scale.** Everything is in centimeters, like architectural drawings.

## Quick start

Requires Rust 1.95+ ([rustup](https://rustup.rs)). On Linux you may also need
`libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev`.

```sh
git clone https://github.com/leandrodaf/3d-new-era-ai
cd 3d-new-era-ai
make run          # editor + HTTP + MCP, with a sample house
```

Run `make` to see every development command.

## Connect an AI agent

With the editor open, the MCP endpoint is `http://127.0.0.1:7878/mcp`.

```sh
# Claude Code
claude mcp add --transport http newera http://127.0.0.1:7878/mcp
```

Clients that spawn a process can use stdio instead:

```json
{ "mcpServers": { "newera": { "command": "newera", "args": ["mcp"] } } }
```

| Tool | What it does |
|------|--------------|
| `get_home` | Compact snapshot of walls, rooms and north direction |
| `create_walls` | Connected walls along a polyline, as one undoable step |
| `create_room` | Named room from a floor polygon, with area |
| `delete` | Delete walls/rooms by id, atomically |
| `set_compass` | Set north direction, position and size |
| `rename_home` | Rename the project |
| `undo` / `redo` | Shared history with the user |

## Modes

```sh
newera               # desktop editor with embedded HTTP + MCP server
newera --no-server   # editor only
newera serve         # headless HTTP + MCP server
newera mcp           # MCP over stdio
```

Options: `--addr 127.0.0.1:7878` (or `NEWERA_ADDR`), `--demo`, log filter via `NEWERA_LOG`.
The server binds to loopback by default and validates the `Host` header.

## Project layout

```
crates/
  newera-core     domain model, commands, undo/redo — no UI, no I/O
  newera-mcp      MCP tools (rmcp), stdio and Streamable HTTP
  newera-server   axum: REST API + MCP endpoint
  newera-app      desktop editor: egui + wgpu
  newera          the binary that wires everything together
```

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for how it fits together and
[docs/ROADMAP.md](docs/ROADMAP.md) for what comes next.

## Contributing

Contributions are welcome — see [CONTRIBUTING.md](CONTRIBUTING.md). `make check`
runs the same checks as CI.

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT), at your option.
