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

## M1 — Working drawings

Make the plan precise enough to draw a real house from measurements.

- [ ] **Background image**: import a scanned plan, calibrate scale by marking a known
      length, set origin, adjust opacity; per level
- [x] Wall joins: mitered corners, T/X junctions and straight continuations (walls sharing endpoints)
- [ ] Edit walls: move endpoints, thickness, height, split, arc walls
- [ ] Dimension lines with offset and automatic wall-length dimensions
- [x] Concave room floors in plan and 3D (ear clipping)
- [ ] Rooms: draw polygon, auto-detect from walls (double-click), ceiling toggle
- [ ] Text labels
- [ ] Magnetism rules: 15° angles, align to walls, length rounding by zoom level
- [ ] Rulers and cursor coordinates; metric/imperial display
- [ ] Save/load native project file (versioned, diff-friendly)
- [ ] MCP: `set_background`, `update_walls`, `create_dimension`, `detect_room`,
      `render_plan` (small PNG so agents can check their work visually)

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
- [ ] Export: OBJ, glTF, SVG/PDF plan, PNG
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
