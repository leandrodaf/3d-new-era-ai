//! The electrical and telecom project: circuits, the load schedule, and
//! what NBR 5410 asks of each room.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ErrorData, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use super::NewEraMcp;
use super::reply::{applied, core, invalid};
use newera_core::electrical;

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct ElectricalParams {
    /// `check` (default): points by kind and what the norm finds;
    /// `circuits`: the load schedule; `assign`: put `ids` on `circuit`
    /// (empty takes them off), with `va` to set their power; `voltage`:
    /// the supply, 127 or 220 V.
    action: Option<String>,
    /// Point ids for `assign`.
    #[serde(default)]
    ids: Vec<String>,
    /// Circuit name, e.g. `C3`.
    circuit: Option<String>,
    /// For `assign`: the whole division at once, `{"C1": [ids], "C2": [ids]}`,
    /// in one undoable step.
    circuits: Option<std::collections::BTreeMap<String, Vec<String>>>,
    /// Power per point, VA, instead of the norm's default.
    va: Option<f64>,
    /// Supply voltage, V.
    volts: Option<f64>,
    /// Findings looked at: `[[key, reason]]`; they stay listed with the reason
    /// and stop counting as pending. An empty reason takes one back.
    #[serde(default)]
    accept: Vec<Vec<String>>,
    /// Drop the acceptances listed in `orphaned`.
    #[serde(default)]
    prune: bool,
    /// For `cable`: what the run carries, `power`, `data` or `tv`.
    kind: Option<String>,
    /// For `cable`: the run's points `[[x,y], …]`, cm.
    #[serde(default)]
    pts: Vec<newera_core::Point2>,
}

