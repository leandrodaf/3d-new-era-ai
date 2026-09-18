# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org).

## [Unreleased]

## [1.4.2] - 2026-09-18

### Added

- The AI has the loudest button in the window: a pill at the head of the top bar that
  says which of three things is true — nothing can reach this window, waiting for your
  AI, or the name of the one that is connected — and pulses as it works. Not on a phone,
  where the row has no room and the status bar already says it.
- The panel behind it was rebuilt around three cards: where the window stands (with the
  address in a well of its own and one button to copy it), the three steps with the
  clients as chips and the command in its own block, and the last calls as a clean list.

### Fixed

- A reload no longer costs the connection. The tab writes down the room it holds and
  walks back into it when the page opens, so the address already pasted into an AI client
  keeps working; the relay holds a room a quarter of an hour for its tab to come back,
  and a room that is gone is forgotten rather than pointed at.

## [1.4.1] - 2026-09-18

### Fixed

- The browser's MCP address stopped answering after a couple of quiet minutes: what
  sits in front of the relay closes a WebSocket that says nothing for about a hundred
  seconds, and from the outside that looks exactly like a tab that went away. The tab
  is pinged every thirty seconds, so an address lasts as long as the tab does.

## [1.4.0] - 2026-09-18

The editor answers to an AI from a browser tab, with nothing installed, and is
usable on a phone.

### Added

- Your AI can drive the editor **in a browser, with nothing installed**. A tab cannot
  listen on a port, so it holds a connection outwards instead: switch the MCP on in the
  AI panel (or Ctrl+Shift+M) and the tab gets an address to paste into Claude Code, Codex,
  Cursor or anything else that speaks MCP. A small relay (`newera-relay`, in this
  repository) passes the calls and stores nothing — the project never leaves the tab, the
  address stops answering the moment it is switched off or the tab is closed, and one
  room's secrets are useless on another. `render_photo` and `video` are not offered there:
  they run for minutes on a CPU and would freeze the window.
- A place in the window that shows the MCP and proves it works: the **AI** menu and a chip
  in the status bar, a panel with the address, the snippet for each client — with a button
  that registers it for you when that client's command line is installed — a sentence to
  ask, and the last twelve tool calls as they land. The window names the client that
  connected and says each tool as it is used, so a working setup and a typo no longer look
  the same.
- The AI has a place in the top bar: a robot with the name of the thing beside it, lit
  when an agent is connected and pulsing on each call, one tap from the panel that says
  how to bring one. It is the first thing in the row, which is what this editor is for.
- Made for a phone. The editor sets its scale to what the screen can hold and, below 720
  points, shows one thing at a time — catalogue, plan or 3D — with a bar in thumb's reach
  and Ctrl+1/2/3 for whoever has a keyboard; the toolbar scrolls instead of falling off the
  edge. On the site every tappable thing gets the 44 px a thumb actually hits and the small
  print goes up a step. `scripts/mobile-audit.mjs` checks all of it at four real handset
  sizes, and CI fails on sideways scroll, a target too small or text too small to read.
- Every MCP tool can be called without a session (`newera_mcp::call`), which is what lets
  the browser answer for itself; a test walks the server's own tool list and fails if
  anything is unreachable, so the two cannot drift.

