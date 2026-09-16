//! The plumbing project: the points each fixture needs, where the water comes
//! from and the sewer goes, and pipe runs laid the way they are built.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ErrorData, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use super::NewEraMcp;
use super::reply::{core, invalid};
use crate::compact;
use newera_core::plumbing::{self, Pipe, PointKind};
use newera_core::routing::{self, Terminal, Via};

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct PlumbingParams {
    /// `check` (default) or `route`.
    action: Option<String>,
    /// For `route`: what the run carries, `cold`, `hot` or `sewer`.
    kind: Option<String>,
    /// For `route`: the points; default every point of the kind.
    #[serde(default)]
    ids: Vec<String>,
    /// For `route`: `ceiling`, `floor` or `wall`; omitted, the cheapest that
    /// can be built.
    via: Option<String>,
    /// For `route`: the piece it starts from (a water meter, a valve, the
    /// heater, the inspection box or stack); default the nearest of its kind.
    from: Option<String>,
    /// For `route` of sewer: the height free under the finished floor, cm
    /// (slab recess, or the ceiling void of the storey below).
    depth: Option<f64>,
    /// Findings looked at: `[[key, reason]]`; an empty reason takes one back.
    #[serde(default)]
    accept: Vec<Vec<String>>,
    /// Drop the acceptances listed in `orphaned`.
    #[serde(default)]
    prune: bool,
}

