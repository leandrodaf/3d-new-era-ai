//! Roofs, and the walls, glass and panels cut to follow them.

use newera_core::Command;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ErrorData, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use super::NewEraMcp;
use super::reply::{core, invalid, ok};
use crate::edit;

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct FitRoofParams {
    /// Walls and pieces (glass, panels) to fit.
    ids: Vec<String>,
    /// Lowest sloping surface that counts, cm above the floor (default 5).
    above: Option<f64>,
    /// Stop following the roof (heights stay as they are).
    #[serde(default)]
    off: bool,
}
#[tool_router(router = roof_router, vis = "pub(crate)")]
impl NewEraMcp {
    #[tool(
        description = "Fit walls, glass and panels to the roof above them: under an A-frame or shed roof a wall gets a sloping top and is split at the ridge, a panel becomes a triangle or trapezoid (a glass gable with no math); a joinery slatted panel gets its slats cut to the roof line. They keep following the roof when it changes, in the same undo step; off stops that. Reply ok with the count."
    )]
    pub(crate) fn fit_roof(
        &self,
        Parameters(p): Parameters<FitRoofParams>,
    ) -> Result<String, ErrorData> {
        let ids = edit::parse_ids(&p.ids).map_err(invalid)?;
        let mut doc = self.document.write();
        if p.off {
            let mut commands = Vec::new();
            for id in &ids {
                match id {
                    newera_core::ElementId::Wall(w) => {
                        if let Some(mut wall) = doc.home().wall(*w).cloned() {
                            wall.properties.remove(newera_core::ROOF_FIT_KEY);
                            commands.push(Command::update(wall));
                        }
                    }
                    newera_core::ElementId::Furniture(f) => {
                        if let Some(mut piece) =
                            doc.home().furniture.iter().find(|x| x.id == *f).cloned()
                        {
                            piece.properties.remove(newera_core::ROOF_FIT_KEY);
                            commands.push(Command::update(piece));
                        }
                    }
                    _ => {}
                }
            }
            doc.execute(Command::Batch { commands }).map_err(core)?;
            return Ok(ok(&doc, &[]));
        }
        // Slatted panels are rebuilt by their rules under the roof line.
        let above = p.above.unwrap_or(newera_core::ROOF_FIT_ABOVE);
        let mut joinery = 0;
        let mut rest = Vec::new();
        for id in ids {
            let is_joinery =
                match id {
                    newera_core::ElementId::Furniture(f) => doc.home().furniture.iter().any(|x| {
                        x.id == f && x.properties.contains_key(newera_joinery::PARAMS_KEY)
                    }),
                    _ => false,
                };
            if let (true, newera_core::ElementId::Furniture(f)) = (is_joinery, id) {
                newera_joinery::fit_joinery_to_roof(&mut doc, f, above).map_err(invalid)?;
                joinery += 1;
            } else {
                rest.push(id);
            }
        }
        let ids = rest;
        if ids.is_empty() {
            return Ok(format!("{} fitted={joinery}", ok(&doc, &[])));
        }
        let before: Vec<String> = doc.home().walls.iter().map(|w| w.id.to_string()).collect();
        let count = newera_core::fit_to_roof(&mut doc, &ids, above).map_err(core)? + joinery;
        let added: Vec<String> = doc
            .home()
            .walls
            .iter()
            .map(|w| w.id.to_string())
            .filter(|id| !before.contains(id))
            .collect();
        Ok(format!("{} fitted={count}", ok(&doc, &added)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::CreateParams;
    use crate::tools::{PlaceParams, server};

    #[test]
    fn roofs_and_beams() {
        let s = server();
        // A 600 × 700 cm A-frame: eaves at the floor, ridge at 675 cm.
        let params: CreateParams = serde_json::from_str(
            r#"{"roofs":[{"pts":[[0,0],[0,700],[600,700],[600,0]],"h":0,"ridge_h":675,"overhang":0,"gables":true}]}"#,
        )
        .unwrap();
        let reply = s.create(Parameters(params)).unwrap();
        assert!(reply.contains("ids=w"), "{reply}");
        {
            let doc = s.document.read();
            let home = doc.home();
            assert_eq!(home.walls.len(), 4, "two sloping walls per gable");
            assert!(
                home.walls
                    .iter()
                    .any(|w| (w.height.max(w.height_at_end.unwrap_or(0.0)) - 675.0).abs() < 1e-6)
            );
            let roof = home.furniture.iter().find(|f| f.is_group()).unwrap();
            // Two slopes and the ridge cap closing the notch between them.
            assert_eq!(roof.children.len(), 3);
            let mut only = home.clone();
            only.walls.clear();
            let mesh =
                newera_render::Mesh::from_home(&only, &newera_render::Selection::new(), &|_| None);
            let points: Vec<[f32; 3]> = mesh.vertices[4..].iter().map(|v| v.position).collect();
            let top = points.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
            assert!((6.75..7.0).contains(&top), "ridge {top}");
            // Up at the ridge the panels meet over the middle of the span.
            let ridge_x: Vec<f32> = points.iter().filter(|p| p[1] > 6.7).map(|p| p[0]).collect();
            assert!(ridge_x.iter().all(|x| (x - 3.0).abs() < 0.2), "{ridge_x:?}");
            // At the eaves they reach the long sides.
            let low = points.iter().filter(|p| p[1] < 0.3).map(|p| p[0]);
            let (min, max) = low.fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(x), b.max(x)));
            assert!(min < 0.1 && max > 5.9, "{min} {max}");
        }
        // A room under the A-frame gets no flat ceiling: at the gables' peak it
        // would stick out through the slopes (seen in photos, which show both
        // sides of every face).
        let room: CreateParams = serde_json::from_str(
            r#"{"rooms":[{"name":"Sala","pts":[[10,10],[590,10],[590,690],[10,690]]}]}"#,
        )
        .unwrap();
        s.create(Parameters(room)).unwrap();
        {
            let doc = s.document.read();
            let mesh = newera_render::Mesh::from_home(
                doc.home(),
                &newera_render::Selection::new(),
                &|_| None,
            );
            let outside = mesh
                .vertices
                .iter()
                .map(|v| v.position)
                .filter(|p| p[1] > 3.0 && (p[0] - 3.0).abs() > 2.5)
                .collect::<Vec<_>>();
            assert!(outside.is_empty(), "{outside:?}");
        }
        let bad: CreateParams =
            serde_json::from_str(r#"{"roofs":[{"pts":[[0,0],[0,700]]}]}"#).unwrap();
        assert!(s.create(Parameters(bad)).is_err());

        // A brace from the floor at the origin to 300 cm up, 300 cm along y.
        let params: PlaceParams = serde_json::from_str(
            r#"{"items":[{"cat":"beam","a":[1000,0,0],"b":[1000,300,300],"w":8,"h":8}]}"#,
        )
        .unwrap();
        s.place(Parameters(params)).unwrap();
        let doc = s.document.read();
        let mut only = doc.home().clone();
        only.walls.clear();
        only.furniture.retain(|f| !f.is_group());
        let mesh =
            newera_render::Mesh::from_home(&only, &newera_render::Selection::new(), &|_| None);
        let near = |target: [f32; 3]| {
            mesh.vertices[4..].iter().any(|v| {
                let p = v.position;
                (p[0] - target[0]).abs() < 0.1
                    && (p[1] - target[1]).abs() < 0.1
                    && (p[2] - target[2]).abs() < 0.1
            })
        };
        assert!(
            near([10.0, 0.0, 0.0]) && near([10.0, 3.0, 3.0]),
            "ends of the brace"
        );
        drop(doc);
        assert!(
            s.place(Parameters(
                serde_json::from_str(r#"{"items":[{"cat":"beam","a":[0,0,0]}]}"#).unwrap()
            ))
            .is_err()
        );

        // A rafter drawn past the ridge stops under the other slope instead of
        // piercing the roof (retest finding 44).
        let rafter: PlaceParams = serde_json::from_str(
            r#"{"items":[{"cat":"beam","a":[20,350,30],"b":[330,350,700],"w":5,"h":18}]}"#,
        )
        .unwrap();
        let id = s
            .place(Parameters(rafter))
            .unwrap()
            .rsplit('=')
            .next()
            .unwrap()
            .to_owned();
        let doc = s.document.read();
        let beam = doc
            .home()
            .furniture
            .iter()
            .find(|f| f.id.to_string() == id)
            .unwrap();
        let (_, top) = beam.height_range();
        assert!(top < 690.0, "stops below the ridge: {top}");
        assert!(beam.depth < 700.0 && beam.depth > 600.0, "{}", beam.depth);
    }
    #[test]
    fn walls_and_glass_follow_an_a_frame_roof() {
        let s = server();
        let params: CreateParams = serde_json::from_str(
            r#"{"roofs":[{"pts":[[0,0],[0,700],[600,700],[600,0]],"h":0,"ridge_h":600,"overhang":30,"t":15}],"walls":[{"pts":[[0,420],[600,420]],"h":250,"t":12}]}"#,
        )
        .unwrap();
        let ids = s.create(Parameters(params)).unwrap();
        let wall = ids
            .rsplit('=')
            .next()
            .unwrap()
            .split(',')
            .find(|i| i.starts_with('w'))
            .unwrap()
            .to_owned();
        let place: PlaceParams = serde_json::from_str(
            r#"{"items":[{"cat":"panel","at":[300,5],"w":600,"d":2,"h":100,"opacity":0.35}]}"#,
        )
        .unwrap();
        let glass = s
            .place(Parameters(place))
            .unwrap()
            .rsplit('=')
            .next()
            .unwrap()
            .to_owned();
        let p: FitRoofParams =
            serde_json::from_str(&format!(r#"{{"ids":["{wall}","{glass}"]}}"#)).unwrap();
        let reply = s.fit_roof(Parameters(p)).unwrap();
        assert!(reply.contains("fitted=3"), "{reply}");
        {
            let doc = s.document.read();
            let walls = &doc.home().walls;
            assert_eq!(walls.len(), 2, "split at the ridge");
            let peak = walls
                .iter()
                .map(|w| w.height.max(w.height_at_end.unwrap_or(0.0)))
                .fold(0.0, f64::max);
            assert!(peak > 590.0, "{walls:?}");
            let panel = doc
                .home()
                .furniture
                .iter()
                .find(|f| f.id.to_string() == glass)
                .unwrap();
            assert!(
                matches!(panel.shape, Some(newera_core::SolidShape::Profile(_))),
                "{panel:?}"
            );
        }
        // Off: the marks go, heights stay.
        let p: FitRoofParams =
            serde_json::from_str(&format!(r#"{{"ids":["{wall}"],"off":true}}"#)).unwrap();
        s.fit_roof(Parameters(p)).unwrap();
        assert!(
            !s.document
                .read()
                .home()
                .wall(wall.parse().unwrap())
                .unwrap()
                .properties
                .contains_key(newera_core::ROOF_FIT_KEY)
        );
        // Nothing overhead is explained.
        let p: FitRoofParams = serde_json::from_str(r#"{"ids":["w999"]}"#).unwrap();
        assert!(s.fit_roof(Parameters(p)).is_err());
    }
}