- The window speaks Spanish and French besides Portuguese and English: pick one in
  Help > Idioma / Language, or let it follow the system (`NEWERA_LANG`, `LC_ALL`,
  `LC_MESSAGES`, `LANG`, and the browser's language in the web editor) the first time
  it opens. The installers, the Linux desktop entry, the `.newera` file type and the
  macOS bundle speak the four as well.

- A visual identity of its own, taken from the site: a drafting studio in warm
  graphite with one blueprint blue to point with, day and night themes chosen in
  Ver › Tema (or left to the system), technical readouts set in mono, and the plan
  drawn on paper by day and on a lit board by night. Rulers, dialogs and the dimming
  behind them follow the theme; exports keep their own white paper.
- The left panel rebuilt: one search well, bands that open on a click anywhere on
  their line and count what is inside, filter keys per kind of element, and one row
  shape everywhere with the accent down the side of what is selected.
- What is picked on the plan lights up in the panel and is scrolled into view, with
  the band holding it opened.
- The catalog speaks the four languages too: every piece, every wall type and
  every finish is named in the language of the window, a piece dropped on the plan
  is created under that name, and the search finds it by the word on screen.
- The browser editor opens on a furnished demo home instead of an empty sheet, says in
  the visitor's language when a browser cannot start it, and is checked by CI in a real
  headless Chrome: it builds the page, draws a wall with the mouse and loads the viewer,
  failing on anything the page logs.
- It runs where there is no WebGPU: with wgpu's WebGL backend compiled in, a browser
  without WebGPU — Firefox today — falls back to WebGL 2 and still draws the plan and the
  3D view. The page says what happened, since Chrome and Edge are the supported browsers
  and Firefox is not guaranteed, and CI now draws the wall twice: once with WebGPU and
  once without.
- The site header fits a phone: the language switch and the GitHub star count moved into
  the menu instead of pushing the row off the screen.
- Found by search and by agents: schema.org data for the program and the questions,
  hreflang for the four languages, a sitemap, a robots.txt that welcomes the crawlers
  behind AI assistants, and an llms.txt that tells an agent what the program is and what
  it can be asked to do. The site header holds up on a phone, with the sections in a menu
  and the GitHub stars in the corner.
- The site opens in the visitor's own language — Portuguese, English, Spanish or
  French, read from the browser and overridable with `?lang=` or the switch in the
  corner — with the picture descriptions and the link preview to match.
- Full trackpad support: pinch to zoom, two fingers to move the plan or to turn the
  3D view (Shift to slide it), a twist to spin it, and momentum carried through —
  a wheel still zooms, told apart from a glide by how the machine reports it.

- The editor in the browser keeps what was drawn: the project is mirrored into the
  browser's own storage a moment after every change and opened again on the next visit,
  so closing a tab is no longer the same as throwing the drawing away. A test draws a
  wall, reloads the page and fails if it does not come back.
- The site, the editor and the viewer are published at https://3dneweraai.com — landing
  page at the root, editor at `/app/`, viewer at `/viewer/` — from one build script, with
  a workflow that puts them on Cloudflare Pages on every push.

### Fixed

- The browser editor stopped loading for anyone who had opened it before: the glue and the
  wasm keep the same names and a cache can hold one half of each build, which does not
  link. Both are stamped with the digest of the build now, so a cache can only ever serve a
  matching pair, and a mismatch that gets through anyway is fetched again past every cache.
- The row of plan versions no longer runs into the discipline, the layers and the storeys
  on a narrow screen: there the three go behind one button, with "compare versions" beside
  them, and the sheets scroll under it.
- Switching the browser's MCP off really closes it: the socket was kept inside the
  state, so a link that had fallen over left nothing to hang up and the old address went
  on answering while a new one was on screen. The socket is held beside the state now and
  closed whatever the state says.
- The console no longer fills with "Unable to preventDefault inside passive event
  listener": the editor's canvas and the viewer say they handle their own gestures, which
  is also what stops a phone scrolling the page while a finger drags the plan.
- The note about WebGL no longer covers the editor's own menus — on a phone that was most
  of the window — and goes on its own after it has been read.

- The window follows the language of the machine it runs on, which made the interface
  tests read English labels on a macOS runner set to `en_US` and fail the whole suite:
  under test it now speaks the language the code is written in, whatever the machine says.

- Dialogs read as part of the program: one width, a titled head, fields grouped under
  mono headings, a body that scrolls instead of running off the screen, and one footer
  with the action filled in the accent and the way out beside it.
- Texts that never had a translation — the tool names in the toolbar and the Plan
  menu, and the status bar lines about opening, saving, exporting and importing —
  now follow the chosen language instead of staying in Portuguese.

## [1.3.0] - 2026-09-16

The electrical, telecom and plumbing projects, checked against the text of the
norms, and a long round of fixes found by an agent working a real 65 m²
apartment through the MCP server.

### Added

- Electrical project: network, TV, Wi-Fi and telecom panel points in the catalog;
  circuits assigned per point or the whole division in one call (MCP `electrical`
  `assign`), a load schedule and the NBR 5410 checks, drawn on the plan with circuit
  numbers and opened from the discipline menu in the editor.
- The supply is suggested by Enel SP's categories (single-, two- or three-phase and its
  entry breaker); the panel is filled in DIN modules with DR, surge protector and the
  spare ways NBR 5410 asks, and says when it does not fit.
- Runs laid the way they are built: along walls, inside the slab or the floor, by the
  cheapest tree from the panel, with their bill of materials (conduit, boxes, wire per
  conductor, cable, connectors), voltage drop and conduit grouping. Cables drawn by hand
  are measured and replaced by the routed run.
- Adhesive flat wiring tape (Eletrofitas) as a way to lay power runs: the model by the
  load and by whether a socket needs earth, refused in wet rooms and over its rating,
  bought as Leroy Merlin kits with codes, prices and splices.
