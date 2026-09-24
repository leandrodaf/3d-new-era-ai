//! Storeys: adding them, naming them, and scoping reads and edits to one.
//!
//! A reference storey is a drawing to trace over, not a floor: it stays out
//! of the layout checks.

use newera_core::{Command, ops};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ErrorData, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use super::NewEraMcp;
use super::reply::{core, invalid, ok, write_action};
use crate::compact;

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct LevelsParams {
    /// `add`, `select`, `update` or `delete`.
    action: Option<String>,
    /// Level id, e.g. `lv3`.
    id: Option<String>,
    name: Option<String>,
    /// Storey height cm for `add` and `update`.
    h: Option<f64>,
    /// Floor elevation cm for `add` and `update` (e.g. a house on stilts).
    elev: Option<f64>,
    /// Mark the storey as a reference layer: a traced plan, a scan, an
    /// earlier version. Its content is drawing, not building, so layout
    /// checks and ergonomics leave it alone even when it sits at the same
    /// elevation as the storey being designed.
    reference: Option<bool>,
}
#[tool_router(router = levels_router, vis = "pub(crate)")]
impl NewEraMcp {
    #[tool(
        name = "levels",
        description = "Storeys: rows [id,name,elev,h,selected,layout_index,viewable,reference]. Other tools act on the selected storey; change them with edit_levels."
    )]
    pub(crate) fn list_levels(
        &self,
        Parameters(_): Parameters<super::Nothing>,
    ) -> Result<String, ErrorData> {
        self.levels(Parameters(LevelsParams::default()))
    }
    #[tool(
        description = "Change the storeys. add {name?,h?,elev?} adds one on top (or at elev cm) and selects it; select {id}; delete {id} removes it and its content. update {id, elev?|h?|name?|reference?}: elev raises a storey with its walls, floors and openings (houses on stilts); reference=true marks it a tracing layer (imported plan, older version) that checks and ergonomics skip, which is what you want when two storeys share an elevation. Other tools act on the selected storey. The list is the levels tool."
    )]
    pub(crate) fn edit_levels(
        &self,
        Parameters(p): Parameters<LevelsParams>,
    ) -> Result<String, ErrorData> {
        write_action(
            p.action.as_deref(),
            &["add", "select", "update", "delete"],
            "levels",
        )?;
        self.levels(Parameters(p))
    }
    /// Storeys: the list, and every change to them.
    pub(crate) fn levels(
        &self,
        Parameters(p): Parameters<LevelsParams>,
    ) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        let id = || -> Result<newera_core::LevelId, ErrorData> {
            p.id.as_deref()
                .ok_or_else(|| invalid("`id` is required"))?
                .parse()
                .map_err(|e| invalid(format!("{e}")))
        };
        match p.action.as_deref().unwrap_or("list") {
            "list" => Ok(compact::levels(doc.home()).to_string()),
            "add" => {
                let level = ops::add_level(&mut doc, p.name.clone(), p.h).map_err(core)?;
                if let Some(elev) = p.elev
                    && let Some(mut raised) = doc.home().level(level).cloned()
                {
                    raised.elevation = elev;
                    doc.execute(Command::update(raised)).map_err(core)?;
                }
                Ok(ok(&doc, &[level.to_string()]))
            }
            "select" => {
                let level = id()?;
                if doc.home().level(level).is_none() {
                    return Err(invalid(format!("{level} not found")));
                }
                doc.select_level(Some(level));
                Ok(ok(&doc, &[]))
            }
            "update" => {
                let level = id()?;
                let mut updated = doc
                    .home()
                    .level(level)
                    .cloned()
                    .ok_or_else(|| invalid(format!("{level} not found")))?;
                if let Some(elev) = p.elev {
                    updated.elevation = elev;
                }
                if let Some(h) = p.h {
                    updated.height = h;
                }
                if let Some(name) = p.name.clone() {
                    updated.name = name;
                }
                if let Some(reference) = p.reference {
                    updated.set_reference(reference);
                }
                doc.execute(Command::update(updated)).map_err(core)?;
                Ok(ok(&doc, &[]))
            }
            "delete" => {
                ops::delete_level(&mut doc, id()?).map_err(core)?;
                Ok(ok(&doc, &[]))
            }
            other => Err(invalid(format!("unknown action `{other}`"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::CreateParams;
    use crate::tools::check::CheckParams;
    use crate::tools::read::GetHomeParams;
    use crate::tools::server;

    #[test]
    fn a_reference_storey_is_drawing_and_checks_leave_it_alone() {
        let s = server();
        s.create(Parameters(
            serde_json::from_str(
                r#"{"walls":[{"pts":[[0,0],[500,0],[500,400],[0,400]],"closed":true}],
                    "rooms":[{"name":"Sala","at":[250,200]}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        s.place(Parameters(
            serde_json::from_str(r#"{"items":[{"cat":"sofa-3","at":[250,200]}]}"#).unwrap(),
        ))
        .unwrap();
        // A second storey at the same elevation, holding a copy of the plan.
        s.levels(Parameters(LevelsParams {
            action: Some("add".into()),
            name: Some("Novo layout".into()),
            elev: Some(0.0),
            ..LevelsParams::default()
        }))
        .unwrap();
        s.place(Parameters(
            serde_json::from_str(r#"{"items":[{"cat":"sofa-3","at":[250,200]}]}"#).unwrap(),
        ))
        .unwrap();
        let list: serde_json::Value =
            serde_json::from_str(&s.levels(Parameters(LevelsParams::default())).unwrap()).unwrap();
        let ground = list[0][0].as_str().unwrap().to_owned();

        // Reading either storey warns that they are stacked.
        let home: serde_json::Value = serde_json::from_str(
            &s.get_home(Parameters(
                serde_json::from_str(r#"{"detail":"summary"}"#).unwrap(),
            ))
            .unwrap(),
        )
        .unwrap();
        assert!(
            home["warnings"][0]
                .as_str()
                .unwrap()
                .contains("share an elevation"),
            "{home}"
        );

        // Checked together, the two copies read as an artefact of the layers,
        // not as a clash.
        let all: serde_json::Value = serde_json::from_str(
            &s.check_layout(Parameters(CheckParams {
                level: Some("all".into()),
                areas: None,
                accept: Vec::new(),
                prune: false,
            }))
            .unwrap(),
        )
        .unwrap();
        let overlap = &all["overlap"][0];
        assert_eq!(overlap["kind"], "cross_level", "{all}");
        assert!(
            overlap["a"]["name"].is_string(),
            "names come with it: {all}"
        );
        assert!(overlap["a"]["bounds"].is_array(), "{all}");
        assert_eq!(all["overlap_kinds"]["cross_level"], 1, "{all}");

        // Marking the old plan as a reference layer takes it out of the check.
        s.levels(Parameters(LevelsParams {
            action: Some("update".into()),
            id: Some(ground),
            reference: Some(true),
            ..LevelsParams::default()
        }))
        .unwrap();
        let all: serde_json::Value = serde_json::from_str(
            &s.check_layout(Parameters(CheckParams {
                level: Some("all".into()),
                areas: None,
                accept: Vec::new(),
                prune: false,
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(all.get("overlap").is_none(), "{all}");
        assert!(all.get("warnings").is_none(), "and the warning goes: {all}");
    }
    #[test]
    fn levels_scope_edits_and_reads_to_the_selected_storey() {
        let s = server();
        let walls = r#"{"walls":[{"pts":[[0,0],[400,0],[400,300],[0,300]],"closed":true}]}"#;
        s.create(Parameters(serde_json::from_str(walls).unwrap()))
            .unwrap();
        let add = |name: &str| {
            s.levels(Parameters(LevelsParams {
                action: Some("add".into()),
                name: Some(name.into()),
                ..LevelsParams::default()
            }))
            .unwrap()
        };
        let reply = add("Superior");
        assert!(reply.starts_with("ok"), "{reply}");
        let list = s.levels(Parameters(LevelsParams::default())).unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_str(&list).unwrap();
        assert_eq!(rows.len(), 2, "{list}");
        assert_eq!(rows[1][4], true, "new storey is selected: {list}");

        // The upper storey starts empty; new walls go on it.
        let home: serde_json::Value =
            serde_json::from_str(&s.get_home(Parameters(GetHomeParams::default())).unwrap())
                .unwrap();
        assert!(
            home.get("walls")
                .is_none_or(|w| w.as_array().unwrap().is_empty()),
            "{home}"
        );
        assert_eq!(home["levels"].as_array().unwrap().len(), 2);
        s.create(Parameters(
            serde_json::from_str(r#"{"walls":[{"pts":[[0,0],[200,0]]}]}"#).unwrap(),
        ))
        .unwrap();
        assert_eq!(s.document.read().home().walls.len(), 5);

        let ground = rows[0][0].as_str().unwrap().to_owned();
        s.levels(Parameters(LevelsParams {
            action: Some("select".into()),
            id: Some(ground),
            ..LevelsParams::default()
        }))
        .unwrap();
        let home: serde_json::Value =
            serde_json::from_str(&s.get_home(Parameters(GetHomeParams::default())).unwrap())
                .unwrap();
        assert_eq!(home["walls"].as_array().unwrap().len(), 4, "{home}");

        let upper = rows[1][0].as_str().unwrap().to_owned();
        s.levels(Parameters(LevelsParams {
            action: Some("delete".into()),
            id: Some(upper),
            ..LevelsParams::default()
        }))
        .unwrap();
        assert_eq!(
            s.document.read().home().walls.len(),
            4,
            "upper walls removed with the storey"
        );
        assert!(
            s.levels(Parameters(LevelsParams {
                action: Some("select".into()),
                id: Some("lv99".into()),
                ..LevelsParams::default()
            }))
            .is_err()
        );
    }
    #[test]
    fn storeys_can_start_above_the_ground() {
        let s = server();
        let reply = s
            .levels(Parameters(LevelsParams {
                action: Some("add".into()),
                elev: Some(55.0),
                ..LevelsParams::default()
            }))
            .unwrap();
        assert!(reply.starts_with("ok"), "{reply}");
        let params: CreateParams =
            serde_json::from_str(r#"{"walls":[{"pts":[[0,0],[300,0]],"h":250}]}"#).unwrap();
        s.create(Parameters(params)).unwrap();
        let doc = s.document.read();
        let home = doc.home();
        let wall = &home.walls[0];
        assert!((home.elevation_of(wall.level) - 55.0).abs() < 1e-9);
        let mesh =
            newera_render::Mesh::from_home(home, &newera_render::Selection::new(), &|_| None);
        let top = mesh
            .vertices
            .iter()
            .map(|v| v.position[1])
            .fold(f32::MIN, f32::max);
        assert!((top - 3.05).abs() < 0.02, "wall top at 55 + 250 cm: {top}");
    }
}
