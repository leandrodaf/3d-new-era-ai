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
    /// Search label text, accent- and case-insensitive, e.g. `porta`.
    q: Option<String>,
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
/// Lowercased and stripped of accents, so `porta` finds `Portão` and a
/// query typed without accents still matches a plan written with them.
fn fold(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'ê' | 'ë' => 'e',
            'í' | 'î' | 'ï' => 'i',
            'ó' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        })
        .collect()
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
        description = "Plan annotations. stale=true lists dimensions and notes that no longer match the drawing: rows [id, written, measured, against, text] — run it after moving geometry, before handing the plan over. q=<text> searches label text. Set any of dims (engineering dimension chains), refs (room reference schedule with tags), details (brand/model/link in refs), legend (symbol legend with counts); bake=true turns the automatic chains into editable dimensions (ids returned). Otherwise returns {dims,refs,details,rooms:[[room,[[tag,name,w,d,h,brand?,model?,url?]]]]}. Give pieces brand/model/url via update."
    )]
    pub(crate) fn annotations(
        &self,
        Parameters(p): Parameters<AnnotationParams>,
    ) -> Result<String, ErrorData> {
        if p.stale.unwrap_or(false) || p.q.is_some() {
            let doc = self.document.read();
            let view = doc.home().level_view(doc.home().current_level());
            let mut out = serde_json::Map::new();
            if p.stale.unwrap_or(false) {
                let rows: Vec<serde_json::Value> = newera_core::stale_annotations(&view)
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
            }
            if let Some(query) = &p.q {
                let needle = fold(query);
                let rows: Vec<serde_json::Value> = view
                    .labels
                    .iter()
                    .filter(|l| fold(&l.text).contains(&needle))
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::{CreateParams, UpdateSpec};
    use crate::tools::{GetHomeParams, PlaceParams, RenderParams, UpdateParams, server};

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

        // Deepen the counter and both the dimension and the note go stale.
        s.update(Parameters(UpdateParams {
            items: serde_json::from_str(r#"[{"id":"f7","d":100,"anchor":"back"}]"#).unwrap(),
            v: None,
            dry: None,
        }))
        .unwrap();
        let stale = annotations(r#"{"stale":true}"#);
        let rows = stale["stale"].as_array().unwrap();
        let dim = rows
            .iter()
            .find(|r| r[0] == "d5")
            .unwrap_or_else(|| panic!("{stale}"));
        assert!((dim[1].as_f64().unwrap() - 240.0).abs() < 0.5, "{stale}");
        assert!((dim[2].as_f64().unwrap() - 200.0).abs() < 0.5, "{stale}");
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
            v: None,
            dry: None,
        }))
        .unwrap();
        let reply = s
            .annotations(Parameters(AnnotationParams {
                stale: None,
                q: None,
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
}