- Wi-Fi access points: coverage per room and band from walls, doors, windows, slabs and
  low-e glass; a suggested placement; standard, PoE, uplink and cable category checks.
- Automation devices (smart relay, smart switch, dimmer, presence sensor, smart lock),
  with their standby load and what each needs (neutral in the box, dimmer load, sensor
  height and reach).
- Outlets built into furniture: pop-up towers (60, 85 and 100 mm), desk boxes and panel
  outlets, modelled in 3D, seated on the top they go into and checked for edge distance,
  room below, sink and cooktop distance and the socket a plug-in tower needs.
- Plumbing project: cold, hot, sewer, vent and gas points; drains as the models they are
  (sizes, flow, fixed in the floor), vent branches routed, gas grilles drawn, and rules
  from NBR 8160, NBR 5626, NBR 13103 and São Paulo's sanitary code.
- Balcony guards told apart: iron railings, glass guards and glass closures, checked for
  height (1,10 m), bar gap (11 cm), laminated glass, and a closure that is no guard; MCP
  `place`/`update` take `glass` and `gap`.
- Lighting, appliances and joinery are layers of the plan, shown or hidden on the plan
  and in 3D; hiding the electrical project hides all of it, lamps included.
- Ergonomics: single beds side by side, accessible tops and beds, gas appliances in
  bathrooms and bedrooms; a fix says what it leaves behind, and two findings that cannot
  both fit say so.
- MCP: `move` takes `to=[x,y]`; `place` copies a piece already in the project;
  `cut_list` reads joinery drawn by hand; a part of a group can be renamed; findings of
  every discipline can be accepted with a reason, and an acceptance that outlived its
  finding is listed, pruned or pointed to the finding that took its place; deleting a
  piece names the labels left pointing at it; `update fixed` declares a piece fixed or
  free-standing when its name does not say.
- Crash reports to Sentry in released binaries, on by default and off from the Help menu.
- Agents can report what a tool could do better, with the whole case.

### Changed

- Every figure and citation was checked against the norm texts: NBR 5410 (lighting load
  by area, kitchen and bathroom outlets, DR, grouping, 4 % drop), NBR 16264, NBR 8160,
  NBR 5626, NBR 13103, NBR 15575-1 annex F, NBR 9050, NBR 14718, NBR 7199 and São
  Paulo's building code (Decreto 57.776) — see `docs/NORMAS.md`.

### Fixed

- A fixed point (outlet, switch, RJ45, Wi-Fi point, drain) is set on its structure —
  wall, ceiling, floor or the top it is built into — never loose in a room, on glass, in
  a door or window span, too close to a frame or inside an appliance; the refusal names
  the nearest free place.
- A piece is classified by what it is, not by a word in its name: a cabinet or countertop
  named after the appliance beside it is joinery, cabinet parts named after the sink are
  not sinks, and "Lavanderia" is no "lava".
- A reply names every change it made, and a change that changes nothing says so with the
  value already there; `update` refuses a field that does not exist.
- Groups resized keep their boards and the face they declare; dimensions held by a part
  follow it; reference numbers stay with their piece; a door nudged along its wall keeps
  its side.
- A painted wall no longer covers the door in it; the load schedule's findings scroll.
- The repository checks out on Windows again (`scripts/aux` was a reserved name there).

## [1.2.0] - 2026-09-15

### Changed

- A rule advises, it never refuses to draw: what the workshop would say travels with the
  drawing in `notes`, and a request that cannot be built as asked builds the nearest thing
  and says so. Only what has no geometry at all fails.

### Added

- The editor says what long work is doing, and it can be walked away from.
- `faces` is read from what was built into a piece (doors, drawer fronts, kick), a
  dimension that holds onto what it marks is measured again on every change, findings carry
  their `weight`, and `dry` can answer `"summary"`.

### Fixed

- An appliance that stopped fitting its niche is reported by `check_layout`.
- Walls join where they touch, not only where they end.
- Light and air cross rooms open to each other and through glass; notes that no longer
  match their piece are reported by `stale`.

## [1.1.0] - 2026-09-15

### Added

- Landing page on GitHub Pages, in Portuguese and English.
- The mark as a real icon set on macOS, Windows and Linux; installers speak the user's
  language.
- MCP answers what an agent actually asks: measure, classify, preview.

### Changed

- The MCP tools moved out of one file into a module per tool, behind a test that freezes
  the tool surface.

### Fixed

- `cargo run -p newera` picks the editor binary.
- Installers find the latest release without the GitHub API.

