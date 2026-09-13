# Roadmap

Goal: the features people rely on in Sweet Home 3D, rebuilt the right way — native,
fast, at real architectural scale, and fully drivable by AI agents through MCP at a
low token cost.

Each milestone ships three things together: **core model + commands**, **editor UI**
and **MCP tools**. A feature is not done until an agent can use it cheaply.

## ✅ M0 — Foundation (v0.1)

- [x] Workspace: core / mcp / server / app / binary
- [x] Commands with undo/redo, atomic batches, revisions
- [x] Walls and rooms; short ids; compact `[x, y]` points
- [x] Floor plan: grid, zoom at cursor, pan, snapping, wall drawing with live length
- [x] Native 3D view: extruded walls, room floors, orbit camera, MSAA
- [x] Compass (north direction) in model, plan and MCP
- [x] MCP over stdio and Streamable HTTP; REST `/api/home`
- [x] CI (fmt, clippy, tests on 3 OSes, MCP smoke test, MSRV, cargo-deny), release builds

## ✅ M1 — Working drawings

Make the plan precise enough to draw a real house from measurements.

- [x] **Background image**: import a scanned plan, calibrate scale by marking a known
      length, drag to position, opacity and visibility
- [x] Wall joins: mitered corners, T/X junctions and straight continuations
- [x] Edit walls: drag endpoints (joined walls follow), thickness, height, split, arc walls
- [x] Dimension lines with offset handles and automatic wall dimensions
- [x] Rooms: draw polygon, detect from walls (double-click), floor/ceiling/area flags,
      concave floors (ear clipping)
- [x] Text labels sized in centimeters
- [x] Magnetism: 15° angles, wall/room points, length rounding by zoom, Shift disables;
      typed exact lengths while drawing
- [x] Rulers and cursor coordinates; cm / m / mm / ft-in display
- [x] Selection (click, Ctrl+click, box), move, nudge, copy/cut/paste/duplicate
- [x] Save/load native project file (versioned JSON), recent files, unsaved-changes guard
- [x] Plan export: SVG at true scale and PNG
- [x] MCP: `create` (walls/rooms/dims/labels in one call), `update`, `move`, `delete`,
      `split_wall`, `set_home`, `set_background`, `render_plan` (PNG so agents see
      their work), `export_plan`, `save_home`/`open_home`/`new_home`
- [x] Interaction tests with `egui_kittest` (drawing, dragging, handles, dialogs, shortcuts)

## M2 — Furniture at real scale

The core of what makes the editor useful: placing real objects with real dimensions.

- [ ] Furniture model: catalog id, position, elevation, angle, width/depth/height in cm,
      mirrored, color/texture; doors and windows as a furniture subtype
- [ ] 3D model loading: OBJ/MTL and glTF; normalize to the catalog's declared size
- [ ] Catalog: categories, search, thumbnails, per-model **creator and license** metadata
- [ ] Import the Sweet Home 3D default catalog — it declares dimensions (cm), model
      and license for every item — keeping only models whose license allows
      redistribution (e.g. CC-BY), with attribution shipped in the app
- [ ] Place by drag-and-drop; rotate/resize handles; keep proportions; snap to walls
- [ ] Doors and windows cut openings in walls (plan symbol + 3D hole)
- [ ] Collision and clearance hints (e.g. 90 cm circulation in front of a door)
- [ ] MCP: `search_catalog` (paged, compact), `place_furniture` (batch),
      `update_furniture`, `list_furniture` with filters by room/category

## M3 — Levels and presentation

- [ ] Levels (floors) with elevation and height; stairs
- [ ] Textures and colors for walls, floors, ceilings
- [ ] Lighting: sun from compass + date/time, light sources from furniture
- [ ] Virtual visitor camera; stored points of view
- [ ] Photo renderer (GPU path tracing) and video along a camera path
- [ ] Export: OBJ, glTF, PDF plan (SVG/PNG plan done in M1)
- [ ] MCP: `set_camera`, `render_photo` with size/quality budget

## M4 — Interop and reach

- [ ] Import `.sh3d` files (ZIP with `Home.xml`)
- [ ] Web build (WebGPU) sharing `newera-core`
- [ ] Auth for exposing the server on a network; multi-user sessions
- [ ] Plugin API on top of commands
- [ ] i18n (UI currently in Portuguese)

## Sweet Home 3D is a reference, not a source

Sweet Home 3D (Java, GPL) is the benchmark for **what** a home design tool should
do and how it should feel. **No code is copied or translated from it** — every
feature here is designed and implemented from scratch in Rust, around the command
model and the AI/MCP workflow that are this project's goal.

Third-party assets (3D models, textures) are only included when their individual
license allows redistribution, with creator and license shipped alongside.