#[tool_router(router = plumbing_router, vis = "pub(crate)")]
impl NewEraMcp {
    #[tool(
        description = "Plumbing project, NBR 5626 (cold and hot water) and NBR 8160 (sewer). Points are the plumbing pieces (catalog plumbing: cold-water, hot-water, sewer, floor-drain, valve, grease-trap, inspection-box, water-meter, gas-point) and what a point is comes from its catalog, never its name; fixtures are the pieces that use water (toilet, basin, kitchen sink, shower, bathtub, washer, laundry sink, dishwasher). check (default): {points:{kind:count}, pipes_m, findings:[[sev, place, msg, src, key, accepted?]], pending, orphaned, sources} — a cold-water point by every fixture and a sewer point (or a floor drain for basin, shower, tub and machines) within reach, the discharge diameter it needs (toilet 100 mm, kitchen sink and machines 50, others 40); hot water where the project has any; a floor drain where there is a shower or tub; a grease trap for a kitchen sink; the premises — where the water comes from (water-meter or valve) and where the sewer goes (inspection-box or stack); points no drawn pipe of their kind reaches. accept=[[key, reason]] and prune=true as in electrical. route {kind: cold|hot|sewer, ids?, via?: ceiling|floor|wall, from?, depth?}: lays the run from its source to the points along the walls and inside them, or through the ceiling or under the floor, sharing the trunk, draws it (replacing the earlier run of the same points) and replies {via, suggested, length_m, by_premise_m, bends, branches, materials: [[item, qty, unit]]} — water: pipe and bars, 90° elbows, tees, threaded elbows at the points, a gate valve per room, adhesive; sewer: the branch at the largest diameter it takes, the drops at each point's own, a 45° Y junction at each branch (never a 90° tee), two 45° elbows per turn, sealing rings, trap boxes. Sewer runs only under the floor, by gravity, with 2 % fall up to 75 mm and 1 % above: the reply says needs_depth_cm, and with depth a run that does not fit is refused saying how much it needs. Without via the cheapest premise that can be built is taken; one that cannot (a point in no wall, sewer up to the ceiling or lying in a wall) is refused with why."
    )]
    pub(crate) fn plumbing(
        &self,
        Parameters(p): Parameters<PlumbingParams>,
    ) -> Result<String, ErrorData> {
        if !p.accept.is_empty() || p.prune {
            let mut doc = self.document.write();
            let mut accepted = doc.home().accepted.clone();
            if p.prune {
                for (key, _) in plumbing::orphaned(doc.home()) {
                    accepted.remove(&key);
                }
            }
            for pair in &p.accept {
                let key = pair
                    .first()
                    .map(|k| k.trim().to_owned())
                    .unwrap_or_default();
                if !key.starts_with("plumb:") {
                    return Err(invalid(format!(
                        "accept: plumbing keys start with plumb: (not {key})"
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
                for point in plumbing::points(home) {
                    *kinds.entry(point.kind.name()).or_default() += 1;
                }
                let findings = plumbing::check(home);
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
                let orphaned: Vec<[String; 2]> = plumbing::orphaned(home)
                    .into_iter()
                    .map(|(k, why)| [k, why])
                    .collect();
                let pipes: serde_json::Map<String, serde_json::Value> =
                    plumbing::pipe_lengths(home)
                        .into_iter()
                        .map(|(pipe, m)| (pipe.key().to_owned(), serde_json::json!(m)))
                        .collect();
                Ok(serde_json::json!({
                    "points": kinds,
                    "pipes_m": pipes,
                    "findings": rows,
                    "pending": pending,
                    "orphaned": orphaned,
                    "sources": super::sources(&codes),
                })
                .to_string())
            }
            "route" => self.route_pipe(&p),
            other => Err(invalid(format!("action: check or route (not {other})"))),
        }
    }
}

impl NewEraMcp {
    #[allow(clippy::too_many_lines)]
    fn route_pipe(&self, p: &PlumbingParams) -> Result<String, ErrorData> {
        let pipe = p
            .kind
            .as_deref()
            .and_then(Pipe::parse)
            .ok_or_else(|| invalid("route kind: cold, hot or sewer"))?;
        let mut doc = self.document.write();
        let home = doc.home();
        let view = home.level_view(home.current_level());
        let all = plumbing::points(home);
        let wanted: Vec<&plumbing::Point> = if p.ids.is_empty() {
            all.iter().filter(|pt| pipe.serves(pt.kind)).collect()
        } else {
            p.ids
                .iter()
                .map(|raw| {
                    all.iter()
                        .find(|pt| pt.id.to_string() == *raw)
                        .filter(|pt| pipe.serves(pt.kind))
                        .ok_or_else(|| {
                            invalid(format!(
                                "{raw} is not a {} point of this storey",
                                pipe.key()
                            ))
                        })
                })
                .collect::<Result<_, _>>()?
        };
        if wanted.is_empty() {
            return Err(invalid(format!(
                "route: no {} points (place them first, or give ids)",
                pipe.key()
            )));
        }
        let first = wanted[0].at;
        let nearest = |kinds: &[PointKind]| {
            all.iter()
                .filter(|pt| kinds.contains(&pt.kind))
                .min_by(|a, b| a.at.distance(first).total_cmp(&b.at.distance(first)))
                .map(plumbing::terminal)
        };
        let source = if let Some(raw) = &p.from {
            view.furniture
                .iter()
                .flat_map(newera_core::Furniture::flatten)
                .find(|f| f.id.to_string() == *raw)
                .map(|f| Terminal {
                    id: Some(f.id),
                    at: f.position,
                    z: f.elevation + f.height / 2.0,
                })
                .ok_or_else(|| invalid(format!("from: {raw} is not a piece of this storey")))?
        } else {
            match pipe {
                Pipe::Cold => nearest(&[PointKind::WaterMeter, PointKind::Valve]).ok_or_else(|| {
                    invalid("route cold: where does the water come from? place the water-meter or a valve (in a flat, by the stack in the shaft), or give from")
                })?,
                Pipe::Hot => view
                    .furniture
                    .iter()
                    .flat_map(newera_core::Furniture::flatten)
                    .filter(|f| plumbing::is_heater(f))
                    .min_by(|a, b| a.position.distance(first).total_cmp(&b.position.distance(first)))
                    .map(|f| Terminal {
                        id: Some(f.id),
                        at: f.position,
                        z: f.elevation + f.height / 2.0,
                    })
                    .ok_or_else(|| {
                        invalid("route hot: where does hot water come from? name the heater (aquecedor) or give from")
                    })?,
                Pipe::Sewer => nearest(&[PointKind::InspectionBox]).ok_or_else(|| {
                    invalid("route sewer: where does it go? place the inspection-box (in a flat, the stack in the shaft), or give from")
                })?,
            }
        };
        let points: Vec<Terminal> = wanted.iter().map(|pt| plumbing::terminal(pt)).collect();
        let storey = home
            .current_level()
            .and_then(|id| home.level(id))
            .map_or(280.0, |l| l.height);
        let sizes: Vec<(PointKind, u32)> = wanted
            .iter()
            .map(|pt| {
                (
                    pt.kind,
                    if pipe == Pipe::Sewer {
                        plumbing::sewer_mm(home, pt)
                    } else {
                        25
                    },
                )
            })
            .collect();
        let trunk = sizes.iter().map(|(_, mm)| *mm).max().unwrap_or(50);
        let laid: Vec<(Via, routing::Route)> = [Via::Ceiling, Via::Floor, Via::Wall]
            .into_iter()
            .map(|via| (via, routing::lay_out(&view, source, &points, via, storey)))
            .collect();
        // What each premise can build, and why not when it cannot.
        let why_not = |via: Via, route: &routing::Route| -> Option<String> {
            if let Some(why) = plumbing::refuses(pipe, via) {
                return Some(why.to_owned());
            }
            if !route.impossible.is_empty() {
                let ids: Vec<String> = route
                    .impossible
                    .iter()
                    .filter_map(|t| t.id.map(|i| i.to_string()))
                    .collect();
                return Some(format!(
                    "{} em parede nenhuma: não há como correr dentro dela",
                    ids.join(", ")
                ));
            }
            if pipe == Pipe::Sewer
                && let Some(depth) = p.depth
            {
                let needs = plumbing::sewer_depth(route, trunk);
                if needs > depth {
                    return Some(format!(
                        "o ramal precisa de {needs} cm sob o piso ({trunk} mm com caimento de {} %) e há {depth}: aproxime os pontos da prumada, divida em ramais, ou aumente o rebaixo",
                        plumbing::slope(trunk) * 100.0
                    ));
                }
            }
            None
        };
        let suggested = laid
            .iter()
            .filter(|(via, r)| why_not(*via, r).is_none())
            .min_by(|a, b| a.1.length().total_cmp(&b.1.length()))
            .map(|(via, _)| *via);
        let via = match p.via.as_deref() {
            Some(raw) => Via::parse(raw).ok_or_else(|| invalid("via: ceiling, floor or wall"))?,
            None => suggested.ok_or_else(|| {
                let reasons: Vec<String> = laid
                    .iter()
                    .filter_map(|(via, r)| why_not(*via, r).map(|w| format!("{}: {w}", via.key())))
                    .collect();
                invalid(format!(
                    "route {}: no premise can be built — {}",
                    pipe.key(),
                    reasons.join("; ")
                ))
            })?,
        };
        let route = laid
            .iter()
            .find(|(v, _)| *v == via)
            .map(|(_, r)| r.clone())
            .expect("every premise is laid out");
        if let Some(why) = why_not(via, &route) {
            return Err(invalid(format!(
                "route {} via {}: {why}",
                pipe.key(),
                via.key()
            )));
        }
        let rooms: std::collections::BTreeSet<_> = wanted.iter().filter_map(|pt| pt.room).collect();
        let bill = plumbing::materials(&route, pipe, &sizes, rooms.len());
        let run = {
            let mut ids: Vec<String> = wanted.iter().map(|pt| pt.id.to_string()).collect();
            ids.sort();
            format!(
                "{}:{}",
                pipe.key(),
                if p.ids.is_empty() {
                    "all".into()
                } else {
                    ids.join("+")
                }
            )
        };
        let mut commands: Vec<newera_core::Command> = home
            .polylines
            .iter()
            .filter(|l| l.properties.get(plumbing::RUN_KEY) == Some(&run))
            .map(|l| newera_core::Command::remove(newera_core::ElementId::Polyline(l.id)))
            .collect();
        let (dash, color) = pipe.style();
        for path in route.paths.clone() {
            let mut line = newera_core::Polyline::new(doc.new_polyline_id(), path);
            line.dash = dash;
            line.color = color;
            line.thickness = 1.5;
            line.discipline = Some(newera_core::Discipline::Plumbing);
            line.properties
                .insert(plumbing::PIPE_KEY.into(), pipe.key().into());
            line.properties
                .insert(plumbing::RUN_KEY.into(), run.clone());
            line.properties
                .insert(plumbing::RUN_CM_KEY.into(), route.length().to_string());
            line.properties.insert("plumb:via".into(), via.key().into());
            commands.push(newera_core::Command::insert(line));
        }
        doc.execute(newera_core::Command::Batch { commands })
            .map_err(core)?;
        let by_premise: serde_json::Map<String, serde_json::Value> = laid
            .iter()
            .map(|(v, r)| {
                (
                    v.key().to_owned(),
                    match why_not(*v, r) {
                        None => compact::num(r.length() / 100.0),
                        Some(why) => serde_json::json!(why),
                    },
                )
            })
            .collect();
        let mut reply = serde_json::json!({
            "run": run,
            "via": via.key(),
            "suggested": suggested.map(Via::key),
            "length_m": {
                "horizontal": compact::num(route.horizontal / 100.0),
                "vertical": compact::num(route.vertical / 100.0),
                "total": compact::num(route.length() / 100.0),
            },
            "by_premise_m": by_premise,
            "bends": route.bends,
            "branches": route.branches,
            "materials": bill.iter().map(|m| serde_json::json!([m.item, m.quantity, m.unit])).collect::<Vec<_>>(),
            "rev": doc.revision(),
        });
        if pipe == Pipe::Sewer {
            reply["trunk_mm"] = serde_json::json!(trunk);
            reply["slope_pct"] = serde_json::json!(plumbing::slope(trunk) * 100.0);
            reply["needs_depth_cm"] = serde_json::json!(plumbing::sewer_depth(&route, trunk));
        }
        Ok(reply.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::server;

    #[test]
    fn a_bathroom_is_checked_and_its_water_and_sewer_are_routed() {
        let s = server();
        s.create(Parameters(
            serde_json::from_str(
                r#"{"walls":[{"pts":[[0,0],[200,0],[200,250],[0,250]],"closed":true}],
                    "rooms":[{"name":"Banho","at":[100,125]}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        s.place(Parameters(
            serde_json::from_str(
                r#"{"items":[{"cat":"toilet","at":[50,35]},{"cat":"shower","at":[150,200]}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        let plumbing = |json: &str| {
            s.plumbing(Parameters(serde_json::from_str(json).unwrap()))
                .map(|r| serde_json::from_str::<serde_json::Value>(&r).unwrap())
        };
        let bare = plumbing("{}").unwrap();
        let text = bare["findings"].to_string();
        assert!(
            text.contains("plumb:cold:") && text.contains("plumb:sewer:"),
            "{bare}"
        );
        assert!(
            bare["sources"]["nbr8160"].is_array() || bare["sources"]["nbr8160"].is_object(),
            "{bare}"
        );

        s.disciplines(Parameters(
            serde_json::from_str(r#"{"action":"select","d":"plumbing"}"#).unwrap(),
        ))
        .unwrap();
        let reply = s
            .place(Parameters(
                serde_json::from_str(
                    r#"{"items":[{"cat":"cold-water","at":[30,5]},{"cat":"sewer","at":[50,10],"name":"Esgoto — vaso"},
                                 {"cat":"cold-water","at":[195,200]},{"cat":"floor-drain","at":[150,200]},
                                 {"cat":"inspection-box","at":[5,125]},{"cat":"water-meter","at":[5,100]}]}"#,
                )
                .unwrap(),
            ))
            .unwrap();
        assert!(reply.contains("ids="), "{reply}");
        let wired = plumbing("{}").unwrap();
        let text = wired["findings"].to_string();
        assert!(
            !text.contains("plumb:cold:") && !text.contains("plumb:sewer:"),
            "{wired}"
        );
        assert!(!text.contains("plumb:source"), "{wired}");

        // Sewer: only under the floor, with the fall it needs.
        let sewer = plumbing(r#"{"action":"route","kind":"sewer"}"#).unwrap();
        assert_eq!(sewer["via"], "floor", "{sewer}");
        assert_eq!(sewer["trunk_mm"], 100, "{sewer}");
        assert!(sewer["by_premise_m"]["ceiling"].is_string(), "{sewer}");
        let bill = sewer["materials"].to_string();
        assert!(
            bill.contains("Anel de vedação") && bill.contains("Caixa sifonada"),
            "{sewer}"
        );
        assert!(!bill.contains("Tê"), "{sewer}");
        let needs = sewer["needs_depth_cm"].as_f64().unwrap();
        let tight = plumbing(&format!(
            r#"{{"action":"route","kind":"sewer","depth":{}}}"#,
            needs - 1.0
        ))
        .unwrap_err();
        assert!(tight.message.contains("precisa de"), "{tight:?}");
        assert!(
            plumbing(r#"{"action":"route","kind":"sewer","via":"ceiling"}"#)
                .unwrap_err()
                .message
                .contains("gravidade")
        );

        // Cold water: the cheapest premise, tees and elbows, a valve.
        let cold = plumbing(r#"{"action":"route","kind":"cold"}"#).unwrap();
        let bill = cold["materials"].to_string();
        assert!(
            bill.contains("PVC soldável 25 mm") && bill.contains("Registro de gaveta"),
            "{cold}"
        );
        let check = plumbing("{}").unwrap();
        assert!(check["pipes_m"]["cold"].as_f64().unwrap() > 0.0, "{check}");
        assert!(check["pipes_m"]["sewer"].as_f64().unwrap() > 0.0, "{check}");
        assert!(
            !check["findings"].to_string().contains("plumb:unreached"),
            "{check}"
        );

        // Hot water has no heater: asked where it comes from.
        assert!(plumbing(r#"{"action":"route","kind":"hot"}"#).is_err());

        // Accepted with a reason.
        let accepted =
            plumbing(r#"{"accept":[["plumb:grease","prédio: caixa coletiva"]]}"#).unwrap();
        assert!(
            accepted["orphaned"].to_string().contains("plumb:grease"),
            "no sink here: {accepted}"
        );
    }
}
