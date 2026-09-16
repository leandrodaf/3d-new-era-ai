//! The electrical and telecom project: circuits, the load schedule, and
//! what NBR 5410 asks of each room.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ErrorData, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use super::NewEraMcp;
use super::reply::{applied, core, invalid};
use crate::compact;
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
    /// For `assign` on automation points: standby consumption, W.
    standby_w: Option<f64>,
    /// For `assign` on a dimmer: the most lighting it takes, W.
    max_w: Option<f64>,
    /// For `assign` on a distribution panel: the DIN modules it holds.
    modules: Option<u32>,
    /// For `voltage`: the presumed short-circuit current at the delivery
    /// point, kA, as the utility informs it.
    short_ka: Option<f64>,
    /// For `voltage`: the earthing scheme, `TN-S`, `TN-C-S` or `TT`.
    earthing: Option<String>,
    /// Supply voltage, V; with `assign`, the points' own, 127 or 220.
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
    /// For `route`: where the run passes, `ceiling` (slab, dropping in the
    /// walls), `floor` or `wall`. Omitted: the cheapest that can be built.
    via: Option<String>,
    /// For `route` of data: `cat5e`, `cat6` (default) or `cat6a`.
    cat: Option<String>,
    /// For `route`: the panel it starts from; default the nearest of its kind.
    from: Option<String>,
    /// For `wifi`: the access points' standard, `wifi5`, `wifi6`, `wifi6e` or
    /// `wifi7`, written on `ids`.
    standard: Option<String>,
    /// For `wifi`: the access points' wired uplink, Gbps (1, 2.5, 5, 10).
    uplink: Option<f64>,
    /// For `wifi`: whether `ids` are fed by their data cable (`PoE`).
    poe: Option<bool>,
    /// For `wifi`: the band the suggestion aims at, `2.4`, `5` (default) or `6`.
    band: Option<String>,
}

