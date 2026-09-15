//! Distances: clearance around a piece, the gap between two, and what a
//! straight line runs into.
//!
//! The answer an agent used to rebuild in a script from a full read.

use newera_core::Point2;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ErrorData, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use super::NewEraMcp;
use super::reply::invalid;
use crate::compact;

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

#[tool_router(router = measure_router, vis = "pub(crate)")]
impl NewEraMcp {
    #[tool(
        description = "Tape measure over the plan, in cm. from=<id> alone: free floor on all four sides, {clear:{\"+y\":[cm,id,name]}} — dirs picks sides (+x -x +y -y, or front/back/left/right of the piece). from+to (ids or [x,y]): the distance between them, {cm}, or the gap along axis. axis+at: what a straight probe runs into, {spans:[[from,to,id,name]]} with id null for free floor — the answer to \"how wide is the corridor here, and between what\". z limits the height band that counts (default 0-200)."
    )]
    pub(crate) fn measure(
        &self,
        Parameters(p): Parameters<MeasureParams>,
    ) -> Result<String, ErrorData> {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::server;

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
}
