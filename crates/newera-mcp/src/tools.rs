//! MCP tool surface. Each tool is a thin adapter over [`crate::edit`] or
//! `newera-core`; all of them share the document the editor is showing.

use std::path::{Path, PathBuf};

use base64::Engine as _;
use newera_core::{
    Command, Compass, Document, Home, Point2, SharedDocument, from_project_json, ops,
    resolve_project_path, to_project_json,
};
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
Points are [x,y]. Id prefixes: w wall, r room, d dimension, t label, f furniture/door/window. \
Reads omit defaults (wall t=15 h=250). Writes reply `ok rev=N [ids=...]`; don't re-read \
unless needed. Every change is one undoable step. Use render_plan to check visually.";

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
    /// Output file; `.svg` or `.png`.
    path: String,
    w: Option<u32>,
    h: Option<u32>,
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
        Self {
            document,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Home state. detail=summary is cheapest.")]
    fn get_home(&self, Parameters(p): Parameters<GetHomeParams>) -> String {
        let doc = self.document.read();
        match p.detail.as_deref() {
            Some("summary") => compact::summary(doc.home(), doc.revision()),
            _ => compact::home(doc.home(), doc.revision()),
        }
        .to_string()
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
            let c = doc.home().compass;
            commands.push(Command::SetCompass {
                compass: Compass {
                    center: p.compass_at.unwrap_or(c.center),
                    diameter: p.compass_d.unwrap_or(c.diameter),
                    north_degrees: p.north.unwrap_or(c.north_degrees),
                    visible: p.compass_visible.unwrap_or(c.visible),
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
        let project = doc.path().map(Path::to_path_buf);
        let size = |path: &str| {
            let resolved = resolve_project_path(project.as_deref(), path);
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

    #[tool(description = "Export the plan to a .svg (true scale, cm) or .png file.")]
    fn export_plan(&self, Parameters(p): Parameters<ExportParams>) -> Result<String, ErrorData> {
        let path = PathBuf::from(&p.path);
        let bytes = match path.extension().and_then(|e| e.to_str()) {
            Some("svg") => {
                let doc = self.document.read();
                let scene = plan_scene(doc.home(), &scene_options());
                to_svg(&scene, &SvgOptions::default()).into_bytes()
            }
            Some("png") => self.render(p.w.unwrap_or(1600), p.h.unwrap_or(1200), None, false)?,
            _ => return Err(invalid("path must end with .svg or .png")),
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
        std::fs::write(&path, to_project_json(doc.home()))
            .map_err(|e| invalid(format!("cannot write {}: {e}", path.display())))?;
        doc.mark_saved(&path);
        Ok(format!("ok {}", path.display()))
    }

    #[tool(description = "Open a project (.newera), replacing the current one.")]
    fn open_home(&self, Parameters(p): Parameters<PathParams>) -> Result<String, ErrorData> {
        let path = PathBuf::from(p.path.ok_or_else(|| invalid("`path` is required"))?);
        let json = std::fs::read_to_string(&path)
            .map_err(|e| invalid(format!("cannot read {}: {e}", path.display())))?;
        let home = from_project_json(&json).map_err(|e| invalid(e.to_string()))?;
        let mut doc = self.document.write();
        doc.load(home);
        doc.mark_saved(&path);
        Ok(ok(&doc, &[]))
    }

    #[tool(description = "Start a new empty project.")]
    fn new_home(&self) -> String {
        let mut doc = self.document.write();
        doc.load(Home::default());
        doc.set_path(None);
        ok(&doc, &[])
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
        compact::issues(self.document.read().home()).to_string()
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
        let scene = plan_scene(doc.home(), &scene_options());
        let project = doc.path().map(Path::to_path_buf);
        drop(doc);
        let options = RenderOptions {
            width: w,
            height: h,
            grid,
            region: region.map(|[a, b]| (a, b)),
            ..RenderOptions::default()
        };
        let load = |path: &str| {
            image::open(resolve_project_path(project.as_deref(), path))
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

fn scene_options() -> SceneOptions {
    SceneOptions {
        show_background: true,
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

fn ok(doc: &Document, ids: &[String]) -> String {
    if ids.is_empty() {
        format!("ok rev={}", doc.revision())
    } else {
        format!("ok rev={} ids={}", doc.revision(), ids.join(","))
    }
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
}