#[tool_router(router = electrical_router, vis = "pub(crate)")]
impl NewEraMcp {
    #[tool(
        description = "Electrical and telecom project, NBR 5410 and NBR 16264 (residential cabling). Points are the electrical pieces (catalog electrical: outlets, switches, lighting points, panel, network-outlet RJ45, tv-outlet, wifi-point, telecom-panel) plus every fixture that lights. check (default): {points:{kind:count}, findings:[[sev, place, msg, src, key, accepted?]], pending, orphaned, sources} — accept=[[key, reason]] with any action marks findings looked at (they stay listed with the reason and stop counting in pending), an empty reason takes one back, orphaned lists acceptances whose finding is gone and prune=true drops them — a ceiling lighting point per room, general-use outlets per room (kitchens and laundries one per 3.5 m of perimeter, bathrooms one by the basin, living rooms and bedrooms one per 5 m), RJ45 and TV outlets per room as NBR 16264 table 1 recommends (2 RJ45 and 1 TV in bedrooms, living rooms, offices, kitchens and laundries; 3 and 2 in a home theater; 1 and 1 elsewhere; a Wi-Fi point is not an RJ45 outlet), a power outlet by the telecom panel, a distribution and a telecom panel, points without a circuit, lighting and outlets sharing a circuit, a dedicated load not alone. circuits: rows [name, kinds, points, VA, V, A, wire mm², breaker A, DR] and main_breaker {a, phases, load_a_per_phase}: the supply to ask for, by Enel SP's categories on 127/220 V (single-phase up to 12 kW with no 220 V circuit, two-phase up to 20 kW, three-phase up to 75 kW) and the smallest of its fixed entry breakers (50, 63, 80, 100… A) — power by NBR 5410 (lighting per room by area: 100 VA up to 6 m² and 60 VA per whole 4 m² beyond, shared by its points; 600 VA for each of the first three outlets of a kitchen, laundry or bathroom, 100 VA after and elsewhere; dedicated points at their rated power, a shower 7500 W and air conditioning 1500 until written) unless set; the section is the smallest from 1.5 mm² (lighting) or 2.5 (with outlets) whose capacity, corrected for the circuits sharing its conduit on the laid-out runs (table 42, or grouping written on the project), admits a breaker between the current and it; DR on every point of a room with a bath or shower, on kitchen, laundry, service and garage points (lighting at 2.50 m or higher excepted) and on outdoor and balcony outlets; a shower over 4.4 kVA runs on 220 V. check also says lighting and outlets sharing a circuit only when NBR 5410 9.5.3.3 forbids it (over 16 A, or all lighting or all outlets on shared circuits), kitchen and laundry outlets sharing with anything else, equipment over 10 A not alone, and a voltage drop over 4 % to the farthest point of a laid-out run. assign {ids, circuit, va?} — or the whole division at once, circuits {\"C1\": [ids], \"C2\": [ids]} — writes the circuits (and power, and volts 127|220 per point — a 220 V outlet makes its circuit 220 V) on points in one undoable step. voltage {volts?, short_ka?, earthing?: TN-S|TN-C-S|TT}. circuits also gives panel {devices: [[device, count, DIN modules]], modules {used, capacity, capacity_written, spare}, dps, earthing, icn_ka, selective}: one-pole breakers for 127 V circuits and two-pole for 220 V between phases, a two-pole DR per circuit that needs one, the main breaker, the surge protector (DPS class II, a module per phase and neutral) and NBR 5410's spare ways (2 up to 6 circuits, 3 up to 12, 4 up to 30, 15 % above); capacity is the panel's modules (assign modules on it) or a guess from its size, and check says when it does not fit, when the main breaker is under twice the largest partial (selectivity) and when the short-circuit level was assumed. cable {kind: power|data|tv, pts} (with ids or circuit instead of pts, it is a route): draws a run of the electrical project, told apart on the plan (power solid, network dashed, TV dash-dot); check then reports cables_m, the length by kind with a tenth for the drops, and network or TV points no run reaches, or a telecom panel none reaches. route {kind: power|data|tv, ids? | circuit?, via?: ceiling|floor|wall, cat?: cat5e|cat6|cat6a, from?}: lays the run the way it is built, along the walls and inside them (or in the slab), from the nearest panel of its kind to the points (all of the kind when none given), sharing the trunk, and draws it replacing the earlier run of the same circuit; replies {via, suggested, length_m {horizontal, vertical, total}, by_premise_m, bends, materials: [[item, qty, unit]]} — conduit, boxes, wire by conductor, cable, connectors. Without via it takes the cheapest premise that can be built; a via that cannot reach a point (wall with a point out of every wall; ceiling with a low point out of every wall, nowhere to drop) is refused naming the points. Automation (catalog smart-relay, smart-switch, dimmer, presence-sensor, smart-lock): each draws its standby on its circuit (assign standby_w; relay, dimmer and sensor 1 W, smart switch 1.2 by default, from manufacturers' sheets), circuits reports standby_w, and check asks a relay or smart switch for a neutral in its box, a dimmer for the room's lighting to fit its max_w (default 1.1 A at the supply voltage, some 140 W at 127 V) and pass 10 W, a ceiling sensor to be between 2.2 and 3 m (manufacturers install at about 2.4) and to see the room's far corner (about 1.45 × its height), a lock to sit on a door. wifi {ids?, standard?: wifi5|wifi6|wifi6e|wifi7, poe?, band?: 2.4|5|6}: writes the standard and PoE on access points (wifi-point) when given, and replies {access_points: [[id, standard, bands, uplink]], coverage: [[room, band, median dBm, worst dBm (9 places in 10), share at -67 dBm or better, grade]], suggested: {standard, band, points: [[x, y, z, room]], short: [rooms still under -67 dBm]}} — signal estimated from free-space loss, distance and each wall crossed by its material and thickness (a door or window where the path goes through one), per band; the suggestion is the fewest ceiling points (up to four) at room centres covering the rooms people use. check also asks each access point for its data cable, power (an outlet within 1.5 m or poe) and a cable category that carries its uplink (uplink up to 2.5 GbE: Cat 5e; 5 GbE: Cat 6; 10 GbE: Cat 6A). Circuit numbers are drawn next to the points on the plan, and with annotations(legend=true) the load schedule under the legend."
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
        // A cable given by its points, not its path, is laid out like a route.
        let action = match p.action.as_deref().unwrap_or("check") {
            "cable" if p.pts.is_empty() && (!p.ids.is_empty() || p.circuit.is_some()) => "route",
            other => other,
        };
        match action {
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
                let standby: f64 = electrical::points(doc.home())
                    .iter()
                    .filter(|pt| pt.kind == electrical::PointKind::Automation)
                    .map(|pt| pt.va)
                    .sum();
                let mut reply = serde_json::json!({"circuits": rows, "total_va": total});
                if standby > 0.0 {
                    reply["standby_w"] = compact::num(standby);
                }
                if let Some(panel) = electrical::panel(doc.home()) {
                    reply["panel"] = serde_json::json!({
                        "devices": panel.devices.iter().map(|d| serde_json::json!([d.0, d.1, d.2])).collect::<Vec<_>>(),
                        "modules": {"used": panel.used, "capacity": panel.capacity, "capacity_written": panel.capacity_written, "spare": panel.spare},
                        "dps": panel.dps,
                        "earthing": panel.earthing,
                        "icn_ka": panel.icn_ka,
                        "selective": panel.main_a >= panel.largest_partial_a * 2,
                    });
                }
                if let Some(supply) = electrical::main_breaker(doc.home()) {
                    reply["main_breaker"] = serde_json::json!({
                        "a": supply.breaker_a,
                        "phases": supply.phases,
                        "load_a_per_phase": supply.amps_per_phase,
                    });
                }
                Ok(reply.to_string())
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
                if p.circuits.is_none()
                    && p.circuit.is_none()
                    && p.va.is_none()
                    && p.volts.is_none()
                    && p.standby_w.is_none()
                    && p.max_w.is_none()
                    && p.modules.is_none()
                {
                    return Err(invalid(
                        "assign needs circuit (empty to clear), va or volts",
                    ));
                }
                if let Some(volts) = p.volts
                    && (volts - 127.0).abs() > 0.5
                    && (volts - 220.0).abs() > 0.5
                {
                    return Err(invalid(format!(
                        "volts on a point: 127 or 220 (not {volts})"
                    )));
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
                        if let Some(w) = p.standby_w {
                            piece
                                .properties
                                .insert(electrical::STANDBY_KEY.into(), w.to_string());
                        }
                        if let Some(m) = p.modules {
                            piece
                                .properties
                                .insert(electrical::MODULES_KEY.into(), m.to_string());
                        }
                        if let Some(w) = p.max_w {
                            piece
                                .properties
                                .insert(electrical::MAX_W_KEY.into(), w.to_string());
                        }
                        if let Some(volts) = p.volts {
                            piece
                                .properties
                                .insert(electrical::VOLTS_KEY.into(), volts.to_string());
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
            "route" => {
                let cable = p
                    .kind
                    .as_deref()
                    .and_then(electrical::Cable::parse)
                    .ok_or_else(|| invalid("route kind: power, data or tv"))?;
                let category = match p.cat.as_deref() {
                    Some(raw) => electrical::Category::parse(raw)
                        .ok_or_else(|| invalid("cat: cat5e, cat6 or cat6a"))?,
                    None => electrical::Category::Cat6,
                };
                let mut doc = self.document.write();
                let before = doc.home().clone();
                let home = doc.home();
                let view = home.level_view(home.current_level());
                let all = electrical::points(home);
                // The points: given, or those of the circuit, or every point of the kind.
                let wanted: Vec<&electrical::Point> = if !p.ids.is_empty() {
                    p.ids
                        .iter()
                        .map(|raw| {
                            all.iter()
                                .find(|pt| pt.id.to_string() == *raw)
                                .ok_or_else(|| {
                                    invalid(format!(
                                        "{raw} is not a point of the electrical project"
                                    ))
                                })
                        })
                        .collect::<Result<_, _>>()?
                } else if let Some(circuit) = &p.circuit {
                    all.iter()
                        .filter(|pt| pt.circuit.as_deref() == Some(circuit.as_str()))
                        .collect()
                } else {
                    all.iter()
                        .filter(|pt| match cable {
                            electrical::Cable::Power => pt.kind.loads(),
                            electrical::Cable::Data => matches!(
                                pt.kind,
                                electrical::PointKind::Network | electrical::PointKind::Wifi
                            ),
                            electrical::Cable::Tv => pt.kind == electrical::PointKind::Tv,
                        })
                        .collect()
                };
                if wanted.is_empty() {
                    return Err(invalid(
                        "route: no points (give ids, a circuit, or place points of the kind)",
                    ));
                }
                let panel_kind = match cable {
                    electrical::Cable::Power => electrical::PointKind::Panel,
                    _ => electrical::PointKind::TelecomPanel,
                };
                let terminal = |id: newera_core::FurnitureId| {
                    view.find_piece(id).map(|f| newera_core::routing::Terminal {
                        id: Some(f.id),
                        at: f.position,
                        z: f.elevation + f.height / 2.0,
                    })
                };
                let source = if let Some(raw) = &p.from {
                    all.iter()
                        .find(|pt| pt.id.to_string() == *raw)
                        .and_then(|pt| terminal(pt.id))
                        .ok_or_else(|| {
                            invalid(format!("from: {raw} is not a panel of this storey"))
                        })?
                } else {
                    let first = terminal(wanted[0].id)
                        .ok_or_else(|| invalid("point not on this storey"))?;
                    all.iter()
                        .filter(|pt| pt.kind == panel_kind)
                        .filter_map(|pt| terminal(pt.id))
                        .min_by(|a, b| a.at.distance(first.at).total_cmp(&b.at.distance(first.at)))
                        .ok_or_else(|| {
                            invalid(match cable {
                                electrical::Cable::Power => {
                                    "route: place a distribution panel first (electrical-panel)"
                                }
                                _ => "route: place a telecom panel first (telecom-panel)",
                            })
                        })?
                };
                let points: Vec<newera_core::routing::Terminal> =
                    wanted.iter().filter_map(|pt| terminal(pt.id)).collect();
                let storey = home
                    .current_level()
                    .and_then(|id| home.level(id))
                    .map_or(280.0, |l| l.height);
                let (best, all_routes) =
                    newera_core::routing::cheapest(&view, source, &points, storey);
                let via = match p.via.as_deref() {
                    Some(raw) => newera_core::routing::Via::parse(raw)
                        .ok_or_else(|| invalid("via: ceiling, floor or wall"))?,
                    None => best,
                };
                let route = all_routes
                    .iter()
                    .find(|(v, _)| *v == via)
                    .map(|(_, r)| r.clone())
                    .expect("every premise is laid out");
                if !route.impossible.is_empty() {
                    let ids: Vec<String> = route
                        .impossible
                        .iter()
                        .filter_map(|t| t.id.map(|i| i.to_string()))
                        .collect();
                    return Err(invalid(format!(
                        "route via {}: {} {}",
                        via.key(),
                        ids.join(", "),
                        match via {
                            newera_core::routing::Via::Ceiling =>
                                "stand low in no wall: from the ceiling there is no wall to drop in; use via floor, or move the point to a wall",
                            _ =>
                                "stand in no wall and cannot be run inside one; use via ceiling or floor, or move the point to a wall",
                        }
                    )));
                }
                let kinds: Vec<electrical::PointKind> = wanted.iter().map(|pt| pt.kind).collect();
                let section = match (&p.circuit, cable) {
                    (Some(c), electrical::Cable::Power) => electrical::circuits(home)
                        .into_iter()
                        .find(|x| x.name == *c)
                        .map_or(2.5, |x| x.wire_mm2),
                    _ => {
                        if kinds.iter().all(|k| *k == electrical::PointKind::Lighting) {
                            1.5
                        } else {
                            2.5
                        }
                    }
                };
                let bill = electrical::materials(&route, cable, section, category, &kinds);
                // Replaces the run drawn before for the same thing.
                // Named by what it serves, so routing the same points again
                // replaces it and routing others adds a run beside it.
                let run = format!(
                    "{}:{}",
                    cable.key(),
                    p.circuit.clone().unwrap_or_else(|| {
                        let mut ids: Vec<String> =
                            wanted.iter().map(|pt| pt.id.to_string()).collect();
                        ids.sort();
                        if p.ids.is_empty() {
                            "all".into()
                        } else {
                            ids.join("+")
                        }
                    })
                );
                let mut commands: Vec<newera_core::Command> = home
                    .polylines
                    .iter()
                    .filter(|l| l.properties.get(electrical::RUN_KEY) == Some(&run))
                    .map(|l| newera_core::Command::remove(newera_core::ElementId::Polyline(l.id)))
                    .collect();
                let (dash, color) = cable.style();
                let paths = route.paths.clone();
                for path in paths {
                    let mut line = newera_core::Polyline::new(doc.new_polyline_id(), path);
                    line.dash = dash;
                    line.color = color;
                    line.thickness = 1.5;
                    line.discipline = Some(newera_core::Discipline::Electrical);
                    line.properties
                        .insert(electrical::CABLE_KEY.into(), cable.key().into());
                    line.properties
                        .insert(electrical::RUN_KEY.into(), run.clone());
                    if cable == electrical::Cable::Data {
                        line.properties
                            .insert(electrical::CATEGORY_KEY.into(), category.key().into());
                    }
                    line.properties
                        .insert(electrical::RUN_CM_KEY.into(), route.length().to_string());
                    let far = route.reach.iter().copied().fold(0.0, f64::max);
                    line.properties
                        .insert(electrical::RUN_FAR_KEY.into(), far.to_string());
                    line.properties.insert("elec:via".into(), via.key().into());
                    commands.push(newera_core::Command::insert(line));
                }
                doc.execute(newera_core::Command::Batch { commands })
                    .map_err(core)?;
                let alternatives: serde_json::Map<String, serde_json::Value> = all_routes
                    .iter()
                    .map(|(v, r)| {
                        (
                            v.key().to_owned(),
                            if r.impossible.is_empty() {
                                compact::num(r.length() / 100.0)
                            } else {
                                serde_json::json!("impossível")
                            },
                        )
                    })
                    .collect();
                let reply = serde_json::json!({
                    "run": run,
                    "via": via.key(),
                    "suggested": best.key(),
                    "length_m": {
                        "horizontal": compact::num(route.horizontal / 100.0),
                        "vertical": compact::num(route.vertical / 100.0),
                        "total": compact::num(route.length() / 100.0),
                    },
                    "by_premise_m": alternatives,
                    "bends": route.bends,
                    "materials": bill.iter().map(|m| serde_json::json!([m.item, m.quantity, m.unit])).collect::<Vec<_>>(),
                });
                let mut reply = reply;
                // A network link past 90 m (the permanent link, NBR 16264 /
                // TIA-568), counting the slack left at the rack and the outlet
                // does not carry its category: say which points.
                if cable == electrical::Cable::Data {
                    let far: Vec<String> = route
                        .terminals
                        .iter()
                        .zip(&route.reach)
                        .filter(|(_, cm)| **cm + 330.0 > 9000.0)
                        .filter_map(|(t, _)| t.id.map(|i| i.to_string()))
                        .collect();
                    if !far.is_empty() {
                        reply["warnings"] = serde_json::json!([format!(
                            "{} passam de 90 m do rack: o enlace não garante a categoria; aproxime o rack ou use um switch no caminho",
                            far.join(", ")
                        )]);
                    }
                }
                reply["rev"] = serde_json::json!(doc.revision());
                let _ = before;
                Ok(reply.to_string())
            }
            "wifi" => {
                use newera_core::wifi;
                let standard = match p.standard.as_deref() {
                    Some(raw) => Some(
                        wifi::Standard::parse(raw)
                            .ok_or_else(|| invalid("standard: wifi5, wifi6, wifi6e or wifi7"))?,
                    ),
                    None => None,
                };
                let band = match p.band.as_deref() {
                    Some(raw) => {
                        wifi::Band::parse(raw).ok_or_else(|| invalid("band: 2.4, 5 or 6"))?
                    }
                    None => wifi::Band::G5,
                };
                let mut doc = self.document.write();
                if !p.ids.is_empty()
                    && (standard.is_some() || p.poe.is_some() || p.uplink.is_some())
                {
                    let mut commands = Vec::new();
                    for raw in &p.ids {
                        let piece = doc
                            .home()
                            .furniture
                            .iter()
                            .flat_map(newera_core::Furniture::flatten)
                            .find(|f| f.id.to_string() == *raw && f.catalog == "wifi-point")
                            .cloned()
                            .ok_or_else(|| invalid(format!("{raw} is not a wifi-point")))?;
                        let mut piece = piece;
                        if let Some(s) = standard {
                            piece
                                .properties
                                .insert(wifi::STANDARD_KEY.into(), s.key().into());
                        }
                        if let Some(gbps) = p.uplink {
                            piece
                                .properties
                                .insert(wifi::UPLINK_KEY.into(), gbps.to_string());
                        }
                        if let Some(poe) = p.poe {
                            piece
                                .properties
                                .insert(wifi::POE_KEY.into(), poe.to_string());
                        }
                        commands.push(newera_core::Command::update(piece));
                    }
                    doc.execute(newera_core::Command::Batch { commands })
                        .map_err(core)?;
                }
                let home = doc.home();
                let aps = wifi::access_points(home);
                let storey = home
                    .current_level()
                    .and_then(|id| home.level(id))
                    .map_or(280.0, |l| l.height);
                let aimed = standard
                    .or_else(|| aps.first().map(|a| a.standard))
                    .unwrap_or(wifi::Standard::Wifi6);
                if !aimed.bands().contains(&band) {
                    return Err(invalid(format!(
                        "{} has no {} GHz band",
                        aimed.name(),
                        band.key()
                    )));
                }
                let rows: Vec<serde_json::Value> = wifi::coverage(home, &aps)
                    .iter()
                    .map(|r| {
                        serde_json::json!([
                            format!("{} {}", r.name, r.room),
                            r.band.key(),
                            r.median,
                            r.worst,
                            r.good,
                            wifi::grade(r.worst)
                        ])
                    })
                    .collect();
                let (points, short) = wifi::suggest(home, aimed, band, storey);
                Ok(serde_json::json!({
                    "access_points": aps.iter().map(|a| serde_json::json!([
                        a.id.map(|i| i.to_string()),
                        a.standard.key(),
                        a.standard.bands().iter().map(|b| b.key()).collect::<Vec<_>>(),
                        format!("{} GbE", a.uplink_gbps),
                    ])).collect::<Vec<_>>(),
                    "coverage": rows,
                    "suggested": {
                        "standard": aimed.key(),
                        "band": band.key(),
                        "points": points.iter().map(|(a, room)| serde_json::json!([compact::num(a.at.x), compact::num(a.at.y), compact::num(a.z), room])).collect::<Vec<_>>(),
                        "short": short,
                    },
                    "rev": doc.revision(),
                })
                .to_string())
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
                if p.volts.is_none() && p.short_ka.is_none() && p.earthing.is_none() {
                    return Err(invalid(
                        "voltage needs volts (127 or 220), short_ka or earthing",
                    ));
                }
                let mut doc = self.document.write();
                let before = doc.home().clone();
                let mut properties = doc.home().properties.clone();
                if let Some(volts) = p.volts {
                    if !(100.0..=400.0).contains(&volts) {
                        return Err(invalid("volts: a supply voltage, 127 or 220"));
                    }
                    properties.insert(electrical::VOLTAGE_KEY.into(), volts.to_string());
                }
                if let Some(ka) = p.short_ka {
                    if !(1.0..=50.0).contains(&ka) {
                        return Err(invalid(
                            "short_ka: the presumed short-circuit current, kA (e.g. 4.5, 6, 10)",
                        ));
                    }
                    properties.insert(electrical::SHORT_KA_KEY.into(), ka.to_string());
                }
                if let Some(scheme) = &p.earthing {
                    let scheme = scheme.trim().to_uppercase();
                    if !["TN-S", "TN-C-S", "TT"].contains(&scheme.as_str()) {
                        return Err(invalid("earthing: TN-S, TN-C-S or TT"));
                    }
                    properties.insert(electrical::EARTHING_KEY.into(), scheme);
                }
                doc.execute(newera_core::Command::SetProperties { properties })
                    .map_err(core)?;
                Ok(applied(&doc, &before))
            }
            other => Err(invalid(format!(
                "action: check, circuits, assign, voltage, cable, route or wifi (not {other})"
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

    #[test]
    fn a_run_is_routed_inside_walls_and_slab_with_its_materials() {
        let s = server();
        s.create(Parameters(
            serde_json::from_str(
                r#"{"walls":[{"pts":[[0,0],[600,0],[600,400],[0,400]],"closed":true}],
                    "rooms":[{"name":"Sala","at":[300,200]}]}"#,
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
                    r#"{"items":[{"cat":"electrical-panel","at":[10,200]},{"cat":"telecom-panel","at":[10,300]},
                                 {"cat":"outlet-low","at":[300,10]},{"cat":"outlet-low","at":[590,200]},
                                 {"cat":"light-ceiling","at":[300,200]},
                                 {"cat":"network-outlet","at":[590,300]},{"cat":"network-outlet","at":[300,390]}]}"#,
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
        let electrical = |json: &str| {
            s.electrical(Parameters(serde_json::from_str(json).unwrap()))
                .map(|r| serde_json::from_str(&r).unwrap_or(serde_json::Value::String(r)))
        };

        // Outlets in the walls: every premise can be built, the cheapest is chosen.
        let outlets = electrical(&format!(
            r#"{{"action":"route","kind":"power","ids":["{}","{}"]}}"#,
            ids[2], ids[3]
        ))
        .unwrap();
        let by = &outlets["by_premise_m"];
        let chosen = outlets["length_m"]["total"].as_f64().unwrap();
        for via in ["ceiling", "floor", "wall"] {
            assert!(by[via].as_f64().unwrap() + 1e-9 >= chosen, "{outlets}");
        }
        assert_eq!(outlets["via"], outlets["suggested"]);
        let bill = outlets["materials"].to_string();
        assert!(
            bill.contains("Eletroduto") && bill.contains("(terra)"),
            "{outlets}"
        );
        let drawn: Vec<_> = s
            .document
            .read()
            .home()
            .polylines
            .iter()
            .filter(|l| l.properties.contains_key(electrical::RUN_KEY))
            .cloned()
            .collect();
        assert!(!drawn.is_empty());
        assert!(
            drawn
                .iter()
                .all(|l| l.discipline == Some(newera_core::Discipline::Electrical))
        );

        // Routing the same run again replaces it, and check measures the route.
        electrical(&format!(
            r#"{{"action":"route","kind":"power","ids":["{}","{}"],"via":"wall"}}"#,
            ids[2], ids[3]
        ))
        .unwrap();
        let runs = s
            .document
            .read()
            .home()
            .polylines
            .iter()
            .filter(|l| l.properties.contains_key(electrical::RUN_KEY))
            .count();
        assert!(runs <= drawn.len().max(3), "the earlier run was replaced");
        let check = electrical("{}").unwrap();
        assert!(
            check["cables_m"]["power"].as_f64().unwrap() > 0.0,
            "{check}"
        );

        // A ceiling light is in no wall: a wall run to it is refused, naming it.
        let refused = electrical(&format!(
            r#"{{"action":"route","kind":"power","ids":["{}"],"via":"wall"}}"#,
            ids[4]
        ))
        .unwrap_err();
        assert!(refused.message.contains(&ids[4]), "{refused:?}");
        let light = electrical(&format!(
            r#"{{"action":"route","kind":"power","ids":["{}"]}}"#,
            ids[4]
        ))
        .unwrap();
        assert_ne!(light["suggested"], "wall", "{light}");

        // cable with ids and no path is the same route.
        let by_ids = electrical(&format!(
            r#"{{"action":"cable","kind":"data","ids":["{}"]}}"#,
            ids[5]
        ))
        .unwrap();
        assert!(by_ids["materials"].to_string().contains("RJ45"), "{by_ids}");

        // Network in star from the telecom panel, Cat 6A as asked.
        let data = electrical(r#"{"action":"route","kind":"data","cat":"cat6a"}"#).unwrap();
        let bill = data["materials"].to_string();
        assert!(bill.contains("Cat 6A") && bill.contains("RJ45"), "{data}");
        assert!(!bill.contains("750 V"), "{data}");

        // 220 V on a point, and the main breaker in the schedule.
        electrical(&format!(
            r#"{{"action":"assign","ids":["{}"],"circuit":"C2","volts":220}}"#,
            ids[3]
        ))
        .unwrap();
        let schedule = electrical(r#"{"action":"circuits"}"#).unwrap();
        assert_eq!(schedule["circuits"][0][4], 220.0, "{schedule}");
        assert!(
            schedule["main_breaker"]["a"].as_u64().unwrap() >= 10,
            "{schedule}"
        );
        assert!(
            electrical(&format!(
                r#"{{"action":"assign","ids":["{}"],"volts":110}}"#,
                ids[3]
            ))
            .is_err()
        );
    }

    #[test]
    fn an_access_point_asks_for_cable_power_and_category_and_its_coverage_is_given() {
        let s = server();
        s.create(Parameters(
            serde_json::from_str(
                r#"{"walls":[{"pts":[[0,0],[800,0],[800,400],[0,400]],"closed":true},{"pts":[[400,0],[400,400]]}],
                    "rooms":[{"name":"Sala","at":[200,200]},{"name":"Quarto","at":[600,200]}]}"#,
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
                    r#"{"items":[{"cat":"telecom-panel","at":[10,200]},{"cat":"wifi-point","at":[200,200]},
                                 {"cat":"network-outlet","at":[390,300]}]}"#,
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
        let ap = ids[1].clone();
        let electrical = |json: &str| {
            s.electrical(Parameters(serde_json::from_str(json).unwrap()))
                .map(|r| serde_json::from_str(&r).unwrap_or(serde_json::Value::String(r)))
        };
        let keys = |check: &serde_json::Value| check["findings"].to_string();
        let check = electrical("{}").unwrap();
        assert!(
            keys(&check).contains(&format!("elec:wifi-power:{ap}")),
            "{check}"
        );

        // A data run that reaches the network outlet but not the access point.
        electrical(&format!(
            r#"{{"action":"route","kind":"data","ids":["{}"]}}"#,
            ids[2]
        ))
        .unwrap();
        let check = electrical("{}").unwrap();
        assert!(
            keys(&check).contains(&ap),
            "the access point has no cable: {check}"
        );

        // Wi-Fi 7 on PoE over Cat 6: powered, cabled, but short of 10 GbE.
        electrical(&format!(
            r#"{{"action":"wifi","ids":["{ap}"],"standard":"wifi7","poe":true,"uplink":10}}"#
        ))
        .unwrap();
        electrical(r#"{"action":"route","kind":"data","cat":"cat6"}"#).unwrap();
        let check = electrical("{}").unwrap();
        let text = keys(&check);
        assert!(!text.contains("elec:wifi-power"), "{check}");
        assert!(!text.contains("elec:unreached:data"), "{check}");
        assert!(text.contains(&format!("elec:wifi-uplink:{ap}")), "{check}");
        electrical(r#"{"action":"route","kind":"data","cat":"cat6a"}"#).unwrap();
        assert!(!keys(&electrical("{}").unwrap()).contains("elec:wifi-uplink"));

        // Coverage per room and band, and where to put the points.
        let wifi = electrical(r#"{"action":"wifi","band":"6"}"#).unwrap();
        let rows = wifi["coverage"].as_array().unwrap();
        assert_eq!(rows.len(), 6, "two rooms × three bands: {wifi}");
        let at = |room: &str, band: &str| {
            rows.iter()
                .find(|r| r[0].as_str().unwrap().starts_with(room) && r[1] == band)
                .unwrap()[2]
                .as_f64()
                .unwrap()
        };
        assert!(at("Sala", "5") > at("Quarto", "5"), "{wifi}");
        assert!(
            at("Quarto", "2.4") - at("Quarto", "6") > at("Sala", "2.4") - at("Sala", "6"),
            "the wall costs the high band more: {wifi}"
        );
        assert_eq!(wifi["suggested"]["band"], "6");
        assert!(
            !wifi["suggested"]["points"].as_array().unwrap().is_empty(),
            "{wifi}"
        );
        assert!(
            electrical(r#"{"action":"wifi","standard":"wifi6","band":"6"}"#).is_err(),
            "Wi-Fi 6 has no 6 GHz"
        );
    }

    #[test]
    fn automation_is_placed_from_the_catalog_and_its_standby_reaches_the_schedule() {
        let s = server();
        s.create(Parameters(
            serde_json::from_str(
                r#"{"walls":[{"pts":[[0,0],[400,0],[400,400],[0,400]],"closed":true}],"rooms":[{"name":"Hall","at":[200,200]}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        let reply = s
            .place(Parameters(
                serde_json::from_str(
                    r#"{"items":[{"cat":"light-ceiling","at":[200,200]},{"cat":"smart-relay","at":[10,100]},{"cat":"presence-sensor","at":[200,200]}]}"#,
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
        let electrical = |json: &str| {
            s.electrical(Parameters(serde_json::from_str(json).unwrap()))
                .map(|r| serde_json::from_str(&r).unwrap_or(serde_json::Value::String(r)))
        };
        electrical(&format!(
            r#"{{"action":"assign","circuits":{{"C1":["{}","{}","{}"]}}}}"#,
            ids[0], ids[1], ids[2]
        ))
        .unwrap();
        electrical(&format!(
            r#"{{"action":"assign","ids":["{}"],"standby_w":1.2}}"#,
            ids[1]
        ))
        .unwrap();
        let schedule = electrical(r#"{"action":"circuits"}"#).unwrap();
        assert!(
            (schedule["standby_w"].as_f64().unwrap() - 2.2).abs() < 1e-9,
            "{schedule}"
        );
        assert_eq!(
            schedule["circuits"][0][1],
            serde_json::json!(["Iluminação"]),
            "{schedule}"
        );
        let check = electrical("{}").unwrap();
        assert_eq!(check["points"]["Automação"], 2, "{check}");
        assert!(
            check["findings"]
                .to_string()
                .contains(&format!("elec:neutral:{}", ids[1])),
            "{check}"
        );
    }

    #[test]
    fn the_panel_is_filled_and_its_protection_given_in_the_schedule() {
        let s = server();
        s.create(Parameters(
            serde_json::from_str(
                r#"{"walls":[{"pts":[[0,0],[400,0],[400,400],[0,400]],"closed":true}],"rooms":[{"name":"Cozinha","at":[200,200]}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        let reply = s
            .place(Parameters(
                serde_json::from_str(
                    r#"{"items":[{"cat":"electrical-panel","at":[10,200]},{"cat":"outlet-mid","at":[100,10]},{"cat":"shower-point","at":[300,10]}]}"#,
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
        let electrical = |json: &str| {
            s.electrical(Parameters(serde_json::from_str(json).unwrap()))
                .map(|r| serde_json::from_str(&r).unwrap_or(serde_json::Value::String(r)))
        };
        electrical(&format!(
            r#"{{"action":"assign","circuits":{{"C1":["{}"],"C2":["{}"]}}}}"#,
            ids[1], ids[2]
        ))
        .unwrap();
        let schedule = electrical(r#"{"action":"circuits"}"#).unwrap();
        let panel = &schedule["panel"];
        assert_eq!(panel["modules"]["spare"], 2, "{schedule}");
        assert!(
            panel["devices"].to_string().contains("bipolar (220 V"),
            "the shower is between phases: {schedule}"
        );
        assert!(
            panel["dps"].as_str().unwrap().contains("Classe II"),
            "{schedule}"
        );
        assert_eq!(
            panel["earthing"], "TN-C-S",
            "the utility's PEN split at the entry"
        );
        assert!(
            electrical("{}").unwrap()["findings"]
                .to_string()
                .contains("elec:short-circuit")
        );

        electrical(r#"{"action":"voltage","short_ka":10,"earthing":"tt"}"#).unwrap();
        electrical(&format!(
            r#"{{"action":"assign","ids":["{}"],"modules":4}}"#,
            ids[0]
        ))
        .unwrap();
        let schedule = electrical(r#"{"action":"circuits"}"#).unwrap();
        assert_eq!(schedule["panel"]["icn_ka"], 10.0, "{schedule}");
        assert_eq!(schedule["panel"]["earthing"], "TT");
        let text = electrical("{}").unwrap()["findings"].to_string();
        assert!(
            text.contains("elec:panel-full") && !text.contains("elec:short-circuit"),
            "{text}"
        );
    }
}
