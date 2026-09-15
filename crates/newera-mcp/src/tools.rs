//! MCP tool surface. Each tool is a thin adapter over [`crate::edit`] or
//! `newera-core`; all of them share the document the editor is showing.

use std::path::PathBuf;

use base64::Engine as _;
use newera_core::{Command, Compass, Document, Home, Point2, SharedDocument, ops};
use newera_draw::{RenderOptions, SceneOptions, SvgOptions, plan_scene, render_png, to_svg};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo};
use rmcp::{ErrorData, ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::compact;
use crate::edit::{self, BackgroundParams, CreateParams, PlaceSpec, UpdateSpec};

const INSTRUCTIONS: &str = "\
Home design editor, live in the user's window. Units: cm. Plan axes: x right, y down. \
Points are [x,y]. Id prefixes: w wall, r room, d dimension, t label, f furniture/door/window, lv storey. \
All kinds share one id counter, and composite pieces (roofs, joinery, cabinet runs) also number their \
parts, so ids have gaps: use the ids a reply returns, never guess the next one. \
Reads omit defaults (wall t=15 h=250). Writes reply `ok rev=N [ids=...]`; don't re-read \
unless needed. Every change is one undoable step. Use render_plan to check visually. \
A project can hold several plan versions (variants tool); tools act on the active one. \
Finishes are short strings: `#rrggbb` paint, a pattern like `tiles #ffffff 60x60 r45` \
(tint, tile cm, rotation) or `img:path 90x90`; `none` clears. Wall types and patterns: materials tool.";

/// The MCP server. Cheap to clone: it only holds a handle to the document.
#[derive(Debug, Clone)]
pub struct NewEraMcp {
    document: SharedDocument,
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct FitRoofParams {
    /// Walls and pieces (glass, panels) to fit.
    ids: Vec<String>,
    /// Lowest sloping surface that counts, cm above the floor (default 5).
    above: Option<f64>,
    /// Stop following the roof (heights stay as they are).
    #[serde(default)]
    off: bool,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct LightingParams {
    /// Room id (default: every room of the current storey).
    room: Option<String>,
    /// Work plane height cm (default 75).
    plane: Option<f64>,
    /// Fill `room` with a grid of this fixture until it reaches the lux wanted:
    /// `downlight`, `led-panel`, `light-ceiling`, `pendant`.
    fill: Option<String>,
    /// Lux wanted instead of the room's reference.
    lux: Option<f64>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct EmbedParams {
    /// Piece already in the plan to embed (id)…
    item: Option<String>,
    /// …or a new one from the catalog: `cooktop`, `sink-bowl`, `oven`, `microwave`.
    cat: Option<String>,
    /// Size of a new item, cm (a real product's measurements).
    w: Option<f64>,
    d: Option<f64>,
    h: Option<f64>,
    /// Joinery countertop (sink, cooktop) or cabinet (oven, microwave: a niche).
    host: String,
    /// Center along the host's width from its left end, cm (default: where the item is, or the middle).
    at: Option<f64>,
    /// Niche floor above the room floor, cm (default: oven 80 and microwave 145 in towers).
    z: Option<f64>,
    /// Check and report only.
    #[serde(default)]
    dry: bool,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct GetHomeParams {
    /// `summary` (counts, bounds, room areas) or `full` (default).
    detail: Option<String>,
    /// Only these ids, group parts included, e.g. `["f833","w24"]`.
    ids: Option<Vec<String>>,
    /// Only these kinds: `walls`, `rooms`, `dims`, `labels`, `furniture`,
    /// `polylines`.
    kinds: Option<Vec<String>>,
    /// Only what stands inside this room, by id or name.
    room: Option<String>,
    /// Only what meets this rectangle, `[[x0,y0],[x1,y1]]` cm.
    rect: Option<[[f64; 2]; 2]>,
    /// Storey to read: an id like `lv3`, or `all`. Default: the one shown.
    level: Option<String>,
    /// Keep only these fields of each element; `id` is always kept.
    fields: Option<Vec<String>>,
    /// List what is inside groups instead of only counting the parts.
    parts: Option<bool>,
    /// One element per line (NDJSON) instead of one JSON object, each line
    /// tagged with its kind. Long answers stay readable a slice at a time.
    ndjson: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct UpdateParams {
    items: Vec<UpdateSpec>,
    /// Plan version (tab) to write to; switches to it first.
    v: Option<usize>,
    /// Try it without applying: reports what would change, the clearances
    /// around every piece it touches, and which layout and ergonomics
    /// findings it would resolve or create. Nothing is written and the
    /// user's window does not move.
    dry: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct IdsParams {
    ids: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct MoveParams {
    ids: Vec<String>,
    dx: f64,
    dy: f64,
    /// Drag endpoints of walls joined to moved walls (default true).
    joined: Option<bool>,
    /// Try it without applying; see `update`.
    dry: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SplitParams {
    id: String,
    /// Split position along the wall, 0..1 (default 0.5).
    t: Option<f64>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct SetHomeParams {
    name: Option<String>,
    /// Clockwise degrees from plan up to north.
    north: Option<f64>,
    compass_at: Option<Point2>,
    /// Compass diameter cm.
    compass_d: Option<f64>,
    compass_visible: Option<bool>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct RenderParams {
    /// Width px (default 640, max 2048).
    w: Option<u32>,
    /// Height px (default 480, max 2048).
    h: Option<u32>,
    /// Plan region `[[minx,miny],[maxx,maxy]]`; default fits the drawing.
    region: Option<[Point2; 2]>,
    /// Draw the grid (default true).
    grid: Option<bool>,
    /// Background image opacity for this render (e.g. 0.5 to compare the
    /// drawing with the scanned reference; 0 hides it).
    bg: Option<f64>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct JoineryParams {
    /// New build: `cabinet`, `slats`, `countertop`, `cove`, `shadow_gap` or `sofa`.
    kind: Option<String>,
    /// Or an existing build's group id: `p` holds only what changes.
    id: Option<String>,
    /// Parameters (flat; everything has a default), see the tool description.
    p: Option<serde_json::Map<String, serde_json::Value>>,
    /// Center on the plan (default: 0,0), or `wall` (+`along` cm) to back it onto a wall.
    at: Option<Point2>,
    wall: Option<String>,
    along: Option<f64>,
    /// Clockwise degrees; bottom above the floor, cm (wall cabinets).
    angle: Option<f64>,
    elev: Option<f64>,
    /// `cove`/`shadow_gap`: room id whose outline to follow.
    room: Option<String>,
    /// Only check and report, create nothing.
    #[serde(default)]
    dry: bool,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct CutListParams {
    /// Build group ids (default: every build on this storey).
    ids: Option<Vec<String>>,
    /// Write `.csv` (spreadsheet), `.dxf` (sheets for CNC) or `.svg` (sheets to view).
    path: Option<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct TraceParams {
    /// Luminance 0..255 below which a gray pixel is ink (default 128; raise it for light gray walls).
    threshold: Option<u8>,
    /// Shortest wall kept, cm (default 60).
    min_len: Option<f64>,
    /// Wall thickness range, cm (default 5..45).
    t_min: Option<f64>,
    t_max: Option<f64>,
    /// Doors and windows up to this wide don't split a wall, cm (default 130).
    max_gap: Option<f64>,
    /// Only this part of the plan `[x0, y0, x1, y1]` cm.
    region: Option<[f64; 4]>,
    /// Create the walls (one undo step) instead of only listing them.
    #[serde(default)]
    create: bool,
    /// Wall height cm when creating (default 250).
    h: Option<f64>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct CheckParams {
    /// Expected room areas in m² by room name or id, e.g. {"Sala": 10.91};
    /// adds rows [room, expected, actual, diff %].
    areas: Option<std::collections::BTreeMap<String, f64>>,
    /// Storey to check: an id like `lv3`, or `all` for every storey that is
    /// not a reference layer. Default: the storey being shown.
    level: Option<String>,
}

/// A point in the plan, or the id of something already drawn.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(untagged)]
pub(crate) enum Spot {
    /// `[x, y]` in cm.
    At([f64; 2]),
    /// An element id, e.g. `f828` or `w24`.
    Id(String),
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct MeasureParams {
    /// What to measure from: an id or `[x,y]`. Alone, reports the free floor
    /// on all four sides of that piece.
    from: Option<Spot>,
    /// What to measure to: an id or `[x,y]`.
    to: Option<Spot>,
    /// Restrict to one axis, `x` or `y`. Between two boxes this is the gap
    /// along that axis (negative when they overlap).
    axis: Option<String>,
    /// Sides to measure free floor on: `+x`, `-x`, `+y`, `-y`, or, relative
    /// to the piece, `front`, `back`, `left`, `right`.
    dirs: Option<Vec<String>>,
    /// Probe line: with `axis`, the other axis' coordinate. Reports every
    /// stretch a straight line crosses, free floor and solids alike.
    at: Option<f64>,
    /// Limits of the probe along `axis`, `[from, to]` cm. Default: the plan.
    range: Option<[f64; 2]>,
    /// Height band that counts, `[z0, z1]` cm above this storey's floor.
    /// Default `[0, 200]`: what a person walking through meets.
    z: Option<[f64; 2]>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ExportParams {
    /// Output file: `.pdf`, `.svg`, `.png`, `.glb` or `.obj`.
    path: String,
    w: Option<u32>,
    h: Option<u32>,
    /// PDF scale denominator (50 → 1:50); omitted fits the sheet.
    scale: Option<f64>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct CatalogParams {
    /// Search words (Portuguese or English), e.g. `cama casal`.
    q: Option<String>,
    /// Category id, e.g. `kitchen`.
    cat: Option<String>,
    /// Max rows (default 40).
    limit: Option<usize>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct PhotoParams {
    /// `aerial` (default) or `visitor`.
    view: Option<String>,
    /// Stored point of view index.
    cam: Option<usize>,
    yaw: Option<f32>,
    pitch: Option<f32>,
    /// `draft` (default), `good`, `best`.
    quality: Option<String>,
    /// Local solar hour, 0–24.
    hour: Option<f64>,
    w: Option<u32>,
    h: Option<u32>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct Render3dParams {
    /// `aerial` (default) or `visitor`.
    view: Option<String>,
    /// Stored point of view index (see cameras).
    cam: Option<usize>,
    /// Aerial turn, degrees.
    yaw: Option<f32>,
    /// Aerial height angle, degrees.
    pitch: Option<f32>,
    /// Aerial distance factor: 1 frames the building, 2 twice as far.
    zoom: Option<f32>,
    /// Elevations: section plane in plan cm (y for front/back, x for left/right,
    /// height for top); what lies between the viewer and it is cut away — walls
    /// and pieces alike. front views from large y, back from y=0.
    cut: Option<f64>,
    w: Option<u32>,
    h: Option<u32>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct AnnotationParams {
    /// Dimensions and notes that no longer match the drawing: rows
    /// [id, written, measured, against, text]. A plan of joinery is read
    /// off its notes, so one that still says 66,5 over a corridor of 86 is
    /// worse than no note at all.
    stale: Option<bool>,
    /// Search label text, accent- and case-insensitive, e.g. `porta`.
    q: Option<String>,
    /// Show engineering dimension chains.
    dims: Option<bool>,
    /// Show the room reference schedule and tags.
    refs: Option<bool>,
    /// Include brand, model and link in references.
    details: Option<bool>,
    /// Convert the automatic dimension chains into editable dimensions.
    bake: Option<bool>,
    /// Legend of electrical/plumbing symbols with counts.
    legend: Option<bool>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct DisciplineParams {
    /// `active` (default), `select`, `show`, `hide`, `quantities`.
    action: Option<String>,
    /// `electrical`, `plumbing` or `architecture`.
    d: Option<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct CamerasParams {
    /// `list` (default), `view`, `aerial`, `store`, `delete`.
    action: Option<String>,
    /// Stored view index.
    i: Option<usize>,
    name: Option<String>,
    x: Option<f64>,
    y: Option<f64>,
    /// Eye height cm.
    z: Option<f64>,
    yaw: Option<f64>,
    /// Degrees down.
    pitch: Option<f64>,
    /// Horizontal field of view, degrees.
    fov: Option<f64>,
    /// For `store`: point `[x,y,z]` cm to look at (sets yaw and pitch).
    look_at: Option<[f64; 3]>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct VideoParams {
    /// `list` (default), `add`, `delete`, `clear`, `orbit`, `set`, `render`.
    action: Option<String>,
    /// Keyframe index (`delete`, or insert position for `add`).
    i: Option<usize>,
    /// Stored view index to add as keyframe.
    cam: Option<usize>,
    x: Option<f64>,
    y: Option<f64>,
    z: Option<f64>,
    yaw: Option<f64>,
    pitch: Option<f64>,
    fov: Option<f64>,
    /// Keyframes for `orbit`.
    n: Option<usize>,
    fps: Option<u32>,
    /// Camera speed m/s.
    speed: Option<f64>,
    /// Output `.avi` for `render`.
    path: Option<String>,
    w: Option<u32>,
    h: Option<u32>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct PluginsParams {
    /// `list` (default) or `run`.
    action: Option<String>,
    name: Option<String>,
    /// Arguments passed to the plugin as JSON.
    args: Option<serde_json::Value>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct LevelsParams {
    /// `list` (default), `add`, `select`, `update`, `delete`.
    action: Option<String>,
    /// Level id, e.g. `lv3`.
    id: Option<String>,
    name: Option<String>,
    /// Storey height cm for `add` and `update`.
    h: Option<f64>,
    /// Floor elevation cm for `add` and `update` (e.g. a house on stilts).
    elev: Option<f64>,
    /// Mark the storey as a reference layer: a traced plan, a scan, an
    /// earlier version. Its content is drawing, not building, so layout
    /// checks and ergonomics leave it alone even when it sits at the same
    /// elevation as the storey being designed.
    reference: Option<bool>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct VariantsParams {
    /// `list` (default), `duplicate` (copy active), `new` (empty), `switch`, `rename`, `delete`.
    action: Option<String>,
    /// Variant index for switch/rename/delete.
    i: Option<usize>,
    /// Name for duplicate/new/rename.
    name: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct PlaceParams {
    items: Vec<PlaceSpec>,
    /// Fields every item takes unless it sets them (cat, w, d, h, color, mat…).
    defaults: Option<PlaceSpec>,
    /// Coordinates (`at`, `into`, `a`, `b`, `along`) are pixels of the background image.
    #[serde(default)]
    px: bool,
    /// Plan version (tab) to write to; switches to it first.
    v: Option<usize>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct ArrangeParams {
    /// `array` (copies in a row), `rotate`, `mirror`, `group`, `ungroup`, `front`, `back`.
    action: String,
    ids: Vec<String>,
    /// array: number of copies (default 1).
    n: Option<usize>,
    /// array: step per copy, cm (dz raises furniture/labels).
    dx: Option<f64>,
    dy: Option<f64>,
    dz: Option<f64>,
    /// rotate: pivot `[x,y]` (default the center of the elements).
    about: Option<Point2>,
    /// rotate: clockwise degrees.
    angle: Option<f64>,
    /// mirror: two points of the mirror line.
    a: Option<Point2>,
    b: Option<Point2>,
    /// rotate/mirror: keep the originals and transform a copy.
    #[serde(default)]
    copy: bool,
    /// group: its name.
    name: Option<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct PathParams {
    /// Project file (`.newera`). Optional for save when already saved once.
    path: Option<String>,
}

#[tool_router]
impl NewEraMcp {
    pub fn new(document: SharedDocument) -> Self {
        let mut tool_router = Self::tool_router();
        for route in tool_router.map.values_mut() {
            let mut schema = serde_json::Value::Object((*route.attr.input_schema).clone());
            crate::schema::compact(&mut schema);
            if let serde_json::Value::Object(map) = schema {
                route.attr.input_schema = std::sync::Arc::new(map);
            }
        }
        Self {
            document,
            tool_router,
        }
    }

    #[tool(
        description = "Home state. detail=summary is cheapest. Ask for less instead of reading everything: ids=[…] resolves ids (group parts included), room=<id|name> and rect=[[x0,y0],[x1,y1]] read one place, kinds=[walls|rooms|dims|labels|furniture|polylines] and fields=[…] trim each row, parts=true opens groups, ndjson=true prints one element per line so a long answer can be read a slice at a time. Every piece carries bounds (plan box with angle applied) and faces (the side it opens toward). Ids share one counter per version (w1, r2, f3…) and are never reused, so a new version may start at any number."
    )]
    fn get_home(&self, Parameters(p): Parameters<GetHomeParams>) -> Result<String, ErrorData> {
        let doc = self.document.read();
        let full = doc.home();
        let view = match p.level.as_deref() {
            Some("all") => full.clone(),
            Some(raw) => {
                let id = raw
                    .parse()
                    .map_err(|_| invalid("level: id like lv3, or all"))?;
                if full.level(id).is_none() {
                    return Err(invalid(format!("no storey {raw}")));
                }
                full.level_view(Some(id))
            }
            None => full.level_view(full.current_level()),
        };
        let mut out = match p.detail.as_deref() {
            Some("summary") => compact::summary(&view, doc.revision()),
            _ if p.ids.is_some() => picked(&view, p.ids.as_deref().unwrap_or_default())?,
            _ => compact::home(&view, doc.revision()),
        };
        if p.parts.unwrap_or(false) {
            expand_parts(&view, &mut out);
        }
        narrow(&view, &mut out, &p)?;
        if !full.levels.is_empty() && p.kinds.is_none() && p.ids.is_none() {
            out["levels"] = compact::levels(full);
        }
        let warnings = compact::warnings(full);
        if !warnings.is_empty() {
            out["warnings"] = serde_json::json!(warnings);
        }
        if p.ndjson.unwrap_or(false) {
            return Ok(ndjson(&out));
        }
        Ok(out.to_string())
    }

    #[tool(
        description = "Create walls (polylines; hs = height per point for gables), rooms (pts, or at=[x,y] to detect from walls), dims (a+b or wall id), labels, roofs (rectangle pts, gable|shed, pitch or ridge_h, eave h, overhang, gables=true closes the ends, skylights [{at,w,d}] cut glazed openings) and solids (pts outline raised by h at elev: slabs/mezzanines of any shape; or profile [[u,z]] swept from a to b: gables, ramps) in one atomic step."
    )]
    fn create(&self, Parameters(mut p): Parameters<CreateParams>) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        on_variant(&mut doc, p.v)?;
        if p.px {
            let bg = background_scale(&doc)?;
            p.map_points(&|q| bg.point(q));
        }
        let ids = edit::create(&mut doc, p).map_err(invalid)?;
        Ok(ok(&doc, &ids))
    }

    #[tool(
        description = "Change fields of elements by id; fields must match the element kind (e.g. furniture mat/opacity/pitch, wall h_end, room auto, polyline divider). anchor on a resize holds one face still (back/front/left/right of the piece, bottom/top, or a plan side) instead of growing around the center, so a run of joinery keeps its back on the wall. dry=true answers what it would do — changed fields, clearances around each piece it touches, findings resolved and created — without writing anything, so a size can be tried before it is applied. Otherwise the reply names what changed."
    )]
    fn update(&self, Parameters(p): Parameters<UpdateParams>) -> Result<String, ErrorData> {
        if p.dry.unwrap_or(false) {
            let doc = self.document.read();
            let items = p.items;
            return preview(&doc, move |scratch| {
                edit::update(scratch, items).map_err(invalid)
            });
        }
        let mut doc = self.document.write();
        on_variant(&mut doc, p.v)?;
        let before = doc.home().clone();
        edit::update(&mut doc, p.items).map_err(invalid)?;
        Ok(applied(&doc, &before))
    }

    #[tool(description = "Delete elements by id, atomically.")]
    fn delete(&self, Parameters(p): Parameters<IdsParams>) -> Result<String, ErrorData> {
        let ids = edit::parse_ids(&p.ids).map_err(invalid)?;
        let mut doc = self.document.write();
        let commands = ids.into_iter().map(Command::remove).collect();
        doc.execute(Command::Batch { commands }).map_err(core)?;
        Ok(ok(&doc, &[]))
    }

    #[tool(
        name = "move",
        description = "Move elements by dx,dy cm. dry=true answers what it would do without writing anything; see `update`."
    )]
    fn move_elements(&self, Parameters(p): Parameters<MoveParams>) -> Result<String, ErrorData> {
        let ids = edit::parse_ids(&p.ids).map_err(invalid)?;
        let joined = p.joined.unwrap_or(true);
        if p.dry.unwrap_or(false) {
            let doc = self.document.read();
            return preview(&doc, move |scratch| {
                ops::translate(scratch, &ids, p.dx, p.dy, joined).map_err(core)
            });
        }
        let mut doc = self.document.write();
        let before = doc.home().clone();
        ops::translate(&mut doc, &ids, p.dx, p.dy, joined).map_err(core)?;
        Ok(applied(&doc, &before))
    }

    #[tool(description = "Split a wall into two joined walls at t (0..1).")]
    fn split_wall(&self, Parameters(p): Parameters<SplitParams>) -> Result<String, ErrorData> {
        let id = p.id.parse().map_err(|e| invalid(format!("{e}")))?;
        let mut doc = self.document.write();
        let second = ops::split_wall(&mut doc, id, p.t.unwrap_or(0.5)).map_err(core)?;
        Ok(ok(&doc, &[second.to_string()]))
    }

    #[tool(description = "Rename the project and/or set the compass (north).")]
    fn set_home(&self, Parameters(p): Parameters<SetHomeParams>) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        let mut commands = Vec::new();
        if let Some(name) = p.name {
            commands.push(Command::RenameHome { name });
        }
        if p.north.is_some()
            || p.compass_at.is_some()
            || p.compass_d.is_some()
            || p.compass_visible.is_some()
        {
            let c = doc.home().compass.clone();
            commands.push(Command::SetCompass {
                compass: Compass {
                    center: p.compass_at.unwrap_or(c.center),
                    diameter: p.compass_d.unwrap_or(c.diameter),
                    north_degrees: p.north.unwrap_or(c.north_degrees),
                    visible: p.compass_visible.unwrap_or(c.visible),
                    ..c.clone()
                },
            });
        }
        if commands.is_empty() {
            return Err(invalid("nothing to change"));
        }
        doc.execute(Command::Batch { commands }).map_err(core)?;
        Ok(ok(&doc, &[]))
    }

    #[tool(
        description = "Set a scanned plan as background at real scale: path, then cm_per_px (+cm_per_px_y), calibrate {a,b px, cm} or calibrations [{a,b,cm}…] (fits X/Y scales), angle (clockwise °); offset/opacity/visible; clear=true removes."
    )]
    fn set_background(
        &self,
        Parameters(p): Parameters<BackgroundParams>,
    ) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        let assets = doc.asset_dir();
        let size = |path: &str| {
            let resolved = newera_core::resolve_asset(assets.as_deref(), path);
            image::image_dimensions(&resolved)
                .map(|(w, h)| [w, h])
                .map_err(|e| format!("cannot read image {}: {e}", resolved.display()))
        };
        edit::set_background(&mut doc, &p, &size).map_err(invalid)?;
        Ok(ok(&doc, &[]))
    }

    #[tool(
        description = "PNG of the floor plan, exactly as the user sees it. bg=0..1 overlays the background image to compare with the reference. Keep w/h small to save tokens."
    )]
    fn render_plan(
        &self,
        Parameters(p): Parameters<RenderParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let png = self.render_with(
            p.w.unwrap_or(640),
            p.h.unwrap_or(480),
            p.region,
            p.grid.unwrap_or(true),
            p.bg,
        )?;
        let data = base64::engine::general_purpose::STANDARD.encode(png);
        Ok(CallToolResult::success(vec![ContentBlock::image(
            data,
            "image/png",
        )]))
    }

    #[tool(
        description = "PNG of the home in 3D (software render with outlines, no GPU needed). view: front|back|left|right|top orthographic elevations — front looks from the plan's bottom edge (large y) toward y=0, back from y=0 toward large y, left from x=0, right from large x; cut=cm makes a section keeping only what is beyond that plane from the viewer (front cut=200 keeps y<200, so the wall at y=0 stays as the backdrop; to remove it look from back), aerial (default; frames the whole building; yaw degrees: 0 from east/+x, 90 from south/plan bottom (default 60); pitch down; zoom >1 farther), visitor (current visitor camera) or cam=i (stored point of view). Keep w/h small."
    )]
    fn render_3d(
        &self,
        Parameters(p): Parameters<Render3dParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let (w, h) = (
            p.w.unwrap_or(480).clamp(64, 1600),
            p.h.unwrap_or(360).clamp(64, 1200),
        );
        #[allow(clippy::cast_precision_loss)]
        let aspect = w as f32 / h as f32;
        let doc = self.document.read();
        let home = doc.home();
        let view = match (p.cam, p.view.as_deref()) {
            (Some(i), _) => {
                let camera = home
                    .cameras
                    .stored
                    .get(i)
                    .ok_or_else(|| invalid(format!("no stored camera {i}")))?;
                newera_render::View::from_camera(camera, aspect)
            }
            (None, Some("visitor")) => {
                newera_render::View::from_camera(&home.cameras.observer, aspect)
            }
            (None, None | Some("aerial")) => newera_render::View::aerial_zoom(
                home,
                p.yaw.unwrap_or(60.0),
                p.pitch.unwrap_or(40.0),
                p.zoom.unwrap_or(1.0),
            ),
            (None, Some(side @ ("front" | "back" | "left" | "right" | "top"))) => {
                let side = match side {
                    "front" => newera_render::Side::Front,
                    "back" => newera_render::Side::Back,
                    "left" => newera_render::Side::Left,
                    "right" => newera_render::Side::Right,
                    _ => newera_render::Side::Top,
                };
                newera_render::View::orthographic(home, side, aspect, p.cut)
            }
            (None, Some(other)) => return Err(invalid(format!("unknown view `{other}`"))),
        };
        let home = home.clone();
        let assets = doc.asset_dir();
        drop(doc);
        let image = newera_render::render_home(&home, &view, w, h, assets.as_deref());
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .map_err(|e| invalid(e.to_string()))?;
        let data = base64::engine::general_purpose::STANDARD.encode(png);
        Ok(CallToolResult::success(vec![ContentBlock::image(
            data,
            "image/png",
        )]))
    }

    #[tool(
        description = "Realistic photo (path traced: sun from compass location and time, lamps, glass). cam=i stored view, or view=visitor/aerial (yaw,pitch). quality draft (~10 s) | good | best; hour = local solar time (e.g. 9, 15.5, 20); w/h small. Returns PNG."
    )]
    fn render_photo(
        &self,
        Parameters(p): Parameters<PhotoParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let (w, h) = (
            p.w.unwrap_or(480).clamp(64, 1600),
            p.h.unwrap_or(360).clamp(64, 1200),
        );
        #[allow(clippy::cast_precision_loss)]
        let aspect = w as f32 / h as f32;
        let quality = match p.quality.as_deref() {
            None | Some("draft") => newera_render::PhotoQuality::Draft,
            Some("good") => newera_render::PhotoQuality::Good,
            Some("best") => newera_render::PhotoQuality::Best,
            Some(other) => return Err(invalid(format!("unknown quality `{other}`"))),
        };
        let doc = self.document.read();
        let home = doc.home().clone();
        let assets = doc.asset_dir();
        drop(doc);
        let (view, time) = match (p.cam, p.view.as_deref()) {
            (Some(i), _) => {
                let camera = home
                    .cameras
                    .stored
                    .get(i)
                    .ok_or_else(|| invalid(format!("no stored camera {i}")))?;
                (
                    newera_render::View::from_camera(camera, aspect),
                    camera.time,
                )
            }
            (None, Some("visitor")) => (
                newera_render::View::from_camera(&home.cameras.observer, aspect),
                home.cameras.observer.time,
            ),
            (None, None | Some("aerial")) => (
                newera_render::View::aerial(&home, p.yaw.unwrap_or(60.0), p.pitch.unwrap_or(40.0)),
                home.cameras.top.time,
            ),
            (None, Some(other)) => return Err(invalid(format!("unknown view `{other}`"))),
        };
        let time = p.hour.map_or(time, |hour| {
            newera_render::at_local_hour(time, hour, home.compass.longitude.unwrap_or(-46.63))
        });
        let image = newera_render::photo_home(&home, &view, time, w, h, assets.as_deref(), quality);
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .map_err(|e| invalid(e.to_string()))?;
        let data = base64::engine::general_purpose::STANDARD.encode(png);
        Ok(CallToolResult::success(vec![ContentBlock::image(
            data,
            "image/png",
        )]))
    }

    #[tool(
        description = "Export to a file by extension: plan .pdf (A3; scale=50/100 or fit), .svg (true scale) or .png; 3D model .glb or .obj."
    )]
    fn export_plan(&self, Parameters(p): Parameters<ExportParams>) -> Result<String, ErrorData> {
        let path = PathBuf::from(&p.path);
        let bytes = match path.extension().and_then(|e| e.to_str()) {
            Some("glb" | "obj") => {
                let (home, assets) = {
                    let doc = self.document.read();
                    (doc.home().clone(), doc.asset_dir())
                };
                newera_render::export_home(&home, &path, assets.as_deref())
                    .map_err(|e| invalid(e.to_string()))?;
                return Ok(format!("ok {}", path.display()));
            }
            Some("pdf") => {
                let doc = self.document.read();
                let view = doc.home().level_view(doc.home().current_level());
                let scene = plan_scene(&view, &scene_options_for(&doc));
                newera_draw::to_pdf(
                    &scene,
                    &newera_draw::PdfOptions {
                        scale: p.scale,
                        title: doc.home().name.clone(),
                        ..newera_draw::PdfOptions::default()
                    },
                )
            }
            Some("svg") => {
                let doc = self.document.read();
                let view = doc.home().level_view(doc.home().current_level());
                let scene = plan_scene(&view, &scene_options_for(&doc));
                to_svg(&scene, &SvgOptions::default()).into_bytes()
            }
            Some("png") => self.render(p.w.unwrap_or(1600), p.h.unwrap_or(1200), None, false)?,
            _ => return Err(invalid("path must end with .pdf, .svg, .png, .glb or .obj")),
        };
        std::fs::write(&path, bytes)
            .map_err(|e| invalid(format!("cannot write {}: {e}", path.display())))?;
        Ok(format!("ok {}", path.display()))
    }

    #[tool(description = "Save the project (.newera). path optional after the first save.")]
    fn save_home(&self, Parameters(p): Parameters<PathParams>) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        let path = match (p.path, doc.path()) {
            (Some(path), _) => with_extension(PathBuf::from(path)),
            (None, Some(path)) => path.to_path_buf(),
            (None, None) => return Err(invalid("`path` is required for the first save")),
        };
        newera_core::save_project(&doc, &path)
            .map_err(|e| invalid(format!("cannot write {}: {e}", path.display())))?;
        doc.mark_saved(&path);
        Ok(format!("ok {}", path.display()))
    }

    #[tool(
        description = "Open a project (.newera) or import a Sweet Home 3D file (.sh3d), replacing the current one."
    )]
    fn open_home(&self, Parameters(p): Parameters<PathParams>) -> Result<String, ErrorData> {
        let path = PathBuf::from(p.path.ok_or_else(|| invalid("`path` is required"))?);
        let mut doc = self.document.write();
        let opened = newera_sh3d::open_file(&mut doc, &path).map_err(invalid)?;
        let mut reply = ok(&doc, &[]);
        if opened.imported {
            reply.push_str(" imported (unsaved)");
        }
        for warning in opened.warnings {
            reply.push_str("\nwarning: ");
            reply.push_str(&warning);
        }
        Ok(reply)
    }

    #[tool(description = "Start a new empty project.")]
    fn new_home(&self) -> String {
        let mut doc = self.document.write();
        doc.load(Home::default());
        doc.set_path(None);
        doc.set_asset_dir(None);
        ok(&doc, &[])
    }

    #[allow(clippy::unused_self)] // tool methods need the receiver
    #[tool(
        description = "Wall types [id,name,t] (drywall, masonry, concrete…) and finish patterns [key,label,color,tile]."
    )]
    fn materials(&self) -> String {
        compact::materials().to_string()
    }

    #[allow(clippy::unused_self)] // tool methods need the receiver
    #[tool(description = "Find catalog items: rows [id,name,w,d,h] in cm.")]
    fn catalog(&self, Parameters(p): Parameters<CatalogParams>) -> String {
        compact::catalog(p.q.as_deref(), p.cat.as_deref(), p.limit.unwrap_or(40)).to_string()
    }

    #[tool(
        description = "Place catalog items: at=[x,y] center (doors/windows near a wall snap into it; into=[x,y] picks the swing side), or wall=id (+along cm) to put doors/windows in a wall or furniture against it. Sizes w/d/h override defaults; pitch/roll tilt; angle clockwise degrees (0: front faces +y, down the plan; back/headboard toward -y); mat finish (wood, marble, img:…; 'img:facade.png fit' stretches one image: a reference board to compare with render_3d view=front) and opacity (glass 0.3); defaults {…} fills every item; px=true reads coordinates as background pixels. cat=beam with a,b=[x,y,z] (z above the floor) and w×h section makes rafters, posts and braces; a beam reaching into a roof stops under it. Pools: pool or pool-oval."
    )]
    fn place(&self, Parameters(p): Parameters<PlaceParams>) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        on_variant(&mut doc, p.v)?;
        let mut items = edit::with_defaults(p.items, p.defaults.as_ref()).map_err(invalid)?;
        if p.px {
            let bg = background_scale(&doc)?;
            for item in &mut items {
                item.at = item.at.map(|q| bg.point(q));
                item.into = item.into.map(|q| bg.point(q));
                item.along = item.along.map(|v| v * bg.scale);
                for end in [&mut item.a, &mut item.b] {
                    *end = end.map(|[x, y, z]| {
                        let q = bg.point(Point2::new(x, y));
                        [q.x, q.y, z]
                    });
                }
            }
        }
        let ids = edit::place(&mut doc, items).map_err(invalid)?;
        Ok(ok(&doc, &ids))
    }

    #[tool(
        description = "Arrange elements in one undo step. array {ids,n,dx,dy,dz} adds n copies stepping by dx/dy/dz cm (rafters, columns); rotate {ids,angle clockwise,about?,copy?}; mirror {ids,a,b,copy?} across the line a-b; group {ids,name?} joins pieces into one box that moves/hides together, ungroup {ids:[group]}; front/back {ids} draws rooms or pieces on top/underneath (pool over deck, rug under sofa). Returns new ids."
    )]
    fn arrange(&self, Parameters(p): Parameters<ArrangeParams>) -> Result<String, ErrorData> {
        use newera_core::arrange::{self, Transform};
        let ids = edit::parse_ids(&p.ids).map_err(invalid)?;
        let mut doc = self.document.write();
        let center = || -> Point2 {
            let home = doc.home();
            let points: Vec<Point2> = ids
                .iter()
                .filter_map(|id| home.element(*id))
                .flat_map(|e| match e {
                    newera_core::Element::Wall(w) => vec![w.start, w.end],
                    newera_core::Element::Room(r) => r.points,
                    newera_core::Element::Dimension(d) => vec![d.start, d.end],
                    newera_core::Element::Label(l) => vec![l.position],
                    newera_core::Element::Polyline(l) => l.points,
                    newera_core::Element::Furniture(f) => f.footprint().to_vec(),
                    newera_core::Element::Level(_) => Vec::new(),
                })
                .collect();
            #[allow(clippy::cast_precision_loss)]
            let n = points.len().max(1) as f64;
            let (x, y) = points
                .iter()
                .fold((0.0, 0.0), |(x, y), q| (x + q.x, y + q.y));
            Point2::new(x / n, y / n)
        };
        let out: Vec<newera_core::ElementId> = match p.action.as_str() {
            "array" => {
                let (dx, dy, dz) = (
                    p.dx.unwrap_or(0.0),
                    p.dy.unwrap_or(0.0),
                    p.dz.unwrap_or(0.0),
                );
                #[allow(clippy::cast_precision_loss)]
                let step = |i: usize| Transform::Translate {
                    dx: dx * i as f64,
                    dy: dy * i as f64,
                    dz: dz * i as f64,
                };
                arrange::array(&mut doc, &ids, step, p.n.unwrap_or(1).clamp(1, 500))
                    .map_err(core)?
            }
            "rotate" => {
                let angle = p.angle.ok_or_else(|| invalid("`angle` is required"))?;
                let about = p.about.unwrap_or_else(center);
                arrange::apply(
                    &mut doc,
                    &ids,
                    Transform::Rotate {
                        about,
                        degrees: angle,
                    },
                    p.copy,
                )
                .map_err(core)?
            }
            "mirror" => {
                let (Some(a), Some(b)) = (p.a, p.b) else {
                    return Err(invalid("`a` and `b` are required"));
                };
                arrange::apply(&mut doc, &ids, Transform::Mirror { a, b }, p.copy).map_err(core)?
            }
            "group" => vec![
                arrange::group(&mut doc, &ids, p.name.as_deref().unwrap_or(""))
                    .map_err(core)?
                    .into(),
            ],
            "ungroup" => {
                let Some(newera_core::ElementId::Furniture(id)) = ids.first().copied() else {
                    return Err(invalid("`ids` must hold one group id"));
                };
                arrange::ungroup(&mut doc, id)
                    .map_err(core)?
                    .into_iter()
                    .map(Into::into)
                    .collect()
            }
            "front" | "back" => {
                arrange::reorder(&mut doc, &ids, p.action == "front").map_err(core)?;
                Vec::new()
            }
            other => return Err(invalid(format!("unknown action `{other}`"))),
        };
        let out: Vec<String> = out.iter().map(ToString::to_string).collect();
        Ok(ok(&doc, &out))
    }

    #[tool(
        description = "Parametric joinery and interiors; the server computes every board, clearance and rule and replies {id,name,size,parts,hardware,notes}, or an error saying what to change. kind + p: cabinet {w,h,d cm; t 15|18|25 mm; back mm; door hinged|sliding|drawers|none; doors; shelves; drawers; dividers; plinth; cooktop; color; front finish} · slats {w,h cm; slat, thickness, gap mm; orientation vertical|horizontal; backing; finish} · countertop {length,depth,height,thickness cm; material; support none|legs|brackets; cutouts [{kind sink|cooktop|grommet, x, w?, d?}]} · cove {room or pts; type open|closed|inverted; ceiling, width, drop, slot cm; led} · shadow_gap {room or pts; ceiling, gap, depth cm; led} · sofa {length,depth,seat,back cm; arms straight|rounded|none; modules; color}. Place with at|wall(+along), angle, elev. Change a build: id + p with only new values (e.g. {\"shelves\":3}). dry=true validates only."
    )]
    fn joinery(&self, Parameters(p): Parameters<JoineryParams>) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        let patch = serde_json::Value::Object(p.p.clone().unwrap_or_default());
        // What to build: a new kind, or an existing group's parameters with changes.
        let (build, existing) = match (&p.id, &p.kind) {
            (Some(id), _) => {
                let id: newera_core::FurnitureId =
                    id.parse().map_err(|e| invalid(format!("{e}")))?;
                let group = doc
                    .home()
                    .furniture
                    .iter()
                    .find(|f| f.id == id)
                    .cloned()
                    .ok_or_else(|| invalid(format!("{id} not found")))?;
                let stored = group
                    .properties
                    .get(newera_joinery::PARAMS_KEY)
                    .ok_or_else(
                        // Saying what the piece IS turns a dead end into the
                        // next call: the tool that owns it is named.
                        || {
                            invalid(format!(
                                "{id} was not made by joinery; it was made by {}",
                                compact::made_by(&group)
                            ))
                        },
                    )?;
                (
                    newera_joinery::merged(stored, &patch).map_err(invalid)?,
                    Some(group),
                )
            }
            (None, Some(kind)) => {
                let mut value = patch.clone();
                value["kind"] = serde_json::Value::String(kind.clone());
                let build: newera_joinery::Build = serde_json::from_value(value)
                    .map_err(|e| invalid(format!("invalid parameters: {e}")))?;
                (build, None)
            }
            (None, None) => {
                return Err(invalid("give `kind` for a new build or `id` to change one"));
            }
        };
        // Coves and shadow gaps follow a room's outline, kept relative to its corner.
        let mut build = build;
        let mut origin = None;
        if let newera_joinery::Build::Cove(newera_joinery::CoveParams { pts, .. })
        | newera_joinery::Build::ShadowGap(newera_joinery::ShadowGapParams { pts, .. }) =
            &mut build
        {
            let absolute: Option<Vec<Point2>> = match (&p.room, existing.is_some()) {
                (Some(room), _) => {
                    let id: newera_core::RoomId =
                        room.parse().map_err(|e| invalid(format!("{e}")))?;
                    Some(
                        doc.home()
                            .rooms
                            .iter()
                            .find(|r| r.id == id)
                            .ok_or_else(|| invalid(format!("{room} not found")))?
                            .points
                            .clone(),
                    )
                }
                (None, false) if !pts.is_empty() => {
                    Some(pts.iter().map(|q| Point2::new(q[0], q[1])).collect())
                }
                _ => None,
            };
            if let Some(points) = absolute {
                let (lo_x, lo_y) = points
                    .iter()
                    .fold((f64::MAX, f64::MAX), |(x, y), q| (x.min(q.x), y.min(q.y)));
                *pts = points.iter().map(|q| [q.x - lo_x, q.y - lo_y]).collect();
                origin = Some(Point2::new(lo_x, lo_y));
            }
        }
        let output = newera_joinery::generate(&build).map_err(invalid)?;
        let [w, d, h] = output.size;
        let summary = |id: &str| {
            serde_json::json!({
                "id": id,
                "name": output.name,
                "size": [compact::num(w), compact::num(d), compact::num(h)],
                "parts": output.parts.len(),
                "hardware": output.hardware,
                "notes": output.notes,
            })
        };
        if p.dry {
            return Ok(summary("").to_string());
        }
        // Placement: an explicit spot, the build's previous one, or the room corner.
        let mut place = newera_core::Furniture {
            width: w,
            depth: d,
            height: h,
            ..newera_core::Furniture::default()
        };
        if let Some(group) = &existing {
            place.position = group.position;
            place.angle = group.angle;
            place.elevation = group.elevation;
        }
        if let Some(o) = origin {
            place.position = Point2::new(o.x + w / 2.0, o.y + d / 2.0);
            place.angle = 0.0;
        }
        if let Some(at) = p.at {
            place.position = at;
        }
        if let Some(wall) = &p.wall {
            let wall_id: newera_core::WallId = wall.parse().map_err(|e| invalid(format!("{e}")))?;
            let wall = doc
                .home()
                .wall(wall_id)
                .cloned()
                .ok_or_else(|| invalid(format!("{wall} not found")))?;
            let along = p
                .along
                .unwrap_or_else(|| wall.start.distance(wall.end) / 2.0);
            newera_core::align_to_wall(&mut place, &wall, along);
            edit::back_to_wall(&doc, &mut place, &wall);
        }
        if let Some(angle) = p.angle {
            place.angle = angle;
        }
        if let Some(elev) = p.elev {
            place.elevation = elev;
        }
        let group_id = existing
            .as_ref()
            .map_or_else(|| doc.new_furniture_id(), |g| g.id);
        let group = {
            let doc_ref = &mut *doc;
            let mut next = || doc_ref.new_furniture_id();
            newera_joinery::assemble(
                &build,
                &output,
                group_id,
                place.position,
                place.angle,
                place.elevation,
                &mut next,
            )
        };
        // A name the user gave survives changes; the generated one follows them.
        let renamed = existing.as_ref().and_then(|g| {
            let stored = g.properties.get(newera_joinery::PARAMS_KEY)?;
            let before = serde_json::from_str(stored)
                .ok()
                .and_then(|b| newera_joinery::generate(&b).ok())?;
            (g.name != before.name).then(|| g.name.clone())
        });
        let mut group = newera_core::Furniture {
            level: existing.as_ref().and_then(|g| g.level),
            name: renamed.unwrap_or_else(|| group.name.clone()),
            ..group
        };
        // Items embedded in it stay in their place on the host.
        if let Some(old) = &existing {
            newera_joinery::carry_embedded(old, &mut group);
        }
        let command = if existing.is_some() {
            Command::update(group)
        } else {
            Command::insert(group)
        };
        doc.execute(command).map_err(core)?;
        Ok(summary(&group_id.to_string()).to_string())
    }

    #[tool(
        description = "Fit walls, glass and panels to the roof above them: under an A-frame or shed roof a wall gets a sloping top and is split at the ridge, a panel becomes a triangle or trapezoid (a glass gable with no math); a joinery slatted panel gets its slats cut to the roof line. They keep following the roof when it changes, in the same undo step; off stops that. Reply ok with the count."
    )]
    fn fit_roof(&self, Parameters(p): Parameters<FitRoofParams>) -> Result<String, ErrorData> {
        let ids = edit::parse_ids(&p.ids).map_err(invalid)?;
        let mut doc = self.document.write();
        if p.off {
            let mut commands = Vec::new();
            for id in &ids {
                match id {
                    newera_core::ElementId::Wall(w) => {
                        if let Some(mut wall) = doc.home().wall(*w).cloned() {
                            wall.properties.remove(newera_core::ROOF_FIT_KEY);
                            commands.push(Command::update(wall));
                        }
                    }
                    newera_core::ElementId::Furniture(f) => {
                        if let Some(mut piece) =
                            doc.home().furniture.iter().find(|x| x.id == *f).cloned()
                        {
                            piece.properties.remove(newera_core::ROOF_FIT_KEY);
                            commands.push(Command::update(piece));
                        }
                    }
                    _ => {}
                }
            }
            doc.execute(Command::Batch { commands }).map_err(core)?;
            return Ok(ok(&doc, &[]));
        }
        // Slatted panels are rebuilt by their rules under the roof line.
        let above = p.above.unwrap_or(newera_core::ROOF_FIT_ABOVE);
        let mut joinery = 0;
        let mut rest = Vec::new();
        for id in ids {
            let is_joinery =
                match id {
                    newera_core::ElementId::Furniture(f) => doc.home().furniture.iter().any(|x| {
                        x.id == f && x.properties.contains_key(newera_joinery::PARAMS_KEY)
                    }),
                    _ => false,
                };
            if let (true, newera_core::ElementId::Furniture(f)) = (is_joinery, id) {
                newera_joinery::fit_joinery_to_roof(&mut doc, f, above).map_err(invalid)?;
                joinery += 1;
            } else {
                rest.push(id);
            }
        }
        let ids = rest;
        if ids.is_empty() {
            return Ok(format!("{} fitted={joinery}", ok(&doc, &[])));
        }
        let before: Vec<String> = doc.home().walls.iter().map(|w| w.id.to_string()).collect();
        let count = newera_core::fit_to_roof(&mut doc, &ids, above).map_err(core)? + joinery;
        let added: Vec<String> = doc
            .home()
            .walls
            .iter()
            .map(|w| w.id.to_string())
            .filter(|id| !before.contains(id))
            .collect();
        Ok(format!("{} fitted={count}", ok(&doc, &added)))
    }

    #[tool(
        description = "Embed an item into joinery with an exact fit: a sink bowl or cooktop into a countertop (cutout from the item's size, generic fixture not drawn), an oven, microwave or other appliance into a cabinet niche (doors above and below, boards around it), a TV onto a slatted panel at seated eye level (z = screen center). The item becomes part of the host and moves with it. item: id in the plan, or cat (+w/d/h) for a new one. Errors say what to change (e.g. use w = 61 no armário). Reply {host, item, kind, cutout|niche, x|bottom, notes}."
    )]
    fn embed(&self, Parameters(p): Parameters<EmbedParams>) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        let host: newera_core::FurnitureId = p.host.parse().map_err(|e| invalid(format!("{e}")))?;
        let (item, existing) = match (&p.item, &p.cat) {
            (Some(id), _) => {
                let id: newera_core::FurnitureId =
                    id.parse().map_err(|e| invalid(format!("{e}")))?;
                let piece = doc
                    .home()
                    .furniture
                    .iter()
                    .find(|f| f.id == id)
                    .cloned()
                    .ok_or_else(|| invalid(format!("{id} not found (embed top-level pieces)")))?;
                (piece, true)
            }
            (None, Some(cat)) => {
                let entry = newera_catalog::find(cat).ok_or_else(|| {
                    invalid(format!("unknown catalog id `{cat}` (use the catalog tool)"))
                })?;
                let mut piece = entry.instantiate(doc.new_furniture_id(), Point2::default());
                piece.width = p.w.unwrap_or(piece.width);
                piece.depth = p.d.unwrap_or(piece.depth);
                piece.height = p.h.unwrap_or(piece.height);
                (piece, false)
            }
            (None, None) => return Err(invalid("give `item` (an id) or `cat` for a new one")),
        };
        let request = newera_joinery::EmbedRequest {
            item,
            existing,
            host,
            at: p.at,
            z: p.z,
            dry: p.dry,
        };
        newera_joinery::embed(&mut doc, &request)
            .map(|v| v.to_string())
            .map_err(invalid)
    }

    #[tool(
        description = "Ergonomics and habitability review for the people living there (occupants, children, elderly, wheelchair, stature cm): room to walk beside beds and in front of kitchen equipment, beds/seats/bathrooms/wardrobes per person, kitchen triangle and heights, doors, ceiling heights, windows, minimum furniture, wheelchair turning. Brazilian references (NBR 9050, NBR 15575-1, IBGE; building codes vary by city). Reply {score, capacity, findings:[[erro|alerta|dica, place, message, fix?]]}; fix, when present, is a checked change as tool arguments (move or update): apply one, then review again (fixes of one review may overlap)."
    )]
    fn ergonomics(&self, Parameters(p): Parameters<newera_ergonomics::Profile>) -> String {
        let doc = self.document.read();
        let report = newera_ergonomics::review(doc.home(), &p);
        let findings: Vec<serde_json::Value> = report
            .findings
            .iter()
            .map(|f| match &f.fix {
                Some(fix) => serde_json::json!([f.severity, f.place, f.message, fix]),
                None => serde_json::json!([f.severity, f.place, f.message]),
            })
            .collect();
        serde_json::json!({
            "score": report.score,
            "capacity": report.capacity,
            "findings": findings,
        })
        .to_string()
    }

    #[tool(
        description = "Lighting design by photometry: every fixture's flux (lm, or W × lamp efficacy), color temperature and distribution (bulb, spot beam, LED panel/strip) lights the work plane by the inverse-square cosine law, walls casting shadows, plus interreflection (split flux). Reply rooms [[id,name,m²,avg lx,min lx,uniformity,reference lx,fixtures,W/m²,verdict]] against ABNT NBR ISO/CIE 8995-1 residential references. fill=<fixture> with room places a verified grid reaching the reference (or lux). Set a piece's light with place/update light {lm|w,lamp,k,beam,area}."
    )]
    fn lighting(&self, Parameters(p): Parameters<LightingParams>) -> Result<String, ErrorData> {
        use newera_core::lighting::{
            Reflectance, emitters, fixtures_needed, grid_positions, room_lighting,
        };
        let plane = p.plane.unwrap_or(75.0);
        let row = |r: &newera_core::RoomLighting, wanted: f64| {
            let verdict = if r.average + 0.5 < wanted {
                format!("abaixo: faltam {} lx", (wanted - r.average).round())
            } else if r.average > wanted * 2.5 {
                format!("acima: {:.1}× a referência", r.average / wanted)
            } else if r.uniformity < 0.4 && r.points > 4 {
                "ok na média, mas pouco uniforme (U0 < 0,4)".to_owned()
            } else {
                "ok".to_owned()
            };
            serde_json::json!([
                r.room.to_string(),
                r.name,
                (r.area_m2 * 10.0).round() / 10.0,
                r.average.round(),
                r.min.round(),
                (r.uniformity * 100.0).round() / 100.0,
                wanted,
                r.fixtures,
                (r.watts_per_m2 * 10.0).round() / 10.0,
                verdict,
            ])
        };
        let mut doc = self.document.write();
        let home = doc.home().clone();
        let view = home.level_view(home.current_level());
        let wanted_room: Option<newera_core::RoomId> = match &p.room {
            Some(id) => Some(id.parse().map_err(|e| invalid(format!("{e}")))?),
            None => None,
        };
        let rooms: Vec<&newera_core::Room> = view
            .rooms
            .iter()
            .filter(|r| wanted_room.is_none_or(|id| r.id == id))
            .collect();
        if rooms.is_empty() {
            return Err(invalid(match &p.room {
                Some(id) => format!("{id} not found on this storey"),
                None => "no rooms on this storey".to_owned(),
            }));
        }
        let Some(cat) = &p.fill else {
            let lights = emitters(&home, &newera_catalog::light_for);
            let rows: Vec<serde_json::Value> = rooms
                .iter()
                .map(|room| {
                    let r = room_lighting(&home, &lights, room, plane, Reflectance::default());
                    let wanted = p.lux.unwrap_or(r.target);
                    row(&r, wanted)
                })
                .collect();
            let lumens: f64 = lights.iter().map(|e| e.flux).sum();
            let watts: f64 = lights.iter().map(|e| e.watts).sum();
            return Ok(serde_json::json!({
                "rooms": rows,
                "fixtures": lights.len(),
                "lm": lumens.round(),
                "W": watts.round(),
            })
            .to_string());
        };
        let [room] = rooms[..] else {
            return Err(invalid("fill needs one `room`"));
        };
        let entry = newera_catalog::find(cat)
            .filter(|e| e.light.is_some())
            .ok_or_else(|| {
                invalid(format!(
                    "`{cat}` is not a light fixture (downlight, led-panel, light-ceiling, pendant)"
                ))
            })?;
        let ceiling = home
            .resolve_level(room.level)
            .and_then(|id| home.levels.iter().find(|l| l.id == id))
            .map_or(home.wall_height, |l| l.height);
        let template = entry.instantiate(newera_core::FurnitureId(0), Point2::default());
        let fixture_lm = newera_catalog::light_for(&template).map_or(0.0, |l| l.flux());
        let before = room_lighting(
            &home,
            &emitters(&home, &newera_catalog::light_for),
            room,
            plane,
            Reflectance::default(),
        );
        let wanted = p.lux.unwrap_or(before.target);
        let area = before.area_m2;
        if before.average + 0.5 >= wanted {
            return Ok(serde_json::json!({
                "placed": [],
                "before": row(&before, wanted),
                "note": "já atende; nada colocado",
            })
            .to_string());
        }
        let layout = |count: usize| -> (Vec<newera_core::Furniture>, newera_core::RoomLighting) {
            // The whole grid, even a few more than asked: symmetric layouts.
            let placed: Vec<newera_core::Furniture> = grid_positions(&room.points, count)
                .into_iter()
                .map(|at| {
                    let mut piece = template.clone();
                    piece.position = at;
                    piece.level = room.level;
                    // Hung from or set into the ceiling of the room (or the roof over it).
                    let top = newera_core::roof_height_at(&view, at, newera_core::ROOF_FIT_ABOVE)
                        .map_or(ceiling, |roof| roof.min(ceiling));
                    piece.elevation = match cat.as_str() {
                        "downlight" => top - piece.height + 0.6,
                        _ => top - piece.height,
                    };
                    piece
                })
                .collect();
            let mut trial = home.clone();
            trial.furniture.extend(placed.iter().cloned());
            let report = room_lighting(
                &trial,
                &emitters(&trial, &newera_catalog::light_for),
                room,
                plane,
                Reflectance::default(),
            );
            (placed, report)
        };
        // The lumen method gives a first count; the photometry of that trial
        // tells what each fixture really adds, then fixtures are added one by
        // one until the room reaches the reference.
        let mut count = fixtures_needed(wanted, area, fixture_lm).clamp(1, 60);
        let (mut placed, mut after) = layout(count);
        #[allow(clippy::cast_precision_loss)]
        let gain = (after.average - before.average) / count as f64;
        if gain > 0.0 {
            // Clamped to 1..60 first, so the cast is exact.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let needed = ((wanted - before.average) / gain).ceil().clamp(1.0, 60.0) as usize;
            count = needed;
            (placed, after) = layout(count);
        }
        while after.average + 0.5 < wanted && count < 60 {
            count += 1;
            (placed, after) = layout(count);
        }
        let mut ids = Vec::new();
        let mut commands = Vec::new();
        for mut piece in placed {
            piece.id = doc.new_furniture_id();
            ids.push(piece.id.to_string());
            commands.push(Command::insert(piece));
        }
        doc.execute(Command::Batch { commands }).map_err(core)?;
        Ok(serde_json::json!({
            "placed": ids,
            "before": row(&before, wanted),
            "after": row(&after, wanted),
        })
        .to_string())
    }

    #[tool(
        description = "Fill a wall with cabinets sized for it: measures the free stretches between corners, doors, windows, fridge and stove, splits each into even modules (30-90 cm, no useless leftovers; 15-30 cm pull-outs, fillers under 15), drawer unit beside the stove, countertop on base rows, cabinet over the fridge and hood gap on wall rows, wardrobes (hanging rails, shelves, drawers) on tall rows facing bedrooms; p.sink/p.cooktop place those cabinets and cutouts. Replaces the cabinets already there (keep ids stay). Reply {modules:[[id,role,from,w]],removed,notes}; dry plans only. Change one module afterwards with joinery id."
    )]
    fn cabinet_run(
        &self,
        Parameters(p): Parameters<newera_joinery::CabinetRunParams>,
    ) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        newera_joinery::cabinet_run(&mut doc, &p)
            .map(|v| v.to_string())
            .map_err(invalid)
    }

    #[tool(
        description = "Cut list of joinery builds: rows [part,board,qty,length,width,thickness mm,edge long+short,cutouts [x,y,w,d] mm?] merged by size, hardware, sheets per board. path .csv, or .dxf/.svg (boards laid out on sheets), writes a file."
    )]
    fn cut_list(&self, Parameters(p): Parameters<CutListParams>) -> Result<String, ErrorData> {
        let doc = self.document.read();
        let home = doc.home();
        let view = home.level_view(home.current_level());
        let wanted: Option<Vec<String>> = p.ids.clone();
        let mut rows: Vec<newera_joinery::CutRow> = Vec::new();
        let mut hardware: Vec<String> = Vec::new();
        let mut builds = 0;
        for group in view.furniture.iter().filter(|f| {
            f.properties.contains_key(newera_joinery::PARAMS_KEY)
                && wanted
                    .as_ref()
                    .is_none_or(|ids| ids.contains(&f.id.to_string()))
        }) {
            let build: newera_joinery::Build =
                serde_json::from_str(&group.properties[newera_joinery::PARAMS_KEY])
                    .map_err(|e| invalid(format!("{}: {e}", group.id)))?;
            let output = newera_joinery::generate(&build).map_err(invalid)?;
            builds += 1;
            hardware.extend(output.hardware.iter().map(|h| format!("{}: {h}", group.id)));
            for row in newera_joinery::cut_list(&output) {
                match rows.iter_mut().find(|r| {
                    r.board == row.board
                        && r.edge == row.edge
                        && r.holes == row.holes
                        && r.size
                            .iter()
                            .zip(row.size)
                            .all(|(a, b)| (a - b).abs() < 0.05)
                }) {
                    Some(existing) => existing.qty += row.qty,
                    None => rows.push(row),
                }
            }
        }
        if builds == 0 {
            return Err(invalid(
                "no joinery builds here (make one with the joinery tool)",
            ));
        }
        drop(doc);
        let mut sheets = Vec::new();
        if let Some(path) = &p.path {
            let ext = std::path::Path::new(path)
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase);
            let layout = match ext.as_deref() {
                Some("dxf") => Some(newera_joinery::cut_list_dxf(&rows)),
                Some("svg") => Some(newera_joinery::cut_list_svg(&rows)),
                Some("csv") => None,
                _ => return Err(invalid("path must end in .csv, .dxf or .svg")),
            };
            let bytes = match layout {
                Some((drawing, used)) => {
                    sheets = used;
                    drawing
                }
                None => newera_joinery::cut_list_csv(&rows),
            };
            std::fs::write(path, bytes).map_err(|e| invalid(format!("{path}: {e}")))?;
        }
        let rows: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                let mut row = serde_json::json!([
                    r.name,
                    r.board,
                    r.qty,
                    r.size[0],
                    r.size[1],
                    r.size[2],
                    format!("{}+{}", r.edge[0], r.edge[1])
                ]);
                // Cutouts `[x, y, w, d]` mm, for stone tops.
                if !r.holes.is_empty()
                    && let Some(cells) = row.as_array_mut()
                {
                    cells.push(serde_json::json!(r.holes));
                }
                row
            })
            .collect();
        let mut reply = serde_json::json!({ "rows": rows, "hardware": hardware });
        if !sheets.is_empty() {
            reply["sheets"] = serde_json::json!(sheets);
        }
        Ok(reply.to_string())
    }

    #[tool(
        description = "Trace walls from the background image (set_background first): thick dark or gray bands across or down the image become walls (colored areas — lawn, plants, furniture — are ignored; collinear pieces split by doors/windows up to max_gap join; region limits the search). Returns rows [[x1,y1],[x2,y2],t] in plan cm; create=true adds them as walls. Check with render_plan bg=0.5."
    )]
    fn trace_background(
        &self,
        Parameters(p): Parameters<TraceParams>,
    ) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        let bg = background_scale(&doc)?.image;
        let path = newera_core::resolve_asset(doc.asset_dir().as_deref(), &bg.path);
        let image = image::open(&path)
            .map_err(|e| invalid(format!("cannot read {}: {e}", path.display())))?
            .to_rgb8();
        // Colored areas (lawn, plants, furniture, cars) are never ink.
        let image = crate::trace::ink_mask(&image, p.threshold.unwrap_or(128));
        let (sx, sy) = bg.scale();
        let px = |cm: f64| cm / sx.min(sy);
        let options = crate::trace::TraceOptions {
            threshold: p.threshold.unwrap_or(128),
            min_length: px(p.min_len.unwrap_or(60.0)),
            min_thickness: p.t_min.unwrap_or(5.0) / sx.max(sy),
            max_thickness: p.t_max.unwrap_or(45.0) / sx.min(sy),
            max_gap: px(p.max_gap.unwrap_or(130.0)),
        };
        let walls: Vec<(Point2, Point2, f64)> = crate::trace::trace(&image, &options)
            .into_iter()
            .map(|t| {
                let horizontal = (t.a[1] - t.b[1]).abs() < (t.a[0] - t.b[0]).abs();
                let thickness = t.thickness * if horizontal { sy } else { sx };
                (
                    bg.plan_point(Point2::new(t.a[0], t.a[1])),
                    bg.plan_point(Point2::new(t.b[0], t.b[1])),
                    (thickness * 10.0).round() / 10.0,
                )
            })
            .filter(|(a, b, _)| {
                p.region.is_none_or(|[x0, y0, x1, y1]| {
                    let inside = |q: &Point2| {
                        q.x >= x0.min(x1)
                            && q.x <= x0.max(x1)
                            && q.y >= y0.min(y1)
                            && q.y <= y0.max(y1)
                    };
                    inside(a) && inside(b)
                })
            })
            .collect();
        if p.create {
            if walls.is_empty() {
                return Err(invalid(
                    "no walls found; try a higher threshold or smaller t_min",
                ));
            }
            let mut ids = Vec::new();
            let commands = walls
                .iter()
                .map(|(a, b, t)| {
                    let mut wall = newera_core::Wall::new(doc.new_wall_id(), *a, *b);
                    wall.thickness = *t;
                    wall.height = p.h.unwrap_or(newera_core::Wall::DEFAULT_HEIGHT);
                    ids.push(wall.id.to_string());
                    Command::insert(wall)
                })
                .collect();
            doc.execute(Command::Batch { commands }).map_err(core)?;
            return Ok(ok(&doc, &ids));
        }
        let rows: Vec<serde_json::Value> = walls
            .iter()
            .map(|(a, b, t)| {
                serde_json::json!([
                    [compact::num(a.x), compact::num(a.y)],
                    [compact::num(b.x), compact::num(b.y)],
                    t
                ])
            })
            .collect();
        Ok(serde_json::json!({ "rows": rows }).to_string())
    }

    #[tool(
        description = "Layout problems: overlap, blocked, in_wall, blocks_door, outside_rooms; {} means none. Each one carries name, bounds and z of both elements. Overlaps are classified kind collision (a real clash, listed first), nesting (built in, resting on, tucked under) or cross_level, with extent [x,y,z] cm of the shared space; overlap_kinds counts them. blocked is a cabinet, fridge or wardrobe whose opening face is against a solid — it cannot be used, and `angle` alone does not show it. level: a storey id or `all`, default the one shown. areas {name|id: m²} compares room areas with the reference drawing."
    )]
    fn check_layout(&self, Parameters(p): Parameters<CheckParams>) -> Result<String, ErrorData> {
        let doc = self.document.read();
        let (view, scope) = match p.level.as_deref() {
            Some("all") => (doc.home().clone(), newera_core::Storeys::All),
            Some(raw) => {
                let id = raw
                    .parse()
                    .map_err(|_| invalid("level: id like lv3, or all"))?;
                if doc.home().level(id).is_none() {
                    return Err(invalid(format!("no storey {raw}")));
                }
                (
                    doc.home().level_view(Some(id)),
                    newera_core::Storeys::One(id),
                )
            }
            None => (
                doc.home().level_view(doc.home().current_level()),
                newera_core::Storeys::Active,
            ),
        };
        let mut report = compact::issues(&view, scope);
        if let Some(expected) = p.areas {
            let rows: Vec<serde_json::Value> = expected
                .iter()
                .map(|(key, m2)| {
                    let room = view
                        .rooms
                        .iter()
                        .find(|r| r.id.to_string() == *key || r.name.eq_ignore_ascii_case(key));
                    match room {
                        Some(r) => {
                            let actual = r.area() / 10_000.0;
                            let diff = if *m2 > 0.0 {
                                (actual - m2) / m2 * 100.0
                            } else {
                                0.0
                            };
                            serde_json::json!([
                                key,
                                round2(*m2),
                                round2(actual),
                                (diff * 10.0).round() / 10.0
                            ])
                        }
                        None => serde_json::json!([key, round2(*m2), null, null]),
                    }
                })
                .collect();
            report["areas"] = serde_json::Value::Array(rows);
        }
        Ok(report.to_string())
    }

    #[tool(
        description = "Tape measure over the plan, in cm. from=<id> alone: free floor on all four sides, {clear:{\"+y\":[cm,id,name]}} — dirs picks sides (+x -x +y -y, or front/back/left/right of the piece). from+to (ids or [x,y]): the distance between them, {cm}, or the gap along axis. axis+at: what a straight probe runs into, {spans:[[from,to,id,name]]} with id null for free floor — the answer to \"how wide is the corridor here, and between what\". z limits the height band that counts (default 0-200)."
    )]
    fn measure(&self, Parameters(p): Parameters<MeasureParams>) -> Result<String, ErrorData> {
        use newera_core::measure::{self, Axis, Dir};
        let doc = self.document.read();
        // Measurements are of one storey: a wall one floor up is not in the way.
        let home = doc.home().level_view(doc.home().current_level());
        let axis = match p.axis.as_deref() {
            Some(raw) => Some(Axis::parse(raw).ok_or_else(|| invalid("axis: x or y"))?),
            None => None,
        };
        let spot = |s: &Spot| -> Result<(Point2, Option<newera_core::ElementId>), ErrorData> {
            match s {
                Spot::At([x, y]) => Ok((Point2::new(*x, *y), None)),
                Spot::Id(raw) => {
                    let id: newera_core::ElementId =
                        raw.parse().map_err(|e| invalid(format!("{e}")))?;
                    let (min, max) = measure::element_bounds(&home, id)
                        .ok_or_else(|| invalid(format!("no {raw} on this storey")))?;
                    Ok((
                        Point2::new(f64::midpoint(min.x, max.x), f64::midpoint(min.y, max.y)),
                        Some(id),
                    ))
                }
            }
        };

        // Probe: what a straight line at `at` runs into, in order.
        if let Some(at) = p.at {
            let axis = axis.ok_or_else(|| invalid("at needs axis: x or y"))?;
            let z = p.z.map_or((0.0, 200.0), |[a, b]| (a, b));
            let spans: Vec<serde_json::Value> =
                measure::free_span(&home, axis, at, p.range.map(|[a, b]| (a, b)), z)
                    .into_iter()
                    .map(|s| {
                        serde_json::json!([
                            compact::num(s.from),
                            compact::num(s.to),
                            s.what.map(|w| w.id().to_string()),
                            s.name,
                        ])
                    })
                    .collect();
            return Ok(serde_json::json!({ "spans": spans }).to_string());
        }

        let from = p
            .from
            .as_ref()
            .ok_or_else(|| invalid("from: an id, or [x,y]"))?;

        // Distance between two things.
        if let Some(to) = &p.to {
            let (a, a_id) = spot(from)?;
            let (b, b_id) = spot(to)?;
            let boxes = a_id
                .and_then(|id| measure::element_bounds(&home, id))
                .zip(b_id.and_then(|id| measure::element_bounds(&home, id)));
            let mut out = serde_json::Map::new();
            match (boxes, axis) {
                // Between two boxes the useful number is the gap, not the
                // distance between centers: it is what fits in between.
                (Some((ba, bb)), Some(axis)) => {
                    out.insert("cm".to_owned(), compact::num(measure::gap(ba, bb, axis)));
                    out.insert("axis".to_owned(), serde_json::json!(axis.name()));
                }
                (Some((ba, bb)), None) => {
                    out.insert("x".to_owned(), compact::num(measure::gap(ba, bb, Axis::X)));
                    out.insert("y".to_owned(), compact::num(measure::gap(ba, bb, Axis::Y)));
                }
                (None, Some(Axis::X)) => {
                    out.insert("cm".to_owned(), compact::num((b.x - a.x).abs()));
                }
                (None, Some(Axis::Y)) => {
                    out.insert("cm".to_owned(), compact::num((b.y - a.y).abs()));
                }
                (None, None) => {
                    out.insert("cm".to_owned(), compact::num((b.x - a.x).hypot(b.y - a.y)));
                }
            }
            return Ok(serde_json::Value::Object(out).to_string());
        }

        // Free floor around a piece.
        let Spot::Id(raw) = from else {
            return Err(invalid("free floor is measured around a piece: from=<id>"));
        };
        let id: newera_core::FurnitureId = raw.parse().map_err(|e| invalid(format!("{e}")))?;
        let piece = home
            .find_piece(id)
            .ok_or_else(|| invalid(format!("no {raw} on this storey")))?;
        let dirs: Vec<Dir> = match &p.dirs {
            Some(raw) => raw
                .iter()
                .map(|d| {
                    Dir::parse(d, Some(piece)).ok_or_else(|| {
                        invalid(format!("dir {d}: +x -x +y -y, front/back/left/right"))
                    })
                })
                .collect::<Result<_, _>>()?,
            None => Dir::PLAN.to_vec(),
        };
        let solids = measure::obstacles(&home, &|_| false);
        let mut clear = serde_json::Map::new();
        for dir in dirs {
            let c = measure::clearance_against(&solids, piece, dir, measure::MAX_REACH);
            clear.insert(
                dir.name().to_owned(),
                serde_json::json!([
                    compact::num(c.cm),
                    c.against.map(|s| s.id().to_string()),
                    c.name,
                ]),
            );
        }
        let (min, max) = measure::plan_bounds(piece);
        Ok(serde_json::json!({
            "id": raw,
            "bounds": [compact::point(min), compact::point(max)],
            "faces": measure::facing(piece),
            "clear": clear,
        })
        .to_string())
    }

    #[tool(
        description = "Electrical and plumbing projects over the plan. active (default) reports {active, hidden}. select {d: electrical|plumbing|architecture}: new symbols (catalog cat electrical/plumbing) and lines go there and the rest is dimmed. show/hide {d}. quantities: {electrical:[[name,count]], plumbing:[...], lines_cm:{...}}."
    )]
    fn disciplines(
        &self,
        Parameters(p): Parameters<DisciplineParams>,
    ) -> Result<String, ErrorData> {
        use newera_core::Discipline;
        let mut doc = self.document.write();
        let parse = |raw: Option<&str>| -> Result<Option<Discipline>, ErrorData> {
            match raw {
                Some("electrical") => Ok(Some(Discipline::Electrical)),
                Some("plumbing") => Ok(Some(Discipline::Plumbing)),
                Some("architecture") => Ok(None),
                _ => Err(invalid("`d` must be electrical, plumbing or architecture")),
            }
        };
        match p.action.as_deref().unwrap_or("active") {
            "active" => {}
            "select" => {
                let d = parse(p.d.as_deref())?;
                doc.set_active_discipline(d);
                if let Some(d) = d {
                    doc.set_discipline_visible(d, true);
                }
            }
            "show" | "hide" => {
                let d = parse(p.d.as_deref())?
                    .ok_or_else(|| invalid("architecture is always shown"))?;
                doc.set_discipline_visible(d, p.action.as_deref() == Some("show"));
            }
            "quantities" => {
                let home = doc.home();
                let mut out = serde_json::Map::new();
                let mut lengths = serde_json::Map::new();
                for d in Discipline::ALL {
                    let key = serde_json::to_value(d)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_owned))
                        .unwrap_or_default();
                    let mut counts: std::collections::BTreeMap<String, usize> =
                        std::collections::BTreeMap::new();
                    for top in &home.furniture {
                        for piece in top.flatten() {
                            if piece.discipline.or(top.discipline) == Some(d) {
                                *counts.entry(piece.name.clone()).or_default() += 1;
                            }
                        }
                    }
                    out.insert(
                        key.clone(),
                        serde_json::json!(counts.into_iter().collect::<Vec<_>>()),
                    );
                    let length: f64 = home
                        .polylines
                        .iter()
                        .filter(|l| l.discipline == Some(d))
                        .map(|l| {
                            l.points
                                .windows(2)
                                .map(|s| s[0].distance(s[1]))
                                .sum::<f64>()
                        })
                        .sum();
                    lengths.insert(key, compact::num(length));
                }
                out.insert("lines_cm".into(), serde_json::Value::Object(lengths));
                return Ok(serde_json::Value::Object(out).to_string());
            }
            other => return Err(invalid(format!("unknown action `{other}`"))),
        }
        let home = doc.home();
        Ok(serde_json::json!({"active": home.active_discipline, "hidden": home.hidden_disciplines}).to_string())
    }

    #[tool(
        description = "Plan annotations. stale=true lists dimensions and notes that no longer match the drawing: rows [id, written, measured, against, text] — run it after moving geometry, before handing the plan over. q=<text> searches label text. Set any of dims (engineering dimension chains), refs (room reference schedule with tags), details (brand/model/link in refs), legend (symbol legend with counts); bake=true turns the automatic chains into editable dimensions (ids returned). Otherwise returns {dims,refs,details,rooms:[[room,[[tag,name,w,d,h,brand?,model?,url?]]]]}. Give pieces brand/model/url via update."
    )]
    fn annotations(
        &self,
        Parameters(p): Parameters<AnnotationParams>,
    ) -> Result<String, ErrorData> {
        if p.stale.unwrap_or(false) || p.q.is_some() {
            let doc = self.document.read();
            let view = doc.home().level_view(doc.home().current_level());
            let mut out = serde_json::Map::new();
            if p.stale.unwrap_or(false) {
                let rows: Vec<serde_json::Value> = newera_core::stale_annotations(&view)
                    .into_iter()
                    .map(|s| {
                        serde_json::json!([
                            s.id.to_string(),
                            compact::num(s.drawn),
                            compact::num(s.measured),
                            s.against.map(|a| a.to_string()),
                            s.text,
                        ])
                    })
                    .collect();
                out.insert("stale".to_owned(), serde_json::json!(rows));
            }
            if let Some(query) = &p.q {
                let needle = fold(query);
                let rows: Vec<serde_json::Value> = view
                    .labels
                    .iter()
                    .filter(|l| fold(&l.text).contains(&needle))
                    .map(compact::label)
                    .collect();
                out.insert("labels".to_owned(), serde_json::json!(rows));
            }
            return Ok(serde_json::Value::Object(out).to_string());
        }
        let mut doc = self.document.write();
        if p.bake.unwrap_or(false) {
            let view = doc.home().level_view(doc.home().current_level());
            let mut commands = Vec::new();
            let mut ids = Vec::new();
            for mut dim in newera_core::auto_dimensions(&view) {
                dim.id = doc.new_dimension_id();
                ids.push(dim.id.to_string());
                commands.push(Command::insert(dim));
            }
            let mut annotations = doc.home().annotations;
            annotations.auto_dimensions = false;
            commands.push(Command::SetAnnotations { annotations });
            doc.execute(Command::Batch { commands }).map_err(core)?;
            return Ok(ok(&doc, &ids));
        }
        let mut next = doc.home().annotations;
        next.auto_dimensions = p.dims.unwrap_or(next.auto_dimensions);
        next.references = p.refs.unwrap_or(next.references);
        next.reference_details = p.details.unwrap_or(next.reference_details);
        next.legend = p.legend.unwrap_or(next.legend);
        if next != doc.home().annotations {
            doc.execute(Command::SetAnnotations { annotations: next })
                .map_err(core)?;
        }
        let view = doc.home().level_view(doc.home().current_level());
        let rooms: Vec<serde_json::Value> = newera_core::room_references(&view)
            .into_iter()
            .map(|g| {
                let items: Vec<serde_json::Value> = g
                    .items
                    .iter()
                    .map(|i| {
                        let mut row = vec![
                            serde_json::json!(i.tag),
                            serde_json::json!(i.name),
                            compact::num(i.size[0]),
                            compact::num(i.size[1]),
                            compact::num(i.size[2]),
                        ];
                        if i.brand.is_some() || i.model.is_some() || i.url.is_some() {
                            row.extend([
                                serde_json::json!(i.brand),
                                serde_json::json!(i.model),
                                serde_json::json!(i.url),
                            ]);
                        }
                        serde_json::Value::Array(row)
                    })
                    .collect();
                serde_json::json!([g.name, items])
            })
            .collect();
        Ok(serde_json::json!({
            "rev": doc.revision(),
            "dims": next.auto_dimensions,
            "refs": next.references,
            "details": next.reference_details,
            "legend": next.legend,
            "rooms": rooms,
        })
        .to_string())
    }

    #[tool(
        description = "Plugins (external programs editing through the HTTP API). list (default): rows [name,title,description]. run {name,args?}: {ok,code,stdout,stderr,edits,revision}."
    )]
    fn plugins(&self, Parameters(p): Parameters<PluginsParams>) -> Result<String, ErrorData> {
        let dirs = newera_plugins::plugin_dirs();
        match p.action.as_deref().unwrap_or("list") {
            "list" => {
                let rows: Vec<serde_json::Value> = newera_plugins::discover(&dirs)
                    .iter()
                    .map(|p| serde_json::json!([p.name, p.title, p.description]))
                    .collect();
                Ok(serde_json::json!({ "rows": rows }).to_string())
            }
            "run" => {
                let name = p
                    .name
                    .as_deref()
                    .ok_or_else(|| invalid("`name` is required"))?;
                let args = p.args.unwrap_or(serde_json::Value::Null);
                newera_plugins::run_for_document(&self.document, &dirs, name, &args)
                    .map(|v| v.to_string())
                    .map_err(|e| invalid(e.to_string()))
            }
            other => Err(invalid(format!("unknown action `{other}`"))),
        }
    }

    #[tool(
        description = "People and agents on this project now: rows [id,name,cursor,selection,edits]."
    )]
    fn sessions(&self) -> String {
        let mut doc = self.document.write();
        doc.sessions_mut().expire(newera_core::collab::now_ms());
        let rows: Vec<serde_json::Value> = doc
            .sessions()
            .list()
            .iter()
            .map(|s| {
                serde_json::json!([
                    s.id,
                    s.name,
                    s.cursor.map(|c| [compact::num(c.x), compact::num(c.y)]),
                    s.selection
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>(),
                    s.edits
                ])
            })
            .collect();
        serde_json::json!({ "rev": doc.revision(), "rows": rows }).to_string()
    }

    #[tool(
        description = "Points of view. list (default): {active, rows [i,name,x,y,z,yaw,pitch,fov]}. view {i} shows stored view i in the 3D window; aerial returns to the orbit view; store {name?,x?,y?,z?,yaw?,pitch?,fov?,look_at?:[x,y,z]} saves one (missing values from the visitor; yaw 0 looks toward +y/plan bottom, 90 toward -x; pitch positive looks down); delete {i}. cm and degrees."
    )]
    fn cameras(&self, Parameters(p): Parameters<CamerasParams>) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        let mut cameras = doc.home().cameras.clone();
        let index = |len: usize| -> Result<usize, ErrorData> {
            p.i.filter(|i| *i < len)
                .ok_or_else(|| invalid(format!("`i` must be below {len}")))
        };
        match p.action.as_deref().unwrap_or("list") {
            "list" => {
                let rows: Vec<serde_json::Value> = cameras
                    .stored
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        serde_json::json!([
                            i,
                            c.name,
                            compact::num(c.x),
                            compact::num(c.y),
                            compact::num(c.z),
                            compact::num(c.yaw),
                            compact::num(c.pitch),
                            compact::num(c.fov)
                        ])
                    })
                    .collect();
                return Ok(serde_json::json!({"active": if cameras.observer_active { "visitor" } else { "aerial" }, "rows": rows}).to_string());
            }
            "view" => {
                let i = index(cameras.stored.len())?;
                cameras.observer = cameras.stored[i].clone();
                cameras.observer_active = true;
            }
            "aerial" => cameras.observer_active = false,
            "store" => {
                let base = cameras.observer.clone();
                let camera = newera_core::Camera {
                    name: p
                        .name
                        .clone()
                        .or_else(|| Some(format!("Ponto de vista {}", cameras.stored.len() + 1))),
                    x: p.x.unwrap_or(base.x),
                    y: p.y.unwrap_or(base.y),
                    z: p.z.unwrap_or(base.z),
                    yaw: p.yaw.unwrap_or(base.yaw),
                    pitch: p.pitch.unwrap_or(base.pitch),
                    fov: p.fov.unwrap_or(base.fov),
                    ..base
                };
                let mut camera = camera;
                if let Some([tx, ty, tz]) = p.look_at {
                    let (dx, dy, dz) = (tx - camera.x, ty - camera.y, tz - camera.z);
                    // `Camera::direction` is (-sin yaw, cos yaw) on the plan.
                    camera.yaw = (-dx).atan2(dy).to_degrees();
                    camera.pitch = (-dz).atan2(dx.hypot(dy)).to_degrees();
                }
                cameras.stored.push(camera);
            }
            "delete" => {
                let i = index(cameras.stored.len())?;
                cameras.stored.remove(i);
            }
            other => return Err(invalid(format!("unknown action `{other}`"))),
        }
        doc.execute(Command::SetCameras { cameras }).map_err(core)?;
        Ok(ok(&doc, &[]))
    }

    #[tool(
        description = "Video camera path. list (default): {fps,speed,secs,rows [i,x,y,z,yaw,pitch,fov]}. add {cam? | x?,y?,z?,yaw?,pitch?,fov?, i?} appends a keyframe (missing values from the visitor); delete {i}; clear; orbit {z?,n?} replaces the path with an aerial tour; set {fps?,speed?}; render {path .avi, w?,h?} writes a Motion-JPEG video. cm, degrees, m/s."
    )]
    fn video(&self, Parameters(p): Parameters<VideoParams>) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        let mut environment = doc.home().environment.clone();
        let path = &mut environment.camera_path;
        match p.action.as_deref().unwrap_or("list") {
            "list" => {
                let rows: Vec<serde_json::Value> = path
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        serde_json::json!([
                            i,
                            compact::num(c.x),
                            compact::num(c.y),
                            compact::num(c.z),
                            compact::num(c.yaw),
                            compact::num(c.pitch),
                            compact::num(c.fov)
                        ])
                    })
                    .collect();
                let video = &environment.video;
                let secs: f64 = newera_render::video::segment_durations(path, video.speed)
                    .iter()
                    .sum();
                return Ok(serde_json::json!({
                    "fps": video.frame_rate,
                    "speed": compact::num(video.speed),
                    "secs": compact::num(secs),
                    "rows": rows,
                })
                .to_string());
            }
            "add" => {
                let cameras = &doc.home().cameras;
                let base = match p.cam {
                    Some(i) => cameras
                        .stored
                        .get(i)
                        .cloned()
                        .ok_or_else(|| invalid(format!("no stored camera {i}")))?,
                    None => cameras.observer.clone(),
                };
                let camera = newera_core::Camera {
                    name: None,
                    x: p.x.unwrap_or(base.x),
                    y: p.y.unwrap_or(base.y),
                    z: p.z.unwrap_or(base.z),
                    yaw: p.yaw.unwrap_or(base.yaw),
                    pitch: p.pitch.unwrap_or(base.pitch),
                    fov: p.fov.unwrap_or(base.fov),
                    ..base
                };
                let at = p.i.unwrap_or(path.len()).min(path.len());
                path.insert(at, camera);
            }
            "delete" => {
                let i =
                    p.i.filter(|i| *i < path.len())
                        .ok_or_else(|| invalid(format!("`i` must be below {}", path.len())))?;
                path.remove(i);
            }
            "clear" => path.clear(),
            "orbit" => {
                *path = newera_render::video::orbit_path(
                    doc.home(),
                    p.z.unwrap_or(800.0),
                    p.n.unwrap_or(8),
                );
            }
            "set" => {
                if let Some(fps) = p.fps {
                    environment.video.frame_rate = fps.clamp(1, 60);
                }
                if let Some(speed) = p.speed {
                    environment.video.speed = speed.clamp(0.05, 50.0);
                }
            }
            "render" => {
                let file = PathBuf::from(
                    p.path
                        .as_deref()
                        .ok_or_else(|| invalid("`path` is required"))?,
                );
                let home = doc.home().clone();
                let assets = doc.asset_dir();
                drop(doc);
                let (w, h) = (
                    p.w.unwrap_or(640).clamp(64, 1920),
                    p.h.unwrap_or(360).clamp(64, 1080),
                );
                let video = &home.environment.video;
                let info = newera_render::video::render_video(
                    &home,
                    &home.environment.camera_path,
                    video.frame_rate,
                    video.speed,
                    (w, h),
                    assets.as_deref(),
                    &file,
                    |_, _| {},
                )
                .map_err(invalid)?;
                return Ok(serde_json::json!({
                    "frames": info.frames,
                    "secs": compact::num(info.seconds),
                    "bytes": info.bytes,
                })
                .to_string());
            }
            other => return Err(invalid(format!("unknown action `{other}`"))),
        }
        doc.execute(Command::SetEnvironment { environment })
            .map_err(core)?;
        Ok(ok(&doc, &[]))
    }

    #[tool(
        description = "Storeys. list (default): rows [id,name,elev,h,selected,layout_index,viewable,reference]. add {name?,h?,elev?} adds one on top (or at elev cm) and selects it; select {id}; delete {id} removes it and its content. update {id, elev?|h?|name?|reference?}: elev raises a storey with its walls, floors and openings (houses on stilts); reference=true marks it a tracing layer (imported plan, older version) that checks and ergonomics skip, which is what you want when two storeys share an elevation. Other tools act on the selected storey."
    )]
    fn levels(&self, Parameters(p): Parameters<LevelsParams>) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        let id = || -> Result<newera_core::LevelId, ErrorData> {
            p.id.as_deref()
                .ok_or_else(|| invalid("`id` is required"))?
                .parse()
                .map_err(|e| invalid(format!("{e}")))
        };
        match p.action.as_deref().unwrap_or("list") {
            "list" => Ok(compact::levels(doc.home()).to_string()),
            "add" => {
                let level = ops::add_level(&mut doc, p.name.clone(), p.h).map_err(core)?;
                if let Some(elev) = p.elev
                    && let Some(mut raised) = doc.home().level(level).cloned()
                {
                    raised.elevation = elev;
                    doc.execute(Command::update(raised)).map_err(core)?;
                }
                Ok(ok(&doc, &[level.to_string()]))
            }
            "select" => {
                let level = id()?;
                if doc.home().level(level).is_none() {
                    return Err(invalid(format!("{level} not found")));
                }
                doc.select_level(Some(level));
                Ok(ok(&doc, &[]))
            }
            "update" => {
                let level = id()?;
                let mut updated = doc
                    .home()
                    .level(level)
                    .cloned()
                    .ok_or_else(|| invalid(format!("{level} not found")))?;
                if let Some(elev) = p.elev {
                    updated.elevation = elev;
                }
                if let Some(h) = p.h {
                    updated.height = h;
                }
                if let Some(name) = p.name.clone() {
                    updated.name = name;
                }
                if let Some(reference) = p.reference {
                    updated.set_reference(reference);
                }
                doc.execute(Command::update(updated)).map_err(core)?;
                Ok(ok(&doc, &[]))
            }
            "delete" => {
                ops::delete_level(&mut doc, id()?).map_err(core)?;
                Ok(ok(&doc, &[]))
            }
            other => Err(invalid(format!("unknown action `{other}`"))),
        }
    }

    #[tool(
        description = "Plan versions (tabs). list: rows [i,name,active,walls,rooms,m2,furniture,issues]. duplicate/new switch to the new one; edits apply to the active version."
    )]
    fn variants(&self, Parameters(p): Parameters<VariantsParams>) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        let need = |i: Option<usize>| i.ok_or_else(|| invalid("`i` is required"));
        match p.action.as_deref().unwrap_or("list") {
            "list" => Ok(compact::variants(&doc).to_string()),
            "duplicate" | "new" => {
                let index = doc.add_variant(p.name, p.action.as_deref() == Some("duplicate"));
                Ok(format!("ok rev={} v={index} i={index}", doc.revision()))
            }
            "switch" => {
                doc.switch_variant(need(p.i)?).map_err(core)?;
                Ok(ok(&doc, &[]))
            }
            "rename" => {
                let name = p.name.ok_or_else(|| invalid("`name` is required"))?;
                doc.rename_variant(need(p.i)?, name).map_err(core)?;
                Ok(ok(&doc, &[]))
            }
            "delete" => {
                doc.remove_variant(need(p.i)?).map_err(core)?;
                Ok(ok(&doc, &[]))
            }
            other => Err(invalid(format!("unknown action `{other}`"))),
        }
    }

    #[tool(description = "Undo the last change, whoever made it.")]
    fn undo(&self) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        doc.undo().map_err(core)?;
        Ok(ok(&doc, &[]))
    }

    #[tool(description = "Redo the last undone change.")]
    fn redo(&self) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        doc.redo().map_err(core)?;
        Ok(ok(&doc, &[]))
    }
}

