# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org).

## [Unreleased]

### Added

- Plugins (external programs over the HTTP API) and multi-user sessions with presence,
  named cursors on the plan and conflict detection.

- Full editor in the browser on WebGPU (`make web-editor`, `/editor/`).

- Live sun in the 3D view from the compass and hour; labels and dimensions shown in
  3D (dialogs and MCP `in3d`/`elev`/`pitch`).

- Video along the camera path (Catmull-Rom, Motion-JPEG AVI): "Criar vídeo" window
  and MCP `video`.

- English interface (Ajuda › Idioma / Language).

- Web viewer in WebAssembly (`web/`, `make web-serve`).

- Symbol legend for electrical/plumbing; sloping walls, baseboards and elevated
  polylines in 3D.

- Server: token auth for network exposure, SSE revision events, REST commands endpoint.

- Furniture top views on the plan (Ver › Móveis na planta), rendered in the background.

- Photo renderer (path tracing with sun, lamps, glass, denoising): app window and MCP
  `render_photo`.

- Exports: glTF binary and OBJ/MTL 3D models, vector PDF plans at scale.

- `newera-render`: software 3D renderer; MCP `render_3d` and REST `/api/view.png`;
  precise dimension magnet; dimensions by intent over MCP; compacted tool schemas.

- Engineering dimension chains and room reference schedules (tags, sizes, brand/model/
  link) drawn on the plan; MCP `annotations`.

- Electrical and plumbing projects over the plan (disciplines, symbols, line tool,
  quantities, MCP `disciplines`); points of view with visitor camera and MCP `cameras`;
  polyline and label style editing; groups carry their pieces.

- Sweet Home 3D import (`.sh3d`) with a lossless model: polylines, text styles, cameras,
  environment, print settings, furniture metadata, groups, lights, sashes, wall
  cut-outs, level layouts; project bundles (`.newera` ZIP with assets).

- Wall types (drywall, masonry, concrete, glass…) and surface finishes for wall sides,
  floors and ceilings: procedural GPU patterns and image textures at real size; room
  ceilings in 3D; MCP `materials` and `type`/`left`/`right`/`sides`/`floor_mat`/`ceil_mat`.

- Levels: storeys with elevation/height/slab, level selector, faint reference of the
  storey below, stacked 3D with stair openings; MCP `levels`.

- Plan variants as tabs: duplicate (Ctrl+T), switch (Ctrl+Tab), rename, close, compare
  versions; project format v2; MCP `variants`.

- M2 furniture at real scale: parametric catalog (55 items, 3D + plan symbols), OBJ/glTF
  import, catalog panel with search, placement with ghost, rotate/resize handles, doors
  and windows cutting walls, layout checks, furniture dialog; MCP `catalog`, `place`,
  `check_layout`.

- M1 working drawings: background image calibration, arc walls, wall splitting and
  endpoint handles, dimensions, labels, room detection from walls, magnetism with typed
  lengths, rulers, units, selection with move/copy/paste, project files and plan export.
- `newera-draw`: one plan scene for the editor, PNG renders (MCP `render_plan`) and SVG.
- Consolidated, token-lean MCP tools (`create`, `update`, `move`, …).
- UI interaction tests with `egui_kittest`.

- Workspace with `newera-core`, `newera-mcp`, `newera-server`, `newera-app` and the `newera` binary.
- Reversible commands with undo/redo, atomic batches and document revisions.
- Walls, rooms and compass; short typed ids (`w12`, `r3`) and compact `[x, y]` points.
- Desktop editor in the Sweet Home 3D layout: catalog and home tree, 2D floor plan, native 3D view.
- MCP server (stdio and Streamable HTTP) with token-efficient tools; REST `GET /api/home`.
- Wall joins (miters, T/X junctions) and concave room floors, shared by plan and 3D.
- `scripts/mcp.sh` / `make mcp` to call MCP tools from the shell.
- CI for formatting, clippy, tests on Linux/macOS/Windows, MCP smoke test, MSRV and cargo-deny; release builds.
