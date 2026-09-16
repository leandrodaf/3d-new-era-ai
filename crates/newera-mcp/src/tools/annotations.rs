//! What the drawing says about itself: dimensions, notes, legends, and the
//! electrical and plumbing layers drawn over the plan.
//!
//! A cote that stopped matching the wall it measures is worse than no cote,
//! so `stale` hunts for them.

use newera_core::Command;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ErrorData, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use super::NewEraMcp;
use super::reply::{core, invalid, ok};
use crate::compact;

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct AnnotationParams {
    /// Dimensions and notes that no longer match the drawing: rows
    /// [id, written, measured, against, text]. A plan of joinery is read
    /// off its notes, so one that still says 66,5 over a corridor of 86 is
    /// worse than no note at all.
    stale: Option<bool>,
    /// Tie every straight dimension to what its ends touch right now, so from
    /// here on they follow the drawing. Run it while the numbers are right.
    anchor: Option<bool>,
    /// Search label text, accent- and case-insensitive, e.g. `porta`; or
    /// `re:<pattern>` for a regular expression.
    q: Option<String>,
    /// Show engineering dimension chains (`auto_dimensions` in the project JSON).
    #[serde(alias = "auto_dimensions")]
    dims: Option<bool>,
    /// Show the room reference schedule and tags (`references` in the JSON).
    #[serde(alias = "references")]
    refs: Option<bool>,
    /// Include brand, model and link in references (`reference_details`).
    #[serde(alias = "reference_details")]
    details: Option<bool>,
    /// Convert the automatic dimension chains into editable dimensions.
    bake: Option<bool>,
    /// Number the schedule again in reading order, closing the gaps pieces
    /// left behind. Numbers are otherwise kept by each piece for good.
    renumber: Option<bool>,
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
#[tool_router(router = annotations_router, vis = "pub(crate)")]
impl NewEraMcp {
    #[tool(
        description = "Electrical and plumbing projects over the plan. active (default) reports {active, hidden}. select {d: electrical|plumbing|architecture}: new symbols (catalog cat electrical/plumbing) and lines go there and the rest is dimmed. show/hide {d}. quantities: {electrical:[[name,count]], plumbing:[...], lines_cm:{...}}."
    )]
    pub(crate) fn disciplines(
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
        description = "Plan annotations. stale=true lists notes whose numbers no longer match the piece they are about, piece names whose sizes (`módulo 70 cm`, `80 × 60`) no longer match the piece, dimensions whose anchor is gone, and unanchored dimensions left with one end in the air a few cm from a face (the drawing moved under them): rows [id, written, measured, against, text]; checked {dims, dims_unanchored, labels, names} counts what was compared — an empty list with nothing checked is not a clean plan — and unverified [[id, text]] lists sizes nothing can confirm: a label about no piece, or a name giving an inner opening, niche, leaf or set (vão, nicho, folha, conjunto) — run it after moving geometry, before handing the plan over. A note says which piece it is about with update(id=t1, about=f5); without that, one standing on a piece or beside a single piece that still shares a number is checked too. anchor=true ties every straight dimension to what its ends touch now, and from then on they are measured again on every change instead of drifting — run it while the numbers are still right; one with an end already off its face is not tied and comes back in left [[id, written, measured, near]], to be fixed first; one whose anchor died with a deleted piece is tied again to what it touches now, or else released (no anchor) instead of staying stale. q=<text> searches label text on every storey; q=re:<pattern> by a regex (`re:^\\[\\d+\\]$` finds index codes). Set any of dims (engineering dimension chains; auto_dimensions in the project JSON), refs (room reference schedule with tags; references — a tag, once given, stays with its piece: new pieces take the next free number and removed ones leave a gap, so a print and the plan a week later agree; renumber=true numbers them again in reading order), details (brand/model/link in refs; reference_details), legend (symbol legend with counts): a switch answers with the modes, changed, and what it shows — chains [[from,to,cm]] for dims, symbols {discipline:[[name,count]]} for legend — not the schedule; refs=true, or no switch at all, returns {dims,refs,details,legend,rooms:[[room,[[tag,name,w,d,h,brand?,model?,url?]]]]}. bake=true turns the automatic chains into editable dimensions (ids returned). Give pieces brand/model/url via update."
    )]
    pub(crate) fn annotations(
        &self,
        Parameters(p): Parameters<AnnotationParams>,
    ) -> Result<String, ErrorData> {
        if p.anchor.unwrap_or(false) {
            let mut doc = self.document.write();
            let view = doc.home().level_view(doc.home().current_level());
            let held = newera_core::anchor_dimensions(&view);
            let ids: Vec<String> = held
                .iter()
                .filter(|d| d.holds.is_some())
                .map(|d| d.id.to_string())
                .collect();
            // Held a piece that is gone and touch nothing now: let go.
            let released: Vec<String> = held
                .iter()
                .filter(|d| d.holds.is_none())
                .map(|d| d.id.to_string())
                .collect();
            // Anchoring reads the drawing as the intent, so a dimension the
            // drawing already moved away from is not tied to it: it is named,
            // with what it would measure, to be fixed by hand first.
            let left: Vec<serde_json::Value> = view
                .dimensions
                .iter()
                .filter_map(|d| {
                    let loose = newera_core::loose_end(&view, d)?;
                    Some(serde_json::json!([
                        d.id.to_string(),
                        compact::num(d.length()),
                        compact::num(loose.measured),
                        loose.near.id.to_string(),
                    ]))
                })
                .collect();
            if !held.is_empty() {
                let commands = held.into_iter().map(Command::update).collect();
                doc.execute(Command::Batch { commands }).map_err(core)?;
            }
            let mut out = serde_json::json!({"anchored": ids});
            if !released.is_empty() {
                out["released"] = serde_json::json!(released);
            }
            if !left.is_empty() {
                out["left"] = serde_json::json!(left);
            }
            return Ok(out.to_string());
        }
        if p.stale.unwrap_or(false) || p.q.is_some() {
            let doc = self.document.read();
            let view = doc.home().level_view(doc.home().current_level());
            let mut out = serde_json::Map::new();
            if p.stale.unwrap_or(false) {
                let check = newera_core::check_annotations(&view);
                let rows: Vec<serde_json::Value> = check
                    .stale
                    .into_iter()
                    .map(|s| {
                        serde_json::json!([
                            s.id.to_string(),
                            compact::num(s.drawn),
                            compact::num(s.measured),
                            s.against.map(|a| a.to_string()),
                            s.text,
                        ])
                    })
                    .collect();
                out.insert("stale".to_owned(), serde_json::json!(rows));
                // An empty list after comparing nothing is not a clean plan.
                out.insert(
                    "checked".to_owned(),
                    serde_json::json!({
                        "dims": check.dimensions,
                        "dims_unanchored": check.unanchored,
                        "labels": check.labels,
                        "names": check.names,
                    }),
                );
                if !check.unverified.is_empty() {
                    let rows: Vec<serde_json::Value> = check
                        .unverified
                        .into_iter()
                        .map(|(id, text)| serde_json::json!([id.to_string(), text]))
                        .collect();
                    out.insert("unverified".to_owned(), serde_json::json!(rows));
                }
            }
            if let Some(query) = &p.q {
                // `re:` searches by a pattern — the way to sweep a naming
                // convention like `[09]` — and anything else by the words,
                // accent- and case-insensitive. Every storey is searched.
                let matches: Box<dyn Fn(&str) -> bool> =
                    if let Some(pattern) = query.strip_prefix("re:") {
                        let re = regex_lite::Regex::new(pattern)
                            .map_err(|e| invalid(format!("q: {e}")))?;
                        Box::new(move |text| re.is_match(text))
                    } else {
                        let needle = newera_core::fold(query);
                        Box::new(move |text| newera_core::fold(text).contains(&needle))
                    };
                let rows: Vec<serde_json::Value> = doc
                    .home()
                    .labels
                    .iter()
                    .filter(|l| matches(&l.text))
                    .map(compact::label)
                    .collect();
                out.insert("labels".to_owned(), serde_json::json!(rows));
            }
            return Ok(serde_json::Value::Object(out).to_string());
        }
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
        if p.renumber.unwrap_or(false) {
            let cleared = newera_core::cleared_references(doc.home());
            if !cleared.is_empty() {
                let commands = cleared.into_iter().map(Command::update).collect();
                doc.execute(Command::Batch { commands }).map_err(core)?;
            }
        }
        let was = doc.home().annotations;
        let mut next = was;
        next.auto_dimensions = p.dims.unwrap_or(next.auto_dimensions);
        next.references = p.refs.unwrap_or(next.references);
        next.reference_details = p.details.unwrap_or(next.reference_details);
        next.legend = p.legend.unwrap_or(next.legend);
        if next != was {
            doc.execute(Command::SetAnnotations { annotations: next })
                .map_err(core)?;
        }
        let modes = serde_json::json!({
            "rev": doc.revision(),
            "dims": next.auto_dimensions,
            "refs": next.references,
            "details": next.reference_details,
            "legend": next.legend,
        });
        // Switching a mode answers with the switch and what that mode shows,
        // not with the whole schedule every time: three switches used to cost
        // the same 126-row list three times, and none showed what was asked.
        let switched = p.dims.is_some() || p.legend.is_some() || p.details.is_some();
        if switched && p.refs != Some(true) {
            let mut out = modes;
            let changed: Vec<&str> = [
                ("dims", was.auto_dimensions != next.auto_dimensions),
                ("refs", was.references != next.references),
                ("details", was.reference_details != next.reference_details),
                ("legend", was.legend != next.legend),
            ]
            .into_iter()
            .filter_map(|(name, moved)| moved.then_some(name))
            .collect();
            out["changed"] = serde_json::json!(changed);
            let view = doc.home().level_view(doc.home().current_level());
            if p.dims == Some(true) {
                // The chains the plan now draws: [from, to, cm].
                let chains: Vec<serde_json::Value> = newera_core::auto_dimensions(&view)
                    .iter()
                    .map(|d| {
                        serde_json::json!([
                            compact::point(d.start),
                            compact::point(d.end),
                            compact::num(d.length())
                        ])
                    })
                    .collect();
                out["chains"] = serde_json::json!(chains);
            }
            if p.legend == Some(true) {
                // The symbols the legend counts: [name, count] per discipline.
                let mut legend = serde_json::Map::new();
                for d in newera_core::Discipline::ALL {
                    let mut counts: std::collections::BTreeMap<String, usize> =
                        std::collections::BTreeMap::new();
                    for top in &view.furniture {
                        for piece in top.flatten() {
                            if piece.discipline.or(top.discipline) == Some(d) {
                                *counts.entry(piece.name.clone()).or_default() += 1;
                            }
                        }
                    }
                    if !counts.is_empty() {
                        let key = serde_json::to_value(d)
                            .ok()
                            .and_then(|v| v.as_str().map(str::to_owned))
                            .unwrap_or_default();
                        legend.insert(
                            key,
                            serde_json::json!(counts.into_iter().collect::<Vec<_>>()),
                        );
                    }
                }
                out["symbols"] = serde_json::Value::Object(legend);
            }
            return Ok(out.to_string());
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
        let mut out = modes;
        out["rooms"] = serde_json::json!(rooms);
        Ok(out.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::{CreateParams, UpdateSpec};
    use crate::tools::elements::UpdateParams;
    use crate::tools::furniture::PlaceParams;
    use crate::tools::read::GetHomeParams;
    use crate::tools::render::RenderParams;
    use crate::tools::server;

    #[test]
    fn a_dimension_anchored_on_a_part_of_a_group_is_alive_and_follows_it() {
        let s = server();
        s.create(Parameters(
            serde_json::from_str(
                r#"{"walls":[{"pts":[[0,0],[500,0],[500,400],[0,400]],"closed":true}],
                    "dims":[{"a":[250,7.5],"b":[250,98.5]}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        // A pull-out broom cupboard grouped from drawn solids; its front is
        // what the dimension ends on, 91 cm from the wall.
        {
            let mut doc = s.document.write();
            let board = |id: u64, name: &str, y: f64, d: f64| newera_core::Furniture {
                id: newera_core::FurnitureId(id),
                catalog: "box".into(),
                name: name.to_owned(),
                position: newera_core::Point2::new(250.0, y),
                width: 30.0,
                depth: d,
                height: 195.0,
                ..newera_core::Furniture::default()
            };
            let mut cupboard = board(100, "vassoureiro", 60.0, 77.0);
            cupboard.children = vec![
                board(101, "corpo", 52.0, 89.0),
                board(102, "frente 30 × 195", 97.5, 2.0),
            ];
            doc.execute(newera_core::Command::insert(cupboard)).unwrap();
        }
        let annotations = |json: &str| -> serde_json::Value {
            serde_json::from_str(
                &s.annotations(Parameters(serde_json::from_str(json).unwrap()))
                    .unwrap(),
            )
            .unwrap()
        };
        let anchored = annotations(r#"{"anchor":true}"#);
        assert_eq!(
            anchored["anchored"].as_array().unwrap().len(),
            1,
            "{anchored}"
        );
        let holds = s.document.read().home().dimensions[0]
            .holds
            .clone()
            .unwrap();
        assert!(
            holds.iter().any(|h| h.id.to_string() == "f102"),
            "{holds:?}"
        );

        let stale = annotations(r#"{"stale":true}"#);
        assert_eq!(
            stale["stale"],
            serde_json::json!([]),
            "the front is there: {stale}"
        );
        assert_eq!(stale["checked"]["dims"], 1, "{stale}");

        // And it follows the front when the cupboard moves.
        s.move_elements(Parameters(
            serde_json::from_str(r#"{"ids":["f100"],"dy":10,"dx":0}"#).unwrap(),
        ))
        .unwrap();
        let length = s.document.read().home().dimensions[0].length();
        assert!((length - 101.0).abs() < 0.01, "{length}");
    }

    #[test]
    fn an_anchor_that_died_with_its_piece_is_tied_again_or_let_go() {
        let s = server();
        s.create(Parameters(
            serde_json::from_str(
                r#"{"walls":[{"pts":[[0,0],[500,0],[500,400],[0,400]],"closed":true}],
                    "dims":[{"a":[250,70],"b":[250,310]}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        let place = || {
            s.place(Parameters(
                serde_json::from_str(
                    r#"{"items":[{"cat":"base-cabinet","at":[250,40],"w":300,"d":60,"h":90}]}"#,
                )
                .unwrap(),
            ))
            .unwrap();
            s.document
                .read()
                .home()
                .furniture
                .last()
                .unwrap()
                .id
                .to_string()
        };
        let first = place();
        s.place(Parameters(
            serde_json::from_str(
                r#"{"items":[{"cat":"base-cabinet","at":[250,340],"w":300,"d":60,"h":90,"angle":180}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        let annotations = |json: &str| -> serde_json::Value {
            serde_json::from_str(
                &s.annotations(Parameters(serde_json::from_str(json).unwrap()))
                    .unwrap(),
            )
            .unwrap()
        };
        let dim = s.document.read().home().dimensions[0].id.to_string();
        assert_eq!(
            annotations(r#"{"anchor":true}"#)["anchored"][0],
            dim.as_str()
        );

        // The cabinet goes and a new one takes its place: the number is right,
        // only the anchor is dead.
        s.delete(Parameters(
            serde_json::from_str(&format!(r#"{{"ids":["{first}"]}}"#)).unwrap(),
        ))
        .unwrap();
        let dead = annotations(r#"{"stale":true}"#);
        assert!(
            dead["stale"][0][4].as_str().unwrap().contains("is gone"),
            "{dead}"
        );
        let second = place();
        let again = annotations(r#"{"anchor":true}"#);
        assert_eq!(again["anchored"][0], dim.as_str(), "tied again: {again}");
        let clean = annotations(r#"{"stale":true}"#);
        assert_eq!(clean["stale"], serde_json::json!([]), "{clean}");
        let holds = s.document.read().home().dimensions[0]
            .holds
            .clone()
            .unwrap();
        assert!(
            holds.iter().any(|h| h.id.to_string() == second),
            "{holds:?}"
        );

        // Gone with nothing in its place: let go, not stale forever.
        s.delete(Parameters(
            serde_json::from_str(&format!(r#"{{"ids":["{second}"]}}"#)).unwrap(),
        ))
        .unwrap();
        let released = annotations(r#"{"anchor":true}"#);
        assert_eq!(released["released"][0], dim.as_str(), "{released}");
        let after = annotations(r#"{"stale":true}"#);
        assert!(
            after["stale"]
                .as_array()
                .unwrap()
                .iter()
                .all(|r| !r[4].as_str().unwrap().contains("is gone")),
            "{after}"
        );
        assert_eq!(after["checked"]["dims_unanchored"], 1, "{after}");
    }

    #[test]
    fn deleting_a_piece_names_the_labels_left_pointing_at_it() {
        let s = server();
        s.create(Parameters(
            serde_json::from_str(
                r#"{"labels":[{"text":"[09]","at":[100,30]},{"text":"lixeira embutida","at":[400,400]},{"text":"[10]","at":[600,30]}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        s.place(Parameters(
            serde_json::from_str(
                r#"{"items":[{"cat":"box","name":"nicho da lixeira","at":[100,30],"w":40,"d":60,"h":85},
                             {"cat":"box","name":"torre","at":[600,30],"w":60,"d":60,"h":220}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        let (niche, labels) = {
            let doc = s.document.read();
            let home = doc.home();
            (
                home.furniture[0].id.to_string(),
                home.labels
                    .iter()
                    .map(|l| l.id.to_string())
                    .collect::<Vec<_>>(),
            )
        };
        s.update(Parameters(UpdateParams {
            items: serde_json::from_str(&format!(
                r#"[{{"id":"{}","about":"{niche}"}}]"#,
                labels[1]
            ))
            .unwrap(),
            rename: None,
            v: None,
            dry: None,
        }))
        .unwrap();
        let delete = |json: String| {
            s.delete(Parameters(serde_json::from_str(&json).unwrap()))
                .unwrap()
        };
        let reply = delete(format!(r#"{{"ids":["{niche}"]}}"#));
        let left: serde_json::Value =
            serde_json::from_str(&reply[reply.find('{').expect(&reply)..]).unwrap();
        assert_eq!(
            left["labels_left"],
            serde_json::json!([[labels[0], "[09]"], [labels[1], "lixeira embutida"]]),
            "the one on the tower stays out of it: {reply}"
        );

        // The note about it is stale too, for whoever did not read the reply.
        let stale: serde_json::Value = serde_json::from_str(
            &s.annotations(Parameters(
                serde_json::from_str(r#"{"stale":true}"#).unwrap(),
            ))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(stale["stale"][0][0], labels[1].as_str(), "{stale}");

        // Or they go in the same step.
        s.document.write().undo().unwrap();
        let reply = delete(format!(r#"{{"ids":["{niche}"],"labels":true}}"#));
        assert!(reply.contains("labels_deleted"), "{reply}");
        let doc = s.document.read();
        assert_eq!(doc.home().labels.len(), 1);
        assert_eq!(doc.home().labels[0].text, "[10]");
    }

    #[test]
    fn a_dimension_the_counter_moved_away_from_is_caught_and_not_anchored() {
        let s = server();
        s.create(Parameters(
            serde_json::from_str(
                r#"{"walls":[{"pts":[[0,0],[500,0],[500,400],[0,400]],"closed":true}],
                    "dims":[{"a":[250,91.5],"b":[250,310]}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        // An 84 cm counter on the top wall and a 60 cm one on the bottom
        // wall: the dimension reads the corridor between them.
        s.place(Parameters(
            serde_json::from_str(
                r#"{"items":[{"cat":"base-cabinet","at":[250,49.5],"w":300,"d":84,"h":90},
                             {"cat":"base-cabinet","at":[250,340],"w":300,"d":60,"h":90,"angle":180}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        let annotations = |json: &str| -> serde_json::Value {
            serde_json::from_str(
                &s.annotations(Parameters(serde_json::from_str(json).unwrap()))
                    .unwrap(),
            )
            .unwrap()
        };
        let before = annotations(r#"{"stale":true}"#);
        assert_eq!(before["stale"], serde_json::json!([]), "{before}");
        assert_eq!(before["checked"]["dims"], 0, "{before}");
        assert_eq!(
            before["checked"]["dims_unanchored"], 1,
            "an empty list over an unanchored dimension says so: {before}"
        );

        // The counter is recessed to 65 cm; the dimension still says 218.5.
        let (counter, dim) = {
            let doc = s.document.read();
            (
                doc.home().furniture[0].id.to_string(),
                doc.home().dimensions[0].id.to_string(),
            )
        };
        s.update(Parameters(UpdateParams {
            items: serde_json::from_str(&format!(
                r#"[{{"id":"{counter}","d":65,"anchor":"back"}}]"#
            ))
            .unwrap(),
            rename: None,
            v: None,
            dry: None,
        }))
        .unwrap();
        let after = annotations(r#"{"stale":true}"#);
        let row = &after["stale"][0];
        assert_eq!(row[0], dim.as_str(), "{after}");
        assert_eq!(row[1], 218.5, "{after}");
        assert_eq!(row[2], 237.5, "what the corridor measures now: {after}");
        assert_eq!(row[3], counter.as_str(), "{after}");

        // Anchoring now would freeze the wrong number: it is left out, named.
        let anchored = annotations(r#"{"anchor":true}"#);
        assert_eq!(anchored["anchored"], serde_json::json!([]), "{anchored}");
        assert_eq!(anchored["left"][0][0], dim.as_str(), "{anchored}");
        assert_eq!(anchored["left"][0][2], 237.5, "{anchored}");
        assert!(
            s.document.read().home().dimensions[0].holds.is_none(),
            "not tied to the wrong face"
        );
    }

    #[test]
    fn stale_says_how_much_it_compared_and_reads_the_names() {
        let s = server();
        let annotations = |json: &str| -> serde_json::Value {
            serde_json::from_str(
                &s.annotations(Parameters(serde_json::from_str(json).unwrap()))
                    .unwrap(),
            )
            .unwrap()
        };
        // Index codes only: nothing to compare, and the answer says so.
        s.create(Parameters(
            serde_json::from_str(r#"{"labels":[{"text":"[09]","at":[900,900]}]}"#).unwrap(),
        ))
        .unwrap();
        let empty = annotations(r#"{"stale":true}"#);
        assert_eq!(empty["stale"], serde_json::json!([]), "{empty}");
        assert_eq!(
            empty["checked"],
            serde_json::json!({"dims": 0, "dims_unanchored": 0, "labels": 0, "names": 0}),
            "{empty}"
        );

        // A joiner's plan keeps its sizes in the names.
        s.place(Parameters(
            serde_json::from_str(
                r#"{"items":[
                    {"cat":"box","name":"Cuba — módulo 70 cm","at":[35,30],"w":64.5,"d":60,"h":92},
                    {"cat":"box","name":"Torre quente 60 × 60 × 220; nicho 61 × 87 cm","at":[200,30],"w":60,"d":60,"h":220},
                    {"cat":"box","name":"Aéreo de 118 cm; duas folhas de 59 cm","at":[400,30],"w":118,"d":35,"h":70}
                ]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        let ids: Vec<String> = s
            .document
            .read()
            .home()
            .furniture
            .iter()
            .map(|f| f.id.to_string())
            .collect();
        let report = annotations(r#"{"stale":true}"#);
        let rows = report["stale"].as_array().unwrap();
        assert_eq!(rows.len(), 1, "{report}");
        assert_eq!(
            rows[0][0],
            ids[0].as_str(),
            "the sink was narrowed: {report}"
        );
        assert_eq!(rows[0][1], 70, "{report}");
        assert_eq!(rows[0][2], 64.5, "{report}");
        assert_eq!(report["checked"]["names"], 3, "{report}");
        let unverified = report["unverified"].as_array().unwrap();
        let said = |id: &str, text: &str| {
            unverified
                .iter()
                .any(|r| r[0] == id && r[1].as_str().unwrap().contains(text))
        };
        assert!(said(&ids[1], "nicho 61 × 87"), "{report}");
        assert!(said(&ids[2], "folhas de 59"), "{report}");
    }

    #[test]
    fn annotations_report_the_notes_that_stopped_being_true() {
        let s = server();
        s.create(Parameters(
            serde_json::from_str(
                r#"{"walls":[{"pts":[[0,0],[500,0],[500,400],[0,400]],"closed":true}],
                    "dims":[{"a":[250,70],"b":[250,310]}],
                    "labels":[{"text":"TORRE 300 × 60 × 90","at":[250,40]}]}"#,
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
        let annotations = |json: &str| -> serde_json::Value {
            serde_json::from_str(
                &s.annotations(Parameters(serde_json::from_str(json).unwrap()))
                    .unwrap(),
            )
            .unwrap()
        };
        // The dimension marks the corridor and still agrees with it.
        assert!(
            annotations(r#"{"stale":true}"#)["stale"]
                .as_array()
                .unwrap()
                .is_empty(),
            "{:?}",
            annotations(r#"{"stale":true}"#)
        );

        // Tie the dimension to the two counters it runs between, while the
        // drawing and the number still agree.
        let anchored = annotations(r#"{"anchor":true}"#);
        assert_eq!(anchored["anchored"][0], "d5", "{anchored}");

        // Deepen the counter: the dimension follows it, the note does not.
        s.update(Parameters(UpdateParams {
            items: serde_json::from_str(r#"[{"id":"f7","d":100,"anchor":"back"}]"#).unwrap(),
            rename: None,
            v: None,
            dry: None,
        }))
        .unwrap();
        let home: serde_json::Value = serde_json::from_str(
            &s.get_home(Parameters(
                serde_json::from_str(r#"{"kinds":["dims"]}"#).unwrap(),
            ))
            .unwrap(),
        )
        .unwrap();
        let dim = &home["dims"][0];
        assert_eq!(dim["a"], serde_json::json!([250, 110]), "{home}");
        assert!(
            (dim["len"].as_f64().unwrap() - 200.0).abs() < 0.5,
            "the dimension measured itself again: {home}"
        );

        let stale = annotations(r#"{"stale":true}"#);
        let rows = stale["stale"].as_array().unwrap();
        assert!(
            !rows.iter().any(|r| r[0] == "d5"),
            "a dimension that follows the drawing is never stale: {stale}"
        );
        let note = rows
            .iter()
            .find(|r| r[0] == "t6")
            .unwrap_or_else(|| panic!("{stale}"));
        assert_eq!(note[3], "f7", "the piece the note sits on: {stale}");
        assert!(
            (note[1].as_f64().unwrap() - 60.0).abs() < 0.01,
            "written: {stale}"
        );
        assert!(
            (note[2].as_f64().unwrap() - 100.0).abs() < 0.01,
            "measured: {stale}"
        );

        // And notes can be found by their text, which no read could do.
        let found = annotations(r#"{"q":"torre"}"#);
        assert_eq!(found["labels"].as_array().unwrap().len(), 1, "{found}");
        assert!(
            annotations(r#"{"q":"varanda"}"#)["labels"]
                .as_array()
                .unwrap()
                .is_empty()
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
        let home = s.get_home(Parameters(GetHomeParams::default())).unwrap();
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
    /// The note that lies is a legend beside its cabinet, not on it: the one
    /// a joiner reads to cut. Three rounds of a real session were spent
    /// finding those by eye.
    #[test]
    fn a_legend_beside_its_piece_is_checked_against_it() {
        let s = server();
        s.create(Parameters(
            serde_json::from_str(
                r#"{"walls":[{"pts":[[0,0],[600,0],[600,400],[0,400]],"closed":true}],
                    "labels":[{"text":"07 Armário portas: 65,83 × 84 × 87","at":[300,150]},
                              {"text":"Escala 1:50","at":[560,380]}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        s.place(Parameters(
            serde_json::from_str(
                r#"{"items":[{"cat":"base-cabinet","at":[300,200],"w":65.83,"d":84,"h":87}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        let annotations = |json: &str| -> serde_json::Value {
            serde_json::from_str(
                &s.annotations(Parameters(serde_json::from_str(json).unwrap()))
                    .unwrap(),
            )
            .unwrap()
        };
        assert!(
            annotations(r#"{"stale":true}"#)["stale"]
                .as_array()
                .unwrap()
                .is_empty(),
            "the legend still matches the cabinet"
        );

        // The cabinet is narrowed and made shallower; the legend stays as
        // typed, which is how a plan ends up lying to the workshop.
        s.update(Parameters(UpdateParams {
            items: serde_json::from_str(r#"[{"id":"f7","w":59.85,"d":72}]"#).unwrap(),
            rename: None,
            v: None,
            dry: None,
        }))
        .unwrap();
        let stale = annotations(r#"{"stale":true}"#);
        let rows = stale["stale"].as_array().unwrap();
        assert_eq!(rows.len(), 1, "{stale}");
        assert_eq!(rows[0][0], "t5", "{stale}");
        assert_eq!(rows[0][3], "f7", "the piece it is about: {stale}");

        // Said outright, the tie holds wherever the note sits.
        s.update(Parameters(UpdateParams {
            items: serde_json::from_str(r#"[{"id":"t5","at":[560,20],"about":"f7"}]"#).unwrap(),
            rename: None,
            v: None,
            dry: None,
        }))
        .unwrap();
        let stale = annotations(r#"{"stale":true}"#);
        assert_eq!(stale["stale"].as_array().unwrap().len(), 1, "{stale}");

        // And a note about nothing in particular is nobody's business.
        assert!(
            !stale["stale"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r[0] == "t6"),
            "{stale}"
        );
    }

    #[test]
    fn a_reference_number_stays_with_its_piece() {
        let s = server();
        s.create(Parameters(
            serde_json::from_str(
                r#"{"walls":[{"pts":[[0,0],[600,0],[600,400],[0,400]],"closed":true}],"rooms":[{"name":"Cozinha","at":[300,200]}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        s.place(Parameters(
            serde_json::from_str(
                r#"{"items":[{"cat":"box","name":"mesa","at":[100,100]},{"cat":"box","name":"aéreo","at":[300,100]},{"cat":"box","name":"arremate","at":[500,100]}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        let annotations = |json: &str| -> serde_json::Value {
            serde_json::from_str(
                &s.annotations(Parameters(serde_json::from_str(json).unwrap()))
                    .unwrap(),
            )
            .unwrap()
        };
        let tags = |report: &serde_json::Value| -> Vec<(String, u64)> {
            report["rooms"][0][1]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| (r[1].as_str().unwrap().to_owned(), r[0].as_u64().unwrap()))
                .collect()
        };
        let first = tags(&annotations(r#"{"refs":true}"#));
        assert_eq!(
            first,
            vec![
                ("mesa".into(), 1),
                ("aéreo".into(), 2),
                ("arremate".into(), 3)
            ]
        );

        // The table goes; the others keep their numbers, gap and all.
        let table = s.document.read().home().furniture[0].id.to_string();
        s.delete(Parameters(
            serde_json::from_str(&format!(r#"{{"ids":["{table}"]}}"#)).unwrap(),
        ))
        .unwrap();
        // A new piece, placed before them in reading order, takes the next number.
        s.place(Parameters(
            serde_json::from_str(r#"{"items":[{"cat":"box","name":"banco","at":[50,50]}]}"#)
                .unwrap(),
        ))
        .unwrap();
        let later = tags(&annotations(r#"{"refs":true}"#));
        assert_eq!(
            later,
            vec![
                ("banco".into(), 4),
                ("aéreo".into(), 2),
                ("arremate".into(), 3)
            ]
        );

        // Undoing the new piece takes its number with it.
        s.document.write().undo().unwrap();
        assert!(
            s.document
                .read()
                .home()
                .furniture
                .iter()
                .all(|f| f.name != "banco")
        );

        // Asked for, the gaps close in reading order.
        let renumbered = tags(&annotations(r#"{"refs":true,"renumber":true}"#));
        assert_eq!(
            renumbered,
            vec![("aéreo".into(), 1), ("arremate".into(), 2)]
        );
    }

    #[test]
    fn a_mode_switch_answers_with_what_it_shows_not_the_schedule() {
        let s = server();
        s.create(Parameters(
            serde_json::from_str(
                r#"{"walls":[{"pts":[[0,0],[400,0],[400,300],[0,300]],"closed":true}],"rooms":[{"name":"Sala","at":[200,150]}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        s.place(Parameters(
            serde_json::from_str(r#"{"items":[{"cat":"sofa-3","at":[200,200]}]}"#).unwrap(),
        ))
        .unwrap();
        s.disciplines(Parameters(DisciplineParams {
            action: Some("select".into()),
            d: Some("electrical".into()),
        }))
        .unwrap();
        s.place(Parameters(
            serde_json::from_str(
                r#"{"items":[{"cat":"outlet-low","at":[10,10]},{"cat":"outlet-low","at":[60,10]}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        let annotations = |json: &str| -> serde_json::Value {
            serde_json::from_str(
                &s.annotations(Parameters(serde_json::from_str(json).unwrap()))
                    .unwrap(),
            )
            .unwrap()
        };

        let legend = annotations(r#"{"legend":true}"#);
        assert!(legend.get("rooms").is_none(), "no schedule: {legend}");
        assert_eq!(legend["changed"], serde_json::json!(["legend"]), "{legend}");
        assert_eq!(
            legend["symbols"]["electrical"],
            serde_json::json!([["Tomada baixa (30 cm)", 2]]),
            "{legend}"
        );

        // The project JSON's name works as well as the MCP's.
        let dims = annotations(r#"{"auto_dimensions":true}"#);
        assert!(dims.get("rooms").is_none(), "{dims}");
        assert_eq!(dims["dims"], true, "{dims}");
        assert!(
            !dims["chains"].as_array().unwrap().is_empty(),
            "the chains drawn: {dims}"
        );

        // Asked for, the schedule comes.
        let refs = annotations(r#"{"references":true}"#);
        assert!(refs["rooms"].is_array(), "{refs}");
        let again = annotations(r#"{"legend":true}"#);
        assert_eq!(
            again["changed"],
            serde_json::json!([]),
            "nothing moved: {again}"
        );
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
        s.update(Parameters(UpdateParams {
            items: vec![spec],
            rename: None,
            v: None,
            dry: None,
        }))
        .unwrap();
        let reply = s
            .annotations(Parameters(AnnotationParams {
                anchor: None,
                stale: None,
                q: None,
                dims: Some(true),
                refs: Some(true),
                details: Some(true),
                bake: None,
                legend: None,
                renumber: None,
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
                anchor: None,
                bake: Some(true),
                ..AnnotationParams::default()
            }))
            .unwrap();
        assert!(
            baked.starts_with("ok rev=") && baked.contains("ids=d"),
            "{baked}"
        );
    }
}
