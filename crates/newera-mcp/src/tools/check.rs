//! Is this layout wrong, and does it work for the people living in it.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ErrorData, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use super::NewEraMcp;
use super::reply::invalid;
use crate::compact;

/// Who lives there, plus what has already been looked at.
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct ErgonomicsParams {
    #[serde(flatten)]
    pub(crate) profile: newera_ergonomics::Profile,
    /// Findings already analysed: `[[key, reason]]`. They keep showing, with
    /// the reason, and stop costing score. An empty reason takes it back.
    #[serde(default)]
    pub(crate) accept: Vec<Vec<String>>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct CheckParams {
    /// Expected room areas in m² by room name or id, e.g. {"Sala": 10.91};
    /// adds rows [room, expected, actual, diff %].
    pub(crate) areas: Option<std::collections::BTreeMap<String, f64>>,
    /// Storey to check: an id like `lv3`, or `all` for every storey that is
    /// not a reference layer. Default: the storey being shown.
    pub(crate) level: Option<String>,
}
/// A server on an empty document, for the domain modules' tests.
fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}
#[tool_router(router = check_router, vis = "pub(crate)")]
impl NewEraMcp {
    #[tool(
        description = "Ergonomics and habitability review for the people living there (occupants, children, elderly, wheelchair, stature cm, city): room to walk beside beds and in front of kitchen equipment, beds/seats/bathrooms/wardrobes per person, kitchen (work triangle, counter heights, Alexander's counter lengths, five work zones, sockets, gas ventilation, extraction), doors, ceiling heights, windows, minimum furniture, wheelchair turning. Reply {score, capacity, findings:[{sev, place, msg, key, weight, src?, fix?, accepted?}], sources:{src:[title, tier, url]}}. weight is what the score would gain if that one went away, so a score that moved can be read; key names the finding for accept. src is the source the finding stands on, empty when it is common practice; resolve it in sources instead of asking. tier is the reliability ladder A obliges (Brazilian standard, municipal code) · B references (foreign standard) · C doctrine · D measured · E survey — and it is why a finding is an error or only a tip. fix, when present, is a checked change as tool arguments (move or update): apply one, then review again (fixes of one review may overlap). city, e.g. `sao-paulo`, lets the municipal code judge instead of only advising; against a standard the more restrictive one wins — set it once with set_home(city=…) so dry runs and check_layout weigh the same rules. accept=[[key, reason]] marks findings already looked at: they stay in the report with their reason and stop costing score, which is what lets a correct plan reach zero pendencies honestly; accept=[[key, empty]] takes it back."
    )]
    pub(crate) fn ergonomics(&self, Parameters(p): Parameters<ErgonomicsParams>) -> String {
        if !p.accept.is_empty() {
            let mut doc = self.document.write();
            let mut accepted = doc.home().accepted.clone();
            for pair in &p.accept {
                let (key, reason) = (pair.first().cloned().unwrap_or_default(), pair.get(1));
                match reason.map(String::as_str) {
                    None | Some("") => accepted.remove(&key),
                    Some(why) => accepted.insert(key, why.to_owned()),
                };
            }
            let _ = doc.execute(newera_core::Command::SetAccepted { accepted });
        }
        let doc = self.document.read();
        let report = newera_ergonomics::review(doc.home(), &p.profile);
        let findings: Vec<serde_json::Value> = report
            .findings
            .iter()
            .map(|f| {
                let mut row = serde_json::json!({
                    "sev": f.severity,
                    "place": f.place,
                    "msg": f.message,
                    "key": f.key,
                    "weight": f.weight,
                });
                if let Some(code) = f.reference {
                    row["src"] = serde_json::json!(code);
                }
                if let Some(fix) = &f.fix {
                    row["fix"] = fix.clone();
                }
                if let Some(why) = &f.accepted {
                    row["accepted"] = serde_json::json!(why);
                }
                row
            })
            .collect();
        // Each source spelled out once, not once per sentence.
        let codes: Vec<&str> = report.refs.iter().map(|r| r.code).collect();
        serde_json::json!({
            "score": report.score,
            "capacity": report.capacity,
            "findings": findings,
            "sources": super::sources(&codes),
        })
        .to_string()
    }
    #[tool(
        description = "Layout problems: overlap, blocked, in_wall, blocks_door, turned, loose_opening, outgrew_niche, outside_rooms; {} means none. Each one carries name, bounds and z of both elements. Overlaps are classified kind collision (a real clash, listed first), nesting (built in, resting on, tucked under) or cross_level, with extent [x,y,z] cm of the shared space; overlap_kinds counts them. blocked is a cabinet, fridge or wardrobe whose opening face is against a solid — it cannot be used, and `angle` alone does not show it. turned is a group whose built fronts (doors, drawer fronts, kick) face one way and whose `angle` says another: the piece opens where the panels are, so fix the angle, not the clearance it seems to lack. loose_opening is a door or window in no wall — a passage drawn as a panel — which reads as an opening in every schedule and opens nothing. outgrew_niche is an appliance its host stopped holding after the joinery was resized around it, with how far it sticks out: built-in pieces are left out of the overlap check by design, which is why nothing else notices. level: a storey id or `all`, default the one shown. areas {name|id: m²} compares room areas with the reference drawing."
    )]
    pub(crate) fn check_layout(
        &self,
        Parameters(p): Parameters<CheckParams>,
    ) -> Result<String, ErrorData> {
        let doc = self.document.read();
        let (view, scope) = match p.level.as_deref() {
            Some("all") => (doc.home().clone(), newera_core::Storeys::All),
            Some(raw) => {
                let id = raw
                    .parse()
                    .map_err(|_| invalid("level: id like lv3, or all"))?;
                if doc.home().level(id).is_none() {
                    return Err(invalid(format!("no storey {raw}")));
                }
                (
                    doc.home().level_view(Some(id)),
                    newera_core::Storeys::One(id),
                )
            }
            None => (
                doc.home().level_view(doc.home().current_level()),
                newera_core::Storeys::Active,
            ),
        };
        let mut report = compact::issues(&view, scope);
        if let Some(expected) = p.areas {
            let rows: Vec<serde_json::Value> = expected
                .iter()
                .map(|(key, m2)| {
                    let room = view
                        .rooms
                        .iter()
                        .find(|r| r.id.to_string() == *key || r.name.eq_ignore_ascii_case(key));
                    match room {
                        Some(r) => {
                            let actual = r.area() / 10_000.0;
                            let diff = if *m2 > 0.0 {
                                (actual - m2) / m2 * 100.0
                            } else {
                                0.0
                            };
                            serde_json::json!([
                                key,
                                round2(*m2),
                                round2(actual),
                                (diff * 10.0).round() / 10.0
                            ])
                        }
                        None => serde_json::json!([key, round2(*m2), null, null]),
                    }
                })
                .collect();
            report["areas"] = serde_json::Value::Array(rows);
        }
        Ok(report.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use newera_core::{Command, Point2};
    use rmcp::model::ContentBlock;

    use crate::edit::CreateParams;
    use crate::tools::furniture::PlaceParams;
    use crate::tools::render::RenderParams;
    use crate::tools::server;

    #[test]
    fn plan_overlay_and_area_comparison() {
        let s = server();
        let params: CreateParams = serde_json::from_str(
            r#"{"walls":[{"pts":[[0,0],[400,0],[400,300],[0,300]],"closed":true}],"rooms":[{"name":"Sala","at":[200,150]}]}"#,
        )
        .unwrap();
        s.create(Parameters(params)).unwrap();
        let report: serde_json::Value = serde_json::from_str(
            &s.check_layout(Parameters(CheckParams {
                areas: Some([("sala".to_owned(), 10.0), ("Cozinha".to_owned(), 8.0)].into()),
                level: None,
            }))
            .unwrap(),
        )
        .unwrap();
        let rows = report["areas"].as_array().unwrap();
        let cozinha = rows.iter().find(|r| r[0] == "Cozinha").unwrap();
        assert!(
            cozinha[2].is_null(),
            "unknown rooms are reported, not guessed"
        );
        let sala = rows.iter().find(|r| r[0] == "sala").unwrap();
        // 385 × 285 cm = 10.97 m², about 9.7 % over the reference.
        assert!((sala[2].as_f64().unwrap() - 10.97).abs() < 0.01, "{sala}");
        assert!((sala[3].as_f64().unwrap() - 9.7).abs() < 0.1, "{sala}");
        let plain: serde_json::Value =
            serde_json::from_str(&s.check_layout(Parameters(CheckParams::default())).unwrap())
                .unwrap();
        assert!(plain.get("areas").is_none());

        // Overlaying a background renders without touching the project.
        {
            let mut doc = s.document.write();
            doc.execute(Command::SetBackground {
                background: Some(newera_core::BackgroundImage {
                    path: "missing.png".into(),
                    size_px: [100, 100],
                    cm_per_px: 4.0,
                    offset: Point2::new(0.0, 0.0),
                    opacity: 0.2,
                    visible: false,
                    ..Default::default()
                }),
            })
            .unwrap();
        }
        let result = s
            .render_plan(Parameters(RenderParams {
                room: None,
                pad: None,
                w: Some(96),
                h: Some(72),
                region: None,
                grid: None,
                bg: Some(0.6),
            }))
            .unwrap();
        assert!(matches!(&result.content[0], ContentBlock::Image(_)));
        let doc = s.document.read();
        let bg = doc.home().background.as_ref().unwrap();
        assert!(!bg.visible && (bg.opacity - 0.2).abs() < 1e-9);
    }
    #[test]
    fn ergonomics_reviews_the_plan_for_its_people() {
        let s = server();
        let params: CreateParams = serde_json::from_str(
            r#"{"walls":[{"pts":[[0,0],[400,0],[400,300],[0,300]],"closed":true}],"rooms":[{"name":"Quarto","at":[200,150]}]}"#,
        )
        .unwrap();
        s.create(Parameters(params)).unwrap();
        let place: PlaceParams = serde_json::from_str(
            r#"{"items":[{"cat":"bed-double","wall":"w1","along":90},{"cat":"door","wall":"w3","along":200,"w":70}]}"#,
        )
        .unwrap();
        s.place(Parameters(place)).unwrap();
        let review = |json: &str| -> serde_json::Value {
            serde_json::from_str(
                &s.ergonomics(Parameters(serde_json::from_str(json).expect("params"))),
            )
            .expect("json")
        };
        let report = review(r#"{"occupants":3,"wheelchair":true}"#);
        let text = report["findings"].to_string();
        assert_eq!(report["capacity"]["beds"], 2, "{report}");
        assert!(text.contains("falta 1"), "{text}");
        assert!(
            text.contains("80 cm livres") && text.contains("nbr9050"),
            "{text}"
        );
        assert_eq!(
            report["sources"]["nbr9050"][1], "A",
            "the ladder travels with the citation: {report}"
        );
        // The bed is 90 cm from the left wall's axis: 7,5 cm wall, 79 cm half bed → 3,5 cm.
        assert!(text.contains("transferência da cadeira"), "{text}");
        let score = report["score"].as_u64().unwrap();
        assert!(score < 80, "{report}");

        // Every finding says what it costs, and the costs add up to the score.
        let findings = report["findings"].as_array().unwrap();
        assert!(
            findings.iter().all(|f| f["key"].is_string()),
            "each finding is named so it can be accepted: {report}"
        );
        let worst = findings
            .iter()
            .max_by_key(|f| f["weight"].as_u64().unwrap_or(0))
            .cloned()
            .unwrap();
        assert!(worst["weight"].as_u64().unwrap() > 0, "{report}");

        // Accept it with a reason: it stays in the report, explained, and
        // stops costing score — which is how a plan that is right can reach
        // zero pendencies without anything being swept away.
        let key = worst["key"].as_str().unwrap().to_owned();
        let after = review(&format!(
            r#"{{"occupants":3,"wheelchair":true,"accept":[["{key}","varanda envidraçada dá a luz"]]}}"#
        ));
        let same = after["findings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["key"] == key.as_str())
            .unwrap_or_else(|| panic!("{after}"));
        assert_eq!(same["accepted"], "varanda envidraçada dá a luz", "{after}");
        assert_eq!(same["weight"], 0, "{after}");
        assert!(
            after["score"].as_u64().unwrap() > score,
            "the score moves by what was accepted: {after}"
        );

        // And it is remembered: the next review does not accuse it again.
        let again = review(r#"{"occupants":3,"wheelchair":true}"#);
        assert_eq!(again["score"], after["score"], "{again}");
    }
}
