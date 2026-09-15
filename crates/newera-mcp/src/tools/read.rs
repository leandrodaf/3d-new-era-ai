//! Reading the plan without dumping it, and the reference data behind it.
//!
//! A full read of a real project is 50 KB on one line, which no normal tool
//! can slice — so this is all about asking for less: by id, by room, by
//! rectangle, by kind, by field, one element per line.

use newera_core::{Home, Point2};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ErrorData, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use super::NewEraMcp;
use super::reply::invalid;
use crate::compact;

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
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct CatalogParams {
    /// Search words (Portuguese or English), e.g. `cama casal`.
    q: Option<String>,
    /// Category id, e.g. `kitchen`.
    cat: Option<String>,
    /// Max rows (default 40).
    limit: Option<usize>,
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
#[tool_router(router = read_router, vis = "pub(crate)")]
impl NewEraMcp {
    #[tool(
        description = "Home state. detail=summary is cheapest. Ask for less instead of reading everything: ids=[…] resolves ids (group parts included), room=<id|name> and rect=[[x0,y0],[x1,y1]] read one place, kinds=[walls|rooms|dims|labels|furniture|polylines] and fields=[…] trim each row, parts=true opens groups, ndjson=true prints one element per line so a long answer can be read a slice at a time. Every piece carries bounds (plan box with angle applied) and faces (the side it opens toward). Ids share one counter per version (w1, r2, f3…) and are never reused, so a new version may start at any number."
    )]
    pub(crate) fn get_home(
        &self,
        Parameters(p): Parameters<GetHomeParams>,
    ) -> Result<String, ErrorData> {
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
    #[allow(clippy::unused_self)] // tool methods need the receiver
    #[tool(
        description = "Wall types [id,name,t] (drywall, masonry, concrete…) and finish patterns [key,label,color,tile]."
    )]
    pub(crate) fn materials(&self) -> String {
        compact::materials().to_string()
    }
    #[allow(clippy::unused_self)] // tool methods need the receiver
    #[tool(description = "Find catalog items: rows [id,name,w,d,h] in cm.")]
    pub(crate) fn catalog(&self, Parameters(p): Parameters<CatalogParams>) -> String {
        compact::catalog(p.q.as_deref(), p.cat.as_deref(), p.limit.unwrap_or(40)).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::server;

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
}