## [1.0.0] - 2026-09-14

First release: the desktop editor, the browser editor and viewer, the HTTP API and the
MCP server, with everything below.

### Added

- Downloads for Windows, macOS (Apple Silicon and Intel) and Linux on every release, with
  one-line installers that need no administrator: `install-windows.ps1` (Start menu and
  desktop shortcuts, user `PATH`, listed in Settings > Apps) and `install-macos.sh`
  (`3D New Era AI.app`, ad-hoc signed; builds from source when there is no binary).
  Both skip what is already installed, clean up after themselves and register the MCP
  server in Claude Code and Codex. The README shows how to connect Gemini CLI, VS Code,
  Cursor, Windsurf, Claude Desktop and MCP apps for other models. `newera-gui.exe` opens the editor without a console window.

- Smaller `.newera` files: every project is a deflated bundle with compact JSON, and large
  JPEG textures are recompressed once on save (quality 85, at most 2048 px). The showcase
  apartment went from 12.3 MB to 2.7 MB. Plain JSON projects from earlier saves still open.

- MCP `lighting`: lux per room by photometry against NBR ISO/CIE 8995-1; `fill` places the
  fixtures a room needs. Photos get a half-strength white balance and render on half the
  cores by default (`NEWERA_RENDER_THREADS`).

- Smart guides while moving pieces on the plan: edges and centers line up with other pieces,
  rooms and wall faces (dashed lines show what they lined up with), equal gaps between two
  neighbors, and live distances to what is around. Shift turns the magnet off.

- Walls, glass and panels fitted to the roof above them (MCP `fit_roof`, Planta > Ajustar ao
  telhado): under an A-frame a partition is split at the ridge with sloping tops and a glass
  gable becomes a triangle; they keep following the roof in the same undo step.

- MCP `embed`: a sink bowl or cooktop set into a joinery countertop with a cutout sized
  from the real item, or an oven, microwave or other appliance into a cabinet niche (boards
  around it, doors above and below, several niches per tower). The item becomes part of the
  host and follows it through moves, edits and cabinet runs; errors give the size to use.
  New catalog items: cooktop, sink bowl and built-in oven.

- "Armários na parede…" in the editor (Planta menu, one wall selected): base, wall or tall
  row, sink and cooktop positions, front color and drawer units, with a preview of the
  modules before building them. The planning moved into `newera-joinery`, shared with MCP.

- "Ergonomia…" window in the editor (Planta menu): the habitability score and findings for
  the occupants you set, kept current while you edit; a finding selects what it is about and
  its checked fix applies in one click.

- MCP `ergonomics`: a review of the plan for the people who live there (occupants, children,
  elderly, wheelchair, the cook's height). Room to walk beside beds and in front of the stove,
  beds, seats, bathrooms and wardrobes per person, the kitchen triangle and heights, doors that
  hit furniture, ceiling heights, windows, minimum furniture and wheelchair turning space, each
  finding with its numbers, the fix and the Brazilian reference (NBR 9050, NBR 15575-1, IBGE).

- MCP `cabinet_run` fills a wall with cabinets sized for it: the free stretches between
  corners, doors (and their swing), windows, fridge and stove become even modules with no
  useless leftovers — pull-outs for 15–30 cm, fillers below — a drawer unit by the stove,
  countertops, a cabinet over the fridge, a gap for the hood, and blind corner modules where
  two runs meet in an L (the other wall is planned again in the same undo step).

- Joinery for AI-driven design: MCP `joinery` builds modular cabinets, slatted panels,
  countertops with sink/cooktop cutouts, plaster coves with LED, shadow gaps and modular
  sofas from flat parameters, edits them incrementally by id and explains broken workshop
  rules with the value to use; `cut_list` exports CSV and DXF/SVG sheet layouts.

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

[Unreleased]: https://github.com/leandrodaf/3d-new-era-ai/compare/v1.4.2...HEAD
[1.4.2]: https://github.com/leandrodaf/3d-new-era-ai/compare/v1.4.1...v1.4.2
[1.4.1]: https://github.com/leandrodaf/3d-new-era-ai/compare/v1.4.0...v1.4.1
[1.4.0]: https://github.com/leandrodaf/3d-new-era-ai/compare/v1.3.0...v1.4.0
[1.3.0]: https://github.com/leandrodaf/3d-new-era-ai/compare/v1.2.0...v1.3.0
[1.2.0]: https://github.com/leandrodaf/3d-new-era-ai/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/leandrodaf/3d-new-era-ai/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/leandrodaf/3d-new-era-ai/releases/tag/v1.0.0
