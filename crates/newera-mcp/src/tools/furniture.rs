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
    /// Try it without applying; see `update`. `"summary"` answers short.
    dry: Option<crate::tools::reply::Dry>,
}
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct ArrangeParams {
    /// `array` (copies in a row), `align`, `distribute`, `flip`, `rotate`,
    /// `mirror`, `group`, `ungroup`, `front`, `back`.
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
    /// align/distribute: `x` or `y`.
    axis: Option<String>,
    /// align: which edge to line up — `low` (left, or top of the plan),
    /// `middle`, `high` — and `value`, the coordinate to put it on.
    edge: Option<String>,
    value: Option<f64>,
    /// distribute: centimeters between one piece and the next (default 0).
    gap: Option<f64>,
}
#[tool_router(router = furniture_router, vis = "pub(crate)")]
impl NewEraMcp {
    #[tool(
        description = "Place catalog items: at=[x,y] center (doors/windows near a wall snap into it; into=[x,y] picks the swing side), or wall=id (+along cm) to put doors/windows in a wall or furniture against it. Sizes w/d/h override defaults; pitch/roll tilt; angle clockwise degrees (0: front faces +y, down the plan; back/headboard toward -y); mat finish (wood, marble, img:…; 'img:facade.png fit' stretches one image: a reference board to compare with render_3d view=front) and opacity (glass 0.3); defaults {…} fills every item; px=true reads coordinates as background pixels. cat=beam with a,b=[x,y,z] (z above the floor) and w×h section makes rafters, posts and braces; a beam reaching into a roof stops under it. Pools: pool or pool-oval. dry=true answers what it would do — what it would add, the clearances around it, the findings it would settle or create — without writing; dry=\"summary\" answers short."
    )]
    pub(crate) fn place(
        &self,
        Parameters(p): Parameters<PlaceParams>,
    ) -> Result<String, ErrorData> {
        let dry = crate::tools::reply::Dry::on(p.dry.as_ref());
        let brief = crate::tools::reply::Dry::brief(p.dry.as_ref());
        let mut items = edit::with_defaults(p.items, p.defaults.as_ref()).map_err(invalid)?;
        if p.px {
            let bg = {
                let doc = self.document.read();
                background_scale(&doc)?
            };
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
        // A dry run does not switch version either: nothing about the plan,
        // or about what the user is looking at, moves.
        if dry {
            let doc = self.document.read();
            return crate::tools::reply::preview_with(&doc, brief, move |scratch| {
                edit::place(scratch, items).map(|_| ()).map_err(invalid)
            });
        }
        let mut doc = self.document.write();
        on_variant(&mut doc, p.v)?;
        let ids = edit::place(&mut doc, items).map_err(invalid)?;
        Ok(ok(&doc, &ids))
    }

    #[tool(
        description = "Arrange elements in one undo step. array {ids,n,dx,dy,dz} adds n copies stepping by dx/dy/dz cm (rafters, columns); align {ids,axis,edge:low|middle|high,value} lines pieces up by an edge — backs on one line, fronts on another — instead of by centers you work out yourself; distribute {ids,axis,gap?} sets them side by side in the order given, from where the first one is (a run of joinery); flip {ids} turns a piece back to front, rebuilding what is inside it (mirror only swaps left and right and leaves the front where it was); rotate {ids,angle clockwise,about?,copy?}; mirror {ids,a,b,copy?} across the line a-b; group {ids,name?} joins pieces into one box that moves/hides together, ungroup {ids:[group]}; front/back {ids} draws rooms or pieces on top/underneath (pool over deck, rug under sofa). Returns new ids."
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
            "align" | "distribute" => {
                let axis = match p.axis.as_deref().map(str::trim) {
                    Some("x" | "+x" | "-x") => newera_core::Axis::X,
                    Some("y" | "+y" | "-y") => newera_core::Axis::Y,
                    _ => return Err(invalid("`axis`: x or y")),
                };
                let pieces: Vec<newera_core::FurnitureId> = ids
                    .iter()
                    .map(|id| match id {
                        newera_core::ElementId::Furniture(f) => Ok(*f),
                        other => Err(invalid(format!("{other} is not a piece of furniture"))),
                    })
                    .collect::<Result<_, _>>()?;
                if p.action == "align" {
                    let edge = match p.edge.as_deref().map(str::trim) {
                        Some("low" | "left" | "top" | "back") => newera_core::arrange::Edge::Low,
                        Some("middle" | "center") => newera_core::arrange::Edge::Middle,
                        Some("high" | "right" | "bottom" | "front") => {
                            newera_core::arrange::Edge::High
                        }
                        _ => return Err(invalid("`edge`: low, middle or high")),
                    };
                    let value = p.value.ok_or_else(|| invalid("`value` is required"))?;
                    newera_core::arrange::align(&mut doc, &pieces, axis, edge, value)
                        .map_err(core)?
                } else {
                    newera_core::arrange::distribute(&mut doc, &pieces, axis, p.gap.unwrap_or(0.0))
                        .map_err(core)?
                }
            }
            // Back to front, the whole piece with it. `mirror` swaps left
            // and right and leaves the front where it was, which is a
            // different thing and the one that surprises people.
            "flip" => {
                let mut commands = Vec::new();
                for id in &ids {
                    let Some(newera_core::Element::Furniture(mut piece)) = doc.home().element(*id)
                    else {
                        return Err(invalid(format!("{id} is not a piece of furniture")));
                    };
                    piece.angle = (piece.angle + 180.0).rem_euclid(360.0);
                    commands.push(newera_core::Command::update(piece));
                }
                doc.execute(newera_core::Command::Batch { commands })
                    .map_err(core)?;
                ids.clone()
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

    /// Lining a run of joinery up by its back, setting the modules side by
    /// side, and turning one back to front: three sentences that used to be
    /// arithmetic with centers, and where a plan quietly drifted.
    #[test]
    fn align_distribute_and_flip_say_it_in_the_words_of_the_drawing() {
        let s = server();
        s.create(Parameters(
            serde_json::from_str(
                r#"{"walls":[{"pts":[[0,0],[600,0],[600,400],[0,400]],"closed":true,"t":15}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        s.place(Parameters(
            serde_json::from_str(
                r#"{"items":[{"cat":"base-cabinet","at":[100,100],"w":60,"d":60,"h":90},
                             {"cat":"base-cabinet","at":[200,140],"w":80,"d":60,"h":90},
                             {"cat":"base-cabinet","at":[320,90],"w":40,"d":60,"h":90}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        let arrange = |json: &str| {
            s.arrange(Parameters(serde_json::from_str(json).unwrap()))
                .unwrap()
        };
        // Backs on y = 60, whatever depth each one has.
        arrange(r#"{"action":"align","ids":["f5","f6","f7"],"axis":"y","edge":"low","value":60}"#);
        let home = s.document.read().home().clone();
        for piece in &home.furniture {
            let (min, _) = newera_core::plan_bounds(piece);
            assert!((min.y - 60.0).abs() < 1e-6, "{} at {min:?}", piece.id);
        }

        // Side by side from where the first one is, no gaps: one run.
        arrange(r#"{"action":"distribute","ids":["f5","f6","f7"],"axis":"x"}"#);
        let home = s.document.read().home().clone();
        let mut edges: Vec<(f64, f64)> = home
            .furniture
            .iter()
            .map(|f| {
                let (min, max) = newera_core::plan_bounds(f);
                (min.x, max.x)
            })
            .collect();
        edges.sort_by(|a, b| a.0.total_cmp(&b.0));
        assert!((edges[0].0 - 70.0).abs() < 1e-6, "{edges:?}");
        for pair in edges.windows(2) {
            assert!((pair[1].0 - pair[0].1).abs() < 1e-6, "{edges:?}");
        }

        // And back to front, which `mirror` does not do.
        let before = s.document.read().home().furniture[0].angle;
        arrange(r#"{"action":"flip","ids":["f5"]}"#);
        let after = s.document.read().home().furniture[0].angle;
        assert!((after - (before + 180.0).rem_euclid(360.0)).abs() < 1e-6);
    }

    /// Placing a door the wrong way round used to be found by the next
    /// check; asked first, it is found before anything is written.
    #[test]
    fn place_can_be_asked_before_it_is_done() {
        let s = server();
        s.create(Parameters(
            serde_json::from_str(
                r#"{"walls":[{"pts":[[0,0],[400,0],[400,300],[0,300]],"closed":true,"t":15}],
                    "rooms":[{"name":"Quarto","at":[200,150]}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        let before = s.document.read().revision();
        let dry: serde_json::Value = serde_json::from_str(
            &s.place(Parameters(
                serde_json::from_str(
                    r#"{"items":[{"cat":"bed-double","at":[200,150]}],"dry":"summary"}"#,
                )
                .unwrap(),
            ))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(dry["dry"], true, "{dry}");
        assert_eq!(dry["added_count"], 1, "{dry}");
        assert!(dry["clearances"].is_object(), "{dry}");
        assert_eq!(
            s.document.read().revision(),
            before,
            "nothing was written: {dry}"
        );
    }
}
