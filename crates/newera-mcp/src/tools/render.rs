//! Pictures and files: the plan, the 3D view, a photo, and the exports.

use std::path::PathBuf;

use base64::Engine as _;
use newera_core::{Document, Point2};
use newera_draw::{RenderOptions, SceneOptions, SvgOptions, plan_scene, render_png, to_svg};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{ErrorData, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use super::NewEraMcp;
use super::reply::invalid;

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct RenderParams {
    /// Width px (default 640, max 2048).
    pub(crate) w: Option<u32>,
    /// Height px (default 480, max 2048).
    pub(crate) h: Option<u32>,
    /// Plan region `[[minx,miny],[maxx,maxy]]`; default fits the drawing.
    /// The region is fitted to the image's aspect and grown on the short
    /// side — never cropped — so everything asked for is in the picture.
    pub(crate) region: Option<[Point2; 2]>,
    /// Instead of `region`: a room id or name to frame, with `pad` cm of
    /// margin around it (default 30).
    pub(crate) room: Option<String>,
    pub(crate) pad: Option<f64>,
    /// Draw the grid (default true).
    pub(crate) grid: Option<bool>,
    /// Background image opacity for this render (e.g. 0.5 to compare the
    /// drawing with the scanned reference; 0 hides it).
    pub(crate) bg: Option<f64>,
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
    /// `up` (default), `cutaway` or `down`.
    walls: Option<String>,
    w: Option<u32>,
    h: Option<u32>,
}
/// Plan options as the user sees them: backgrounds, and top views for
/// imported models.
#[cfg(not(target_arch = "wasm32"))]
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

/// Match the browser canvas, which uses symbols instead of disk-backed top
/// views. Asking for the native cache directory calls `std::env::temp_dir`,
/// which panics on wasm32 and aborts the whole editor, even for an empty plan.
#[cfg(target_arch = "wasm32")]
fn scene_options_for(_doc: &Document) -> SceneOptions {
    SceneOptions {
        show_background: true,
        ..SceneOptions::default()
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

#[tool_router(router = render_router, vis = "pub(crate)")]
impl NewEraMcp {
    #[tool(
        description = "PNG of the floor plan, exactly as the user sees it. region=[[minx,miny],[maxx,maxy]] frames a place — it is fitted to the image's aspect by growing the short side, never by cropping, so everything asked for is in the picture — or room=<id|name> with pad cm (default 30) frames a room without working the rectangle out. bg=0..1 overlays the background image to compare with the reference. Keep w/h small to save tokens."
    )]
    pub(crate) fn render_plan(
        &self,
        Parameters(p): Parameters<RenderParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let region = match &p.room {
            Some(raw) => {
                let doc = self.document.read();
                let home = doc.home().level_view(doc.home().current_level());
                let room = home
                    .rooms
                    .iter()
                    .find(|r| r.id.to_string() == *raw || r.name.eq_ignore_ascii_case(raw))
                    .ok_or_else(|| invalid(format!("no room `{raw}` on this storey")))?;
                let (min, max) = newera_core::element_bounds(&home, room.id.into())
                    .ok_or_else(|| invalid("that room has no outline"))?;
                let pad = p.pad.unwrap_or(30.0);
                Some([
                    Point2::new(min.x - pad, min.y - pad),
                    Point2::new(max.x + pad, max.y + pad),
                ])
            }
            None => p.region,
        };
        let png = self.render_with(
            p.w.unwrap_or(640),
            p.h.unwrap_or(480),
            region,
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
        description = "PNG of the home in 3D (software render with outlines, no GPU needed). view: front|back|left|right|top orthographic elevations — front looks from the plan's bottom edge (large y) toward y=0, back from y=0 toward large y, left from x=0, right from large x; cut=cm makes a section keeping only what is beyond that plane from the viewer (front cut=200 keeps y<200, so the wall at y=0 stays as the backdrop; to remove it look from back), aerial (default; frames the whole building; yaw degrees: 0 from east/+x, 90 from south/plan bottom (default 60); pitch down; zoom >1 farther), visitor (current visitor camera) or cam=i (stored point of view). walls=cutaway drops the walls between the eye and a room to 40 cm, walls=down drops them all; either hides ceilings, roofs and the doors, windows and wall pieces of lowered walls, to see the furniture from the side. Keep w/h small."
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
        let cutaway = match p.walls.as_deref() {
            None | Some("up") => None,
            Some("cutaway") => {
                let toward = view.target - view.eye;
                Some(newera_render::Cutaway::facing(
                    home,
                    Point2::new(f64::from(toward.x), f64::from(toward.z)),
                ))
            }
            Some("down") => Some(newera_render::Cutaway::all(home)),
            Some(other) => return Err(invalid(format!("unknown walls `{other}`"))),
        };
        let home = home.clone();
        let assets = doc.asset_dir();
        drop(doc);
        let image =
            newera_render::render_home_cut(&home, &view, cutaway.as_ref(), w, h, assets.as_deref());
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::CreateParams;
    use crate::tools::server;

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
    fn render_3d_brings_the_walls_down() {
        let s = server();
        let params: CreateParams = serde_json::from_str(
            r#"{"walls":[{"pts":[[0,0],[400,0],[400,300],[0,300]],"closed":true}],"rooms":[{"name":"Sala","at":[200,150]}]}"#,
        )
        .unwrap();
        s.create(Parameters(params)).unwrap();
        let png = |walls: &str| {
            let result = s
                .render_3d(Parameters(Render3dParams {
                    walls: Some(walls.into()),
                    w: Some(96),
                    h: Some(72),
                    ..Render3dParams::default()
                }))
                .unwrap();
            let ContentBlock::Image(image) = &result.content[0] else {
                panic!("expected image")
            };
            image.data.clone()
        };
        let up = png("up");
        assert_ne!(png("cutaway"), up, "the near walls came down");
        assert_ne!(png("down"), up);
        assert!(
            s.render_3d(Parameters(Render3dParams {
                walls: Some("sideways".into()),
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
}
