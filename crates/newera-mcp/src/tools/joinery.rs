//! Parametric joinery: one build at a time, and the boards it comes to.
//!
//! The server computes every board, clearance and rule, so a build either
//! answers with its parts or says what to change.

use newera_core::{Command, Point2};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ErrorData, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use super::NewEraMcp;
use super::reply::{core, invalid};
use crate::compact;
use crate::edit;

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
#[tool_router(router = joinery_router, vis = "pub(crate)")]
impl NewEraMcp {
    #[tool(
        description = "Parametric joinery and interiors; the server computes every board, clearance and rule and replies {id,name,size,parts,hardware,notes}. What a workshop would say — a board nobody stocks, a shelf that will sag, a drawer front too short to grip, a niche shallower than the cooktop standard wants — comes back in notes and is built anyway: the rules advise, they never refuse. Only what has no geometry at all fails, and says why. kind + p: cabinet {w,h,d cm; t 15|18|25 mm; back mm; door hinged|sliding|drawers|none; doors; shelves; drawers; dividers; plinth; cooktop; color; front finish} · slats {w,h cm; slat, thickness, gap mm; orientation vertical|horizontal; backing; finish} · countertop {length,depth,height,thickness cm; material; support none|legs|brackets; cutouts [{kind sink|cooktop|grommet, x, w?, d?}]} · cove {room or pts; type open|closed|inverted; ceiling, width, drop, slot cm; led} · shadow_gap {room or pts; ceiling, gap, depth cm; led} · sofa {length,depth,seat,back cm; arms straight|rounded|none; modules; color}. Place with at|wall(+along), angle, elev. Change a build: id + p with only new values (e.g. {\"shelves\":3}). dry=true validates only."
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
                let build: newera_joinery::Build =
                    newera_joinery::parse_params(&value).map_err(invalid)?;
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
        description = "Cut list of joinery builds: rows [part,board,qty,length,width,thickness mm,edge long+short,cutouts [x,y,w,d] mm?] merged by size, hardware, sheets per board. path .csv, or .dxf/.svg (boards laid out on sheets), writes a file. sources names the panel standards behind the boards: MDF is a dry-process fibreboard (NBR 15316), MDP a particleboard of 551 to 750 kg/m³ that holds screws better (NBR 14810)."
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
        let mut reply = serde_json::json!({
            "rows": rows,
            "hardware": hardware,
            "sources": super::sources(&["nbr14810", "nbr15316"]),
        });
        if !sheets.is_empty() {
            reply["sheets"] = serde_json::json!(sheets);
        }
        Ok(reply.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::CreateParams;
    use crate::tools::server;

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
        // A request the workshop would argue with is built, and argued with
        // in the notes; dry runs still create nothing.
        let shallow =
            joinery(r#"{"kind":"cabinet","p":{"d":35,"cooktop":true},"dry":true}"#).unwrap();
        assert!(shallow.contains("cooktop"), "{shallow}");
        // What has no geometry at all still fails, and says why.
        let flat = joinery(r#"{"kind":"cabinet","p":{"w":2}}"#).unwrap_err();
        assert!(flat.message.contains("lado de dentro"), "{}", flat.message);
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
        // The boards say which standard defines them, resolved once.
        assert_eq!(list["sources"]["nbr15316"][1], "A", "{list}");
        assert!(
            list["sources"]["nbr14810"][0]
                .as_str()
                .unwrap()
                .contains("14810"),
            "{list}"
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
}
