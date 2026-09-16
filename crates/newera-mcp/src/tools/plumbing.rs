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
        description = "Plumbing project, NBR 5626 (cold and hot water) and NBR 8160 (sewer). Points are the plumbing pieces (catalog plumbing: cold-water, hot-water, sewer, valve, grease-trap, inspection-box, water-meter, gas-point, vent-pipe, and the drains as the models they are: floor-drain caixa sifonada 150x150x50, floor-drain-100, floor-drain-75 (150x185x75, up to 15 UHC), trap-drain-small (seal under 50 mm, no trap), dry-drain, linear-drain (w 50/70/90, no trap), linear-drain-trap, rain-drain for open areas; drains are set flush in the floor of a room, never in a wall, a door span or under a cabinet) and what a point is comes from its catalog, never its name; fixtures are the pieces that use water (toilet, basin, kitchen sink, shower, bathtub, washer, laundry sink, dishwasher). check (default): {points:{kind:count}, pipes_m, findings:[[sev, place, msg, src, key, accepted?]], pending, orphaned, sources} — a cold-water point by every fixture and a sewer point (or a floor drain for basin, shower, tub and machines) within reach, the discharge diameter it needs (toilet 100 mm, kitchen sink and machines 50, others 40); hot water where the project has any; a floor drain in every bathroom, kitchen and laundry, inside the shower area where there is one, at least one real trap (50 mm seal) per room, the UHC its outlet takes (50 mm: 6, 75 mm: 15), rain drains for open terraces; a grease trap for a kitchen sink; the premises — where the water comes from (water-meter or valve) and where the sewer goes (inspection-box or stack); points no drawn pipe of their kind reaches. accept=[[key, reason]] and prune=true as in electrical. route {kind: cold|hot|sewer|vent, ids?, via?: ceiling|floor|wall, from?, depth?}: lays the run from its source to the points along the walls and inside them, or through the ceiling or under the floor, sharing the trunk, draws it (replacing the earlier run of the same points and the lines drawn by hand to them, listed in replaced_drawn) and replies {via, suggested, length_m, by_premise_m, bends, branches, materials: [[item, qty, unit]]} — water: pipe and bars, 90° elbows, tees, threaded elbows at the points, a gate valve per room, adhesive; sewer: the branch at the largest diameter it takes, the drops at each point's own, a 45° Y junction at each branch (never a 90° tee), two 45° elbows per turn, sealing rings, trap boxes. Sewer runs only under the floor, by gravity, with 2 % fall up to 75 mm and 1 % above: the reply says needs_depth_cm, and with depth a run that does not fit is refused saying how much it needs. Without via the cheapest premise that can be built is taken; one that cannot (a point in no wall; from the ceiling a low point in no wall, nowhere to drop; sewer up to the ceiling or lying in a wall) is refused with why."
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
                let live: Vec<String> = findings.iter().map(|f| f.key.clone()).collect();
                let orphaned = super::orphan_rows(plumbing::orphaned(home), &live);
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
            .ok_or_else(|| invalid("route kind: cold, hot, sewer or vent"))?;
        let mut doc = self.document.write();
        let home = doc.home();
        let view = home.level_view(home.current_level());
        let all = plumbing::points(home);
        let mut wanted: Vec<&plumbing::Point> = if p.ids.is_empty() {
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
        // Points an earlier run of this pipe already serves: that run is laid
        // again with them, never a second run stacked beside it.
        let asked: std::collections::BTreeSet<String> =
            wanted.iter().map(|pt| pt.id.to_string()).collect();
        let mut merged_into: Option<String> = None;
        if !p.ids.is_empty() {
            let superset = home
                .polylines
                .iter()
                .filter(|l| plumbing::pipe_of(l) == Some(pipe))
                .filter_map(|l| {
                    let ends: std::collections::BTreeSet<String> = l
                        .properties
                        .get(plumbing::ENDS_KEY)?
                        .split(',')
                        .filter(|e| !e.is_empty())
                        .map(str::to_owned)
                        .collect();
                    (ends.is_superset(&asked) && ends.len() > asked.len())
                        .then(|| (l.properties.get(plumbing::RUN_KEY).cloned(), ends))
                })
                .max_by_key(|(_, ends)| ends.len());
            if let Some((run, ends)) = superset {
                wanted = all
                    .iter()
                    .filter(|pt| ends.contains(&pt.id.to_string()))
                    .collect();
                merged_into = run;
            }
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
                .ok_or_else(|| {
                    invalid(format!(
                        "from: {raw} is not a piece of this storey; from is a piece id such as \"f801\", not coordinates"
                    ))
                })?
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
                        invalid("route hot: where does hot water come from? name the heater (aquecedor), or give from as the id of the piece it starts from (a heater, a shaft, a column), e.g. from=\"f801\"")
                    })?,
                Pipe::Sewer => nearest(&[PointKind::InspectionBox]).ok_or_else(|| {
                    invalid("route sewer: where does it go? place the inspection-box (in a flat, the stack in the shaft), or give from")
                })?,
                Pipe::Vent => nearest(&[PointKind::Vent]).ok_or_else(|| {
                    invalid("route vent: place the vent-pipe (the vent stack) first, or give from as its id")
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
                    match pipe {
                        Pipe::Sewer => plumbing::sewer_mm(home, pt),
                        Pipe::Vent => 50,
                        _ => 25,
                    },
                )
            })
            .collect();
        let trunk = sizes.iter().map(|(_, mm)| *mm).max().unwrap_or(50);
        // The drains on the run, by their model.
        let drains: Vec<(String, plumbing::DrainSpec)> = wanted
            .iter()
            .filter_map(|pt| {
                view.find_piece(pt.id)
                    .and_then(|f| plumbing::drain_spec(&f.catalog).map(|d| (f.id.to_string(), d)))
            })
            .collect();
        let drain_depth = drains.iter().map(|(_, d)| d.depth_cm).fold(0.0, f64::max);
        let feeds_toilet = wanted
            .iter()
            .any(|pt| plumbing::served_by(home, pt.at) == Some(plumbing::Fixture::Toilet));
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
                    "{} {}",
                    ids.join(", "),
                    if via == Via::Ceiling {
                        "fora de parede e abaixo do teto: do forro não há parede onde descer"
                    } else {
                        "em parede nenhuma: não há como correr dentro dela"
                    }
                ));
            }
            if pipe == Pipe::Sewer
                && let Some(depth) = p.depth
            {
                let mms: Vec<u32> = sizes.iter().map(|(_, mm)| *mm).collect();
                let needs = plumbing::sewer_depth_with(route, &mms, drain_depth);
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
        let mut bill = plumbing::materials(&route, pipe, &sizes, rooms.len());
        if pipe == Pipe::Sewer && !drains.is_empty() {
            // Each drain as the model it is, not a generic trap box.
            bill.retain(|m| !m.item.starts_with("Caixa sifonada"));
            let mut by_item: std::collections::BTreeMap<&str, f64> =
                std::collections::BTreeMap::new();
            for (_, d) in &drains {
                *by_item.entry(d.item).or_default() += 1.0;
            }
            for (item, quantity) in by_item {
                bill.push(newera_core::electrical::Material {
                    item: item.to_owned(),
                    quantity,
                    unit: "un",
                });
            }
        }
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
        let mut ends: Vec<newera_core::Point2> = wanted.iter().map(|pt| pt.at).collect();
        ends.push(source.at);
        let replaced = plumbing::drawn_runs(home, pipe, &ends);
        // An earlier run of the same pipe whose points this one takes all.
        let now: std::collections::BTreeSet<String> =
            wanted.iter().map(|pt| pt.id.to_string()).collect();
        let covered = |l: &newera_core::Polyline| {
            plumbing::pipe_of(l) == Some(pipe)
                && l.properties.get(plumbing::ENDS_KEY).is_some_and(|ends| {
                    ends.split(',')
                        .filter(|e| !e.is_empty())
                        .all(|e| now.contains(e))
                })
        };
        let mut commands: Vec<newera_core::Command> = home
            .polylines
            .iter()
            .filter(|l| {
                l.properties.get(plumbing::RUN_KEY) == Some(&run)
                    || replaced.contains(&l.id)
                    || covered(l)
            })
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
            line.properties.insert(
                plumbing::ENDS_KEY.into(),
                wanted
                    .iter()
                    .map(|pt| pt.id.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            );
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
            "replaced_drawn": replaced.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "merged_into": merged_into,
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
        if pipe == Pipe::Cold && feeds_toilet {
            reply["notes"] = serde_json::json!([
                "25 mm vale para bacia com caixa acoplada; válvula de descarga pede ramal exclusivo de 50 mm (1½\") e reservatório ou pressão que a atenda."
            ]);
        }
        if pipe == Pipe::Sewer {
            reply["trunk_mm"] = serde_json::json!(trunk);
            reply["slope_pct"] = serde_json::json!(plumbing::slope(trunk) * 100.0);
            let mms: Vec<u32> = sizes.iter().map(|(_, mm)| *mm).collect();
            reply["needs_depth_cm"] =
                serde_json::json!(plumbing::sewer_depth_with(&route, &mms, drain_depth));
            let loose: Vec<&str> = drains
                .iter()
                .filter(|(_, d)| !d.is_trap())
                .map(|(id, _)| id.as_str())
                .collect();
            let mut notes = vec![
                "Piso com caimento para o ralo: 1,5 % a 2,5 % dentro do box, 0,5 % no resto da área molhada (NBR 13753).".to_owned(),
                "Tubulação com o caimento mínimo; nos subcoletores e no coletor predial, no máximo 5 % (NBR 8160 4.2.5.2).".to_owned(),
            ];
            if !loose.is_empty() {
                notes.push(format!(
                    "{} não têm fecho hídrico de 50 mm: ligue-os a uma caixa sifonada do cômodo, não direto ao ramal.",
                    loose.join(", ")
                ));
            }
            reply["notes"] = serde_json::json!(notes);
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

        // Lines drawn by hand before: the one from the meter to a cold point
        // goes when cold water is routed; the one to the inspection box stays.
        let (meter, tap, stack, drain) = {
            let doc = s.document.read();
            let at = |cat: &str| {
                doc.home()
                    .furniture
                    .iter()
                    .find(|f| f.catalog == cat)
                    .unwrap()
                    .position
            };
            (
                at("water-meter"),
                at("cold-water"),
                at("inspection-box"),
                at("floor-drain"),
            )
        };
        s.create(Parameters(
            serde_json::from_str(&format!(
                r#"{{"polylines":[{{"pts":[[{},{}],[{},{}]]}},{{"pts":[[{},{}],[{},{}]]}}]}}"#,
                meter.x, meter.y, tap.x, tap.y, stack.x, stack.y, drain.x, drain.y
            ))
            .unwrap(),
        ))
        .unwrap();
        let drawn: Vec<String> = s
            .document
            .read()
            .home()
            .polylines
            .iter()
            .filter(|l| !l.properties.contains_key(newera_core::plumbing::RUN_KEY))
            .map(|l| l.id.to_string())
            .collect();
        assert_eq!(drawn.len(), 2, "both drawn in the plumbing project");
        assert!(
            plumbing("{}").unwrap()["findings"]
                .to_string()
                .contains("plumb:untyped-lines")
        );

        // Cold water: the cheapest premise, tees and elbows, a valve.
        let cold = plumbing(r#"{"action":"route","kind":"cold"}"#).unwrap();
        assert_eq!(
            cold["replaced_drawn"],
            serde_json::json!([drawn[0]]),
            "{cold}"
        );
        let left: Vec<String> = s
            .document
            .read()
            .home()
            .polylines
            .iter()
            .map(|l| l.id.to_string())
            .collect();
        assert!(
            !left.contains(&drawn[0]) && left.contains(&drawn[1]),
            "{left:?}"
        );
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

        // A vent branch from the vent stack to the traps, never buried.
        s.place(Parameters(
            serde_json::from_str(r#"{"items":[{"cat":"vent-pipe","at":[10,60]}]}"#).unwrap(),
        ))
        .unwrap();
        // A vent branch for one trap, then for all: the second takes the
        // first's place, not a run stacked on it.
        let traps: Vec<String> = s
            .document
            .read()
            .home()
            .furniture
            .iter()
            .filter(|f| matches!(f.catalog.as_str(), "sewer" | "floor-drain"))
            .map(|f| f.id.to_string())
            .collect();
        plumbing(&format!(
            r#"{{"action":"route","kind":"vent","ids":["{}"]}}"#,
            traps[0]
        ))
        .unwrap();
        let vent = plumbing(r#"{"action":"route","kind":"vent"}"#).unwrap();
        assert_ne!(vent["via"], "floor", "{vent}");
        assert!(
            vent["materials"]
                .to_string()
                .contains("ramal de ventilação"),
            "{vent}"
        );
        assert!(plumbing(r#"{"action":"route","kind":"vent","via":"floor"}"#).is_err());
        assert!(
            !plumbing("{}").unwrap()["findings"]
                .to_string()
                .contains("plumb:vent-far")
        );
        let runs: std::collections::BTreeSet<String> = s
            .document
            .read()
            .home()
            .polylines
            .iter()
            .filter(|l| {
                l.properties
                    .get(newera_core::plumbing::PIPE_KEY)
                    .map(String::as_str)
                    == Some("vent")
            })
            .filter_map(|l| l.properties.get(newera_core::plumbing::RUN_KEY).cloned())
            .collect();
        assert_eq!(runs.len(), 1, "the one-trap run was replaced: {runs:?}");
        // Routing one of its traps again lays that same run, not a second one.
        let again = plumbing(&format!(
            r#"{{"action":"route","kind":"vent","ids":["{}"]}}"#,
            traps[0]
        ))
        .unwrap();
        assert!(again["merged_into"].is_string(), "{again}");
        let count = s
            .document
            .read()
            .home()
            .polylines
            .iter()
            .filter(|l| {
                l.properties
                    .get(newera_core::plumbing::PIPE_KEY)
                    .map(String::as_str)
                    == Some("vent")
            })
            .filter_map(|l| l.properties.get(newera_core::plumbing::RUN_KEY).cloned())
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        assert_eq!(count, 1);
        assert!(
            !plumbing("{}").unwrap()["findings"]
                .to_string()
                .contains("plumb:unreached:vent")
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
