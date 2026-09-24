//! The reference image under the plan, and the walls traced out of it.
//!
//! Scanned plans arrive as pixels; `px` coordinates and tracing are how they
//! become walls without retyping every corner.

use newera_core::{Command, Point2};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ErrorData, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use super::NewEraMcp;
use super::reply::{background_scale, core, invalid, ok};
use crate::compact;
use crate::edit::{self, BackgroundParams};

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
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
    #[schemars(skip)]
    create: bool,
    /// Wall height cm when creating (default 250).
    h: Option<f64>,
}
#[tool_router(router = background_router, vis = "pub(crate)")]
impl NewEraMcp {
    #[tool(
        description = "Set a scanned plan as background at real scale: path, then cm_per_px (+cm_per_px_y), calibrate {a,b px, cm} or calibrations [{a,b,cm}…] (fits X/Y scales), angle (clockwise °); offset/opacity/visible; clear=true removes."
    )]
    pub(crate) fn set_background(
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
        name = "trace_background",
        description = "Trace walls from the background image (set_background first): thick dark or gray bands across or down the image become walls (colored areas — lawn, plants, furniture — are ignored; collinear pieces split by doors/windows up to max_gap join; region limits the search). Returns rows [[x1,y1],[x2,y2],t] in plan cm; trace_walls adds them as walls. Check with render_plan bg=0.5."
    )]
    pub(crate) fn read_trace(
        &self,
        Parameters(p): Parameters<TraceParams>,
    ) -> Result<String, ErrorData> {
        if p.create {
            return Err(invalid(
                "trace_background only lists; trace_walls creates them",
            ));
        }
        self.trace_background(Parameters(p))
    }
    #[tool(
        description = "Trace walls from the background image, as trace_background does, and create them in one undo step (h: wall height cm, default 250). Check with render_plan bg=0.5."
    )]
    pub(crate) fn trace_walls(
        &self,
        Parameters(mut p): Parameters<TraceParams>,
    ) -> Result<String, ErrorData> {
        p.create = true;
        self.trace_background(Parameters(p))
    }
    /// Traces the background, and creates the walls when asked.
    pub(crate) fn trace_background(
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::CreateParams;
    use crate::tools::server;

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
}
