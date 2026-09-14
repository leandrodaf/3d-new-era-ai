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
pub(crate) struct GetHomeParams {
    /// `summary` (counts, bounds, room areas) or `full` (default).
    detail: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct UpdateParams {
    items: Vec<UpdateSpec>,
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
    w: Option<u32>,
    h: Option<u32>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct AnnotationParams {
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
    /// `list` (default), `add`, `select`, `delete`.
    action: Option<String>,
    /// Level id, e.g. `lv3`.
    id: Option<String>,
    name: Option<String>,
    /// Storey height cm for `add`.
    h: Option<f64>,
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

    #[tool(description = "Home state. detail=summary is cheapest.")]
    fn get_home(&self, Parameters(p): Parameters<GetHomeParams>) -> String {
        let doc = self.document.read();
        let full = doc.home();
        let view = full.level_view(full.current_level());
        let mut out = match p.detail.as_deref() {
            Some("summary") => compact::summary(&view, doc.revision()),
            _ => compact::home(&view, doc.revision()),
        };
        if !full.levels.is_empty() {
            out["levels"] = compact::levels(full);
        }
        out.to_string()
    }

    #[tool(
        description = "Create walls (polylines), rooms (pts, or at=[x,y] to detect from walls), dims (a+b or wall id) and labels in one atomic step."
    )]
    fn create(&self, Parameters(p): Parameters<CreateParams>) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        let ids = edit::create(&mut doc, p).map_err(invalid)?;
        Ok(ok(&doc, &ids))
    }

    #[tool(description = "Change fields of elements by id; fields must match the element kind.")]
    fn update(&self, Parameters(p): Parameters<UpdateParams>) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        edit::update(&mut doc, p.items).map_err(invalid)?;
        Ok(ok(&doc, &[]))
    }

    #[tool(description = "Delete elements by id, atomically.")]
    fn delete(&self, Parameters(p): Parameters<IdsParams>) -> Result<String, ErrorData> {
        let ids = edit::parse_ids(&p.ids).map_err(invalid)?;
        let mut doc = self.document.write();
        let commands = ids.into_iter().map(Command::remove).collect();
        doc.execute(Command::Batch { commands }).map_err(core)?;
        Ok(ok(&doc, &[]))
    }

    #[tool(name = "move", description = "Move elements by dx,dy cm.")]
    fn move_elements(&self, Parameters(p): Parameters<MoveParams>) -> Result<String, ErrorData> {
        let ids = edit::parse_ids(&p.ids).map_err(invalid)?;
        let mut doc = self.document.write();
        ops::translate(&mut doc, &ids, p.dx, p.dy, p.joined.unwrap_or(true)).map_err(core)?;
        Ok(ok(&doc, &[]))
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
        description = "Set a scanned plan as background at real scale: path, then cm_per_px or calibrate {a,b px, cm}; offset/opacity/visible; clear=true removes."
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
        description = "PNG of the floor plan, exactly as the user sees it. Keep w/h small to save tokens."
    )]
    fn render_plan(
        &self,
        Parameters(p): Parameters<RenderParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let png = self.render(
            p.w.unwrap_or(640),
            p.h.unwrap_or(480),
            p.region,
            p.grid.unwrap_or(true),
        )?;
        let data = base64::engine::general_purpose::STANDARD.encode(png);
        Ok(CallToolResult::success(vec![ContentBlock::image(
            data,
            "image/png",
        )]))
    }

    #[tool(
        description = "PNG of the home in 3D (software render, no GPU needed). view: aerial (default; frames the whole building; yaw degrees: 0 from east/+x, 90 from south/plan bottom (default 60); pitch down; zoom >1 farther), visitor (current visitor camera) or cam=i (stored point of view). Keep w/h small."
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
        description = "Place catalog items: at=[x,y] center, or wall=id (+along cm) to put doors/windows in a wall or furniture against it. Sizes w/d/h override defaults."
    )]
    fn place(&self, Parameters(p): Parameters<PlaceParams>) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        let ids = edit::place(&mut doc, p.items).map_err(invalid)?;
        Ok(ok(&doc, &ids))
    }

    #[tool(
        description = "Layout problems: overlap, in_wall, blocks_door, outside_rooms. {} means none."
    )]
    fn check_layout(&self) -> String {
        let doc = self.document.read();
        compact::issues(&doc.home().level_view(doc.home().current_level())).to_string()
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
        description = "Plan annotations. Set any of dims (engineering dimension chains), refs (room reference schedule with tags), details (brand/model/link in refs), legend (symbol legend with counts); bake=true turns the automatic chains into editable dimensions (ids returned). Otherwise returns {dims,refs,details,rooms:[[room,[[tag,name,w,d,h,brand?,model?,url?]]]]}. Give pieces brand/model/url via update."
    )]
    fn annotations(
        &self,
        Parameters(p): Parameters<AnnotationParams>,
    ) -> Result<String, ErrorData> {
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
        description = "Storeys. list (default): rows [id,name,elev,h,selected,layout_index,viewable]. add {name?,h?} adds one on top and selects it; select {id}; delete {id} removes it and its content. Other tools act on the selected storey."
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
        let (w, h) = (w.clamp(64, 2048), h.clamp(64, 2048));
        let doc = self.document.read();
        let view = doc.home().level_view(doc.home().current_level());
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
            serde_json::from_str(&s.get_home(Parameters(GetHomeParams::default()))).unwrap();
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
            serde_json::from_str(&s.get_home(Parameters(GetHomeParams::default()))).unwrap();
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
        s.update(Parameters(UpdateParams { items: specs })).unwrap();
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
        s.update(Parameters(UpdateParams { items: vec![spec] }))
            .unwrap();
        let home = s.get_home(Parameters(GetHomeParams::default()));
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
        let home = s.get_home(Parameters(GetHomeParams::default()));
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
        s.update(Parameters(UpdateParams { items: vec![spec] }))
            .unwrap();
        let reply = s
            .annotations(Parameters(AnnotationParams {
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
        s.update(Parameters(UpdateParams { items: vec![spec] }))
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
}