impl NewEraMcp {
    fn render(
        &self,
        w: u32,
        h: u32,
        region: Option<[Point2; 2]>,
        grid: bool,
    ) -> Result<Vec<u8>, ErrorData> {
        self.render_with(w, h, region, grid, None)
    }

    fn render_with(
        &self,
        w: u32,
        h: u32,
        region: Option<[Point2; 2]>,
        grid: bool,
        bg: Option<f64>,
    ) -> Result<Vec<u8>, ErrorData> {
        let (w, h) = (w.clamp(64, 2048), h.clamp(64, 2048));
        let doc = self.document.read();
        let mut view = doc.home().level_view(doc.home().current_level());
        if let Some(opacity) = bg {
            let opacity = opacity.clamp(0.0, 1.0);
            let level = view.current_level();
            let target = match level.and_then(|id| view.levels.iter_mut().find(|l| l.id == id)) {
                Some(level) if level.background.is_some() => level.background.as_mut(),
                _ => view.background.as_mut(),
            };
            if let Some(background) = target {
                background.opacity = opacity;
                background.visible = opacity > 0.0;
            }
        }
        let scene = plan_scene(&view, &scene_options_for(&doc));
        let project = doc.asset_dir();
        drop(doc);
        let options = RenderOptions {
            width: w,
            height: h,
            grid,
            region: region.map(|[a, b]| (a, b)),
            ..RenderOptions::default()
        };
        let load = |path: &str| {
            image::open(newera_core::resolve_asset(project.as_deref(), path))
                .ok()
                .map(|img| img.to_rgba8())
        };
        render_png(&scene, &options, &load)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))
    }
}

