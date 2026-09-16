//! Lighting by photometry: is the room bright enough, and what would make it.
//!
//! Rated against the ABNT NBR ISO/CIE 8995-1 residential references.

use newera_core::{Command, Point2};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ErrorData, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use super::NewEraMcp;
use super::reply::{core, invalid};

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct LightingParams {
    /// Room id (default: every room of the current storey).
    room: Option<String>,
    /// Work plane height cm (default 75).
    plane: Option<f64>,
    /// Fill `room` with a grid of this fixture until it reaches the lux wanted:
    /// `downlight`, `led-panel`, `light-ceiling`, `pendant`.
    fill: Option<String>,
    /// Lux wanted instead of the room's reference.
    lux: Option<f64>,
}
#[tool_router(router = lighting_router, vis = "pub(crate)")]
impl NewEraMcp {
    #[tool(
        description = "Lighting design by photometry: every fixture's flux (lm, or W × lamp efficacy), color temperature and distribution (bulb, spot beam, LED panel/strip) lights the work plane by the inverse-square cosine law, walls casting shadows, plus interreflection (split flux). Reply rooms [[id,name,m²,avg lx,min lx,uniformity,reference lx,fixtures,W/m²,verdict]] against ABNT NBR ISO/CIE 8995-1 residential references. fill=<fixture> with room places a verified grid reaching the reference (or lux). Set a piece's light with place/update light {lm|w,lamp,k,beam,area}."
    )]
    pub(crate) fn lighting(
        &self,
        Parameters(p): Parameters<LightingParams>,
    ) -> Result<String, ErrorData> {
        use newera_core::lighting::{
            Reflectance, emitters, fixtures_needed, grid_positions, room_lighting,
        };
        let plane = p.plane.unwrap_or(75.0);
        let row = |r: &newera_core::RoomLighting, wanted: f64| {
            let verdict = if r.average + 0.5 < wanted {
                format!("abaixo: faltam {} lx", (wanted - r.average).round())
            } else if r.average > wanted * 2.5 {
                format!("acima: {:.1}× a referência", r.average / wanted)
            } else if r.uniformity < 0.4 && r.points > 4 {
                "ok na média, mas pouco uniforme (U0 < 0,4)".to_owned()
            } else {
                "ok".to_owned()
            };
            serde_json::json!([
                r.room.to_string(),
                r.name,
                (r.area_m2 * 10.0).round() / 10.0,
                r.average.round(),
                r.min.round(),
                (r.uniformity * 100.0).round() / 100.0,
                wanted,
                r.fixtures,
                (r.watts_per_m2 * 10.0).round() / 10.0,
                verdict,
            ])
        };
        let mut doc = self.document.write();
        let home = doc.home().clone();
        let view = home.level_view(home.current_level());
        let wanted_room: Option<newera_core::RoomId> = match &p.room {
            Some(id) => Some(id.parse().map_err(|e| invalid(format!("{e}")))?),
            None => None,
        };
        let rooms: Vec<&newera_core::Room> = view
            .rooms
            .iter()
            .filter(|r| wanted_room.is_none_or(|id| r.id == id))
            .collect();
        if rooms.is_empty() {
            return Err(invalid(match &p.room {
                Some(id) => format!("{id} not found on this storey"),
                None => "no rooms on this storey".to_owned(),
            }));
        }
        let Some(cat) = &p.fill else {
            let lights = emitters(&home, &newera_catalog::light_for);
            let rows: Vec<serde_json::Value> = rooms
                .iter()
                .map(|room| {
                    let r = room_lighting(&home, &lights, room, plane, Reflectance::default());
                    let wanted = p.lux.unwrap_or(r.target);
                    row(&r, wanted)
                })
                .collect();
            let lumens: f64 = lights.iter().map(|e| e.flux).sum();
            let watts: f64 = lights.iter().map(|e| e.watts).sum();
            return Ok(serde_json::json!({
                "rooms": rows,
                "fixtures": lights.len(),
                "lm": lumens.round(),
                "W": watts.round(),
                "sources": super::sources(&["nbr5413", "nbr8995"]),
            })
            .to_string());
        };
        let [room] = rooms[..] else {
            return Err(invalid("fill needs one `room`"));
        };
        let entry = newera_catalog::find(cat)
            .filter(|e| e.light.is_some())
            .ok_or_else(|| {
                invalid(format!(
                    "`{cat}` is not a light fixture (downlight, led-panel, light-ceiling, pendant)"
                ))
            })?;
        let ceiling = home
            .resolve_level(room.level)
            .and_then(|id| home.levels.iter().find(|l| l.id == id))
            .map_or(home.wall_height, |l| l.height);
        let template = entry.instantiate(newera_core::FurnitureId(0), Point2::default());
        let fixture_lm = newera_catalog::light_for(&template).map_or(0.0, |l| l.flux());
        let before = room_lighting(
            &home,
            &emitters(&home, &newera_catalog::light_for),
            room,
            plane,
            Reflectance::default(),
        );
        let wanted = p.lux.unwrap_or(before.target);
        let area = before.area_m2;
        if before.average + 0.5 >= wanted {
            return Ok(serde_json::json!({
                "placed": [],
                "before": row(&before, wanted),
                "note": "já atende; nada colocado",
            })
            .to_string());
        }
        let layout = |count: usize| -> (Vec<newera_core::Furniture>, newera_core::RoomLighting) {
            // The whole grid, even a few more than asked: symmetric layouts.
            let placed: Vec<newera_core::Furniture> = grid_positions(&room.points, count)
                .into_iter()
                .map(|at| {
                    let mut piece = template.clone();
                    piece.position = at;
                    piece.level = room.level;
                    // Hung from or set into the ceiling of the room (or the roof over it).
                    let top = newera_core::roof_height_at(&view, at, newera_core::ROOF_FIT_ABOVE)
                        .map_or(ceiling, |roof| roof.min(ceiling));
                    piece.elevation = match cat.as_str() {
                        "downlight" => top - piece.height + 0.6,
                        _ => top - piece.height,
                    };
                    piece
                })
                .collect();
            let mut trial = home.clone();
            trial.furniture.extend(placed.iter().cloned());
            let report = room_lighting(
                &trial,
                &emitters(&trial, &newera_catalog::light_for),
                room,
                plane,
                Reflectance::default(),
            );
            (placed, report)
        };
        // The lumen method gives a first count; the photometry of that trial
        // tells what each fixture really adds, then fixtures are added one by
        // one until the room reaches the reference.
        let mut count = fixtures_needed(wanted, area, fixture_lm).clamp(1, 60);
        let (mut placed, mut after) = layout(count);
        #[allow(clippy::cast_precision_loss)]
        let gain = (after.average - before.average) / count as f64;
        if gain > 0.0 {
            // Clamped to 1..60 first, so the cast is exact.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let needed = ((wanted - before.average) / gain).ceil().clamp(1.0, 60.0) as usize;
            count = needed;
            (placed, after) = layout(count);
        }
        while after.average + 0.5 < wanted && count < 60 {
            count += 1;
            (placed, after) = layout(count);
        }
        let mut ids = Vec::new();
        let mut commands = Vec::new();
        for mut piece in placed {
            piece.id = doc.new_furniture_id();
            ids.push(piece.id.to_string());
            commands.push(Command::insert(piece));
        }
        doc.execute(Command::Batch { commands }).map_err(core)?;
        Ok(serde_json::json!({
            "placed": ids,
            "before": row(&before, wanted),
            "after": row(&after, wanted),
        })
        .to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::CreateParams;
    use crate::tools::elements::UpdateParams;
    use crate::tools::furniture::PlaceParams;
    use crate::tools::server;

    #[test]
    fn lighting_rates_rooms_and_fills_them_to_the_reference() {
        let s = server();
        let params: CreateParams = serde_json::from_str(
            r#"{"walls":[{"pts":[[0,0],[500,0],[500,400],[0,400]],"closed":true}],"rooms":[{"name":"Cozinha","at":[250,200]}]}"#,
        )
        .unwrap();
        s.create(Parameters(params)).unwrap();
        let rate = |p: &str| -> serde_json::Value {
            serde_json::from_str(
                &s.lighting(Parameters(serde_json::from_str(p).unwrap()))
                    .unwrap(),
            )
            .unwrap()
        };
        let dark = rate("{}");
        // A home kitchen: NBR 5413's residential 150 lx (300 at the counter).
        assert_eq!(dark["rooms"][0][6], 150.0, "{dark}");
        // The reference lux carries the standards it comes from.
        assert!(dark["sources"]["nbr5413"].is_array(), "{dark}");
        assert!(
            dark["rooms"][0][9].as_str().unwrap().starts_with("abaixo"),
            "{dark}"
        );
        // A warm bulb set by watts: 60 W incandescent is 720 lm.
        let place: PlaceParams = serde_json::from_str(
            r#"{"items":[{"cat":"pendant","at":[250,200],"light":{"w":60,"lamp":"incandescent","k":2700,"beam":0}}]}"#,
        )
        .unwrap();
        s.place(Parameters(place)).unwrap();
        let home = s.document.read().home().clone();
        let bulb = home.furniture.last().unwrap().light.clone().unwrap();
        assert!((bulb.flux() - 720.0).abs() < 1e-9 && bulb.beam.is_none());
        let room = home.rooms[0].id.to_string();
        let filled = rate(&format!(r#"{{"room":"{room}","fill":"downlight"}}"#));
        let placed = filled["placed"].as_array().unwrap().len();
        assert!(placed >= 3, "{filled}");
        let after = &filled["after"];
        assert!(after[3].as_f64().unwrap() >= 149.5, "{filled}");
        assert_eq!(after[7], placed + 1);
        // The spots hang at the ceiling, recessed.
        let home = s.document.read().home().clone();
        let spot = home.furniture.last().unwrap();
        assert!(
            spot.catalog == "downlight"
                && (spot.elevation + spot.height - home.wall_height - 0.6).abs() < 1e-9
        );
        assert!(!s.document.read().can_redo());
        let rated = rate(&format!(r#"{{"room":"{room}"}}"#));
        assert!((rated["rooms"][0][3].as_f64().unwrap() - after[3].as_f64().unwrap()).abs() < 1.0);
        // Turning a light off through update.
        let id = home.furniture.last().unwrap().id.to_string();
        let update: UpdateParams = serde_json::from_str(&format!(
            r#"{{"items":[{{"id":"{id}","light":{{"on":false}}}}]}}"#
        ))
        .unwrap();
        s.update(Parameters(update)).unwrap();
        assert!(
            s.document
                .read()
                .home()
                .furniture
                .last()
                .unwrap()
                .light
                .as_ref()
                .unwrap()
                .flux()
                < 1e-9
        );
        assert!(
            s.lighting(Parameters(
                serde_json::from_str(&format!(r#"{{"fill":"sofa-3","room":"{room}"}}"#)).unwrap()
            ))
            .is_err()
        );
    }
}
