# Architecture

## The one rule

**Every change to a home is a `Command` executed by a `Document`.**

The desktop UI, the MCP server and the REST API are all just clients of that rule.
That is what makes these properties hold everywhere, for free:

- undo/redo works the same for a mouse click and for an agent's tool call;
- batches are atomic: if one step fails, nothing is applied;
- every client sees changes from every other client, because they share one document.

```
                    ┌───────────────────────────────┐
                    │          newera-core          │
                    │  Home ─ Command ─ Document    │
                    │  (model)  (reversible) (history, revision)
                    └──────────────┬────────────────┘
                                   │ SharedDocument (Arc<RwLock<Document>>)
          ┌────────────────────────┼────────────────────────┐
┌─────────┴─────────┐   ┌──────────┴─────────┐   ┌──────────┴─────────┐
│    newera-app     │   │     newera-mcp     │   │   newera-server    │
│ egui + wgpu       │   │ rmcp tools         │   │ axum: /api, /mcp   │
│ plan 2D · view 3D │   │ stdio / HTTP       │   │                    │
└─────────┬─────────┘   └──────────┬─────────┘   └──────────┬─────────┘
          └────────────────────────┴────────────────────────┘
                                   │
                          ┌────────┴────────┐
                          │     newera      │  binary: picks the mode,
                          │  (main thread:  │  window on the main thread,
                          │   window)       │  servers on a tokio runtime
                          └─────────────────┘
```

## Crates

| Crate | Depends on | Responsibility |
|-------|-----------|----------------|
| `newera-core` | serde, schemars | Model (`Home`, `Wall`, `Room`, `Compass`), geometry, `Command`, `Document`. No UI, no async, no I/O. |
| `newera-mcp` | core, rmcp | MCP tools and their token-efficient wire format. |
| `newera-server` | core, mcp, axum | HTTP transport: REST API and the Streamable HTTP MCP endpoint. |
| `newera-app` | core, eframe | Desktop editor. Never talks to the network. |
| `newera` | all | CLI entry point and process wiring. |

Dependencies only point downwards. `newera-core` must stay free of heavy
dependencies so it can later compile to WebAssembly and power a web client.

## Units and coordinates

- Plan units are **centimeters** (`f64`), matching Sweet Home 3D and architectural drawings.
- Plan axes: **x grows right, y grows down** (screen-like, as in Sweet Home 3D).
- 3D view: plan `(x, y)` cm maps to world `(x, height, y)` **meters**, Y up.
- The compass `north_degrees` is clockwise from plan up (−y) to geographic north.

## Commands and history

`Command::apply(self, &mut Home) -> Result<Command>` applies a change and returns
its inverse. `Document` keeps two stacks of inverses:

```
execute(c): inverse = c.apply(home); undo.push(inverse); redo.clear()
undo():     c = undo.pop(); redo.push(c.apply(home))
redo():     c = redo.pop(); undo.push(c.apply(home))
```

`Command::Batch` applies its children in order and, on the first error, applies the
inverses of what already ran, so the home is untouched. Removals remember their index,
so undo restores the original order exactly.

`Document::revision` increments on every successful change. Views compare it to
decide when to rebuild derived data (the 3D mesh is rebuilt only when it changes).

## Ids

Ids are short and typed: `w12` (wall), `r3` (room), and more prefixes as element
types are added. They come from one monotonic counter stored in `Home`, so an id is
**never reused**, even after undo. An agent can safely hold on to an id it saw earlier.

## MCP design: tokens are a budget

Agents pay for every byte they read and write, so the protocol surface is designed
like an API for a slow network:

| Principle | Example |
|-----------|---------|
| Short ids | `w12` instead of a 36-character UUID |
| Compact geometry | `[800, 0]` instead of `{"x": 800.0, "y": 0.0}` |
| Omit defaults | walls include `t`/`h` only when not 15/250 cm |
| Round numbers | 0.1 cm precision; integers without `.0` |
| Writes don't echo state | `ok rev=7 ids=w8,w9` |
| Coarse-grained tools | one `create_walls` polyline instead of N `create_wall` calls |
| Atomic multi-target tools | `delete` takes many ids of any type |

The REST API (`/api/home`) serves the full, self-describing model for tooling and
debugging; MCP serves the compact view.

## Desktop editor

- `eframe`/`egui` with the `wgpu` backend.
- Layout follows Sweet Home 3D: catalog and home tree on the left, floor plan on top,
  3D view below; both splitters are resizable.
- The 3D view renders into its **own offscreen texture** (MSAA + depth) that egui shows
  as an image. Owning the pass keeps 3D rendering independent from egui's renderer.
- The app polls the document revision a few times per second while idle, so edits
  from MCP/HTTP appear without user input.

## Threads

The window must own the main thread (a macOS requirement). The binary starts a tokio
runtime on a background thread for the HTTP/MCP server, binds the port **before**
opening the window so a busy port fails loudly, and cancels the server when the
window closes.

## Security

The server binds to `127.0.0.1` by default. The MCP endpoint validates the `Host`
header against loopback names to block DNS rebinding from web pages. Exposing it on
a network interface is an explicit choice (`--addr`) and should come with auth
(tracked in the roadmap).