// The macro generates async trait methods that resolve immediately.
#[allow(clippy::unused_async_trait_impl)]
#[tool_handler(router = self.tool_router)]
impl ServerHandler for NewEraMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new("3d-new-era-ai", env!("CARGO_PKG_VERSION"))
                    .with_title("3D New Era AI"),
            )
            .with_instructions(INSTRUCTIONS)
    }
}

/// Plan options as the user sees them: backgrounds, and top views for
/// imported models.
fn scene_options_for(doc: &Document) -> SceneOptions {
    let views = newera_render::TopViews::new(
        newera_core::cache_dir().join("topviews"),
        doc.asset_dir(),
        false,
    );
    SceneOptions {
        show_background: true,
        piece_images: Some(newera_draw::PieceImages(std::sync::Arc::new(
            move |piece| views.image_for(piece),
        ))),
        ..SceneOptions::default()
    }
}

fn with_extension(path: PathBuf) -> PathBuf {
    if path.extension().is_some() {
        path
    } else {
        path.with_extension(newera_core::PROJECT_EXTENSION)
    }
}

/// `ok rev=N [v=I] [ids=…]`; the active variant is named once there are
/// several, so an agent notices when it writes to another tab than it meant.
fn ok(doc: &Document, ids: &[String]) -> String {
    use std::fmt::Write as _;
    let mut reply = format!("ok rev={}", doc.revision());
    if doc.variant_count() > 1 {
        let _ = write!(reply, " v={}", doc.active_variant());
    }
    if !ids.is_empty() {
        let _ = write!(reply, " ids={}", ids.join(","));
    }
    reply
}