#[tool_router(router = electrical_router, vis = "pub(crate)")]
impl NewEraMcp {
    #[tool(
        description = "Electrical and telecom project, NBR 5410 and NBR 14565. Points are the electrical pieces (catalog electrical: outlets, switches, lighting points, panel, network-outlet RJ45, tv-outlet, wifi-point, telecom-panel) plus every fixture that lights. check (default): {points:{kind:count}, findings:[[sev, place, msg, src, key, accepted?]], pending, orphaned, sources} — accept=[[key, reason]] with any action marks findings looked at (they stay listed with the reason and stop counting in pending), an empty reason takes one back, orphaned lists acceptances whose finding is gone and prune=true drops them — a ceiling lighting point per room, general-use outlets per room (kitchens and laundries one per 3.5 m of perimeter, bathrooms one by the basin, living rooms and bedrooms one per 5 m), a network point in long-stay rooms, a TV point in living rooms and bedrooms, a distribution and a telecom panel, points without a circuit, lighting and outlets sharing a circuit, a dedicated load not alone. circuits: rows [name, kinds, points, VA, V, A, wire mm², breaker A, DR] — power by the norm's defaults (100 VA per lighting point; 600 VA for each of the first three outlets of a kitchen, laundry or bathroom, 100 VA after and elsewhere; shower 5500, air conditioning 1500) unless set; wire the larger of what the current needs and 1.5 mm² for lighting or 2.5 for power; DR where a circuit serves a wet room or a balcony; a shower runs on 220 V. assign {ids, circuit, va?} — or the whole division at once, circuits {\"C1\": [ids], \"C2\": [ids]} — writes the circuits (and power) on points in one undoable step. voltage {volts}. cable {kind: power|data|tv, pts}: draws a run of the electrical project, told apart on the plan (power solid, network dashed, TV dash-dot); check then reports cables_m, the length by kind with a tenth for the drops, and network or TV points no run reaches, or a telecom panel none reaches. Circuit numbers are drawn next to the points on the plan, and with annotations(legend=true) the load schedule under the legend."
    )]
    pub(crate) fn electrical(
        &self,
        Parameters(p): Parameters<ElectricalParams>,
    ) -> Result<String, ErrorData> {
        if !p.accept.is_empty() || p.prune {
            let mut doc = self.document.write();
            let mut accepted = doc.home().accepted.clone();
            if p.prune {
                for (key, _) in electrical::orphaned(doc.home()) {
                    accepted.remove(&key);
                }
            }
            for pair in &p.accept {
                let key = pair
                    .first()
                    .map(|k| k.trim().to_owned())
                    .unwrap_or_default();
                if !key.starts_with("elec:") {
                    return Err(invalid(format!(
                        "accept: electrical keys start with elec: (not {key})"
                    )));
                }
                match pair.get(1).map(|w| w.trim()) {
                    None | Some("") => accepted.remove(&key),
                    Some(why) => accepted.insert(key, why.to_owned()),
                };
            }
            if accepted != doc.home().accepted {
                doc.execute(newera_core::Command::SetAccepted { accepted })
                    .map_err(core)?;
            }
        }
        match p.action.as_deref().unwrap_or("check") {
            "check" => {
                let doc = self.document.read();
                let home = doc.home();
                let mut kinds: std::collections::BTreeMap<&str, usize> =
                    std::collections::BTreeMap::new();
                for point in electrical::points(home) {
                    *kinds.entry(point.kind.name()).or_default() += 1;
                }
                let findings = electrical::check(home);
                let mut codes: Vec<&str> = findings.iter().map(|f| f.source).collect();
                codes.sort_unstable();
                codes.dedup();
                let rows: Vec<serde_json::Value> = findings
                    .iter()
                    .map(|f| {
                        let mut row =
                            serde_json::json!([f.severity, f.place, f.message, f.source, f.key]);
                        if let (Some(why), Some(cells)) = (&f.accepted, row.as_array_mut()) {
                            cells.push(serde_json::json!(why));
                        }
                        row
                    })
                    .collect();
                let pending = findings.iter().filter(|f| f.accepted.is_none()).count();
                let orphaned: Vec<[String; 2]> = electrical::orphaned(home)
                    .into_iter()
                    .map(|(k, why)| [k, why])
                    .collect();
                let cables: serde_json::Map<String, serde_json::Value> =
                    electrical::cable_lengths(home)
                        .into_iter()
                        .map(|(cable, m)| (cable.key().to_owned(), serde_json::json!(m)))
                        .collect();
                Ok(serde_json::json!({
                    "points": kinds,
                    "cables_m": cables,
                    "findings": rows,
                    "pending": pending,
                    "orphaned": orphaned,
                    "sources": super::sources(&codes),
                })
                .to_string())
            }
            "circuits" => {
                let doc = self.document.read();
                let rows: Vec<serde_json::Value> = electrical::circuits(doc.home())
                    .iter()
                    .map(|c| {
                        serde_json::json!([
                            c.name,
                            c.kinds.iter().map(|k| k.name()).collect::<Vec<_>>(),
                            c.points.len(),
                            c.va,
                            c.volts,
                            c.amps,
                            c.wire_mm2,
                            c.breaker_a,
                            c.rcd,
                        ])
                    })
                    .collect();
                let total: f64 = electrical::circuits(doc.home()).iter().map(|c| c.va).sum();
                Ok(serde_json::json!({"circuits": rows, "total_va": total}).to_string())
            }
            "assign" => {
                // One circuit and its ids, or the whole division as a map.
                let mut plan: Vec<(Option<String>, Vec<String>)> = Vec::new();
                if let Some(map) = &p.circuits {
                    plan.extend(
                        map.iter()
                            .map(|(c, ids)| (Some(c.trim().to_owned()), ids.clone())),
                    );
                }
                if !p.ids.is_empty() {
                    plan.push((
                        p.circuit.as_deref().map(|c| c.trim().to_owned()),
                        p.ids.clone(),
                    ));
                }
                if plan.is_empty() {
                    return Err(invalid(
                        "assign needs ids with circuit (empty to clear) or va, or circuits {C1: [ids]}",
                    ));
                }
                if p.circuits.is_none() && p.circuit.is_none() && p.va.is_none() {
                    return Err(invalid("assign needs circuit (empty to clear) or va"));
                }
                let mut doc = self.document.write();
                let before = doc.home().clone();
                let mut pieces: std::collections::BTreeMap<
                    newera_core::FurnitureId,
                    newera_core::Furniture,
                > = std::collections::BTreeMap::new();
                for (circuit, ids) in plan {
                    for raw in ids {
                        let id: newera_core::FurnitureId =
                            raw.parse().map_err(|e| invalid(format!("{raw}: {e}")))?;
                        let piece = match pieces.remove(&id) {
                            Some(piece) => piece,
                            None => doc.home().piece(id).cloned().ok_or_else(|| {
                                invalid(format!("{raw} not found (a point of the plan)"))
                            })?,
                        };
                        let mut piece = piece;
                        match circuit.as_deref() {
                            Some("") => {
                                piece.properties.remove(electrical::CIRCUIT_KEY);
                            }
                            Some(name) => {
                                piece
                                    .properties
                                    .insert(electrical::CIRCUIT_KEY.into(), name.to_owned());
                            }
                            None => {}
                        }
                        if let Some(va) = p.va {
                            piece
                                .properties
                                .insert(electrical::VA_KEY.into(), va.to_string());
                        }
                        pieces.insert(id, piece);
                    }
                }
                let commands = pieces
                    .into_values()
                    .map(newera_core::Command::update)
                    .collect();
                doc.execute(newera_core::Command::Batch { commands })
                    .map_err(core)?;
                Ok(applied(&doc, &before))
            }
            "cable" => {
                let cable = p
                    .kind
                    .as_deref()
                    .and_then(electrical::Cable::parse)
                    .ok_or_else(|| invalid("cable kind: power, data or tv"))?;
                if p.pts.len() < 2 {
                    return Err(invalid("cable pts: at least two points"));
                }
                let mut doc = self.document.write();
                let before = doc.home().clone();
                let mut line = newera_core::Polyline::new(doc.new_polyline_id(), p.pts.clone());
                let (dash, color) = cable.style();
                line.dash = dash;
                line.color = color;
                line.thickness = 1.5;
                line.discipline = Some(newera_core::Discipline::Electrical);
                line.properties
                    .insert(electrical::CABLE_KEY.into(), cable.key().into());
                doc.execute(newera_core::Command::insert(line))
                    .map_err(core)?;
                Ok(applied(&doc, &before))
            }
            "voltage" => {
                let volts = p
                    .volts
                    .ok_or_else(|| invalid("voltage needs volts: 127 or 220"))?;
                if !(100.0..=400.0).contains(&volts) {
                    return Err(invalid("volts: a supply voltage, 127 or 220"));
                }
                let mut doc = self.document.write();
                let before = doc.home().clone();
                let mut properties = doc.home().properties.clone();
                properties.insert(electrical::VOLTAGE_KEY.into(), volts.to_string());
                doc.execute(newera_core::Command::SetProperties { properties })
                    .map_err(core)?;
                Ok(applied(&doc, &before))
            }
            other => Err(invalid(format!(
                "action: check, circuits, assign or voltage (not {other})"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::server;

    #[test]
    fn a_small_flat_is_wired_checked_and_scheduled() {
        let s = server();
        s.create(Parameters(
            serde_json::from_str(
                r#"{"walls":[{"pts":[[0,0],[600,0],[600,400],[0,400]],"closed":true},{"pts":[[300,0],[300,400]]}],
                    "rooms":[{"name":"Cozinha","at":[150,200]},{"name":"Quarto","at":[450,200]}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        s.disciplines(Parameters(
            serde_json::from_str(r#"{"action":"select","d":"electrical"}"#).unwrap(),
        ))
        .unwrap();
        let reply = s
            .place(Parameters(
                serde_json::from_str(
                    r#"{"items":[{"cat":"electrical-panel","at":[20,20]},
                                 {"cat":"light-ceiling","at":[150,200]},{"cat":"light-ceiling","at":[450,200]},
                                 {"cat":"outlet-mid","at":[100,10]},{"cat":"outlet-mid","at":[200,10]},
                                 {"cat":"outlet-low","at":[310,100]},{"cat":"network-outlet","at":[590,200]}]}"#,
                )
                .unwrap(),
            ))
            .unwrap();
        let ids: Vec<String> = reply
            .rsplit("ids=")
            .next()
            .unwrap()
            .split(',')
            .map(|s| s.trim().to_owned())
            .collect();
        let electrical = |json: &str| -> serde_json::Value {
            serde_json::from_str(
                &s.electrical(Parameters(serde_json::from_str(json).unwrap()))
                    .unwrap(),
            )
            .unwrap()
        };
        let check = electrical("{}");
        assert_eq!(check["points"]["Rede"], 1, "{check}");
        let text = check["findings"].to_string();
        assert!(text.contains("pontos sem circuito"), "{check}");
        assert!(text.contains("quadro de telecom"), "{check}");
        assert!(check["sources"]["nbr5410"].is_array(), "{check}");

        let assign = |ids: &[&String], circuit: &str| {
            s.electrical(Parameters(
                serde_json::from_str(&format!(
                    r#"{{"action":"assign","ids":{},"circuit":"{circuit}"}}"#,
                    serde_json::json!(ids)
                ))
                .unwrap(),
            ))
            .unwrap()
        };
        assign(&[&ids[1], &ids[2]], "C1");
        assign(&[&ids[3], &ids[4]], "C2");
        assign(&[&ids[5]], "C3");
        let schedule = electrical(r#"{"action":"circuits"}"#);
        let c2 = schedule["circuits"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c[0] == "C2")
            .unwrap()
            .clone();
        assert_eq!(c2[3], 1200.0, "{schedule}");
        assert_eq!(c2[6], 2.5, "{schedule}");
        assert_eq!(c2[8], true, "kitchen outlets need a DR: {schedule}");
        assert!(
            !electrical("{}")["findings"]
                .to_string()
                .contains("pontos sem circuito")
        );

        // The whole division in one call, one undo step.
        s.electrical(Parameters(
            serde_json::from_str(&format!(
                r#"{{"action":"assign","circuits":{{"L1":["{}","{}"],"T1":["{}","{}"],"T2":["{}"]}}}}"#,
                ids[1], ids[2], ids[3], ids[4], ids[5]
            ))
            .unwrap(),
        ))
        .unwrap();
        let names: Vec<String> = electrical(r#"{"action":"circuits"}"#)["circuits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c[0].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(names, ["L1", "T1", "T2"]);
        s.document.write().undo().unwrap();
        assert_eq!(
            electrical(r#"{"action":"circuits"}"#)["circuits"]
                .as_array()
                .unwrap()
                .len(),
            3,
            "one undo takes the whole division back to C1–C3"
        );

        // A network cable from the point to where the rack will be.
        s.electrical(Parameters(
            serde_json::from_str(
                r#"{"action":"cable","kind":"data","pts":[[590,200],[590,20],[20,20]]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        let check = electrical("{}");
        assert!(check["cables_m"]["data"].as_f64().unwrap() > 7.5, "{check}");
        let line = s.document.read().home().polylines.last().cloned().unwrap();
        assert_eq!(line.discipline, Some(newera_core::Discipline::Electrical));
        assert_eq!(line.dash, newera_core::DashStyle::Dash);
        s.document.write().undo().unwrap();

        // Undo takes the circuit off in one step.
        s.document.write().undo().unwrap();
        assert_eq!(
            electrical(r#"{"action":"circuits"}"#)["circuits"]
                .as_array()
                .unwrap()
                .len(),
            2
        );

        // A finding looked at and accepted stays, with its reason.
        let check = electrical("{}");
        let first = check["findings"][0].clone();
        let key = first[4].as_str().unwrap().to_owned();
        assert!(key.starts_with("elec:"), "{check}");
        let before = check["pending"].as_u64().unwrap();
        let accepted = electrical(&format!(
            r#"{{"accept":[["{key}","rede sai pelo Wi-Fi da sala"]]}}"#
        ));
        let row = accepted["findings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r[4] == key.as_str())
            .unwrap_or_else(|| panic!("{accepted}"))
            .clone();
        assert_eq!(row[5], "rede sai pelo Wi-Fi da sala", "{accepted}");
        assert_eq!(
            accepted["pending"].as_u64().unwrap(),
            before - 1,
            "{accepted}"
        );
        let wrong = s
            .electrical(Parameters(
                serde_json::from_str(r#"{"accept":[["overlap:f1+f2","x"]]}"#).unwrap(),
            ))
            .unwrap_err();
        assert!(wrong.message.contains("elec:"), "{wrong:?}");
    }
}
