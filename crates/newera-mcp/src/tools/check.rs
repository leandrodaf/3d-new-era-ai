//! Is this layout wrong, and does it work for the people living in it.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ErrorData, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use super::NewEraMcp;
use super::reply::invalid;
use crate::compact;

/// Who lives there, as far as this call says; the rest is the project's.
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct People {
    /// Disciplines included in the score. Architecture always counts; excluded findings remain visible. Kept for later reviews and dry runs.
    scope: Option<newera_ergonomics::ReviewScope>,
    /// People living in the home (default 2).
    occupants: Option<u32>,
    /// Of them, children (sleep in single beds or cribs).
    children: Option<u32>,
    /// Of them, elderly people.
    elderly: Option<u32>,
    /// Someone uses a wheelchair: NBR 9050 turning space, doors and reach.
    wheelchair: Option<bool>,
    /// Height of the main cook, cm, to size the countertop (default 165).
    stature: Option<f64>,
    /// City whose building code applies for this call only, e.g.
    /// `sao-paulo`; `set_home(city=…)` keeps it with the project.
    city: Option<String>,
}

impl People {
    fn given(&self) -> bool {
        self.scope.is_some()
            || self.occupants.is_some()
            || self.children.is_some()
            || self.elderly.is_some()
            || self.wheelchair.is_some()
            || self.stature.is_some()
    }

    /// The project's people with what this call says on top.
    fn over(&self, home: &newera_core::Home) -> newera_ergonomics::Profile {
        let kept = newera_ergonomics::Profile::of(home);
        newera_ergonomics::Profile {
            scope: self.scope.unwrap_or(kept.scope),
            occupants: self.occupants.unwrap_or(kept.occupants),
            children: self.children.unwrap_or(kept.children),
            elderly: self.elderly.unwrap_or(kept.elderly),
            wheelchair: self.wheelchair.unwrap_or(kept.wheelchair),
            stature: self.stature.or(kept.stature),
            city: self.city.clone(),
        }
    }
}