/// Background image placement, to read coordinates given in its pixels.
struct BackgroundScale {
    image: newera_core::BackgroundImage,
    scale: f64,
}

impl BackgroundScale {
    fn point(&self, px: Point2) -> Point2 {
        self.image.plan_point(px)
    }
}

fn background_scale(doc: &Document) -> Result<BackgroundScale, ErrorData> {
    let home = doc.home();
    let bg = home
        .current_level()
        .and_then(|id| home.level(id))
        .and_then(|l| l.background.as_ref())
        .or(home.background.as_ref())
        .ok_or_else(|| invalid("`px` needs a background image (set_background)"))?;
    Ok(BackgroundScale {
        image: bg.clone(),
        scale: bg.cm_per_px,
    })
}

/// Makes version `v` the active one before a write, so the write lands where
/// the agent means even if someone switched tabs meanwhile.
fn on_variant(doc: &mut Document, v: Option<usize>) -> Result<(), ErrorData> {
    match v {
        Some(v) if v != doc.active_variant() => doc.switch_variant(v).map_err(core),
        _ => Ok(()),
    }
}

/// The elements with these ids, grouped by kind like a full read.
///
/// Resolving an id was the commonest thing an agent wanted and the one thing
/// a read could not do: the answer was to dump the whole home and search it.
fn picked(home: &Home, ids: &[String]) -> Result<serde_json::Value, ErrorData> {
    let mut out = serde_json::Map::new();
    for raw in ids {
        let id: newera_core::ElementId = raw.parse().map_err(|e| invalid(format!("{e}")))?;
        let Some(value) = compact::element(home, id) else {
            return Err(invalid(format!("no {raw} on this storey")));
        };
        out.entry(compact::kind_of(id).to_owned())
            .or_insert_with(|| serde_json::Value::Array(Vec::new()))
            .as_array_mut()
            .expect("array")
            .push(value);
    }
    Ok(serde_json::Value::Object(out))
}

/// Lists what is inside every group, next to the group that holds it.
///
/// A group is otherwise a black box: `parts: 23` and nothing else, so there
/// is no way to see whether an edit rebuilt its insides.
fn expand_parts(home: &Home, out: &mut serde_json::Value) {
    let Some(list) = out.get_mut("furniture").and_then(|v| v.as_array_mut()) else {
        return;
    };
    let cuts = home.wall_cuts();
    let mut expanded = Vec::with_capacity(list.len());
    for value in list.drain(..) {
        let group = value["id"]
            .as_str()
            .and_then(|raw| raw.parse().ok())
            .and_then(|id| home.find_piece(id))
            .filter(|f| f.is_group());
        let parts: Vec<serde_json::Value> = group
            .map(|g| {
                g.flatten()
                    .into_iter()
                    .skip(1)
                    .map(|f| compact::piece(home, &cuts, f))
                    .collect()
            })
            .unwrap_or_default();
        let mut value = value;
        if !parts.is_empty() {
            value["inside"] = serde_json::Value::Array(parts);
        }
        expanded.push(value);
    }
    *list = expanded;
}

