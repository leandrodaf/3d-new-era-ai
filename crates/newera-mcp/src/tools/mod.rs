//! MCP tool surface. Each tool is a thin adapter over [`crate::edit`] or
//! `newera-core`; all of them share the document the editor is showing.

use std::path::PathBuf;

use base64::Engine as _;
use newera_core::{Command, Document, Point2, SharedDocument, ops};
use newera_draw::{RenderOptions, SceneOptions, SvgOptions, plan_scene, render_png, to_svg};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo};
use rmcp::{ErrorData, ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::compact;
use crate::edit::{self, CreateParams, PlaceSpec, UpdateSpec};

mod annotations;
mod background;
mod cameras;
mod check;
mod levels;
mod lighting;
mod measure;
mod project;
mod read;
mod reply;
mod roof;

use reply::{applied, background_scale, core, invalid, ok, on_variant, preview};

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

/// The MCP server, assembled from one router per domain.
///
/// A new tool means a `#[tool]` in a domain module and its router in
/// `parts`. Two domains claiming one name would overwrite in silence — the
/// merge is a map insert — so the count is checked here, and the whole
/// surface is frozen by `tool_surface_is_unchanged`.
impl NewEraMcp {
    pub fn new(document: SharedDocument) -> Self {
        let parts = [
            Self::tool_router(),
            Self::levels_router(),
            Self::measure_router(),
            Self::check_router(),
            Self::background_router(),
            Self::roof_router(),
            Self::lighting_router(),
            Self::annotations_router(),
            Self::cameras_router(),
            Self::project_router(),
            Self::read_router(),
        ];
        let expected: usize = parts.iter().map(|r| r.map.len()).sum();
        let mut tool_router = parts
            .into_iter()
            .fold(ToolRouter::new(), |all, part| all + part);
        debug_assert_eq!(
            tool_router.map.len(),
            expected,
            "two domains registered the same tool name"
        );
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
}

#[tool_router]
impl NewEraMcp {
    #[tool(
        description = "Create walls (polylines; hs = height per point for gables), rooms (pts, or at=[x,y] to detect from walls), dims (a+b or wall id), labels, roofs (rectangle pts, gable|shed, pitch or ridge_h, eave h, overhang, gables=true closes the ends, skylights [{at,w,d}] cut glazed openings) and solids (pts outline raised by h at elev: slabs/mezzanines of any shape; or profile [[u,z]] swept from a to b: gables, ramps) in one atomic step."
    )]
    pub(crate) fn create(
        &self,
        Parameters(mut p): Parameters<CreateParams>,
    ) -> Result<String, ErrorData> {
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
    pub(crate) fn update(
        &self,
        Parameters(p): Parameters<UpdateParams>,
    ) -> Result<String, ErrorData> {
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
    pub(crate) fn delete(&self, Parameters(p): Parameters<IdsParams>) -> Result<String, ErrorData> {
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
    pub(crate) fn move_elements(
        &self,
        Parameters(p): Parameters<MoveParams>,
    ) -> Result<String, ErrorData> {
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
    pub(crate) fn split_wall(
        &self,
        Parameters(p): Parameters<SplitParams>,
    ) -> Result<String, ErrorData> {
        let id = p.id.parse().map_err(|e| invalid(format!("{e}")))?;
        let mut doc = self.document.write();
        let second = ops::split_wall(&mut doc, id, p.t.unwrap_or(0.5)).map_err(core)?;
        Ok(ok(&doc, &[second.to_string()]))
    }

    #[tool(
        description = "PNG of the floor plan, exactly as the user sees it. bg=0..1 overlays the background image to compare with the reference. Keep w/h small to save tokens."
    )]
    pub(crate) fn render_plan(
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
    pub(crate) fn render_3d(
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
    pub(crate) fn render_photo(
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
    pub(crate) fn export_plan(
        &self,
        Parameters(p): Parameters<ExportParams>,
    ) -> Result<String, ErrorData> {
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

    #[tool(
        description = "Place catalog items: at=[x,y] center (doors/windows near a wall snap into it; into=[x,y] picks the swing side), or wall=id (+along cm) to put doors/windows in a wall or furniture against it. Sizes w/d/h override defaults; pitch/roll tilt; angle clockwise degrees (0: front faces +y, down the plan; back/headboard toward -y); mat finish (wood, marble, img:…; 'img:facade.png fit' stretches one image: a reference board to compare with render_3d view=front) and opacity (glass 0.3); defaults {…} fills every item; px=true reads coordinates as background pixels. cat=beam with a,b=[x,y,z] (z above the floor) and w×h section makes rafters, posts and braces; a beam reaching into a roof stops under it. Pools: pool or pool-oval."
    )]
    pub(crate) fn place(
        &self,
        Parameters(p): Parameters<PlaceParams>,
    ) -> Result<String, ErrorData> {
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
    pub(crate) fn arrange(
        &self,
        Parameters(p): Parameters<ArrangeParams>,
    ) -> Result<String, ErrorData> {
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
    pub(crate) fn joinery(
        &self,
        Parameters(p): Parameters<JoineryParams>,
    ) -> Result<String, ErrorData> {
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
        description = "Embed an item into joinery with an exact fit: a sink bowl or cooktop into a countertop (cutout from the item's size, generic fixture not drawn), an oven, microwave or other appliance into a cabinet niche (doors above and below, boards around it), a TV onto a slatted panel at seated eye level (z = screen center). The item becomes part of the host and moves with it. item: id in the plan, or cat (+w/d/h) for a new one. Errors say what to change (e.g. use w = 61 no armário). Reply {host, item, kind, cutout|niche, x|bottom, notes}."
    )]
    pub(crate) fn embed(
        &self,
        Parameters(p): Parameters<EmbedParams>,
    ) -> Result<String, ErrorData> {
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
        description = "Fill a wall with cabinets sized for it: measures the free stretches between corners, doors, windows, fridge and stove, splits each into even modules (30-90 cm, no useless leftovers; 15-30 cm pull-outs, fillers under 15), drawer unit beside the stove, countertop on base rows, cabinet over the fridge and hood gap on wall rows, wardrobes (hanging rails, shelves, drawers) on tall rows facing bedrooms; p.sink/p.cooktop place those cabinets and cutouts. Replaces the cabinets already there (keep ids stay). Reply {modules:[[id,role,from,w]],removed,notes}; dry plans only. Change one module afterwards with joinery id."
    )]
    pub(crate) fn cabinet_run(
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
    pub(crate) fn cut_list(
        &self,
        Parameters(p): Parameters<CutListParams>,
    ) -> Result<String, ErrorData> {
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

#[cfg(test)]
fn server() -> NewEraMcp {
    NewEraMcp::new(SharedDocument::new(Document::default()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::cameras::CamerasParams;
    use crate::tools::check::CheckParams;
    use crate::tools::read::GetHomeParams;

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
