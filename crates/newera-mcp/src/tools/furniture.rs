//! Putting catalog pieces into the plan, and moving them around in batches.

use newera_core::Point2;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ErrorData, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use super::NewEraMcp;
use super::reply::{background_scale, core, invalid, ok, on_variant};
use crate::edit::{self, PlaceSpec};

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
#[tool_router(router = furniture_router, vis = "pub(crate)")]
impl NewEraMcp {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use newera_core::Command;

    use crate::edit::CreateParams;
    use crate::tools::server;

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
}