/// Applies `kinds`, `room`, `rect` and `fields` to a read.
fn narrow(home: &Home, out: &mut serde_json::Value, p: &GetHomeParams) -> Result<(), ErrorData> {
    if let Some(kinds) = &p.kinds {
        for kind in kinds {
            if !compact::KINDS.contains(&kind.as_str()) {
                return Err(invalid(format!(
                    "kind `{kind}`: one of {}",
                    compact::KINDS.join(", ")
                )));
            }
        }
        for kind in compact::KINDS {
            if !kinds.iter().any(|k| k == kind) {
                out.as_object_mut().expect("object").remove(kind);
            }
        }
    }

    // A room and a rectangle are the same filter: a box everything is
    // tested against, so asking for both keeps only what meets both.
    let mut boxes: Vec<(Point2, Point2)> = Vec::new();
    if let Some(raw) = &p.room {
        let room = home
            .rooms
            .iter()
            .find(|r| r.id.to_string() == *raw || r.name.eq_ignore_ascii_case(raw))
            .ok_or_else(|| invalid(format!("no room `{raw}` on this storey")))?;
        let (min, max) = newera_core::element_bounds(home, room.id.into())
            .ok_or_else(|| invalid("that room has no outline"))?;
        boxes.push((min, max));
    }
    if let Some([[x0, y0], [x1, y1]]) = p.rect {
        boxes.push((
            Point2::new(x0.min(x1), y0.min(y1)),
            Point2::new(x0.max(x1), y0.max(y1)),
        ));
    }
    if !boxes.is_empty() {
        let meets = |id: newera_core::ElementId| {
            newera_core::element_bounds(home, id).is_some_and(|(min, max)| {
                boxes.iter().all(|(lo, hi)| {
                    min.x <= hi.x && lo.x <= max.x && min.y <= hi.y && lo.y <= max.y
                })
            })
        };
        for kind in compact::KINDS {
            if let Some(list) = out.get_mut(kind).and_then(|v| v.as_array_mut()) {
                list.retain(|e| {
                    e["id"]
                        .as_str()
                        .and_then(|raw| raw.parse().ok())
                        .is_some_and(meets)
                });
            }
        }
    }

    if let Some(fields) = &p.fields {
        for kind in compact::KINDS {
            let Some(list) = out.get_mut(kind).and_then(|v| v.as_array_mut()) else {
                continue;
            };
            for element in list {
                if let Some(map) = element.as_object_mut() {
                    map.retain(|k, _| k == "id" || fields.iter().any(|f| f == k));
                }
            }
        }
    }
    // Empty arrays say nothing; dropping them keeps "not here" unambiguous.
    for kind in compact::KINDS {
        if out
            .get(kind)
            .and_then(|v| v.as_array())
            .is_some_and(Vec::is_empty)
        {
            out.as_object_mut().expect("object").remove(kind);
        }
    }
    Ok(())
}

/// One element per line, each tagged with its kind, headers last.
///
/// A single 50 KB line cannot be read in slices by any normal tool, which
/// forces a script; one line per element can.
fn ndjson(out: &serde_json::Value) -> String {
    let mut lines = Vec::new();
    let mut head = out.clone();
    for kind in compact::KINDS {
        let Some(list) = head.as_object_mut().and_then(|m| m.remove(kind)) else {
            continue;
        };
        for element in list.as_array().into_iter().flatten() {
            let mut row = serde_json::json!({ "k": kind });
            if let Some(map) = element.as_object() {
                for (k, v) in map {
                    row[k] = v.clone();
                }
            }
            lines.push(row.to_string());
        }
    }
    lines.insert(0, head.to_string());
    lines.join("\n")
}

/// Runs an edit against a copy of the plan and reports what it would do.
///
/// Trying a size used to mean applying it, reviewing, and undoing — a round
/// trip that showed in the user's window and burned a revision each time.
/// The copy has no history and is thrown away, so nothing of that happens.
fn preview(
    doc: &Document,
    apply: impl FnOnce(&mut Document) -> Result<(), ErrorData>,
) -> Result<String, ErrorData> {
    let before = doc.home().clone();
    let mut scratch = Document::new(before.clone());
    apply(&mut scratch)?;
    let after = scratch.home().clone();
    let mut out = compact::diff(&before, &after);
    let object = out.as_object_mut().expect("object");
    object.insert("dry".to_owned(), serde_json::json!(true));

    // Free floor around every piece the change touched: the number the
    // change was made for, without a second call.
    let touched: Vec<newera_core::FurnitureId> = out["changed"]
        .as_array()
        .into_iter()
        .flatten()
        .chain(out["added"].as_array().into_iter().flatten())
        .filter_map(|c| {
            let raw = if c.is_string() {
                c.as_str()?
            } else {
                c["id"].as_str()?
            };
            raw.parse().ok()
        })
        .collect();
    let mut clearances = serde_json::Map::new();
    let view = after.level_view(after.current_level());
    // Gathered once for the storey, not once per piece per side.
    let solids = newera_core::obstacles(&view, &|_| false);
    for id in touched.iter().take(12) {
        let Some(piece) = view.find_piece(*id) else {
            continue;
        };
        let sides: serde_json::Map<String, serde_json::Value> = newera_core::Dir::PLAN
            .iter()
            .map(|dir| {
                let c = newera_core::measure::clearance_against(
                    &solids,
                    piece,
                    *dir,
                    newera_core::measure::MAX_REACH,
                );
                (
                    dir.name().to_owned(),
                    serde_json::json!([
                        compact::num(c.cm),
                        c.against.map(|s| s.id().to_string()),
                        c.name
                    ]),
                )
            })
            .collect();
        clearances.insert(id.to_string(), serde_json::Value::Object(sides));
    }
    if !clearances.is_empty() {
        out.as_object_mut().expect("object").insert(
            "clearances".to_owned(),
            serde_json::Value::Object(clearances),
        );
    }

    // Which findings it would settle, and which it would create.
    let defects = |home: &newera_core::Home| -> std::collections::BTreeSet<String> {
        newera_core::check_layout(&home.level_view(home.current_level()))
            .into_iter()
            .filter(newera_core::Issue::is_defect)
            .map(|i| {
                i.ids()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("+")
            })
            .collect()
    };
    let (was, now) = (defects(&before), defects(&after));
    for (key, list) in [
        ("issues_resolved", was.difference(&now).collect::<Vec<_>>()),
        ("issues_new", now.difference(&was).collect()),
    ] {
        if !list.is_empty() {
            out.as_object_mut()
                .expect("object")
                .insert(key.to_owned(), serde_json::json!(list));
        }
    }

    let profile = newera_ergonomics::Profile::default();
    let (was, now) = (
        newera_ergonomics::review(&before, &profile),
        newera_ergonomics::review(&after, &profile),
    );
    // Findings are matched without their numbers, so one that merely got
    // better reads as improved rather than as one gone and one new.
    let key = |f: &newera_ergonomics::Finding| {
        let text: String = f.message.chars().filter(|c| !c.is_ascii_digit()).collect();
        format!("{} {text}", f.place)
    };
    let old_keys: std::collections::BTreeSet<String> = was.findings.iter().map(&key).collect();
    let new_keys: std::collections::BTreeSet<String> = now.findings.iter().map(&key).collect();
    let object = out.as_object_mut().expect("object");
    if was.score != now.score {
        object.insert(
            "score".to_owned(),
            serde_json::json!([was.score, now.score]),
        );
    }
    for (label, findings, other) in [
        ("resolved", &was.findings, &new_keys),
        ("new_findings", &now.findings, &old_keys),
    ] {
        let list: Vec<serde_json::Value> = findings
            .iter()
            .filter(|f| !other.contains(&key(f)))
            .map(|f| serde_json::json!([f.severity, f.place, f.message]))
            .collect();
        if !list.is_empty() {
            object.insert(label.to_owned(), serde_json::json!(list));
        }
    }
    Ok(serde_json::Value::Object(object.clone()).to_string())
}

/// How many changed elements a write names before it just counts them.
const DIFF_LIMIT: usize = 20;

/// `ok rev=N` with what the write actually did appended.
///
/// A write that says only `ok` forces a full read to learn its effect —
/// which is how a plan ends up edited blind. A batch touching hundreds of
/// pieces is counted instead of listed: past a point the list is the read
/// it was meant to save.
fn applied(doc: &Document, before: &Home) -> String {
    let mut diff = compact::diff(before, doc.home());
    let object = diff.as_object_mut().expect("object");
    if object.is_empty() {
        return ok(doc, &[]);
    }
    for key in ["changed", "added", "gone"] {
        let Some(list) = object.get_mut(key).and_then(|v| v.as_array_mut()) else {
            continue;
        };
        if list.len() > DIFF_LIMIT {
            let n = list.len();
            *object.get_mut(key).expect("present") = serde_json::json!(n);
        }
    }
    format!("{} {diff}", ok(doc, &[]))
}