/// Who lives there, plus what has already been looked at.
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct ErgonomicsParams {
    #[serde(flatten)]
    pub(crate) people: People,
    /// Findings already analysed: `[[key, reason]]`. They keep showing, with
    /// the reason, and stop costing score. An empty reason takes it back.
    #[serde(default)]
    pub(crate) accept: Vec<Vec<String>>,
    /// Drop the acceptances listed in `orphaned`, in one undoable step.
    #[serde(default)]
    pub(crate) prune: bool,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct CheckParams {
    /// Expected room areas in m² by room name or id, e.g. {"Sala": 10.91};
    /// adds rows [room, expected, actual, diff %].
    pub(crate) areas: Option<std::collections::BTreeMap<String, f64>>,
    /// Storey to check: an id like `lv3`, or `all` for every storey that is
    /// not a reference layer. Default: the storey being shown.
    pub(crate) level: Option<String>,
    /// Pairs already looked at: `[[key, reason]]`, key as the report names
    /// it (`overlap:f817+f830`, or just `f817+f830`). An empty reason takes
    /// it back.
    #[serde(default)]
    pub(crate) accept: Vec<Vec<String>>,
    /// Drop the acceptances listed in `orphaned`, in one undoable step.
    #[serde(default)]
    pub(crate) prune: bool,
}
/// A server on an empty document, for the domain modules' tests.
fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}
#[tool_router(router = check_router, vis = "pub(crate)")]
impl NewEraMcp {
    #[tool(
        description = "Ergonomics and habitability review for the people living there. It never blocks anything: it reads the drawing and says what it finds, and the drawing stays the user's — somebody sketching to learn or to see an idea is not stopped by a standard. (occupants, children, elderly, wheelchair, stature cm, city; the people given are kept with the project and used when a later call or a dry run gives none, so a dry run's score is the one this review gives): room to walk beside beds and in front of kitchen equipment, beds/seats/bathrooms/wardrobes per person, kitchen (work triangle, counter heights, Alexander's counter lengths, five work zones, sockets, gas ventilation, extraction), doors, ceiling heights, windows, minimum furniture, wheelchair turning. Reply {score, score_basis, scope, scores:{architecture,electrical,plumbing}, layout, coverage, capacity, findings:[{sev, place, msg, key, weight, discipline, in_scope, src?, fix?, accepted?}], sources:{src:[title, tier, url]}}. scope={electrical:false,plumbing:false} scores architecture only; all findings remain visible, excluded ones weigh zero. The scope is saved for later reviews and dry runs. layout includes geometric checks on the active storey; coverage declares limits. Scores are heuristic, not project completion or certification. weight is what the score would gain if that one went away, so a score that moved can be read; key names the finding for accept. src is the source the finding stands on, empty when it is common practice; resolve it in sources instead of asking. tier is the reliability ladder A obliges (Brazilian standard, municipal code) · B references (foreign standard) · C doctrine · D measured · E survey — and it is why a finding is an error or only a tip. fix, when present, is a checked change as tool arguments (move or update): apply one, then review again (fixes of one review may overlap). city, e.g. `sao-paulo`, lets the municipal code judge instead of only advising; against a standard the more restrictive one wins — set it once with set_home(city=…) so dry runs and check_layout weigh the same rules. accept=[[key, reason]] marks findings already looked at: they stay in the report with their reason and stop costing score, which is what lets a correct plan reach zero pendencies honestly; accept=[[key, empty]] takes it back. orphaned [[key, reason]] lists acceptances no current finding answers to, on any storey, for these people — the problem was fixed, and would come back already silenced; prune=true drops them."
    )]
    pub(crate) fn ergonomics(&self, Parameters(p): Parameters<ErgonomicsParams>) -> String {
        let profile = p.people.over(self.document.read().home());
        if p.people.given() {
            // The people a review is conducted for become the project's, so
            // the next dry run scores for them too.
            let mut doc = self.document.write();
            let kept = newera_ergonomics::Profile {
                city: None,
                ..profile.clone()
            };
            if newera_ergonomics::Profile::of(doc.home()) != kept {
                let mut properties = doc.home().properties.clone();
                properties.insert(
                    newera_ergonomics::PEOPLE.to_owned(),
                    serde_json::to_string(&kept).unwrap_or_default(),
                );
                let _ = doc.execute(newera_core::Command::SetProperties { properties });
            }
        }
        if !p.accept.is_empty() || p.prune {
            let mut doc = self.document.write();
            let mut accepted = doc.home().accepted.clone();
            for pair in &p.accept {
                let (key, reason) = (pair.first().cloned().unwrap_or_default(), pair.get(1));
                match reason.map(String::as_str) {
                    None | Some("") => accepted.remove(&key),
                    Some(why) => accepted.insert(key, why.to_owned()),
                };
            }
            if p.prune {
                for (key, _) in newera_ergonomics::orphaned(doc.home(), &profile) {
                    accepted.remove(&key);
                }
            }
            if accepted != doc.home().accepted {
                let _ = doc.execute(newera_core::Command::SetAccepted { accepted });
            }
        }
        let doc = self.document.read();
        let report = newera_ergonomics::review(doc.home(), &profile);
        let live: Vec<String> = report.findings.iter().map(|f| f.key.clone()).collect();
        let orphaned = super::orphan_rows(newera_ergonomics::orphaned(doc.home(), &profile), &live);
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
                    "discipline": f.discipline(),
                    "in_scope": report.scope.includes(f),
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
        let mut out = serde_json::json!({
            "score": report.score,
            "score_basis": "habitability_heuristic",
            "scope": report.scope,
            "scores": report.scores,
            "layout": compact::issues(&doc.home().level_view(doc.home().current_level()), newera_core::Storeys::Active),
            "coverage": {
                "storey": doc.home().current_level().map(|id| id.to_string()),
                "checked": ["layout_geometry", "use_and_circulation", "opening_obstructions", "habitability", "modeled_electrical_rules", "modeled_plumbing_rules"],
                "not_verified": ["photometric_simulation", "visual_composition", "manufacturer_clearances", "construction_documents", "complete_regulatory_compliance"],
                "excluded_findings": "visible_without_score_penalty",
                "electrical_precondition": "furnished_storey",
                "completion": "not_certified"
            },
            "capacity": report.capacity,
            "findings": findings,
            "sources": super::sources(&codes),
        });
        if !orphaned.is_empty() {
            out["orphaned"] = serde_json::json!(orphaned);
        }
        out.to_string()
    }
    #[tool(
        description = "Layout problems: above_ceiling, overlap, blocked, in_wall, blocks_door, blocks_window, no_door, unrated_light, turned, unclear_front, loose_opening, outgrew_niche, outside_rooms, loose (a fixed point with nothing to be fixed to: loose in a room, on glass, in a door or window span, hanging under the ceiling, with why); {} means none. Each one carries name, bounds and z of both elements. Overlaps are classified kind collision (a real clash, listed first), nesting (built in, resting on, tucked under), served (a project point inside a piece on purpose: the water point in the basin, the outlet behind the fridge or set into a cabinet) or cross_level, with extent [x,y,z] cm of the shared space; overlap_kinds counts them. blocks_window reports nearby tall/elevated solids masking the window, with extent [width,height] cm; compact countertop objects are exempt, so this does not certify sash operation or ventilation. blocks_door includes a 60 cm approach on either face, even for sliding doors and passages. blocked is a cabinet, fridge or wardrobe whose opening face is against a solid — it cannot be used, and `angle` alone does not show it. turned is a group whose built fronts (doors, drawer fronts, kick) face one way and whose `angle` says another: the piece opens where the panels are, so fix the angle, not the clearance it seems to lack. unrated_light is a light fixture with neither lumens nor watts — only the relative power an import carries — so the lighting tool's lux for its room are a guess: set its output with update(light={lm or w}). no_door is a bedroom or bathroom (by name) with no door — only open passages, listed, or no way in at all — said once the storey has doors somewhere; a living room, kitchen or balcony left open is not. unclear_front is a group whose parts name fronts on more than one face with no clear winner (candidates, strongest first; placed is the angle's guess every \"in front of\" falls back to) — rename the misleading part or set the angle. Handles weigh most. loose_opening is a door or window in no wall — a passage drawn as a panel — which reads as an opening in every schedule and opens nothing. outgrew_niche is an appliance its host stopped holding after the joinery was resized around it, with how far it sticks out: built-in pieces are left out of the overlap check by design, which is why nothing else notices. above_ceiling reports luminaires above a flat room ceiling, with ceiling/top/over in cm; sloped or hidden ceilings are not inferred. Every row is an object with the `key` it is accepted by (in_wall {key, piece, wall}, blocks_door {key, door, by}). accept=[[key, reason]] marks one looked at and right as drawn — an imported model whose box is bigger than the piece it draws: it leaves the sections, the variant count and every dry run, and is listed under accepted {key, kind, why, extent} with its reason, kept in the project; accept=[[key, \"\"]] takes it back; orphaned [[key, reason]] lists acceptances whose finding is gone on every storey, and prune=true drops them. level: a storey id or `all`, default the one shown. areas {name|id: m²} compares room areas with the reference drawing."
    )]
    pub(crate) fn check_layout(
        &self,
        Parameters(p): Parameters<CheckParams>,
    ) -> Result<String, ErrorData> {
        if !p.accept.is_empty() || p.prune {
            let mut doc = self.document.write();
            let mut accepted = doc.home().accepted.clone();
            if p.prune {
                for (key, _) in newera_core::Issue::orphaned(doc.home()) {
                    accepted.remove(&key);
                }
            }
            for pair in &p.accept {
                let key =
                    newera_core::Issue::normalize_key(pair.first().map_or("", String::as_str));
                match pair.get(1).map(|why| why.trim()) {
                    None | Some("") => accepted.remove(&key),
                    Some(why) => accepted.insert(key, why.to_owned()),
                };
            }
            if accepted != doc.home().accepted {
                doc.execute(newera_core::Command::SetAccepted { accepted })
                    .map_err(super::reply::core)?;
            }
        }
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
        let orphaned: Vec<[String; 2]> = newera_core::Issue::orphaned(doc.home())
            .into_iter()
            .map(|(key, why)| [key, why])
            .collect();
        if !orphaned.is_empty() {
            report["orphaned"] = serde_json::json!(orphaned);
        }
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
                accept: Vec::new(),
                prune: false,
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
    fn a_dry_run_scores_for_the_people_the_review_is_for() {
        let s = server();
        s.create(Parameters(
            serde_json::from_str(
                r#"{"walls":[{"pts":[[0,0],[400,0],[400,300],[0,300]],"closed":true}],"rooms":[{"name":"Quarto","at":[200,150]}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        s.place(Parameters(
            serde_json::from_str(r#"{"items":[{"cat":"bed-double","wall":"w1","along":90}]}"#)
                .unwrap(),
        ))
        .unwrap();
        let review = |json: &str| -> serde_json::Value {
            serde_json::from_str(&s.ergonomics(Parameters(serde_json::from_str(json).unwrap())))
                .unwrap()
        };
        let for_two = review("{}")["score"].as_u64().unwrap();
        let for_four = review(r#"{"occupants":4,"children":1}"#)["score"]
            .as_u64()
            .unwrap();
        assert_ne!(for_two, for_four, "four people need more beds");
        assert_eq!(
            review("{}")["score"].as_u64().unwrap(),
            for_four,
            "the people stay with the project"
        );

        // A dry run that moves the score starts from the review's number.
        let bed = s.document.read().home().furniture[0].id.to_string();
        let dry: serde_json::Value = serde_json::from_str(
            &s.move_elements(Parameters(
                serde_json::from_str(&format!(
                    r#"{{"ids":["{bed}"],"dx":100,"dy":0,"dry":true}}"#
                ))
                .unwrap(),
            ))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(dry["score"][0].as_u64(), Some(for_four), "{dry}");

        // And a call can still ask about someone else, which then sticks.
        assert_eq!(
            review(r#"{"occupants":2,"children":0}"#)["score"].as_u64(),
            Some(for_two)
        );
    }

    #[test]
    fn an_acceptance_outliving_its_finding_is_listed_and_can_be_pruned() {
        let s = server();
        s.create(Parameters(
            serde_json::from_str(
                r#"{"walls":[{"pts":[[0,0],[400,0],[400,300],[0,300]],"closed":true}],"rooms":[{"name":"Quarto","at":[200,150]}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        s.place(Parameters(
            serde_json::from_str(
                r#"{"items":[{"cat":"bed-double","wall":"w1","along":90},
                             {"cat":"box","name":"a","at":[300,150],"w":60,"d":60,"h":90},
                             {"cat":"box","name":"b","at":[340,150],"w":60,"d":60,"h":90}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        let (bed, a, b) = {
            let doc = s.document.read();
            let f = &doc.home().furniture;
            (
                f[0].id.to_string(),
                f[1].id.to_string(),
                f[2].id.to_string(),
            )
        };
        let review = |json: &str| -> serde_json::Value {
            serde_json::from_str(&s.ergonomics(Parameters(serde_json::from_str(json).unwrap())))
                .unwrap()
        };
        let check = |json: &str| -> serde_json::Value {
            serde_json::from_str(
                &s.check_layout(Parameters(serde_json::from_str(json).unwrap()))
                    .unwrap(),
            )
            .unwrap()
        };
        // A walk beside the bed, accepted with the measure as the reason.
        let first = review("{}");
        let key = first["findings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["place"].as_str().unwrap().contains(bed.as_str()))
            .unwrap_or_else(|| panic!("{first}"))["key"]
            .as_str()
            .unwrap()
            .to_owned();
        let accepted = review(&format!(r#"{{"accept":[["{key}","corredor de 3 cm"]]}}"#));
        assert!(accepted.get("orphaned").is_none(), "{accepted}");
        check(&format!(r#"{{"accept":[["{a}+{b}","caixa do modelo"]]}}"#));

        let revision = s.document.read().revision();
        let dry: serde_json::Value = serde_json::from_str(
            &s.move_elements(Parameters(
                serde_json::from_str(&format!(
                    r#"{{"ids":["{b}"],"dx":40,"dy":0,"dry":"summary"}}"#
                ))
                .unwrap(),
            ))
            .unwrap(),
        )
        .unwrap();
        let cleanup = &dry["acceptance_cleanup"];
        assert_eq!(cleanup["based_on_revision"], revision);
        assert_eq!(cleanup["automatic"], false);
        let call = cleanup["calls"]
            .as_array()
            .unwrap()
            .iter()
            .find(|call| call["tool"] == "check_layout")
            .unwrap();
        assert_eq!(
            call["orphaned"],
            serde_json::json!([[format!("overlap:{a}+{b}"), "caixa do modelo"]])
        );
        assert_eq!(s.document.read().revision(), revision);
        assert!(
            check("{}").get("orphaned").is_none(),
            "preview must preserve live acceptance"
        );
        let bed_preview: serde_json::Value = serde_json::from_str(
            &s.update(Parameters(
                serde_json::from_str(&format!(
                    r#"{{"items":[{{"id":"{bed}","visible":false}}],"dry":true}}"#
                ))
                .unwrap(),
            ))
            .unwrap(),
        )
        .unwrap();
        let ergonomics_call = bed_preview["acceptance_cleanup"]["calls"]
            .as_array()
            .unwrap()
            .iter()
            .find(|call| call["tool"] == "ergonomics")
            .unwrap();
        assert_eq!(
            ergonomics_call["orphaned"],
            serde_json::json!([[key, "corredor de 3 cm"]])
        );
        assert_eq!(s.document.read().revision(), revision);

        // Both problems are fixed for real: the bed goes, the boxes part.
        s.delete(Parameters(
            serde_json::from_str(&format!(r#"{{"ids":["{bed}"]}}"#)).unwrap(),
        ))
        .unwrap();
        s.move_elements(Parameters(
            serde_json::from_str(&format!(r#"{{"ids":["{b}"],"dx":40,"dy":0}}"#)).unwrap(),
        ))
        .unwrap();

        // The reasons are still in the project, and now they say so.
        let after = review("{}");
        assert_eq!(
            after["orphaned"],
            serde_json::json!([[key, "corredor de 3 cm"]]),
            "{after}"
        );
        let layout = check("{}");
        let pair = format!("overlap:{a}+{b}");
        assert_eq!(
            layout["orphaned"],
            serde_json::json!([[pair, "caixa do modelo"]]),
            "{layout}"
        );

        // The exact proposed call cleans only its listed keys, in one
        // undoable command; restoring history also restores the reason.
        check(&call["arguments"].to_string());
        assert!(check("{}").get("orphaned").is_none());
        assert_eq!(review("{}")["orphaned"], after["orphaned"]);
        s.document.write().undo().unwrap();
        assert_eq!(check("{}")["orphaned"], layout["orphaned"]);

        // Pruned in one step each, and only the ones that are orphans.
        let pruned = review(r#"{"prune":true}"#);
        assert!(pruned.get("orphaned").is_none(), "{pruned}");
        assert!(
            check("{}")["orphaned"].is_array(),
            "ergonomics leaves layout acceptances alone"
        );
        assert!(check(r#"{"prune":true}"#).get("orphaned").is_none());
        assert!(s.document.read().home().accepted.is_empty());
        s.document.write().undo().unwrap();
        assert_eq!(s.document.read().home().accepted.len(), 1, "undoable");
    }

    #[test]
    fn renaming_a_room_changes_the_description_not_the_missing_installation() {
        let s = server();
        {
            let mut doc = s.document.write();
            doc.execute(Command::insert(newera_core::Room::new(
                newera_core::RoomId(1),
                "Banho social",
                vec![
                    newera_core::Point2::new(0.0, 0.0),
                    newera_core::Point2::new(300.0, 0.0),
                    newera_core::Point2::new(300.0, 300.0),
                    newera_core::Point2::new(0.0, 300.0),
                ],
            )))
            .unwrap();
            doc.execute(Command::insert(newera_core::Furniture {
                id: newera_core::FurnitureId(2),
                catalog: "toilet".into(),
                position: newera_core::Point2::new(100.0, 100.0),
                ..newera_core::Furniture::default()
            }))
            .unwrap();
        }
        let dry = s
            .update(Parameters(
                serde_json::from_str(
                    r#"{"items":[{"id":"r1","name":"Banho de hóspedes"}],"dry":true}"#,
                )
                .unwrap(),
            ))
            .unwrap();
        let dry: serde_json::Value = serde_json::from_str(&dry).unwrap();
        for field in ["resolved", "new_findings"] {
            assert!(
                dry[field].as_array().is_none_or(|rows| rows
                    .iter()
                    .all(|r| !r.to_string().contains("tomadas de uso geral"))),
                "{dry}"
            );
        }
        assert!(
            dry["findings_changed"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["key"] == "elec:outlets:r1"),
            "{dry}"
        );
        assert_eq!(s.document.read().home().rooms[0].name, "Banho social");
    }

    #[test]
    fn ceiling_excess_is_measured_and_a_dry_move_reports_its_resolution() {
        let s = server();
        {
            let mut doc = s.document.write();
            let ceiling = doc.home().wall_height;
            doc.execute(Command::insert(newera_core::Room::new(
                newera_core::RoomId(1),
                "Sala",
                vec![
                    newera_core::Point2::new(0.0, 0.0),
                    newera_core::Point2::new(400.0, 0.0),
                    newera_core::Point2::new(400.0, 400.0),
                    newera_core::Point2::new(0.0, 400.0),
                ],
            )))
            .unwrap();
            doc.execute(Command::insert(newera_core::Furniture {
                id: newera_core::FurnitureId(2),
                catalog: "pendant".into(),
                position: newera_core::Point2::new(200.0, 200.0),
                elevation: ceiling - 50.0,
                height: 90.0,
                ..newera_core::Furniture::default()
            }))
            .unwrap();
        }
        let check = || {
            serde_json::from_str::<serde_json::Value>(
                &s.check_layout(Parameters(serde_json::from_str("{}").unwrap()))
                    .unwrap(),
            )
            .unwrap()
        };
        assert_eq!(check()["above_ceiling"][0]["over"], 40);
        let ceiling = s.document.read().home().wall_height;
        let dry = s
            .update(Parameters(
                serde_json::from_value(serde_json::json!({
                    "items":[{"id":"f2","elev":ceiling-90.0}],"dry":true
                }))
                .unwrap(),
            ))
            .unwrap();
        let dry: serde_json::Value = serde_json::from_str(&dry).unwrap();
        assert!(
            dry["issues_resolved"]
                .as_array()
                .unwrap()
                .iter()
                .any(|i| i["kind"] == "above_ceiling")
        );
        assert!(
            check()["above_ceiling"].is_array(),
            "dry run leaves the drawing untouched"
        );
    }

    #[test]
    fn a_fixture_with_no_rated_output_is_reported_until_it_has_one() {
        let s = server();
        {
            let mut doc = s.document.write();
            // Imported from Sweet Home 3D: a relative power, no lumens, no watts.
            let fixture = newera_core::Furniture {
                id: newera_core::FurnitureId(1),
                catalog: "imported".into(),
                name: "Cozinha — geral".into(),
                width: 120.0,
                depth: 8.0,
                height: 3.0,
                elevation: 250.0,
                light: Some(newera_core::Light {
                    power: 0.5,
                    sources: Vec::new(),
                    source_materials: Vec::new(),
                    lumens: None,
                    watts: None,
                    lamp: None,
                    kelvin: None,
                    beam: None,
                    area: None,
                    panel_upward: None,
                }),
                ..newera_core::Furniture::default()
            };
            doc.execute(Command::insert(fixture)).unwrap();
        }
        // A catalog fixture comes rated, on the ceiling of a room.
        s.create(Parameters(
            serde_json::from_str(
                r#"{"walls":[{"pts":[[220,20],[380,20],[380,180],[220,180]],"closed":true}],"rooms":[{"name":"Sala","at":[300,100]}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        s.place(Parameters(
            serde_json::from_str(r#"{"items":[{"cat":"led-panel","at":[300,100]}]}"#).unwrap(),
        ))
        .unwrap();
        let check = || -> serde_json::Value {
            serde_json::from_str(&s.check_layout(Parameters(CheckParams::default())).unwrap())
                .unwrap()
        };
        let report = check();
        let rows = report["unrated_light"]
            .as_array()
            .unwrap_or_else(|| panic!("{report}"));
        assert_eq!(rows.len(), 1, "{report}");
        assert_eq!(rows[0]["id"], "f1", "{report}");
        assert_eq!(rows[0]["key"], "unrated_light:f1", "{report}");

        s.update(Parameters(
            serde_json::from_str(r#"{"items":[{"id":"f1","light":{"lm":2400}}]}"#).unwrap(),
        ))
        .unwrap();
        assert!(check().get("unrated_light").is_none(), "{}", check());
    }

    #[test]
    fn a_bedroom_without_a_door_is_a_layout_problem_a_living_room_is_not() {
        let s = server();
        s.create(Parameters(
            serde_json::from_str(
                r#"{"walls":[{"pts":[[0,0],[600,0],[600,400],[0,400]],"closed":true},{"pts":[[300,0],[300,400]]}],
                    "rooms":[{"name":"Sala","at":[150,200]},{"name":"Dormitório","at":[450,200]}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        // The flat's entrance, into the living room.
        s.place(Parameters(
            serde_json::from_str(r#"{"items":[{"cat":"door","wall":"w4","along":200,"w":90}]}"#)
                .unwrap(),
        ))
        .unwrap();
        let check = || -> serde_json::Value {
            serde_json::from_str(&s.check_layout(Parameters(CheckParams::default())).unwrap())
                .unwrap()
        };
        // Sealed: the bedroom has no way in.
        let sealed = check();
        let row = &sealed["no_door"][0];
        assert_eq!(row["name"], "Dormitório", "{sealed}");
        assert_eq!(row["passages"], serde_json::json!([]), "{sealed}");
        assert_eq!(
            sealed["no_door"].as_array().unwrap().len(),
            1,
            "the living room is fine: {sealed}"
        );

        // Only a passage: still no door, the passage named.
        let reply = s
            .place(Parameters(
                serde_json::from_str(
                    r#"{"items":[{"cat":"passage","wall":"w5","along":200,"w":80}]}"#,
                )
                .unwrap(),
            ))
            .unwrap();
        let passage = reply.rsplit("ids=").next().unwrap().trim().to_owned();
        let open = check();
        assert_eq!(
            open["no_door"][0]["passages"],
            serde_json::json!([passage]),
            "{open}"
        );

        // A real door, and it is gone.
        s.delete(Parameters(
            serde_json::from_str(&format!(r#"{{"ids":["{passage}"]}}"#)).unwrap(),
        ))
        .unwrap();
        s.place(Parameters(
            serde_json::from_str(r#"{"items":[{"cat":"door","wall":"w5","along":200,"w":80}]}"#)
                .unwrap(),
        ))
        .unwrap();
        assert!(check().get("no_door").is_none(), "{}", check());
    }

    #[test]
    fn a_blind_in_its_wall_and_a_shaft_outside_rooms_can_be_accepted() {
        let s = server();
        s.create(Parameters(
            serde_json::from_str(
                r#"{"walls":[{"pts":[[0,0],[400,0],[400,300],[0,300]],"closed":true}],
                    "rooms":[{"name":"Sala","at":[200,150]}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        s.place(Parameters(
            serde_json::from_str(
                r#"{"items":[{"cat":"box","name":"persiana integrada","at":[200,9],"w":120,"d":10,"h":30},
                             {"cat":"box","name":"shaft","at":[600,150],"w":40,"d":40,"h":250}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        let check = |json: &str| -> serde_json::Value {
            serde_json::from_str(
                &s.check_layout(Parameters(serde_json::from_str(json).unwrap()))
                    .unwrap(),
            )
            .unwrap()
        };
        let (blind, shaft) = {
            let doc = s.document.read();
            let f = &doc.home().furniture;
            (f[0].id.to_string(), f[1].id.to_string())
        };
        let report = check("{}");
        // Each row names the key it is accepted by, so nobody invents one.
        let blind_key = report["in_wall"][0]["key"]
            .as_str()
            .unwrap_or_else(|| panic!("{report}"))
            .to_owned();
        let shaft_key = report["outside_rooms"][0]["key"]
            .as_str()
            .unwrap_or_else(|| panic!("{report}"))
            .to_owned();
        let wall = report["in_wall"][0]["wall"]["id"]
            .as_str()
            .unwrap_or_else(|| panic!("{report}"));
        assert_eq!(
            report["in_wall"][0]["piece"]["id"],
            blind.as_str(),
            "{report}"
        );
        assert_eq!(report["outside_rooms"][0]["id"], shaft.as_str(), "{report}");

        let report = check(&format!(
            r#"{{"accept":[["{blind_key}","persiana de rolo embutida na parede"],
                          ["{shaft_key}","shaft fora dos cômodos, correto"]]}}"#
        ));
        assert_eq!(blind_key, format!("in_wall:{blind}+{wall}"));
        assert!(
            report.get("orphaned").is_none(),
            "the keys given are live: {report}"
        );
        assert!(report.get("in_wall").is_none(), "{report}");
        assert!(report.get("outside_rooms").is_none(), "{report}");
        let accepted = report["accepted"].as_array().unwrap();
        assert_eq!(accepted.len(), 2, "{report}");
        assert!(
            accepted.iter().any(
                |a| a["kind"] == "in_wall" && a["why"] == "persiana de rolo embutida na parede"
            ),
            "{report}"
        );
        let doc = s.document.read();
        assert!(
            newera_core::check_layout(doc.home())
                .iter()
                .all(|i| !i.is_pending(doc.home())),
            "nothing left to fix"
        );
    }

    #[test]
    fn a_clash_looked_at_can_be_accepted_with_its_reason() {
        let s = server();
        // A 70 cm sink model whose box is a whole stone, 5.4 cm over the
        // dishwasher in the niche beside it: the drawing is right, the box
        // is not the piece.
        s.place(Parameters(
            serde_json::from_str(
                r#"{"items":[{"cat":"box","name":"cuba","at":[35,30],"w":70,"d":60,"h":92},
                             {"cat":"box","name":"lava-louças","at":[94.5,30],"w":59.8,"d":58,"h":85}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        let (sink, washer) = {
            let doc = s.document.read();
            let f = &doc.home().furniture;
            (f[0].id.to_string(), f[1].id.to_string())
        };
        let check = |json: &str| -> serde_json::Value {
            serde_json::from_str(
                &s.check_layout(Parameters(serde_json::from_str(json).unwrap()))
                    .unwrap(),
            )
            .unwrap()
        };
        let pending = |doc: &newera_core::Document| {
            newera_core::check_layout(doc.home())
                .iter()
                .filter(|i| i.is_pending(doc.home()))
                .count()
        };
        let report = check("{}");
        let clash = &report["overlap"][0];
        assert_eq!(clash["kind"], "collision", "{report}");
        let key = format!("overlap:{sink}+{washer}");
        assert_eq!(clash["key"], key.as_str(), "{report}");
        assert_eq!(pending(&s.document.read()), 1);

        // Accepted by the pair written either way round.
        let report = check(&format!(
            r#"{{"accept":[["{washer}+{sink}","caixa do modelo; a cuba cabe no nicho"]]}}"#
        ));
        assert!(report.get("overlap").is_none(), "{report}");
        assert!(report.get("overlap_kinds").is_none(), "{report}");
        let accepted = &report["accepted"][0];
        assert_eq!(accepted["key"], key.as_str(), "{report}");
        assert_eq!(accepted["kind"], "collision", "{report}");
        assert_eq!(accepted["why"], "caixa do modelo; a cuba cabe no nicho");
        assert_eq!(accepted["extent"][0], 5.4, "{report}");
        assert_eq!(pending(&s.document.read()), 0, "the variant badge agrees");
        assert!(check("{}").get("overlap").is_none(), "it is remembered");

        // A dry run no longer calls it new when the pair is touched.
        let dry: serde_json::Value = serde_json::from_str(
            &s.move_elements(Parameters(
                serde_json::from_str(&format!(
                    r#"{{"ids":["{washer}"],"dx":-1,"dy":0,"dry":true}}"#
                ))
                .unwrap(),
            ))
            .unwrap(),
        )
        .unwrap();
        assert!(dry.get("issues_changed").is_none(), "{dry}");
        assert!(dry.get("issues_new").is_none(), "{dry}");

        // It is one undoable step, and an empty reason takes it back.
        let report = check(&format!(r#"{{"accept":[["{key}",""]]}}"#));
        assert_eq!(report["overlap"][0]["kind"], "collision", "{report}");
        assert!(report.get("accepted").is_none(), "{report}");
        s.document.write().undo().unwrap();
        assert_eq!(check("{}")["accepted"][0]["key"], key.as_str());
    }
    #[test]
    fn review_scope_is_persisted_and_geometry_is_reported_alongside_habitability() {
        let s = server();
        s.create(Parameters(
            serde_json::from_value(serde_json::json!({
                "rooms":[{"name":"Quarto","pts":[[0,0],[400,0],[400,400],[0,400]]}]
            }))
            .unwrap(),
        ))
        .unwrap();
        s.place(Parameters(
            serde_json::from_value(serde_json::json!({"items":[
                {"cat":"wardrobe","at":[200,200]}, {"cat":"fridge","at":[200,200]}
            ]}))
            .unwrap(),
        ))
        .unwrap();
        let report: serde_json::Value = serde_json::from_str(
            &s.ergonomics(Parameters(
                serde_json::from_value(
                    serde_json::json!({"scope":{"electrical":false,"plumbing":false}}),
                )
                .unwrap(),
            )),
        )
        .unwrap();
        assert_eq!(report["score"], report["scores"]["architecture"]);
        assert!(!report["layout"].as_object().unwrap().is_empty());
        assert_eq!(report["coverage"]["completion"], "not_certified");
        assert!(report["findings"].as_array().unwrap().iter().any(|f| {
            f["msg"]
                .as_str()
                .is_some_and(|msg| msg.contains("Sem janela"))
        }));
        assert!(
            report["findings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["discipline"] == "electrical"
                    && f["in_scope"] == false
                    && f["weight"] == 0)
        );
        let kept: serde_json::Value =
            serde_json::from_str(&s.ergonomics(Parameters(ErgonomicsParams::default()))).unwrap();
        assert_eq!(kept["scope"], report["scope"]);
        let dry: serde_json::Value = serde_json::from_str(
            &s.update(Parameters(
                serde_json::from_value(
                    serde_json::json!({"dry":true,"items":[{"id":"f2","name":"Armário"}]}),
                )
                .unwrap(),
            ))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(dry["score_scope"], report["scope"]);
        assert!(s.document.read().home().accepted.is_empty());
        assert!(
            !newera_ergonomics::Profile::of(s.document.read().home())
                .scope
                .electrical
        );
    }

    #[test]
    fn explicit_room_use_is_created_read_updated_and_undone() {
        let s = server();
        s.create(Parameters(
            serde_json::from_value(serde_json::json!({"rooms":[
                {"name":"Copa","room_use":"bathroom","pts":[[0,0],[400,0],[400,400],[0,400]]}
            ]}))
            .unwrap(),
        ))
        .unwrap();
        {
            let doc = s.document.read();
            let room = &doc.home().rooms[0];
            assert_eq!(room.usage, newera_core::RoomUse::Bathroom);
            assert_eq!(crate::compact::room(room)["room_use"], "bathroom");
        }
        s.update(Parameters(
            serde_json::from_value(serde_json::json!({"items":[{"id":"r1","name":"Azul"}]}))
                .unwrap(),
        ))
        .unwrap();
        assert_eq!(
            s.document.read().home().rooms[0].semantic_name(),
            "banheiro"
        );
        s.update(Parameters(
            serde_json::from_value(serde_json::json!({"items":[{"id":"r1","room_use":"auto"}]}))
                .unwrap(),
        ))
        .unwrap();
        assert_eq!(s.document.read().home().rooms[0].semantic_name(), "Azul");
        s.document.write().undo().unwrap();
        assert_eq!(
            s.document.read().home().rooms[0].usage,
            newera_core::RoomUse::Bathroom
        );
        assert!(
            serde_json::from_value::<crate::edit::UpdateSpec>(
                serde_json::json!({"id":"r1","room_use":"not_a_room_type"})
            )
            .is_err()
        );
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