/// Lowercased and stripped of accents, so `porta` finds `Portão` and a
/// query typed without accents still matches a plan written with them.
fn fold(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'ê' | 'ë' => 'e',
            'í' | 'î' | 'ï' => 'i',
            'ó' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn invalid(message: impl Into<String>) -> ErrorData {
    ErrorData::invalid_params(message.into(), None)
}

#[allow(clippy::needless_pass_by_value)] // used as `map_err(core)`
fn core(err: newera_core::CoreError) -> ErrorData {
    invalid(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server() -> NewEraMcp {
        NewEraMcp::new(SharedDocument::new(Document::default()))
    }

    #[test]
    fn create_then_render_returns_a_png_image() {
        let s = server();
        let params: CreateParams = serde_json::from_str(
            r#"{"walls":[{"pts":[[0,0],[400,0],[400,300],[0,300]],"closed":true}],"rooms":[{"name":"Sala","at":[200,150]}]}"#,
        )
        .unwrap();
        assert_eq!(
            s.create(Parameters(params)).unwrap(),
            "ok rev=1 ids=w1,w2,w3,w4,r5"
        );
        let result = s
            .render_plan(Parameters(RenderParams {
                w: Some(200),
                h: Some(150),
                ..RenderParams::default()
            }))
            .unwrap();
        let ContentBlock::Image(image) = &result.content[0] else {
            panic!("expected image")
        };
        assert_eq!(image.mime_type, "image/png");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&image.data)
            .unwrap();
        assert_eq!(&bytes[1..4], b"PNG");
    }

    #[test]
    fn variants_duplicate_switch_and_list() {
        let s = server();
        let params: CreateParams =
            serde_json::from_str(r#"{"walls":[{"pts":[[0,0],[300,0]]}]}"#).unwrap();
        s.create(Parameters(params)).unwrap();
        let reply = s
            .variants(Parameters(VariantsParams {
                action: Some("duplicate".into()),
                name: Some("B".into()),
                i: None,
            }))
            .unwrap();
        assert!(reply.ends_with("i=1"), "{reply}");
        let params: CreateParams =
            serde_json::from_str(r#"{"walls":[{"pts":[[0,100],[300,100]]}]}"#).unwrap();
        s.create(Parameters(params)).unwrap();
        let list = s.variants(Parameters(VariantsParams::default())).unwrap();
        assert_eq!(
            list,
            r#"[[0,"Versão 1",false,1,0,0.0,0,0],[1,"B",true,2,0,0.0,0,0]]"#
        );
        s.variants(Parameters(VariantsParams {
            action: Some("switch".into()),
            i: Some(0),
            name: None,
        }))
        .unwrap();
        assert_eq!(s.document.read().home().walls.len(), 1);
        assert!(
            s.variants(Parameters(VariantsParams {
                action: Some("switch".into()),
                i: Some(9),
                name: None
            }))
            .is_err()
        );
    }

    #[test]
    fn save_open_round_trip() {
        let s = server();
        let dir = std::env::temp_dir().join(format!("newera-mcp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("casa");
        let params: CreateParams =
            serde_json::from_str(r#"{"labels":[{"text":"Oi","at":[1,2]}]}"#).unwrap();
        s.create(Parameters(params)).unwrap();
        let reply = s
            .save_home(Parameters(PathParams {
                path: Some(path.display().to_string()),
            }))
            .unwrap();
        assert!(reply.ends_with("casa.newera"), "{reply}");
        s.new_home();
        assert!(s.document.read().home().labels.is_empty());
        s.open_home(Parameters(PathParams {
            path: Some(dir.join("casa.newera").display().to_string()),
        }))
        .unwrap();
        assert_eq!(s.document.read().home().labels[0].text, "Oi");
        assert!(!s.document.read().is_modified());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn reads_answer_by_id_room_and_rectangle_instead_of_dumping_everything() {
        let s = server();
        s.create(Parameters(
            serde_json::from_str(
                r#"{"walls":[{"pts":[[0,0],[500,0],[500,400],[0,400]],"closed":true}],
                    "rooms":[{"name":"Sala","at":[250,200]}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        s.place(Parameters(
            serde_json::from_str(
                r#"{"items":[{"cat":"sofa-3","at":[100,100]},
                             {"cat":"dining-table-4","at":[400,300],"angle":90,"w":140,"d":80}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        let read = |json: &str| -> serde_json::Value {
            serde_json::from_str(
                &s.get_home(Parameters(serde_json::from_str(json).unwrap()))
                    .unwrap(),
            )
            .unwrap()
        };

        // One id, resolved, instead of the whole home.
        let one = read(r#"{"ids":["f6"]}"#);
        assert_eq!(one["furniture"].as_array().unwrap().len(), 1, "{one}");
        assert_eq!(one["furniture"][0]["id"], "f6");
        assert!(one.get("walls").is_none(), "{one}");
        assert!(
            s.get_home(Parameters(
                serde_json::from_str(r#"{"ids":["f999"]}"#).unwrap()
            ))
            .is_err(),
            "an id that is not there is an error, not silence"
        );

        // A quarter turn swaps width and depth: `bounds` is already resolved
        // and `faces` says which way the piece opens.
        let turned = read(r#"{"ids":["f7"],"fields":["bounds","faces","wdh"]}"#);
        let piece = &turned["furniture"][0];
        let (w, d) = (
            piece["wdh"][0].as_f64().unwrap(),
            piece["wdh"][1].as_f64().unwrap(),
        );
        let bounds = &piece["bounds"];
        let span_x = bounds[1][0].as_f64().unwrap() - bounds[0][0].as_f64().unwrap();
        let span_y = bounds[1][1].as_f64().unwrap() - bounds[0][1].as_f64().unwrap();
        assert!(
            (span_x - d).abs() < 0.1 && (span_y - w).abs() < 0.1,
            "{piece}"
        );
        assert_eq!(piece["faces"], "-x", "{piece}");
        assert!(piece.get("at").is_none(), "fields trims the rest: {piece}");

        // A rectangle around the sofa leaves the table out.
        let corner = read(r#"{"rect":[[0,0],[200,200]],"kinds":["furniture"]}"#);
        let ids: Vec<&str> = corner["furniture"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["f6"], "{corner}");
        assert!(corner.get("walls").is_none(), "kinds drops the rest");
        assert!(corner.get("rooms").is_none(), "{corner}");

        // The room by name reaches everything standing in it.
        let sala = read(r#"{"room":"Sala","kinds":["furniture"]}"#);
        assert_eq!(sala["furniture"].as_array().unwrap().len(), 2, "{sala}");

        // NDJSON: one element per line, so a long answer can be read in slices.
        let lines = s
            .get_home(Parameters(
                serde_json::from_str(r#"{"ndjson":true,"kinds":["furniture"]}"#).unwrap(),
            ))
            .unwrap();
        let rows: Vec<&str> = lines.lines().collect();
        assert_eq!(rows.len(), 3, "a header and two pieces: {lines}");
        for row in &rows[1..] {
            let value: serde_json::Value = serde_json::from_str(row).unwrap();
            assert_eq!(value["k"], "furniture", "{row}");
        }
    }

    #[test]
    fn measure_answers_clearances_gaps_and_corridors() {
        let s = server();
        s.create(Parameters(
            serde_json::from_str(
                r#"{"walls":[{"pts":[[0,0],[500,0],[500,400],[0,400]],"closed":true}],
                    "rooms":[{"name":"Cozinha","at":[250,200]}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        // Two counters facing each other across the room.
        s.place(Parameters(
            serde_json::from_str(
                r#"{"items":[{"cat":"base-cabinet","at":[250,40],"w":300,"d":60,"h":90},
                             {"cat":"base-cabinet","at":[250,340],"w":300,"d":60,"h":90}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        let measure = |json: &str| -> serde_json::Value {
            serde_json::from_str(
                &s.measure(Parameters(serde_json::from_str(json).unwrap()))
                    .unwrap(),
            )
            .unwrap()
        };

        // Free floor in front of the first counter: 70 → 310 is 240 cm.
        let clear = measure(r#"{"from":"f6"}"#);
        assert!(
            (clear["clear"]["+y"][0].as_f64().unwrap() - 240.0).abs() < 0.5,
            "{clear}"
        );
        assert_eq!(clear["clear"]["+y"][1], "f7", "{clear}");
        // And behind it, 2.5 cm to the inner face of a 15 cm wall.
        assert!(
            (clear["clear"]["-y"][0].as_f64().unwrap() - 2.5).abs() < 0.5,
            "{clear}"
        );
        assert_eq!(clear["clear"]["-y"][1], "w1", "{clear}");

        // The same number as the gap between the two boxes.
        let gap = measure(r#"{"from":"f6","to":"f7","axis":"y"}"#);
        assert!((gap["cm"].as_f64().unwrap() - 240.0).abs() < 0.5, "{gap}");

        // A probe down the middle: wall, counter, corridor, counter, wall.
        let spans = measure(r#"{"axis":"y","at":250}"#);
        let rows = spans["spans"].as_array().unwrap();
        let free: Vec<f64> = rows
            .iter()
            .filter(|r| r[2].is_null())
            .map(|r| r[1].as_f64().unwrap() - r[0].as_f64().unwrap())
            .collect();
        assert!(
            free.iter().any(|cm| (cm - 240.0).abs() < 0.5),
            "the corridor is one free stretch: {spans}"
        );

        // Two points, plainly.
        let straight = measure(r#"{"from":[0,0],"to":[30,40]}"#);
        assert!(
            (straight["cm"].as_f64().unwrap() - 50.0).abs() < 0.01,
            "{straight}"
        );
    }

    #[test]
    fn a_reference_storey_is_drawing_and_checks_leave_it_alone() {
        let s = server();
        s.create(Parameters(
            serde_json::from_str(
                r#"{"walls":[{"pts":[[0,0],[500,0],[500,400],[0,400]],"closed":true}],
                    "rooms":[{"name":"Sala","at":[250,200]}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        s.place(Parameters(
            serde_json::from_str(r#"{"items":[{"cat":"sofa-3","at":[250,200]}]}"#).unwrap(),
        ))
        .unwrap();
        // A second storey at the same elevation, holding a copy of the plan.
        s.levels(Parameters(LevelsParams {
            action: Some("add".into()),
            name: Some("Novo layout".into()),
            elev: Some(0.0),
            ..LevelsParams::default()
        }))
        .unwrap();
        s.place(Parameters(
            serde_json::from_str(r#"{"items":[{"cat":"sofa-3","at":[250,200]}]}"#).unwrap(),
        ))
        .unwrap();
        let list: serde_json::Value =
            serde_json::from_str(&s.levels(Parameters(LevelsParams::default())).unwrap()).unwrap();
        let ground = list[0][0].as_str().unwrap().to_owned();

        // Reading either storey warns that they are stacked.
        let home: serde_json::Value = serde_json::from_str(
            &s.get_home(Parameters(
                serde_json::from_str(r#"{"detail":"summary"}"#).unwrap(),
            ))
            .unwrap(),
        )
        .unwrap();
        assert!(
            home["warnings"][0]
                .as_str()
                .unwrap()
                .contains("share an elevation"),
            "{home}"
        );

        // Checked together, the two copies read as an artefact of the layers,
        // not as a clash.
        let all: serde_json::Value = serde_json::from_str(
            &s.check_layout(Parameters(CheckParams {
                level: Some("all".into()),
                areas: None,
            }))
            .unwrap(),
        )
        .unwrap();
        let overlap = &all["overlap"][0];
        assert_eq!(overlap["kind"], "cross_level", "{all}");
        assert!(
            overlap["a"]["name"].is_string(),
            "names come with it: {all}"
        );
        assert!(overlap["a"]["bounds"].is_array(), "{all}");
        assert_eq!(all["overlap_kinds"]["cross_level"], 1, "{all}");

        // Marking the old plan as a reference layer takes it out of the check.
        s.levels(Parameters(LevelsParams {
            action: Some("update".into()),
            id: Some(ground),
            reference: Some(true),
            ..LevelsParams::default()
        }))
        .unwrap();
        let all: serde_json::Value = serde_json::from_str(
            &s.check_layout(Parameters(CheckParams {
                level: Some("all".into()),
                areas: None,
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(all.get("overlap").is_none(), "{all}");
        assert!(all.get("warnings").is_none(), "and the warning goes: {all}");
    }

    #[test]
    fn a_dry_write_answers_the_question_without_touching_the_plan() {
        let s = server();
        s.create(Parameters(
            serde_json::from_str(
                r#"{"walls":[{"pts":[[0,0],[500,0],[500,400],[0,400]],"closed":true}],
                    "rooms":[{"name":"Cozinha","at":[250,200]}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        s.place(Parameters(
            serde_json::from_str(
                r#"{"items":[{"cat":"base-cabinet","at":[250,40],"w":300,"d":60,"h":90},
                             {"cat":"base-cabinet","at":[250,340],"w":300,"d":60,"h":90}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        let rev = s.document.read().revision();
        let depth = s.document.read().home().furniture[0].depth;

        // "And if the counter were 100 cm deep?" — asked, not applied.
        let dry: serde_json::Value = serde_json::from_str(
            &s.update(Parameters(UpdateParams {
                items: serde_json::from_str(r#"[{"id":"f6","d":100}]"#).unwrap(),
                v: None,
                dry: Some(true),
            }))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(dry["dry"], true, "{dry}");
        assert_eq!(dry["changed"][0]["id"], "f6", "{dry}");
        assert_eq!(dry["changed"][0]["to"]["wdh"][1], 100.0, "{dry}");
        // The corridor it would leave, measured on the copy. Depth grows
        // around the center, so the front only advances 20 cm — and the back
        // ends up inside the wall, which the dry run says before it happens.
        assert!(
            (dry["clearances"]["f6"]["+y"][0].as_f64().unwrap() - 220.0).abs() < 0.5,
            "{dry}"
        );
        assert_eq!(dry["issues_new"][0], "f6+w1", "{dry}");
        assert_eq!(
            s.document.read().revision(),
            rev,
            "a dry run writes nothing"
        );
        assert!(
            (s.document.read().home().furniture[0].depth - depth).abs() < 1e-9,
            "and changes nothing"
        );

        // Applied for real, the reply says what moved instead of only `ok`.
        let reply = s
            .update(Parameters(UpdateParams {
                items: serde_json::from_str(r#"[{"id":"f6","d":100}]"#).unwrap(),
                v: None,
                dry: None,
            }))
            .unwrap();
        assert!(reply.starts_with("ok rev="), "{reply}");
        let diff: serde_json::Value =
            serde_json::from_str(&reply[reply.find('{').expect("a diff")..]).unwrap();
        assert_eq!(diff["changed"][0]["id"], "f6", "{reply}");
        assert_eq!(diff["changed"][0]["from"]["wdh"][1], 60.0, "{reply}");
    }

    #[test]
    fn a_resize_can_hold_one_face_instead_of_growing_around_the_center() {
        let s = server();
        s.create(Parameters(
            serde_json::from_str(
                r#"{"walls":[{"pts":[[0,0],[500,0],[500,400],[0,400]],"closed":true}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        // A counter with its back on the top wall, and one turned a quarter
        // turn with its back on the left wall.
        s.place(Parameters(
            serde_json::from_str(
                r#"{"items":[{"cat":"base-cabinet","at":[250,37.5],"w":300,"d":60,"h":90},
                             {"cat":"base-cabinet","at":[37.5,250],"w":200,"d":60,"h":90,"angle":270}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        let deepen = |id: &str, anchor: &str| {
            s.update(Parameters(UpdateParams {
                items: serde_json::from_str(&format!(
                    r#"[{{"id":"{id}","d":80,"anchor":"{anchor}"}}]"#
                ))
                .unwrap(),
                v: None,
                dry: None,
            }))
            .unwrap();
        };

        deepen("f5", "back");
        let home = s.document.read();
        let counter = home.home().find_piece("f5".parse().unwrap()).unwrap();
        let (min, max) = newera_core::plan_bounds(counter);
        assert!(
            (min.y - 7.5).abs() < 0.01,
            "the back stays on the wall: {min:?}"
        );
        assert!((max.y - 87.5).abs() < 0.01, "the front advances: {max:?}");
        drop(home);

        // Turned 270°, the piece's back looks at -x: the same word holds the
        // face against the left wall, not a plan side worked out by hand.
        deepen("f6", "back");
        let home = s.document.read();
        let turned = home.home().find_piece("f6".parse().unwrap()).unwrap();
        let (min, max) = newera_core::plan_bounds(turned);
        assert_eq!(newera_core::facing(turned), "+x", "{turned:?}");
        assert!((min.x - 7.5).abs() < 0.01, "{min:?}");
        assert!((max.x - 87.5).abs() < 0.01, "{max:?}");
        drop(home);

        // Without an anchor the center is what stays, which is the old trap.
        s.update(Parameters(UpdateParams {
            items: serde_json::from_str(r#"[{"id":"f5","d":100}]"#).unwrap(),
            v: None,
            dry: None,
        }))
        .unwrap();
        let home = s.document.read();
        let counter = home.home().find_piece("f5".parse().unwrap()).unwrap();
        let (min, _) = newera_core::plan_bounds(counter);
        assert!((min.y - (-2.5)).abs() < 0.01, "{min:?}");
    }

    #[test]
    fn annotations_report_the_notes_that_stopped_being_true() {
        let s = server();
        s.create(Parameters(
            serde_json::from_str(
                r#"{"walls":[{"pts":[[0,0],[500,0],[500,400],[0,400]],"closed":true}],
                    "dims":[{"a":[250,70],"b":[250,310]}],
                    "labels":[{"text":"TORRE 300 × 60 × 90","at":[250,40]}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        s.place(Parameters(
            serde_json::from_str(
                r#"{"items":[{"cat":"base-cabinet","at":[250,40],"w":300,"d":60,"h":90},
                             {"cat":"base-cabinet","at":[250,340],"w":300,"d":60,"h":90}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        let annotations = |json: &str| -> serde_json::Value {
            serde_json::from_str(
                &s.annotations(Parameters(serde_json::from_str(json).unwrap()))
                    .unwrap(),
            )
            .unwrap()
        };
        // The dimension marks the corridor and still agrees with it.
        assert!(
            annotations(r#"{"stale":true}"#)["stale"]
                .as_array()
                .unwrap()
                .is_empty(),
            "{:?}",
            annotations(r#"{"stale":true}"#)
        );

        // Deepen the counter and both the dimension and the note go stale.
        s.update(Parameters(UpdateParams {
            items: serde_json::from_str(r#"[{"id":"f7","d":100,"anchor":"back"}]"#).unwrap(),
            v: None,
            dry: None,
        }))
        .unwrap();
        let stale = annotations(r#"{"stale":true}"#);
        let rows = stale["stale"].as_array().unwrap();
        let dim = rows
            .iter()
            .find(|r| r[0] == "d5")
            .unwrap_or_else(|| panic!("{stale}"));
        assert!((dim[1].as_f64().unwrap() - 240.0).abs() < 0.5, "{stale}");
        assert!((dim[2].as_f64().unwrap() - 200.0).abs() < 0.5, "{stale}");
        let note = rows
            .iter()
            .find(|r| r[0] == "t6")
            .unwrap_or_else(|| panic!("{stale}"));
        assert_eq!(note[3], "f7", "the piece the note sits on: {stale}");
        assert!(
            (note[1].as_f64().unwrap() - 60.0).abs() < 0.01,
            "written: {stale}"
        );
        assert!(
            (note[2].as_f64().unwrap() - 100.0).abs() < 0.01,
            "measured: {stale}"
        );

        // And notes can be found by their text, which no read could do.
        let found = annotations(r#"{"q":"torre"}"#);
        assert_eq!(found["labels"].as_array().unwrap().len(), 1, "{found}");
        assert!(
            annotations(r#"{"q":"varanda"}"#)["labels"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn levels_scope_edits_and_reads_to_the_selected_storey() {
        let s = server();
        let walls = r#"{"walls":[{"pts":[[0,0],[400,0],[400,300],[0,300]],"closed":true}]}"#;
        s.create(Parameters(serde_json::from_str(walls).unwrap()))
            .unwrap();
        let add = |name: &str| {
            s.levels(Parameters(LevelsParams {
                action: Some("add".into()),
                name: Some(name.into()),
                ..LevelsParams::default()
            }))
            .unwrap()
        };
        let reply = add("Superior");
        assert!(reply.starts_with("ok"), "{reply}");
        let list = s.levels(Parameters(LevelsParams::default())).unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_str(&list).unwrap();
        assert_eq!(rows.len(), 2, "{list}");
        assert_eq!(rows[1][4], true, "new storey is selected: {list}");

        // The upper storey starts empty; new walls go on it.
        let home: serde_json::Value =
            serde_json::from_str(&s.get_home(Parameters(GetHomeParams::default())).unwrap())
                .unwrap();
        assert!(
            home.get("walls")
                .is_none_or(|w| w.as_array().unwrap().is_empty()),
            "{home}"
        );
        assert_eq!(home["levels"].as_array().unwrap().len(), 2);
        s.create(Parameters(
            serde_json::from_str(r#"{"walls":[{"pts":[[0,0],[200,0]]}]}"#).unwrap(),
        ))
        .unwrap();
        assert_eq!(s.document.read().home().walls.len(), 5);

        let ground = rows[0][0].as_str().unwrap().to_owned();
        s.levels(Parameters(LevelsParams {
            action: Some("select".into()),
            id: Some(ground),
            ..LevelsParams::default()
        }))
        .unwrap();
        let home: serde_json::Value =
            serde_json::from_str(&s.get_home(Parameters(GetHomeParams::default())).unwrap())
                .unwrap();
        assert_eq!(home["walls"].as_array().unwrap().len(), 4, "{home}");

        let upper = rows[1][0].as_str().unwrap().to_owned();
        s.levels(Parameters(LevelsParams {
            action: Some("delete".into()),
            id: Some(upper),
            ..LevelsParams::default()
        }))
        .unwrap();
        assert_eq!(
            s.document.read().home().walls.len(),
            4,
            "upper walls removed with the storey"
        );
        assert!(
            s.levels(Parameters(LevelsParams {
                action: Some("select".into()),
                id: Some("lv99".into()),
                ..LevelsParams::default()
            }))
            .is_err()
        );
    }

    #[test]
    fn labels_and_dimensions_in_3d() {
        let s = server();
        let params: CreateParams = serde_json::from_str(
            r#"{"labels":[{"text":"Sala","at":[10,10],"pitch":90,"elev":150},{"text":"Plano","at":[0,0]}],
                "dims":[{"a":[0,0],"b":[300,0],"off":30,"in3d":true,"elev":250,"pitch":90}]}"#,
        )
        .unwrap();
        s.create(Parameters(params)).unwrap();
        {
            let doc = s.document.read();
            let home = doc.home();
            assert_eq!(home.labels[0].pitch, Some(90.0));
            assert!((home.labels[0].elevation - 150.0).abs() < 1e-9);
            assert_eq!(home.labels[1].pitch, None);
            let d = &home.dimensions[0];
            assert!(d.visible_in_3d && (d.elevation[1] - 250.0).abs() < 1e-9);
        }
        let ids: Vec<String> = {
            let doc = s.document.read();
            vec![
                doc.home().labels[0].id.to_string(),
                doc.home().labels[1].id.to_string(),
                doc.home().dimensions[0].id.to_string(),
            ]
        };
        let specs: Vec<UpdateSpec> = serde_json::from_str(&format!(
            r#"[{{"id":"{}","in3d":false}},{{"id":"{}","pitch":0}},{{"id":"{}","in3d":false}}]"#,
            ids[0], ids[1], ids[2]
        ))
        .unwrap();
        s.update(Parameters(UpdateParams {
            items: specs,
            v: None,
            dry: None,
        }))
        .unwrap();
        let doc = s.document.read();
        let home = doc.home();
        assert_eq!(home.labels[0].pitch, None);
        assert_eq!(home.labels[1].pitch, Some(0.0));
        assert!(!home.dimensions[0].visible_in_3d);
    }

    #[test]
    fn polylines_label_styles_and_cameras() {
        let s = server();
        let params: CreateParams = serde_json::from_str(
            r#"{"labels":[{"text":"Tomada","at":[10,10]}],
                "polylines":[{"pts":[[0,0],[100,0],[100,50]],"t":2,"color":[200,0,0],"dash":"dash","arrows":["none","delta"]}]}"#,
        )
        .unwrap();
        assert_eq!(s.create(Parameters(params)).unwrap(), "ok rev=1 ids=t1,pl2");
        let spec: UpdateSpec =
            serde_json::from_str(r#"{"id":"t1","bold":true,"align":"left","color":[0,0,255]}"#)
                .unwrap();
        s.update(Parameters(UpdateParams {
            items: vec![spec],
            v: None,
            dry: None,
        }))
        .unwrap();
        let home = s.get_home(Parameters(GetHomeParams::default())).unwrap();
        assert!(
            home.contains(r#""bold":true"#) && home.contains(r#""align":"left""#),
            "{home}"
        );
        assert!(
            home.contains(r#""dash":"dash""#) && home.contains(r#""arrows":["none","delta"]"#),
            "{home}"
        );

        let store = |name: &str| CamerasParams {
            action: Some("store".into()),
            name: Some(name.into()),
            x: Some(100.0),
            y: Some(200.0),
            ..CamerasParams::default()
        };
        s.cameras(Parameters(store("Sala"))).unwrap();
        s.cameras(Parameters(CamerasParams {
            action: Some("view".into()),
            i: Some(0),
            ..CamerasParams::default()
        }))
        .unwrap();
        let list = s.cameras(Parameters(CamerasParams::default())).unwrap();
        assert!(
            list.contains(r#""active":"visitor""#) && list.contains("Sala"),
            "{list}"
        );
        assert!(
            s.cameras(Parameters(CamerasParams {
                action: Some("view".into()),
                i: Some(5),
                ..CamerasParams::default()
            }))
            .is_err()
        );
        s.undo().unwrap();
        assert!(
            !s.document.read().home().cameras.observer_active,
            "undo restores the aerial view"
        );
    }

    #[test]
    fn electrical_project_over_the_plan() {
        let s = server();
        s.disciplines(Parameters(DisciplineParams {
            action: Some("select".into()),
            d: Some("electrical".into()),
        }))
        .unwrap();
        let params: PlaceParams = serde_json::from_str(
            r#"{"items":[{"cat":"outlet-low","at":[10,10]},{"cat":"outlet-low","at":[60,10]},{"cat":"switch","at":[100,10]}]}"#,
        )
        .unwrap();
        s.place(Parameters(params)).unwrap();
        let lines: CreateParams =
            serde_json::from_str(r#"{"polylines":[{"pts":[[10,10],[110,10]]}]}"#).unwrap();
        s.create(Parameters(lines)).unwrap();
        let home = s.get_home(Parameters(GetHomeParams::default())).unwrap();
        assert!(home.contains(r#""layer":"electrical""#), "{home}");
        let q = s
            .disciplines(Parameters(DisciplineParams {
                action: Some("quantities".into()),
                d: None,
            }))
            .unwrap();
        assert!(q.contains(r#"["Tomada baixa (30 cm)",2]"#), "{q}");
        assert!(q.contains(r#""electrical":100"#), "{q}");
        let png = s
            .render_plan(Parameters(RenderParams {
                w: Some(200),
                h: Some(150),
                ..RenderParams::default()
            }))
            .unwrap();
        assert!(!png.content.is_empty());
    }

    #[test]
    fn annotations_list_rooms_with_details() {
        let s = server();
        let params: CreateParams = serde_json::from_str(
            r#"{"walls":[{"pts":[[0,0],[500,0],[500,400],[0,400]],"closed":true}],"rooms":[{"name":"Sala","at":[250,200]}]}"#,
        )
        .unwrap();
        s.create(Parameters(params)).unwrap();
        let place: PlaceParams =
            serde_json::from_str(r#"{"items":[{"cat":"sofa-3","at":[250,300]}]}"#).unwrap();
        let ids = s.place(Parameters(place)).unwrap();
        let id = ids.rsplit('=').next().unwrap().to_owned();
        let spec: UpdateSpec = serde_json::from_str(&format!(
            r#"{{"id":"{id}","brand":"Tok&Stok","url":"https://example.com/sofa"}}"#
        ))
        .unwrap();
        s.update(Parameters(UpdateParams {
            items: vec![spec],
            v: None,
            dry: None,
        }))
        .unwrap();
        let reply = s
            .annotations(Parameters(AnnotationParams {
                stale: None,
                q: None,
                dims: Some(true),
                refs: Some(true),
                details: Some(true),
                bake: None,
                legend: None,
            }))
            .unwrap();
        assert!(reply.contains(r#""rooms":[["Sala",[[1,"#), "{reply}");
        assert!(
            reply.contains("Tok&Stok") && reply.contains(r#""dims":true"#),
            "{reply}"
        );
        let png = s.render_plan(Parameters(RenderParams {
            w: Some(320),
            h: Some(240),
            ..RenderParams::default()
        }));
        assert!(png.is_ok());
    }

    #[test]
    fn dimensions_by_intent_in_one_call() {
        let s = server();
        let params: CreateParams = serde_json::from_str(
            r#"{"walls":[{"pts":[[0,0],[400,0],[400,300],[0,300]],"closed":true}],"rooms":[{"name":"Sala","at":[200,150]}]}"#,
        )
        .unwrap();
        s.create(Parameters(params)).unwrap();
        let place: PlaceParams =
            serde_json::from_str(r#"{"items":[{"cat":"window","wall":"w1","along":200}]}"#)
                .unwrap();
        s.place(Parameters(place)).unwrap();
        let dims: CreateParams = serde_json::from_str(
            r#"{"dims":[{"wall":"w1","side":"out"},{"wall":"w1","side":"in"},{"wall":"w1","chain":true,"off":70},{"room":"r5"}]}"#,
        )
        .unwrap();
        let reply = s.create(Parameters(dims)).unwrap();
        assert_eq!(reply.matches(",d").count() + 1, 7, "{reply}");
        let doc = s.document.read();
        let lengths: Vec<f64> = doc
            .home()
            .dimensions
            .iter()
            .map(|d| (d.length() * 10.0).round() / 10.0)
            .collect();
        assert_eq!(&lengths[..2], &[415.0, 385.0]);
        assert!(lengths.contains(&285.0));
        drop(doc);
        let baked = s
            .annotations(Parameters(AnnotationParams {
                bake: Some(true),
                ..AnnotationParams::default()
            }))
            .unwrap();
        assert!(
            baked.starts_with("ok rev=") && baked.contains("ids=d"),
            "{baked}"
        );
    }

    /// The whole tool surface, frozen: the names an agent can call and the
    /// exact bytes of what it reads to learn them.
    ///
    /// `list_all` sorts by name, so this is stable across runs. It is the
    /// guard for moving tools between modules: a dropped router, two modules
    /// claiming one name (the merge overwrites in silence), a description
    /// left behind or a schema that missed compaction all change it.
    #[test]
    fn tool_surface_is_unchanged() {
        const NAMES: &str = "annotations,arrange,cabinet_run,cameras,catalog,check_layout,\
create,cut_list,delete,disciplines,embed,ergonomics,export_plan,fit_roof,get_home,joinery,\
levels,lighting,materials,measure,move,new_home,open_home,place,plugins,redo,render_3d,\
render_photo,render_plan,save_home,sessions,set_background,set_home,split_wall,\
trace_background,undo,update,variants,video";

        let tools = server().tool_router.list_all();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert_eq!(names.join(","), NAMES, "the set of tools changed");
        let bytes = serde_json::to_string(&tools).unwrap().len();
        assert_eq!(
            bytes, 49408,
            "a description or schema changed; this test guards a pure move"
        );
    }

    #[test]
    #[ignore = "prints the size of the tool list"]
    fn tool_list_size() {
        let tools = server().tool_router.list_all();
        let json = serde_json::to_string(&tools).unwrap();
        println!(
            "{} tools, {} bytes (~{} tokens)",
            tools.len(),
            json.len(),
            json.len() / 4
        );
        let mut sizes: Vec<(usize, String)> = tools
            .iter()
            .map(|t| (serde_json::to_string(t).unwrap().len(), t.name.to_string()))
            .collect();
        sizes.sort();
        for (n, name) in sizes.iter().rev() {
            println!("{n:6} {name}");
        }
        let update = tools.iter().find(|t| t.name == "update").unwrap();
        println!("{}", serde_json::to_string(&update.input_schema).unwrap());
    }

    #[test]
    fn render_3d_returns_a_png() {
        let s = server();
        let params: CreateParams = serde_json::from_str(
            r#"{"walls":[{"pts":[[0,0],[400,0],[400,300],[0,300]],"closed":true}],"rooms":[{"name":"Sala","at":[200,150],"floor_mat":"wood"}]}"#,
        )
        .unwrap();
        s.create(Parameters(params)).unwrap();
        let result = s
            .render_3d(Parameters(Render3dParams {
                w: Some(96),
                h: Some(72),
                ..Render3dParams::default()
            }))
            .unwrap();
        let ContentBlock::Image(image) = &result.content[0] else {
            panic!("expected image")
        };
        assert_eq!(image.mime_type, "image/png");
        assert!(
            s.render_3d(Parameters(Render3dParams {
                cam: Some(3),
                ..Render3dParams::default()
            }))
            .is_err()
        );
    }

    #[test]
    fn exports_pdf_and_3d_models() {
        let s = server();
        let params: CreateParams = serde_json::from_str(
            r#"{"walls":[{"pts":[[0,0],[400,0],[400,300],[0,300]],"closed":true}]}"#,
        )
        .unwrap();
        s.create(Parameters(params)).unwrap();
        let dir = std::env::temp_dir().join(format!("newera-mcp-export-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for file in ["casa.pdf", "casa.glb", "casa.obj"] {
            let path = dir.join(file).display().to_string();
            let reply = s
                .export_plan(Parameters(ExportParams {
                    path: path.clone(),
                    w: None,
                    h: None,
                    scale: Some(50.0),
                }))
                .unwrap();
            assert!(reply.starts_with("ok"), "{reply}");
            assert!(std::fs::metadata(&path).unwrap().len() > 100, "{file}");
        }
        assert!(
            std::fs::read(dir.join("casa.pdf"))
                .unwrap()
                .starts_with(b"%PDF")
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn render_photo_returns_a_png_at_any_hour() {
        let s = server();
        let params: CreateParams = serde_json::from_str(
            r#"{"walls":[{"pts":[[0,0],[300,0],[300,200],[0,200]],"closed":true}],"rooms":[{"name":"Sala","at":[150,100]}]}"#,
        )
        .unwrap();
        s.create(Parameters(params)).unwrap();
        for hour in [9.0, 22.0] {
            let result = s
                .render_photo(Parameters(PhotoParams {
                    w: Some(48),
                    h: Some(36),
                    hour: Some(hour),
                    ..PhotoParams::default()
                }))
                .unwrap();
            assert!(matches!(&result.content[0], ContentBlock::Image(_)));
        }
        assert!(
            s.render_photo(Parameters(PhotoParams {
                quality: Some("ultra".into()),
                ..PhotoParams::default()
            }))
            .is_err()
        );
    }

    #[test]
    fn lighting_rates_rooms_and_fills_them_to_the_reference() {
        let s = server();
        let params: CreateParams = serde_json::from_str(
            r#"{"walls":[{"pts":[[0,0],[500,0],[500,400],[0,400]],"closed":true}],"rooms":[{"name":"Cozinha","at":[250,200]}]}"#,
        )
        .unwrap();
        s.create(Parameters(params)).unwrap();
        let rate = |p: &str| -> serde_json::Value {
            serde_json::from_str(
                &s.lighting(Parameters(serde_json::from_str(p).unwrap()))
                    .unwrap(),
            )
            .unwrap()
        };
        let dark = rate("{}");
        assert_eq!(dark["rooms"][0][6], 300.0, "{dark}");
        assert!(
            dark["rooms"][0][9].as_str().unwrap().starts_with("abaixo"),
            "{dark}"
        );
        // A warm bulb set by watts: 60 W incandescent is 720 lm.
        let place: PlaceParams = serde_json::from_str(
            r#"{"items":[{"cat":"pendant","at":[250,200],"light":{"w":60,"lamp":"incandescent","k":2700,"beam":0}}]}"#,
        )
        .unwrap();
        s.place(Parameters(place)).unwrap();
        let home = s.document.read().home().clone();
        let bulb = home.furniture.last().unwrap().light.clone().unwrap();
        assert!((bulb.flux() - 720.0).abs() < 1e-9 && bulb.beam.is_none());
        let room = home.rooms[0].id.to_string();
        let filled = rate(&format!(r#"{{"room":"{room}","fill":"downlight"}}"#));
        let placed = filled["placed"].as_array().unwrap().len();
        assert!(placed >= 6, "{filled}");
        let after = &filled["after"];
        assert!(after[3].as_f64().unwrap() >= 299.5, "{filled}");
        assert_eq!(after[7], placed + 1);
        // The spots hang at the ceiling, recessed.
        let home = s.document.read().home().clone();
        let spot = home.furniture.last().unwrap();
        assert!(
            spot.catalog == "downlight"
                && (spot.elevation + spot.height - home.wall_height - 0.6).abs() < 1e-9
        );
        assert!(!s.document.read().can_redo());
        let rated = rate(&format!(r#"{{"room":"{room}"}}"#));
        assert!((rated["rooms"][0][3].as_f64().unwrap() - after[3].as_f64().unwrap()).abs() < 1.0);
        // Turning a light off through update.
        let id = home.furniture.last().unwrap().id.to_string();
        let update: UpdateParams = serde_json::from_str(&format!(
            r#"{{"items":[{{"id":"{id}","light":{{"on":false}}}}]}}"#
        ))
        .unwrap();
        s.update(Parameters(update)).unwrap();
        assert!(
            s.document
                .read()
                .home()
                .furniture
                .last()
                .unwrap()
                .light
                .as_ref()
                .unwrap()
                .flux()
                < 1e-9
        );
        assert!(
            s.lighting(Parameters(
                serde_json::from_str(&format!(r#"{{"fill":"sofa-3","room":"{room}"}}"#)).unwrap()
            ))
            .is_err()
        );
    }

    #[test]
    fn video_path_keyframes_and_render() {
        let s = server();
        let params: CreateParams = serde_json::from_str(
            r#"{"walls":[{"pts":[[0,0],[400,0],[400,300],[0,300]],"closed":true}]}"#,
        )
        .unwrap();
        s.create(Parameters(params)).unwrap();
        let act = |action: &str| VideoParams {
            action: Some(action.into()),
            ..VideoParams::default()
        };
        assert!(s.video(Parameters(act("render"))).is_err());
        s.video(Parameters(act("orbit"))).unwrap();
        s.video(Parameters(VideoParams {
            fps: Some(4),
            speed: Some(20.0),
            ..act("set")
        }))
        .unwrap();
        s.video(Parameters(VideoParams {
            x: Some(200.0),
            y: Some(150.0),
            i: Some(0),
            ..act("add")
        }))
        .unwrap();
        s.video(Parameters(VideoParams {
            i: Some(0),
            ..act("delete")
        }))
        .unwrap();
        let list: serde_json::Value =
            serde_json::from_str(&s.video(Parameters(VideoParams::default())).unwrap()).unwrap();
        assert_eq!(list["rows"].as_array().unwrap().len(), 9);
        assert_eq!(list["fps"], 4);
        let dir = std::env::temp_dir().join(format!("newera-mcp-video-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("tour.avi");
        let reply: serde_json::Value = serde_json::from_str(
            &s.video(Parameters(VideoParams {
                path: Some(file.display().to_string()),
                w: Some(64),
                h: Some(64),
                ..act("render")
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(reply["frames"].as_u64().unwrap() >= 2, "{reply}");
        assert!(std::fs::read(&file).unwrap().starts_with(b"RIFF"));
        s.video(Parameters(act("clear"))).unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn plugins_and_sessions_tools() {
        let s = server();
        let list: serde_json::Value =
            serde_json::from_str(&s.plugins(Parameters(PluginsParams::default())).unwrap())
                .unwrap();
        assert!(list["rows"].is_array());
        // Without an HTTP server there is nothing for plugins to call back.
        assert!(
            s.plugins(Parameters(PluginsParams {
                action: Some("run".into()),
                ..PluginsParams::default()
            }))
            .is_err()
        );
        s.document
            .write()
            .sessions_mut()
            .join("Ana", newera_core::collab::now_ms());
        let rows: serde_json::Value = serde_json::from_str(&s.sessions()).unwrap();
        assert_eq!(rows["rows"][0][1], "Ana");
    }

    #[test]
    fn sloping_walls_and_tilted_pieces() {
        let s = server();
        // A gable: 600 cm base rising to 675 cm in the middle.
        let params: CreateParams =
            serde_json::from_str(r#"{"walls":[{"pts":[[0,0],[300,0],[600,0]],"hs":[10,675,10]}]}"#)
                .unwrap();
        s.create(Parameters(params)).unwrap();
        {
            let doc = s.document.read();
            let walls = &doc.home().walls;
            assert_eq!(
                (walls[0].height, walls[0].height_at_end),
                (10.0, Some(675.0))
            );
            assert_eq!(
                (walls[1].height, walls[1].height_at_end),
                (675.0, Some(10.0))
            );
        }
        let bad: CreateParams =
            serde_json::from_str(r#"{"walls":[{"pts":[[0,0],[100,0]],"hs":[10]}]}"#).unwrap();
        assert!(s.create(Parameters(bad)).is_err());
        let wall = s.document.read().home().walls[0].id.to_string();
        let spec: UpdateSpec =
            serde_json::from_str(&format!(r#"{{"id":"{wall}","h_end":300}}"#)).unwrap();
        s.update(Parameters(UpdateParams {
            items: vec![spec],
            v: None,
            dry: None,
        }))
        .unwrap();
        assert_eq!(s.document.read().home().walls[0].height_at_end, Some(300.0));

        // A 400 cm rafter tilted 45°: its far end rises.
        let params: PlaceParams = serde_json::from_str(
            r#"{"items":[{"cat":"box","at":[300,300],"w":10,"d":400,"h":10,"elev":100,"pitch":45}]}"#,
        )
        .unwrap();
        s.place(Parameters(params)).unwrap();
        let doc = s.document.read();
        let piece = doc.home().furniture.last().unwrap().clone();
        assert!((piece.pitch - 45.0).abs() < 1e-9);
        let mut only = doc.home().clone();
        only.walls.clear();
        let mesh =
            newera_render::Mesh::from_home(&only, &newera_render::Selection::new(), &|_| None);
        let top = mesh
            .vertices
            .iter()
            .map(|v| v.position[1])
            .fold(f32::MIN, f32::max);
        // Centered at 105 cm, half its length at 45° adds ~141 cm.
        assert!(top > 2.3 && top < 2.6, "{top}");
    }

    #[test]
    fn cameras_can_look_at_a_point() {
        let s = server();
        s.cameras(Parameters(CamerasParams {
            action: Some("store".into()),
            x: Some(0.0),
            y: Some(0.0),
            z: Some(170.0),
            look_at: Some([100.0, 100.0, 70.0]),
            ..CamerasParams::default()
        }))
        .unwrap();
        let doc = s.document.read();
        let camera = &doc.home().cameras.stored[0];
        let (dx, dy) = camera.direction();
        let k = std::f64::consts::FRAC_1_SQRT_2;
        assert!((dx - k).abs() < 1e-9 && (dy - k).abs() < 1e-9, "{dx} {dy}");
        // 100 cm down over 141 cm: about 35° below the horizon.
        assert!((camera.pitch - 35.26).abs() < 0.1, "{}", camera.pitch);
    }

    #[test]
    fn writes_name_the_active_variant_once_there_are_several() {
        let s = server();
        let wall = || -> CreateParams {
            serde_json::from_str(r#"{"walls":[{"pts":[[0,0],[100,0]]}]}"#).unwrap()
        };
        assert!(!s.create(Parameters(wall())).unwrap().contains(" v="));
        let reply = s
            .variants(Parameters(VariantsParams {
                action: Some("new".into()),
                ..VariantsParams::default()
            }))
            .unwrap();
        assert!(reply.contains(" v=1 i=1"), "{reply}");
        let reply = s.create(Parameters(wall())).unwrap();
        assert!(reply.contains(" v=1 ids="), "{reply}");
    }

    #[test]
    fn storeys_can_start_above_the_ground() {
        let s = server();
        let reply = s
            .levels(Parameters(LevelsParams {
                action: Some("add".into()),
                elev: Some(55.0),
                ..LevelsParams::default()
            }))
            .unwrap();
        assert!(reply.starts_with("ok"), "{reply}");
        let params: CreateParams =
            serde_json::from_str(r#"{"walls":[{"pts":[[0,0],[300,0]],"h":250}]}"#).unwrap();
        s.create(Parameters(params)).unwrap();
        let doc = s.document.read();
        let home = doc.home();
        let wall = &home.walls[0];
        assert!((home.elevation_of(wall.level) - 55.0).abs() < 1e-9);
        let mesh =
            newera_render::Mesh::from_home(home, &newera_render::Selection::new(), &|_| None);
        let top = mesh
            .vertices
            .iter()
            .map(|v| v.position[1])
            .fold(f32::MIN, f32::max);
        assert!((top - 3.05).abs() < 0.02, "wall top at 55 + 250 cm: {top}");
    }

    #[test]
    fn roofs_and_beams() {
        let s = server();
        // A 600 × 700 cm A-frame: eaves at the floor, ridge at 675 cm.
        let params: CreateParams = serde_json::from_str(
            r#"{"roofs":[{"pts":[[0,0],[0,700],[600,700],[600,0]],"h":0,"ridge_h":675,"overhang":0,"gables":true}]}"#,
        )
        .unwrap();
        let reply = s.create(Parameters(params)).unwrap();
        assert!(reply.contains("ids=w"), "{reply}");
        {
            let doc = s.document.read();
            let home = doc.home();
            assert_eq!(home.walls.len(), 4, "two sloping walls per gable");
            assert!(
                home.walls
                    .iter()
                    .any(|w| (w.height.max(w.height_at_end.unwrap_or(0.0)) - 675.0).abs() < 1e-6)
            );
            let roof = home.furniture.iter().find(|f| f.is_group()).unwrap();
            // Two slopes and the ridge cap closing the notch between them.
            assert_eq!(roof.children.len(), 3);
            let mut only = home.clone();
            only.walls.clear();
            let mesh =
                newera_render::Mesh::from_home(&only, &newera_render::Selection::new(), &|_| None);
            let points: Vec<[f32; 3]> = mesh.vertices[4..].iter().map(|v| v.position).collect();
            let top = points.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
            assert!((6.75..7.0).contains(&top), "ridge {top}");
            // Up at the ridge the panels meet over the middle of the span.
            let ridge_x: Vec<f32> = points.iter().filter(|p| p[1] > 6.7).map(|p| p[0]).collect();
            assert!(ridge_x.iter().all(|x| (x - 3.0).abs() < 0.2), "{ridge_x:?}");
            // At the eaves they reach the long sides.
            let low = points.iter().filter(|p| p[1] < 0.3).map(|p| p[0]);
            let (min, max) = low.fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(x), b.max(x)));
            assert!(min < 0.1 && max > 5.9, "{min} {max}");
        }
        // A room under the A-frame gets no flat ceiling: at the gables' peak it
        // would stick out through the slopes (seen in photos, which show both
        // sides of every face).
        let room: CreateParams = serde_json::from_str(
            r#"{"rooms":[{"name":"Sala","pts":[[10,10],[590,10],[590,690],[10,690]]}]}"#,
        )
        .unwrap();
        s.create(Parameters(room)).unwrap();
        {
            let doc = s.document.read();
            let mesh = newera_render::Mesh::from_home(
                doc.home(),
                &newera_render::Selection::new(),
                &|_| None,
            );
            let outside = mesh
                .vertices
                .iter()
                .map(|v| v.position)
                .filter(|p| p[1] > 3.0 && (p[0] - 3.0).abs() > 2.5)
                .collect::<Vec<_>>();
            assert!(outside.is_empty(), "{outside:?}");
        }
        let bad: CreateParams =
            serde_json::from_str(r#"{"roofs":[{"pts":[[0,0],[0,700]]}]}"#).unwrap();
        assert!(s.create(Parameters(bad)).is_err());

        // A brace from the floor at the origin to 300 cm up, 300 cm along y.
        let params: PlaceParams = serde_json::from_str(
            r#"{"items":[{"cat":"beam","a":[1000,0,0],"b":[1000,300,300],"w":8,"h":8}]}"#,
        )
        .unwrap();
        s.place(Parameters(params)).unwrap();
        let doc = s.document.read();
        let mut only = doc.home().clone();
        only.walls.clear();
        only.furniture.retain(|f| !f.is_group());
        let mesh =
            newera_render::Mesh::from_home(&only, &newera_render::Selection::new(), &|_| None);
        let near = |target: [f32; 3]| {
            mesh.vertices[4..].iter().any(|v| {
                let p = v.position;
                (p[0] - target[0]).abs() < 0.1
                    && (p[1] - target[1]).abs() < 0.1
                    && (p[2] - target[2]).abs() < 0.1
            })
        };
        assert!(
            near([10.0, 0.0, 0.0]) && near([10.0, 3.0, 3.0]),
            "ends of the brace"
        );
        drop(doc);
        assert!(
            s.place(Parameters(
                serde_json::from_str(r#"{"items":[{"cat":"beam","a":[0,0,0]}]}"#).unwrap()
            ))
            .is_err()
        );

        // A rafter drawn past the ridge stops under the other slope instead of
        // piercing the roof (retest finding 44).
        let rafter: PlaceParams = serde_json::from_str(
            r#"{"items":[{"cat":"beam","a":[20,350,30],"b":[330,350,700],"w":5,"h":18}]}"#,
        )
        .unwrap();
        let id = s
            .place(Parameters(rafter))
            .unwrap()
            .rsplit('=')
            .next()
            .unwrap()
            .to_owned();
        let doc = s.document.read();
        let beam = doc
            .home()
            .furniture
            .iter()
            .find(|f| f.id.to_string() == id)
            .unwrap();
        let (_, top) = beam.height_range();
        assert!(top < 690.0, "stops below the ridge: {top}");
        assert!(beam.depth < 700.0 && beam.depth > 600.0, "{}", beam.depth);
    }

    #[test]
    fn dividers_and_rooms_that_follow_walls() {
        let s = server();
        let params: CreateParams = serde_json::from_str(
            r#"{"walls":[{"pts":[[0,0],[800,0],[800,400],[0,400]],"closed":true}],
                "polylines":[{"pts":[[450,0],[450,400]],"divider":true}],
                "rooms":[{"name":"Sala","at":[200,200]},{"name":"Jantar","at":[600,200]}]}"#,
        )
        .unwrap();
        s.create(Parameters(params)).unwrap();
        let (sala, jantar, line) = {
            let doc = s.document.read();
            let h = doc.home();
            assert!(h.polylines[0].room_divider && h.rooms.iter().all(|r| r.auto));
            (
                h.rooms[0].area(),
                h.rooms[1].area(),
                h.polylines[0].id.to_string(),
            )
        };
        assert!(sala > jantar + 90.0 * 300.0, "{sala} {jantar}");
        let spec: UpdateSpec =
            serde_json::from_str(&format!(r#"{{"id":"{line}","pts":[[350,0],[350,400]]}}"#))
                .unwrap();
        s.update(Parameters(UpdateParams {
            items: vec![spec],
            v: None,
            dry: None,
        }))
        .unwrap();
        let doc = s.document.read();
        let h = doc.home();
        assert!(
            h.rooms[1].area() > h.rooms[0].area(),
            "rooms followed the divider"
        );
    }

    #[test]
    fn arrange_defaults_finishes_and_pixels() {
        let s = server();
        // Seven rafters from one template.
        let params: PlaceParams = serde_json::from_str(
            r#"{"defaults":{"cat":"box","w":8,"d":400,"h":18,"mat":"wood","color":[160,110,70]},
                "items":[{"at":[0,0]},{"at":[100,0],"opacity":0.4}]}"#,
        )
        .unwrap();
        let reply = s.place(Parameters(params)).unwrap();
        let ids: Vec<String> = reply
            .split("ids=")
            .nth(1)
            .unwrap()
            .split(',')
            .map(str::to_owned)
            .collect();
        {
            let doc = s.document.read();
            let f = &doc.home().furniture;
            assert!(
                f.iter()
                    .all(|p| p.catalog == "box" && (p.depth - 400.0).abs() < 1e-9)
            );
            assert_eq!(
                f[0].texture.as_ref().and_then(|t| t.pattern),
                Some(newera_core::Pattern::Wood)
            );
            assert_eq!(f[1].opacity, Some(0.4));
        }
        let arr = s
            .arrange(Parameters(ArrangeParams {
                action: "array".into(),
                ids: vec![ids[0].clone()],
                n: Some(5),
                dx: Some(100.0),
                ..ArrangeParams::default()
            }))
            .unwrap();
        assert_eq!(arr.split("ids=").nth(1).unwrap().split(',').count(), 5);
        assert_eq!(s.document.read().home().furniture.len(), 7);
        s.arrange(Parameters(ArrangeParams {
            action: "rotate".into(),
            ids: vec![ids[1].clone()],
            angle: Some(90.0),
            ..ArrangeParams::default()
        }))
        .unwrap();
        assert!((s.document.read().home().furniture[1].angle - 90.0).abs() < 1e-9);
        let grouped = s
            .arrange(Parameters(ArrangeParams {
                action: "group".into(),
                ids: ids.clone(),
                name: Some("Caibros".into()),
                ..ArrangeParams::default()
            }))
            .unwrap();
        assert_eq!(s.document.read().home().furniture.len(), 6);
        let group = grouped.split("ids=").nth(1).unwrap().to_owned();
        s.arrange(Parameters(ArrangeParams {
            action: "mirror".into(),
            ids: vec![group.clone()],
            a: Some(Point2::new(-100.0, 0.0)),
            b: Some(Point2::new(-100.0, 10.0)),
            copy: true,
            ..ArrangeParams::default()
        }))
        .unwrap();
        assert_eq!(s.document.read().home().furniture.len(), 7);
        assert!(
            s.arrange(Parameters(ArrangeParams {
                action: "spin".into(),
                ..ArrangeParams::default()
            }))
            .is_err()
        );

        // Pixels of a background at 2 cm/px offset by (100, 50).
        assert!(
            s.create(Parameters(
                serde_json::from_str(r#"{"px":true,"walls":[{"pts":[[0,0],[10,0]]}]}"#).unwrap()
            ))
            .is_err()
        );
        {
            let mut doc = s.document.write();
            doc.execute(Command::SetBackground {
                background: Some(newera_core::BackgroundImage {
                    path: "plan.png".into(),
                    size_px: [1000, 800],
                    cm_per_px: 2.0,
                    offset: Point2::new(100.0, 50.0),
                    opacity: 0.5,
                    visible: true,
                    ..Default::default()
                }),
            })
            .unwrap();
        }
        let params: CreateParams =
            serde_json::from_str(r#"{"px":true,"walls":[{"pts":[[0,0],[150,0]]}]}"#).unwrap();
        s.create(Parameters(params)).unwrap();
        let doc = s.document.read();
        let wall = doc.home().walls.last().unwrap();
        assert_eq!(
            (wall.start, wall.end),
            (Point2::new(100.0, 50.0), Point2::new(400.0, 50.0))
        );
    }

    #[test]
    fn solids_and_skylights() {
        let s = server();
        let params: CreateParams = serde_json::from_str(
            r#"{"roofs":[{"pts":[[0,0],[0,700],[600,700],[600,0]],"h":0,"ridge_h":675,"overhang":0,"gables":true,
                          "skylights":[{"at":[150,350],"w":100,"d":80}]}],
                "solids":[{"pts":[[150,400],[450,400],[300,650]],"h":15,"elev":300,"mat":"wood"},
                          {"profile":[[-100,0],[100,0],[0,150]],"a":[1000,0],"b":[1000,300]}]}"#,
        )
        .unwrap();
        s.create(Parameters(params)).unwrap();
        let doc = s.document.read();
        let home = doc.home();
        let roof = home.furniture.iter().find(|f| f.is_group()).unwrap();
        // The slope with the skylight is split around it, plus the glass and the ridge cap.
        assert_eq!(
            roof.children.len(),
            1 + 5 + 1,
            "{:?}",
            roof.children.iter().map(|c| &c.name).collect::<Vec<_>>()
        );
        assert_eq!(
            roof.children.iter().filter(|c| c.opacity.is_some()).count(),
            1
        );
        let slab = home
            .furniture
            .iter()
            .find(|f| matches!(f.shape, Some(newera_core::SolidShape::Outline(_))))
            .unwrap();
        assert!((slab.width - 300.0).abs() < 1e-9 && (slab.elevation - 300.0).abs() < 1e-9);
        let gable = home
            .furniture
            .iter()
            .find(|f| matches!(f.shape, Some(newera_core::SolidShape::Profile(_))))
            .unwrap();
        assert!((gable.depth - 300.0).abs() < 1e-9 && (gable.height - 150.0).abs() < 1e-9);
        assert!(
            (gable.position.x - 1000.0).abs() < 1e-6 && (gable.position.y - 150.0).abs() < 1e-6
        );
        let mut only = home.clone();
        only.walls.clear();
        only.furniture.retain(|f| f.shape.is_some());
        let mesh =
            newera_render::Mesh::from_home(&only, &newera_render::Selection::new(), &|_| None);
        let top = mesh
            .vertices
            .iter()
            .map(|v| v.position[1])
            .fold(f32::MIN, f32::max);
        assert!((top - 3.15).abs() < 0.01, "slab top {top}");
        drop(doc);
        let bad: CreateParams =
            serde_json::from_str(r#"{"solids":[{"profile":[[0,0],[1,0],[0,1]]}]}"#).unwrap();
        assert!(s.create(Parameters(bad)).is_err());
    }

    #[test]
    fn plan_overlay_and_area_comparison() {
        let s = server();
        let params: CreateParams = serde_json::from_str(
            r#"{"walls":[{"pts":[[0,0],[400,0],[400,300],[0,300]],"closed":true}],"rooms":[{"name":"Sala","at":[200,150]}]}"#,
        )
        .unwrap();
        s.create(Parameters(params)).unwrap();
        let report: serde_json::Value = serde_json::from_str(
            &s.check_layout(Parameters(CheckParams {
                areas: Some([("sala".to_owned(), 10.0), ("Cozinha".to_owned(), 8.0)].into()),
                level: None,
            }))
            .unwrap(),
        )
        .unwrap();
        let rows = report["areas"].as_array().unwrap();
        let cozinha = rows.iter().find(|r| r[0] == "Cozinha").unwrap();
        assert!(
            cozinha[2].is_null(),
            "unknown rooms are reported, not guessed"
        );
        let sala = rows.iter().find(|r| r[0] == "sala").unwrap();
        // 385 × 285 cm = 10.97 m², about 9.7 % over the reference.
        assert!((sala[2].as_f64().unwrap() - 10.97).abs() < 0.01, "{sala}");
        assert!((sala[3].as_f64().unwrap() - 9.7).abs() < 0.1, "{sala}");
        let plain: serde_json::Value =
            serde_json::from_str(&s.check_layout(Parameters(CheckParams::default())).unwrap())
                .unwrap();
        assert!(plain.get("areas").is_none());

        // Overlaying a background renders without touching the project.
        {
            let mut doc = s.document.write();
            doc.execute(Command::SetBackground {
                background: Some(newera_core::BackgroundImage {
                    path: "missing.png".into(),
                    size_px: [100, 100],
                    cm_per_px: 4.0,
                    offset: Point2::new(0.0, 0.0),
                    opacity: 0.2,
                    visible: false,
                    ..Default::default()
                }),
            })
            .unwrap();
        }
        let result = s
            .render_plan(Parameters(RenderParams {
                w: Some(96),
                h: Some(72),
                region: None,
                grid: None,
                bg: Some(0.6),
            }))
            .unwrap();
        assert!(matches!(&result.content[0], ContentBlock::Image(_)));
        let doc = s.document.read();
        let bg = doc.home().background.as_ref().unwrap();
        assert!(!bg.visible && (bg.opacity - 0.2).abs() < 1e-9);
    }

    #[test]
    fn traces_walls_from_a_rotated_background() {
        let dir = std::env::temp_dir().join(format!("newera-trace-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("scan.png");
        let mut image = image::GrayImage::from_pixel(220, 160, image::Luma([250]));
        for (x0, y0, x1, y1) in [
            (10, 10, 210, 18),
            (10, 142, 210, 150),
            (10, 10, 18, 150),
            (202, 10, 210, 150),
        ] {
            for y in y0..y1 {
                for x in x0..x1 {
                    image.put_pixel(x, y, image::Luma([10]));
                }
            }
        }
        image.save(&file).unwrap();
        let s = server();
        // 2.5 cm per px across, 2 cm per px down (an unevenly resized scan).
        let params: BackgroundParams = serde_json::from_value(serde_json::json!({
            "path": file.display().to_string(),
            "calibrations": [
                {"a": [14, 14], "b": [206, 14], "cm": 480},
                {"a": [14, 14], "b": [14, 146], "cm": 264}
            ]
        }))
        .unwrap();
        s.set_background(Parameters(params)).unwrap();
        {
            let doc = s.document.read();
            let bg = doc.home().background.as_ref().unwrap();
            assert!(
                (bg.cm_per_px - 2.5).abs() < 1e-6 && (bg.cm_per_px_y.unwrap() - 2.0).abs() < 1e-6,
                "{bg:?}"
            );
        }
        let listed: serde_json::Value = serde_json::from_str(
            &s.trace_background(Parameters(TraceParams {
                t_min: Some(10.0),
                ..TraceParams::default()
            }))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(listed["rows"].as_array().unwrap().len(), 4, "{listed}");
        let reply = s
            .trace_background(Parameters(TraceParams {
                t_min: Some(10.0),
                create: true,
                ..TraceParams::default()
            }))
            .unwrap();
        assert!(reply.contains("ids=w"), "{reply}");
        let doc = s.document.read();
        let walls = &doc.home().walls;
        assert_eq!(walls.len(), 4);
        // Top wall: axis at y = 14 px → 28 cm, 8 px thick → 16 cm, from x 35 to 515 cm.
        let top = walls
            .iter()
            .find(|w| (w.start.y - 28.0).abs() < 0.5 && (w.end.y - 28.0).abs() < 0.5)
            .unwrap();
        assert!((top.thickness - 16.0).abs() < 0.5, "{top:?}");
        assert!(
            (top.start.x.min(top.end.x) - 35.0).abs() < 1.0
                && (top.start.x.max(top.end.x) - 515.0).abs() < 1.0,
            "{top:?}"
        );
        drop(doc);
        // A room detected inside the traced walls.
        let room: CreateParams =
            serde_json::from_str(r#"{"rooms":[{"name":"Sala","at":[275,150]}]}"#).unwrap();
        s.create(Parameters(room)).unwrap();
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn writes_can_name_their_version() {
        let s = server();
        s.variants(Parameters(VariantsParams {
            action: Some("new".into()),
            ..VariantsParams::default()
        }))
        .unwrap();
        // Someone switches back to the first tab meanwhile.
        s.document.write().switch_variant(0).unwrap();
        let params: CreateParams =
            serde_json::from_str(r#"{"v":1,"walls":[{"pts":[[0,0],[100,0]]}]}"#).unwrap();
        let reply = s.create(Parameters(params)).unwrap();
        assert!(reply.contains(" v=1 "), "{reply}");
        let doc = s.document.read();
        assert_eq!(doc.active_variant(), 1);
        assert_eq!(doc.home().walls.len(), 1);
        drop(doc);
        let bad: CreateParams =
            serde_json::from_str(r#"{"v":9,"walls":[{"pts":[[0,0],[100,0]]}]}"#).unwrap();
        assert!(s.create(Parameters(bad)).is_err());
    }

    #[test]
    fn joinery_builds_change_and_list_their_cuts() {
        let s = server();
        let params: CreateParams = serde_json::from_str(
            r#"{"walls":[{"pts":[[0,0],[500,0],[500,400],[0,400]],"closed":true}],"rooms":[{"name":"Sala","at":[250,200]}]}"#,
        )
        .unwrap();
        s.create(Parameters(params)).unwrap();
        let joinery = |json: &str| {
            let p: JoineryParams = serde_json::from_str(json).unwrap();
            s.joinery(Parameters(p))
        };
        let reply: serde_json::Value = serde_json::from_str(
            &joinery(r#"{"kind":"cabinet","p":{"w":120,"h":210,"d":55},"wall":"w1"}"#).unwrap(),
        )
        .unwrap();
        let id = reply["id"].as_str().unwrap().to_owned();
        assert!(
            reply["hardware"].to_string().contains("dobradiças"),
            "{reply}"
        );
        {
            let doc = s.document.read();
            let group = doc
                .home()
                .furniture
                .iter()
                .find(|f| f.id.to_string() == id)
                .unwrap();
            // Backed onto the top wall: its back half a wall thickness below y = 0.
            assert!(
                (group.position.y - (7.5 + 27.5)).abs() < 0.5,
                "{:?}",
                group.position
            );
            assert!(group.children.len() > 10);
        }
        // Change only the shelves; the rest of the build and its place stay.
        let changed: serde_json::Value = serde_json::from_str(
            &joinery(&format!(r#"{{"id":"{id}","p":{{"shelves":5}}}}"#)).unwrap(),
        )
        .unwrap();
        assert_eq!(changed["id"], id.as_str());
        assert!(
            changed["parts"].as_u64() > reply["parts"].as_u64(),
            "{changed}"
        );
        // Impossible requests explain what to change, and dry runs create nothing.
        let err =
            joinery(r#"{"kind":"cabinet","p":{"d":35,"cooktop":true},"dry":true}"#).unwrap_err();
        assert!(
            err.message.contains("Ajuste a profundidade para 55 cm"),
            "{}",
            err.message
        );
        let before = s.document.read().home().furniture.len();
        joinery(r#"{"kind":"slats","p":{"w":100},"dry":true}"#).unwrap();
        assert_eq!(s.document.read().home().furniture.len(), before);
        // A cove follows the room given by id.
        let cove: serde_json::Value =
            serde_json::from_str(&joinery(r#"{"kind":"cove","room":"r5"}"#).unwrap()).unwrap();
        assert!(
            cove["hardware"].to_string().contains("fita de LED"),
            "{cove}"
        );
        assert!(joinery(r#"{"id":"w1","p":{}}"#).is_err());

        let dir = std::env::temp_dir().join(format!("newera-cut-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let csv = dir.join("corte.csv");
        let list: serde_json::Value = serde_json::from_str(
            &s.cut_list(Parameters(CutListParams {
                ids: None,
                path: Some(csv.display().to_string()),
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(
            list["rows"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r[0] == "Lateral esquerda" && r[2] == 2),
            "{list}"
        );
        assert!(
            std::fs::read_to_string(&csv)
                .unwrap()
                .starts_with("peca;material")
        );
        let dxf = dir.join("corte.dxf");
        let with_sheets: serde_json::Value = serde_json::from_str(
            &s.cut_list(Parameters(CutListParams {
                ids: Some(vec![id]),
                path: Some(dxf.display().to_string()),
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(with_sheets["sheets"].is_array(), "{with_sheets}");
        assert!(std::fs::read_to_string(&dxf).unwrap().contains("ENTITIES"));
        let svg = dir.join("corte.svg");
        s.cut_list(Parameters(CutListParams {
            ids: None,
            path: Some(svg.display().to_string()),
        }))
        .unwrap();
        assert!(std::fs::read_to_string(&svg).unwrap().starts_with("<svg"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn cabinet_runs_fill_a_kitchen_wall_around_its_appliances() {
        let s = server();
        let params: CreateParams = serde_json::from_str(
            r#"{"walls":[{"pts":[[0,0],[400,0],[400,300],[0,300]],"closed":true}],"rooms":[{"name":"Cozinha","at":[200,150]}]}"#,
        )
        .unwrap();
        s.create(Parameters(params)).unwrap();
        let place: PlaceParams = serde_json::from_str(
            r#"{"items":[{"cat":"fridge","wall":"w1","along":355},{"cat":"stove","wall":"w1","along":180},{"cat":"window","wall":"w1","along":80,"elev":110}]}"#,
        )
        .unwrap();
        s.place(Parameters(place)).unwrap();
        let run = |json: &str| -> serde_json::Value {
            let p: newera_joinery::CabinetRunParams = serde_json::from_str(json).unwrap();
            serde_json::from_str(&s.cabinet_run(Parameters(p)).unwrap()).unwrap()
        };
        let base = run(r#"{"wall":"w1"}"#);
        let modules = base["modules"].as_array().unwrap();
        let spans: Vec<(String, f64, f64)> = modules
            .iter()
            .map(|m| {
                (
                    m[1].as_str().unwrap().to_owned(),
                    m[2].as_f64().unwrap(),
                    m[3].as_f64().unwrap(),
                )
            })
            .collect();
        // Stove 150..210, fridge 320..390: cabinets stay clear of both with their gaps.
        for (role, from, w) in &spans {
            let to = from + w;
            assert!(
                to <= 148.1 || *from >= 211.9,
                "{role} {from}+{w} hits the stove: {spans:?}"
            );
            assert!(
                to <= 315.1,
                "{role} {from}+{w} takes the fridge air: {spans:?}"
            );
        }
        // The corner filler, then modules; the drawer unit touches the stove.
        assert_eq!(spans[0].0, "filler", "{spans:?}");
        let drawers = spans.iter().find(|m| m.0 == "drawers").unwrap();
        assert!(
            (drawers.1 + drawers.2 - 148.0).abs() < 0.2 || (drawers.1 - 212.0).abs() < 0.2,
            "{spans:?}"
        );
        assert!(base["notes"].to_string().contains("ventilação"), "{base}");
        // Nothing left over: every stretch is modules, pull-outs or fillers.
        let covered: f64 = spans
            .iter()
            .filter(|m| m.0 != "countertop")
            .map(|m| m.2)
            .sum();
        assert!(
            (covered - (148.0 - 7.5) - (315.0 - 212.0)).abs() < 0.3,
            "{covered} {spans:?}"
        );
        let issues = s.check_layout(Parameters(CheckParams::default())).unwrap();
        assert!(!issues.contains("overlap"), "{issues}");

        // Wall cabinets: split by the window, a gap for the hood, one over the fridge.
        // The wall named by a piece against it: the fridge (f6).
        let upper = run(r#"{"near":"f6","p":{"row":"wall"}}"#);
        assert_eq!(upper["wall"], "w1");
        let roles: Vec<&str> = upper["modules"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m[1].as_str().unwrap())
            .collect();
        assert!(roles.contains(&"over"), "{upper}");
        assert!(upper["notes"].to_string().contains("coifa"), "{upper}");
        for m in upper["modules"].as_array().unwrap() {
            let (from, w) = (m[2].as_f64().unwrap(), m[3].as_f64().unwrap());
            assert!(from + w <= 30.1 || from >= 129.9, "window 30..130: {upper}");
            assert!(
                m[1] == "over" || from + w <= 150.1 || from >= 209.9,
                "hood: {upper}"
            );
        }

        // Again with two drawer units: the old base modules are replaced, not piled up.
        let before = s.document.read().home().furniture.len();
        let again = run(r#"{"wall":"w1","p":{"drawers":2,"front":"wood"}}"#);
        assert_eq!(
            again["removed"].as_array().unwrap().len(),
            modules.len(),
            "{again}"
        );
        assert_eq!(s.document.read().home().furniture.len(), before);
        assert_eq!(again["modules"].to_string().matches("drawers").count(), 2);
        // An L: the side wall's run stops at this one's countertop with a corner filler.
        let side = run(r#"{"wall":"w4"}"#);
        assert!(side["removed"].as_array().unwrap().is_empty(), "{side}");
        let last = side["modules"]
            .as_array()
            .unwrap()
            .iter()
            .rfind(|m| m[1] != "countertop")
            .unwrap()
            .clone();
        assert_eq!(last[1], "filler", "{side}");
        // w4 runs from y=300 up to y=0: w1's countertop front is at y = 7.5 + 58.
        assert!(
            (last[2].as_f64().unwrap() + last[3].as_f64().unwrap() - (300.0 - 65.5)).abs() < 0.6,
            "{side}"
        );
        // …and w1, planned again in the same step, turns its corner module blind.
        let adjusted = &side["adjusted"][0];
        assert_eq!(adjusted["wall"], "w1", "{side}");
        let corner = &adjusted["modules"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m[1] != "countertop")
            .unwrap()
            .clone();
        assert_eq!(corner[1], "corner", "{adjusted}");
        assert!(
            (corner[2].as_f64().unwrap() - 7.5).abs() < 0.1,
            "{adjusted}"
        );
        let issues = s.check_layout(Parameters(CheckParams::default())).unwrap();
        assert!(!issues.contains("overlap"), "{issues}");
        s.document.write().undo().unwrap();
        // Planning w4 again with nothing new leaves w1 alone.
        run(r#"{"wall":"w4"}"#);
        let quiet = run(r#"{"wall":"w4"}"#);
        assert!(quiet.get("adjusted").is_none(), "{quiet}");
        assert!(!quiet["removed"].as_array().unwrap().is_empty(), "{quiet}");
        s.document.write().undo().unwrap();
        s.document.write().undo().unwrap();
        // Dry runs change nothing; one undo brings the previous run back.
        run(r#"{"wall":"w1","p":{"drawers":0},"dry":true}"#);
        assert_eq!(s.document.read().home().furniture.len(), before);
        s.document.write().undo().unwrap();
        let ids: Vec<String> = s
            .document
            .read()
            .home()
            .furniture
            .iter()
            .map(|f| f.id.to_string())
            .collect();
        assert!(ids.contains(&modules[1][0].as_str().unwrap().to_owned()));
        let p: newera_joinery::CabinetRunParams =
            serde_json::from_str(r#"{"wall":"w1","p":{"max":10}}"#).unwrap();
        assert!(
            s.cabinet_run(Parameters(p))
                .unwrap_err()
                .message
                .contains("max entre")
        );
    }

    #[test]
    fn ergonomics_reviews_the_plan_for_its_people() {
        let s = server();
        let params: CreateParams = serde_json::from_str(
            r#"{"walls":[{"pts":[[0,0],[400,0],[400,300],[0,300]],"closed":true}],"rooms":[{"name":"Quarto","at":[200,150]}]}"#,
        )
        .unwrap();
        s.create(Parameters(params)).unwrap();
        let place: PlaceParams = serde_json::from_str(
            r#"{"items":[{"cat":"bed-double","wall":"w1","along":90},{"cat":"door","wall":"w3","along":200,"w":70}]}"#,
        )
        .unwrap();
        s.place(Parameters(place)).unwrap();
        let profile: newera_ergonomics::Profile =
            serde_json::from_str(r#"{"occupants":3,"wheelchair":true}"#).unwrap();
        let report: serde_json::Value =
            serde_json::from_str(&s.ergonomics(Parameters(profile))).unwrap();
        let text = report["findings"].to_string();
        assert_eq!(report["capacity"]["beds"], 2, "{report}");
        assert!(text.contains("falta 1"), "{text}");
        assert!(text.contains("NBR 9050 pede 80 cm"), "{text}");
        // The bed is 90 cm from the left wall's axis: 7,5 cm wall, 79 cm half bed → 3,5 cm.
        assert!(text.contains("transferência da cadeira"), "{text}");
        assert!(report["score"].as_u64().unwrap() < 80, "{report}");
    }

    #[test]
    fn cabinet_runs_place_sink_and_cooktop_and_line_up_the_wall_row() {
        let s = server();
        let params: CreateParams = serde_json::from_str(
            r#"{"walls":[{"pts":[[0,0],[420,0],[420,300],[0,300]],"closed":true}],"rooms":[{"name":"Cozinha","at":[210,150]}]}"#,
        )
        .unwrap();
        s.create(Parameters(params)).unwrap();
        let place: PlaceParams = serde_json::from_str(
            r#"{"items":[{"cat":"fridge","wall":"w1","along":372},{"cat":"window","wall":"w1","along":130,"elev":110,"w":100}]}"#,
        )
        .unwrap();
        s.place(Parameters(place)).unwrap();
        let run = |json: &str| -> serde_json::Value {
            let p: newera_joinery::CabinetRunParams = serde_json::from_str(json).unwrap();
            serde_json::from_str(&s.cabinet_run(Parameters(p)).unwrap()).unwrap()
        };
        let base = run(r#"{"wall":"w1","p":{"sink":130,"cooktop":260}}"#);
        let modules = base["modules"].as_array().unwrap();
        let find = |role: &str| {
            modules
                .iter()
                .find(|m| m[1] == role)
                .unwrap_or_else(|| panic!("no {role}: {base}"))
        };
        let (sink, cooktop) = (find("sink"), find("cooktop"));
        let span = |m: &serde_json::Value| {
            (
                m[2].as_f64().unwrap(),
                m[2].as_f64().unwrap() + m[3].as_f64().unwrap(),
            )
        };
        assert!(span(sink).0 <= 90.0 && span(sink).1 >= 170.0, "{base}");
        assert!(
            span(cooktop).0 <= 230.0 && span(cooktop).1 >= 290.0,
            "{base}"
        );
        // The countertop carries both cutouts.
        let top_id = find("countertop")[0].as_str().unwrap().to_owned();
        let stored = s
            .document
            .read()
            .home()
            .furniture
            .iter()
            .find(|f| f.id.to_string() == top_id)
            .unwrap()
            .properties[newera_joinery::PARAMS_KEY]
            .clone();
        assert!(
            stored.contains("\"sink\"") && stored.contains("\"cooktop\""),
            "{stored}"
        );
        // Wall row: one hood gap over the cooktop, and the modules after it start where
        // the base cabinets do.
        let upper = run(r#"{"wall":"w1","p":{"row":"wall"}}"#);
        assert_eq!(
            upper["notes"].to_string().matches("coifa").count(),
            1,
            "{upper}"
        );
        let after_hood = upper["modules"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m[1] == "doors" && m[2].as_f64().unwrap() > 200.0)
            .unwrap()
            .clone();
        assert!(
            (after_hood[2].as_f64().unwrap() - span(cooktop).1).abs() < 0.2,
            "{upper}"
        );
    }

    #[test]
    fn cabinet_runs_redo_hand_drawn_cabinets_around_the_appliances_in_them() {
        let s = server();
        let params: CreateParams = serde_json::from_str(
            r#"{"walls":[{"pts":[[0,0],[400,0],[400,300],[0,300]],"closed":true}],"rooms":[{"name":"Cozinha","at":[200,150]}]}"#,
        )
        .unwrap();
        s.create(Parameters(params)).unwrap();
        // Imported-style pieces: cabinets and a cooktop known only by their names.
        let place: PlaceParams = serde_json::from_str(
            r#"{"items":[
                {"cat":"box","name":"Geladeira Electrolux 480 L","wall":"w1","along":45,"w":70,"d":72,"h":185},
                {"cat":"box","name":"7 — Armário portas ao lado cooktop","wall":"w1","along":160,"w":80,"d":58,"h":87},
                {"cat":"box","name":"Gavetões sob cooktop","wall":"w1","along":245,"w":90,"d":58,"h":87},
                {"cat":"box","name":"Cooktop Brastemp — 4 bocas","wall":"w1","along":245,"w":59,"d":48,"h":8,"elev":87},
                {"cat":"box","name":"Bancada contínua","wall":"w1","along":230,"w":220,"d":60,"h":3,"elev":87}
            ]}"#,
        )
        .unwrap();
        let ids = s.place(Parameters(place)).unwrap();
        let ids: Vec<String> = ids
            .rsplit('=')
            .next()
            .unwrap()
            .split(',')
            .map(str::to_owned)
            .collect();
        let p: newera_joinery::CabinetRunParams = serde_json::from_str(r#"{"near":"f6"}"#).unwrap();
        let reply: serde_json::Value =
            serde_json::from_str(&s.cabinet_run(Parameters(p)).unwrap()).unwrap();
        let removed = reply["removed"].to_string();
        // The cabinets and the old countertop go; the fridge and the cooktop stay.
        for id in [&ids[1], &ids[2], &ids[4]] {
            assert!(removed.contains(id.as_str()), "{id} not replaced: {reply}");
        }
        assert!(
            !removed.contains(&format!("\"{}\"", ids[0]))
                && !removed.contains(&format!("\"{}\"", ids[3])),
            "{reply}"
        );
        // The new drawer unit stands under the cooktop that was there.
        let cooktop = reply["modules"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m[1] == "cooktop")
            .unwrap_or_else(|| panic!("{reply}"))
            .clone();
        let (from, w) = (cooktop[2].as_f64().unwrap(), cooktop[3].as_f64().unwrap());
        assert!(from <= 215.5 && from + w >= 274.5, "{reply}");
        assert!(
            reply["notes"].to_string().contains("Cooktop existente"),
            "{reply}"
        );
        assert!(
            reply["notes"]
                .to_string()
                .contains("ventilação da geladeira"),
            "{reply}"
        );
    }

    #[test]
    fn embedded_items_fit_their_host_and_survive_its_changes() {
        let s = server();
        let params: CreateParams = serde_json::from_str(
            r#"{"walls":[{"pts":[[0,0],[420,0],[420,300],[0,300]],"closed":true}],"rooms":[{"name":"Cozinha","at":[210,150]}]}"#,
        )
        .unwrap();
        s.create(Parameters(params)).unwrap();
        let run = |json: &str| -> serde_json::Value {
            let p: newera_joinery::CabinetRunParams = serde_json::from_str(json).unwrap();
            serde_json::from_str(&s.cabinet_run(Parameters(p)).unwrap()).unwrap()
        };
        run(r#"{"wall":"w1","p":{"cooktop":260}}"#);
        // Planned again without parameters, the cooktop stays where it was.
        let base = run(r#"{"wall":"w1"}"#);
        assert!(base["modules"].to_string().contains("cooktop"), "{base}");
        let top = base["modules"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m[1] == "countertop")
            .unwrap()[0]
            .as_str()
            .unwrap()
            .to_owned();
        let embed = |json: &str| -> Result<serde_json::Value, String> {
            let p: EmbedParams = serde_json::from_str(json).unwrap();
            s.embed(Parameters(p))
                .map(|r| serde_json::from_str(&r).unwrap())
                .map_err(|e| e.message.to_string())
        };
        // A real 5-burner cooktop, 75 × 50, where the generic one was.
        let reply = embed(&format!(
            r#"{{"cat":"cooktop","w":75,"d":50,"h":6,"host":"{top}","at":252.5}}"#
        ))
        .unwrap();
        assert_eq!(reply["cutout"], serde_json::json!([71, 46]), "{reply}");
        let cooktop_id = reply["item"].as_str().unwrap().to_owned();
        let host_of = |id: &str| {
            s.document
                .read()
                .home()
                .furniture
                .iter()
                .find(|f| f.children.iter().any(|c| c.id.to_string() == id))
                .map(|f| {
                    (
                        f.id.to_string(),
                        f.properties[newera_joinery::PARAMS_KEY].clone(),
                    )
                })
        };
        let (host, params) = host_of(&cooktop_id).expect("embedded");
        assert_eq!(host, top);
        assert_eq!(
            params.matches("\"cooktop\"").count(),
            1,
            "one cooktop hole: {params}"
        );
        // Planning the wall again keeps the real cooktop in the new countertop.
        let again = run(r#"{"wall":"w1","p":{"drawers":2}}"#);
        let (new_host, params) = host_of(&cooktop_id).unwrap_or_else(|| panic!("lost: {again}"));
        assert_ne!(new_host, top);
        assert!(params.contains("\"drawn\":false"), "{params}");
        // An oven into a tower: too narrow first, then it fits and follows a change.
        let tower: serde_json::Value = serde_json::from_str(
            &s.joinery(Parameters(serde_json::from_str::<JoineryParams>(r#"{"kind":"cabinet","p":{"w":55,"h":220,"d":58,"plinth":10},"wall":"w4","along":200}"#).unwrap())).unwrap(),
        )
        .unwrap();
        let tower_id = tower["id"].as_str().unwrap().to_owned();
        let err = embed(&format!(
            r#"{{"cat":"oven","host":"{tower_id}","dry":true}}"#
        ))
        .unwrap_err();
        assert!(err.contains("use w = 61"), "{err}");
        s.joinery(Parameters(
            serde_json::from_str::<JoineryParams>(&format!(
                r#"{{"id":"{tower_id}","p":{{"w":64}}}}"#
            ))
            .unwrap(),
        ))
        .unwrap();
        let oven = embed(&format!(r#"{{"cat":"oven","host":"{tower_id}"}}"#)).unwrap();
        let oven_id = oven["item"].as_str().unwrap().to_owned();
        s.joinery(Parameters(
            serde_json::from_str::<JoineryParams>(&format!(
                r#"{{"id":"{tower_id}","p":{{"shelves":3}}}}"#
            ))
            .unwrap(),
        ))
        .unwrap();
        assert_eq!(
            host_of(&oven_id).map(|h| h.0),
            Some(tower_id.clone()),
            "the oven stays in the tower"
        );
    }

    #[test]
    fn walls_and_glass_follow_an_a_frame_roof() {
        let s = server();
        let params: CreateParams = serde_json::from_str(
            r#"{"roofs":[{"pts":[[0,0],[0,700],[600,700],[600,0]],"h":0,"ridge_h":600,"overhang":30,"t":15}],"walls":[{"pts":[[0,420],[600,420]],"h":250,"t":12}]}"#,
        )
        .unwrap();
        let ids = s.create(Parameters(params)).unwrap();
        let wall = ids
            .rsplit('=')
            .next()
            .unwrap()
            .split(',')
            .find(|i| i.starts_with('w'))
            .unwrap()
            .to_owned();
        let place: PlaceParams = serde_json::from_str(
            r#"{"items":[{"cat":"panel","at":[300,5],"w":600,"d":2,"h":100,"opacity":0.35}]}"#,
        )
        .unwrap();
        let glass = s
            .place(Parameters(place))
            .unwrap()
            .rsplit('=')
            .next()
            .unwrap()
            .to_owned();
        let p: FitRoofParams =
            serde_json::from_str(&format!(r#"{{"ids":["{wall}","{glass}"]}}"#)).unwrap();
        let reply = s.fit_roof(Parameters(p)).unwrap();
        assert!(reply.contains("fitted=3"), "{reply}");
        {
            let doc = s.document.read();
            let walls = &doc.home().walls;
            assert_eq!(walls.len(), 2, "split at the ridge");
            let peak = walls
                .iter()
                .map(|w| w.height.max(w.height_at_end.unwrap_or(0.0)))
                .fold(0.0, f64::max);
            assert!(peak > 590.0, "{walls:?}");
            let panel = doc
                .home()
                .furniture
                .iter()
                .find(|f| f.id.to_string() == glass)
                .unwrap();
            assert!(
                matches!(panel.shape, Some(newera_core::SolidShape::Profile(_))),
                "{panel:?}"
            );
        }
        // Off: the marks go, heights stay.
        let p: FitRoofParams =
            serde_json::from_str(&format!(r#"{{"ids":["{wall}"],"off":true}}"#)).unwrap();
        s.fit_roof(Parameters(p)).unwrap();
        assert!(
            !s.document
                .read()
                .home()
                .wall(wall.parse().unwrap())
                .unwrap()
                .properties
                .contains_key(newera_core::ROOF_FIT_KEY)
        );
        // Nothing overhead is explained.
        let p: FitRoofParams = serde_json::from_str(r#"{"ids":["w999"]}"#).unwrap();
        assert!(s.fit_roof(Parameters(p)).is_err());
    }

    #[test]
    fn cutouts_land_over_their_cabinets_on_walls_run_either_way() {
        let s = server();
        // Counter-clockwise walls: the kitchen side of w2 is to its right.
        let params: CreateParams = serde_json::from_str(
            r#"{"walls":[{"pts":[[0,0],[0,300],[420,300],[420,0]],"closed":true}],"rooms":[{"name":"Cozinha","at":[210,150]}]}"#,
        )
        .unwrap();
        s.create(Parameters(params)).unwrap();
        for wall in ["w1", "w2", "w3", "w4"] {
            let p: newera_joinery::CabinetRunParams = serde_json::from_str(&format!(
                r#"{{"wall":"{wall}","p":{{"cooktop":210,"sink":80}}}}"#
            ))
            .unwrap();
            let Ok(reply) = s.cabinet_run(Parameters(p)) else {
                continue;
            };
            let reply: serde_json::Value = serde_json::from_str(&reply).unwrap();
            let doc = s.document.read();
            let find = |role: &str| {
                let id = reply["modules"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|m| m[1] == role)?[0]
                    .as_str()?
                    .to_owned();
                doc.home()
                    .furniture
                    .iter()
                    .find(|f| f.id.to_string() == id)
                    .cloned()
            };
            let (Some(top), Some(cooktop)) = (find("countertop"), find("cooktop")) else {
                continue;
            };
            let params: newera_joinery::Build =
                serde_json::from_str(&top.properties[newera_joinery::PARAMS_KEY]).unwrap();
            let newera_joinery::Build::Countertop(t) = params else {
                panic!()
            };
            let cut = t
                .cutouts
                .iter()
                .find(|c| c.kind == newera_joinery::CutoutKind::Cooktop)
                .unwrap();
            let hole = top.to_plan((cut.x - t.length / 2.0, 0.0));
            // The hole is over the cooktop's drawer unit, whichever way the wall runs.
            assert!(
                hole.distance(cooktop.position) < 35.0,
                "{wall}: hole {hole:?} vs cabinet {:?}",
                cooktop.position
            );
        }
    }
}
