//! The project as an object: open it, save it, name it, version it, undo it,
//! and see who else is editing.

use std::path::PathBuf;

use newera_core::{Command, Compass, Home, Point2};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ErrorData, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use super::NewEraMcp;
use super::reply::{core, invalid, ok};
use crate::compact;

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct SetHomeParams {
    name: Option<String>,
    /// Clockwise degrees from plan up to north.
    north: Option<f64>,
    compass_at: Option<Point2>,
    /// Compass diameter cm.
    compass_d: Option<f64>,
    compass_visible: Option<bool>,
    /// City whose building code applies, e.g. `sao-paulo`; `""` clears it.
    /// Kept with the project, so `ergonomics`, `check_layout` and every dry
    /// run weigh the same municipal rules.
    city: Option<String>,
    /// Project properties to set, `{key: "value"}`, or remove, `{key: null}`
    /// — what an import leaves behind (window sizes, panel dividers, ids of
    /// the program it came from). With `level`, that storey's instead.
    properties: Option<serde_json::Map<String, serde_json::Value>>,
    /// Storey whose properties `properties` changes, e.g. `lv2`.
    level: Option<String>,
}
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct PluginsParams {
    /// `list` (default) or `run`.
    action: Option<String>,
    name: Option<String>,
    /// Arguments passed to the plugin as JSON.
    args: Option<serde_json::Value>,
}
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct VariantsParams {
    /// `list` (default), `duplicate` (copy active), `new` (empty), `switch`, `rename`, `delete`.
    action: Option<String>,
    /// Variant index for switch/rename/delete.
    i: Option<usize>,
    /// Name for duplicate/new/rename.
    name: Option<String>,
}
/// A point in the work, by name.
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct CheckpointParams {
    /// `list` (default), `checkpoint` (remember here), `revert` (go back).
    action: Option<String>,
    label: Option<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct PathParams {
    /// Project file (`.newera`). Optional for save when already saved once.
    path: Option<String>,
}
fn with_extension(path: PathBuf) -> PathBuf {
    if path.extension().is_some() {
        path
    } else {
        path.with_extension(newera_core::PROJECT_EXTENSION)
    }
}
#[tool_router(router = project_router, vis = "pub(crate)")]
impl NewEraMcp {
    #[tool(
        description = "Rename the project, set or remove project properties (properties {key: value|null}, or a storey's with level), set the compass (north), and set the city whose building code applies (city=sao-paulo). The city belongs to the project: ergonomics, check_layout and every dry run then weigh the same municipal rules, so a change can be tested against the score it moves."
    )]
    pub(crate) fn set_home(
        &self,
        Parameters(p): Parameters<SetHomeParams>,
    ) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        let mut commands = Vec::new();
        if let Some(name) = p.name {
            commands.push(Command::RenameHome { name });
        }
        if p.north.is_some()
            || p.compass_at.is_some()
            || p.compass_d.is_some()
            || p.compass_visible.is_some()
            || p.city.is_some()
        {
            let c = doc.home().compass.clone();
            commands.push(Command::SetCompass {
                compass: Compass {
                    center: p.compass_at.unwrap_or(c.center),
                    diameter: p.compass_d.unwrap_or(c.diameter),
                    north_degrees: p.north.unwrap_or(c.north_degrees),
                    visible: p.compass_visible.unwrap_or(c.visible),
                    city: match p.city.as_deref().map(str::trim) {
                        Some("") => None,
                        Some(city) => Some(city.to_owned()),
                        None => c.city.clone(),
                    },
                    ..c.clone()
                },
            });
        }
        if let Some(changes) = &p.properties {
            let apply = |target: &mut newera_core::Properties| -> Result<(), ErrorData> {
                for (key, value) in changes {
                    match value {
                        serde_json::Value::Null => {
                            target.remove(key);
                        }
                        serde_json::Value::String(text) => {
                            target.insert(key.clone(), text.clone());
                        }
                        other => {
                            target.insert(key.clone(), other.to_string());
                        }
                    }
                }
                Ok(())
            };
            if let Some(raw) = &p.level {
                let id: newera_core::LevelId =
                    raw.parse().map_err(|e| invalid(format!("level: {e}")))?;
                let mut level = doc
                    .home()
                    .level(id)
                    .cloned()
                    .ok_or_else(|| invalid(format!("no storey {raw}")))?;
                apply(&mut level.properties)?;
                commands.push(Command::update(level));
            } else {
                let mut properties = doc.home().properties.clone();
                apply(&mut properties)?;
                commands.push(Command::SetProperties { properties });
            }
        }
        if commands.is_empty() {
            return Err(invalid("nothing to change"));
        }
        doc.execute(Command::Batch { commands }).map_err(core)?;
        Ok(ok(&doc, &[]))
    }
    #[tool(description = "Save the project (.newera). path optional after the first save.")]
    pub(crate) fn save_home(
        &self,
        Parameters(p): Parameters<PathParams>,
    ) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        let path = match (p.path, doc.path()) {
            (Some(path), _) => with_extension(PathBuf::from(path)),
            (None, Some(path)) => path.to_path_buf(),
            (None, None) => return Err(invalid("`path` is required for the first save")),
        };
        newera_core::save_project(&doc, &path)
            .map_err(|e| invalid(format!("cannot write {}: {e}", path.display())))?;
        doc.mark_saved(&path);
        Ok(format!("ok {}", path.display()))
    }
    #[tool(
        description = "Open a project (.newera) or import a Sweet Home 3D file (.sh3d), replacing the current one."
    )]
    pub(crate) fn open_home(
        &self,
        Parameters(p): Parameters<PathParams>,
    ) -> Result<String, ErrorData> {
        let path = PathBuf::from(p.path.ok_or_else(|| invalid("`path` is required"))?);
        let mut doc = self.document.write();
        let opened = newera_sh3d::open_file(&mut doc, &path).map_err(invalid)?;
        let mut reply = ok(&doc, &[]);
        if opened.imported {
            reply.push_str(" imported (unsaved)");
        }
        for warning in opened.warnings {
            reply.push_str("\nwarning: ");
            reply.push_str(&warning);
        }
        Ok(reply)
    }
    #[tool(description = "Start a new empty project.")]
    pub(crate) fn new_home(&self) -> String {
        let mut doc = self.document.write();
        doc.load(Home::default());
        doc.set_path(None);
        doc.set_asset_dir(None);
        ok(&doc, &[])
    }
    #[tool(
        description = "Plugins (external programs editing through the HTTP API). list (default): rows [name,title,description]. run {name,args?}: {ok,code,stdout,stderr,edits,revision}."
    )]
    pub(crate) fn plugins(
        &self,
        Parameters(p): Parameters<PluginsParams>,
    ) -> Result<String, ErrorData> {
        let dirs = newera_plugins::plugin_dirs();
        match p.action.as_deref().unwrap_or("list") {
            "list" => {
                let rows: Vec<serde_json::Value> = newera_plugins::discover(&dirs)
                    .iter()
                    .map(|p| serde_json::json!([p.name, p.title, p.description]))
                    .collect();
                Ok(serde_json::json!({ "rows": rows }).to_string())
            }
            "run" => {
                let name = p
                    .name
                    .as_deref()
                    .ok_or_else(|| invalid("`name` is required"))?;
                let args = p.args.unwrap_or(serde_json::Value::Null);
                newera_plugins::run_for_document(&self.document, &dirs, name, &args)
                    .map(|v| v.to_string())
                    .map_err(|e| invalid(e.to_string()))
            }
            other => Err(invalid(format!("unknown action `{other}`"))),
        }
    }
    #[tool(
        description = "People and agents on this project now: rows [id,name,cursor,selection,edits]."
    )]
    pub(crate) fn sessions(&self) -> String {
        let mut doc = self.document.write();
        doc.sessions_mut().expire(newera_core::collab::now_ms());
        let rows: Vec<serde_json::Value> = doc
            .sessions()
            .list()
            .iter()
            .map(|s| {
                serde_json::json!([
                    s.id,
                    s.name,
                    s.cursor.map(|c| [compact::num(c.x), compact::num(c.y)]),
                    s.selection
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>(),
                    s.edits
                ])
            })
            .collect();
        serde_json::json!({ "rev": doc.revision(), "rows": rows }).to_string()
    }
    #[tool(
        description = "Plan versions (tabs). list: rows [i,name,active,walls,rooms,m2,furniture,issues]. duplicate/new switch to the new one; edits apply to the active version."
    )]
    pub(crate) fn variants(
        &self,
        Parameters(p): Parameters<VariantsParams>,
    ) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        let need = |i: Option<usize>| i.ok_or_else(|| invalid("`i` is required"));
        match p.action.as_deref().unwrap_or("list") {
            "list" => Ok(compact::variants(&doc).to_string()),
            "duplicate" | "new" => {
                let index = doc.add_variant(p.name, p.action.as_deref() == Some("duplicate"));
                Ok(format!("ok rev={} v={index} i={index}", doc.revision()))
            }
            "switch" => {
                doc.switch_variant(need(p.i)?).map_err(core)?;
                Ok(ok(&doc, &[]))
            }
            "rename" => {
                let name = p.name.ok_or_else(|| invalid("`name` is required"))?;
                doc.rename_variant(need(p.i)?, name).map_err(core)?;
                Ok(ok(&doc, &[]))
            }
            "delete" => {
                doc.remove_variant(need(p.i)?).map_err(core)?;
                Ok(ok(&doc, &[]))
            }
            other => Err(invalid(format!("unknown action `{other}`"))),
        }
    }
    #[tool(
        description = "Name where the plan is now, and come back to it. checkpoint {label} remembers this point; revert {label} undoes back down to it, keeping every id — which duplicating a version cannot do, since a copy renumbers. list (default) shows what is remembered and how many changes ago it was. A checkpoint lives with the project and survives saving."
    )]
    pub(crate) fn checkpoint(
        &self,
        Parameters(p): Parameters<CheckpointParams>,
    ) -> Result<String, ErrorData> {
        const KEY: &str = "checkpoint:";
        let action = p.action.as_deref().unwrap_or("list");
        let mut doc = self.document.write();
        let depth = doc.undo_depth();
        let mut properties = doc.home().properties.clone();
        match action {
            "list" => {
                let rows: Vec<serde_json::Value> = properties
                    .iter()
                    .filter_map(|(key, value)| {
                        let label = key.strip_prefix(KEY)?;
                        let at: usize = value.parse().ok()?;
                        Some(serde_json::json!([label, depth.saturating_sub(at)]))
                    })
                    .collect();
                Ok(serde_json::json!({ "checkpoints": rows }).to_string())
            }
            "checkpoint" | "set" => {
                let label = p.label.ok_or_else(|| invalid("`label` is required"))?;
                // Writing the checkpoint is itself a change: the point to
                // come back to is the one just after it, so coming back does
                // not undo the checkpoint along with the work.
                properties.insert(format!("{KEY}{label}"), (depth + 1).to_string());
                doc.execute(Command::SetProperties { properties })
                    .map_err(core)?;
                Ok(ok(&doc, &[]))
            }
            "revert" => {
                let label = p.label.ok_or_else(|| invalid("`label` is required"))?;
                let at: usize = properties
                    .get(&format!("{KEY}{label}"))
                    .and_then(|v| v.parse().ok())
                    .ok_or_else(|| invalid(format!("no checkpoint `{label}`")))?;
                if at > depth {
                    return Err(invalid(format!(
                        "`{label}` is ahead of where the plan is: nothing to undo"
                    )));
                }
                for _ in 0..(depth - at) {
                    doc.undo().map_err(core)?;
                }
                Ok(ok(&doc, &[]))
            }
            other => Err(invalid(format!("unknown action `{other}`"))),
        }
    }
    #[tool(description = "Undo the last change, whoever made it.")]
    pub(crate) fn undo(&self) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        let before = doc.home().clone();
        doc.undo().map_err(core)?;
        // What was undone, named: an undo that takes the plan's reference
        // numbers with it is otherwise found only in a render.
        Ok(super::reply::applied(&doc, &before))
    }
    #[tool(description = "Redo the last undone change.")]
    pub(crate) fn redo(&self) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        let before = doc.home().clone();
        doc.redo().map_err(core)?;
        Ok(super::reply::applied(&doc, &before))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::CreateParams;
    use crate::tools::server;

    #[test]
    fn an_import_is_cleaned_by_rule_not_one_element_at_a_time() {
        let s = server();
        {
            let mut doc = s.document.write();
            let part = |id: u64, name: &str| newera_core::Furniture {
                id: newera_core::FurnitureId(id),
                catalog: "box".into(),
                name: name.to_owned(),
                width: 30.0,
                depth: 30.0,
                height: 30.0,
                ..newera_core::Furniture::default()
            };
            let mut tower = part(1, "12 — Torre quente");
            tower.children = vec![
                part(2, "48 — Gabinete do tanque 80,5 cm"),
                part(3, "puxador"),
            ];
            doc.execute(Command::Batch {
                commands: vec![
                    Command::insert(tower),
                    Command::insert(part(4, "Mesa Dover")),
                    Command::SetProperties {
                        properties: [
                            (
                                "com.eteks.sweethome3d.SweetHome3D.FrameX".to_owned(),
                                "40".to_owned(),
                            ),
                            ("keep".to_owned(), "yes".to_owned()),
                        ]
                        .into(),
                    },
                ],
            })
            .unwrap();
            for text in ["[09]", "[10]", "Vidro canelado"] {
                let label = newera_core::Label {
                    id: doc.new_label_id(),
                    text: text.to_owned(),
                    ..newera_core::Label::default()
                };
                doc.execute(Command::insert(label)).unwrap();
            }
        }
        // The index codes, found by pattern.
        let found: serde_json::Value = serde_json::from_str(
            &s.annotations(Parameters(
                serde_json::from_str(r#"{"q":"re:^\\[\\d+\\]$"}"#).unwrap(),
            ))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(found["labels"].as_array().unwrap().len(), 2, "{found}");

        // The numbered prefixes go in one call, parts of groups included.
        let rename = |json: &str| s.update(Parameters(serde_json::from_str(json).unwrap()));
        let dry: serde_json::Value = serde_json::from_str(
            &rename(r#"{"rename":{"pattern":"^\\d+ — ","to":""},"dry":true}"#).unwrap(),
        )
        .unwrap();
        assert_eq!(dry["changed"].as_array().unwrap().len(), 2, "{dry}");
        rename(r#"{"rename":{"pattern":"^\\d+ — ","to":""}}"#).unwrap();
        let names: Vec<String> = {
            let doc = s.document.read();
            doc.home()
                .furniture
                .iter()
                .flat_map(newera_core::Furniture::flatten)
                .map(|f| f.name.clone())
                .collect()
        };
        assert_eq!(
            names,
            [
                "Torre quente",
                "Gabinete do tanque 80,5 cm",
                "puxador",
                "Mesa Dover"
            ]
        );
        let nothing = rename(r#"{"rename":{"pattern":"^\\d+ — ","to":""}}"#).unwrap_err();
        assert!(nothing.message.contains("nothing matches"), "{nothing:?}");

        // The properties an import left, removed by name.
        s.set_home(Parameters(
            serde_json::from_str(
                r#"{"properties":{"com.eteks.sweethome3d.SweetHome3D.FrameX":null,"source":"limpo"}}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        let doc = s.document.read();
        let props = &doc.home().properties;
        assert!(
            !props.contains_key("com.eteks.sweethome3d.SweetHome3D.FrameX"),
            "{props:?}"
        );
        assert_eq!(props.get("keep").map(String::as_str), Some("yes"));
        assert_eq!(props.get("source").map(String::as_str), Some("limpo"));
    }

    #[test]
    fn an_undo_that_switches_the_plans_annotations_off_says_so() {
        let s = server();
        s.annotations(Parameters(
            serde_json::from_str(r#"{"refs":true}"#).unwrap(),
        ))
        .unwrap();
        let reply = s.undo().unwrap();
        let diff: serde_json::Value =
            serde_json::from_str(&reply[reply.find('{').expect(&reply)..]).unwrap();
        assert_eq!(diff["annotations"]["from"]["refs"], true, "{reply}");
        assert_eq!(diff["annotations"]["to"]["refs"], false, "{reply}");
        let reply = s.redo().unwrap();
        assert!(
            reply.contains(r#""to":{"details":false,"dims":false,"legend":false,"refs":true}"#),
            "{reply}"
        );
    }

    #[test]
    fn variants_duplicate_switch_and_list() {
        let s = server();
        let params: CreateParams =
            serde_json::from_str(r#"{"walls":[{"pts":[[0,0],[300,0]]}]}"#).unwrap();
        s.create(Parameters(params)).unwrap();
        let reply = s
            .variants(Parameters(VariantsParams {
                action: Some("duplicate".into()),
                name: Some("B".into()),
                i: None,
            }))
            .unwrap();
        assert!(reply.ends_with("i=1"), "{reply}");
        let params: CreateParams =
            serde_json::from_str(r#"{"walls":[{"pts":[[0,100],[300,100]]}]}"#).unwrap();
        s.create(Parameters(params)).unwrap();
        let list = s.variants(Parameters(VariantsParams::default())).unwrap();
        assert_eq!(
            list,
            r#"[[0,"Versão 1",false,1,0,0.0,0,0],[1,"B",true,2,0,0.0,0,0]]"#
        );
        s.variants(Parameters(VariantsParams {
            action: Some("switch".into()),
            i: Some(0),
            name: None,
        }))
        .unwrap();
        assert_eq!(s.document.read().home().walls.len(), 1);
        assert!(
            s.variants(Parameters(VariantsParams {
                action: Some("switch".into()),
                i: Some(9),
                name: None
            }))
            .is_err()
        );
    }
    #[test]
    fn save_open_round_trip() {
        let s = server();
        let dir = std::env::temp_dir().join(format!("newera-mcp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("casa");
        let params: CreateParams =
            serde_json::from_str(r#"{"labels":[{"text":"Oi","at":[1,2]}]}"#).unwrap();
        s.create(Parameters(params)).unwrap();
        let reply = s
            .save_home(Parameters(PathParams {
                path: Some(path.display().to_string()),
            }))
            .unwrap();
        assert!(reply.ends_with("casa.newera"), "{reply}");
        s.new_home();
        assert!(s.document.read().home().labels.is_empty());
        s.open_home(Parameters(PathParams {
            path: Some(dir.join("casa.newera").display().to_string()),
        }))
        .unwrap();
        assert_eq!(s.document.read().home().labels[0].text, "Oi");
        assert!(!s.document.read().is_modified());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn plugins_and_sessions_tools() {
        let s = server();
        let list: serde_json::Value =
            serde_json::from_str(&s.plugins(Parameters(PluginsParams::default())).unwrap())
                .unwrap();
        assert!(list["rows"].is_array());
        // Without an HTTP server there is nothing for plugins to call back.
        assert!(
            s.plugins(Parameters(PluginsParams {
                action: Some("run".into()),
                ..PluginsParams::default()
            }))
            .is_err()
        );
        s.document
            .write()
            .sessions_mut()
            .join("Ana", newera_core::collab::now_ms());
        let rows: serde_json::Value = serde_json::from_str(&s.sessions()).unwrap();
        assert_eq!(rows["rows"][0][1], "Ana");
    }
    #[test]
    fn writes_name_the_active_variant_once_there_are_several() {
        let s = server();
        let wall = || -> CreateParams {
            serde_json::from_str(r#"{"walls":[{"pts":[[0,0],[100,0]]}]}"#).unwrap()
        };
        assert!(!s.create(Parameters(wall())).unwrap().contains(" v="));
        let reply = s
            .variants(Parameters(VariantsParams {
                action: Some("new".into()),
                ..VariantsParams::default()
            }))
            .unwrap();
        assert!(reply.contains(" v=1 i=1"), "{reply}");
        let reply = s.create(Parameters(wall())).unwrap();
        assert!(reply.contains(" v=1 ids="), "{reply}");
    }
    #[test]
    fn writes_can_name_their_version() {
        let s = server();
        s.variants(Parameters(VariantsParams {
            action: Some("new".into()),
            ..VariantsParams::default()
        }))
        .unwrap();
        // Someone switches back to the first tab meanwhile.
        s.document.write().switch_variant(0).unwrap();
        let params: CreateParams =
            serde_json::from_str(r#"{"v":1,"walls":[{"pts":[[0,0],[100,0]]}]}"#).unwrap();
        let reply = s.create(Parameters(params)).unwrap();
        assert!(reply.contains(" v=1 "), "{reply}");
        let doc = s.document.read();
        assert_eq!(doc.active_variant(), 1);
        assert_eq!(doc.home().walls.len(), 1);
        drop(doc);
        let bad: CreateParams =
            serde_json::from_str(r#"{"v":9,"walls":[{"pts":[[0,0],[100,0]]}]}"#).unwrap();
        assert!(s.create(Parameters(bad)).is_err());
    }

    /// Somewhere to come back to that does not renumber the plan: the reason
    /// a real session edited the user's own drawing instead of a copy.
    #[test]
    fn a_checkpoint_comes_back_without_renumbering_anything() {
        let s = server();
        s.create(Parameters(
            serde_json::from_str(
                r#"{"walls":[{"pts":[[0,0],[400,0],[400,300],[0,300]],"closed":true,"t":15}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        let ids = |s: &NewEraMcp| -> Vec<String> {
            s.document
                .read()
                .home()
                .walls
                .iter()
                .map(|w| w.id.to_string())
                .collect()
        };
        let before = ids(&s);
        let checkpoint = |json: &str| -> serde_json::Value {
            serde_json::from_str(
                &s.checkpoint(Parameters(serde_json::from_str(json).unwrap()))
                    .unwrap(),
            )
            .unwrap_or_default()
        };
        checkpoint(r#"{"action":"checkpoint","label":"banheiros"}"#);

        s.create(Parameters(
            serde_json::from_str(r#"{"walls":[{"pts":[[200,0],[200,300]],"t":10}]}"#).unwrap(),
        ))
        .unwrap();
        s.create(Parameters(
            serde_json::from_str(r#"{"walls":[{"pts":[[0,150],[200,150]],"t":10}]}"#).unwrap(),
        ))
        .unwrap();
        assert_eq!(s.document.read().home().walls.len(), 6);
        let listed = checkpoint(r#"{"action":"list"}"#);
        assert_eq!(listed["checkpoints"][0][0], "banheiros", "{listed}");
        assert_eq!(listed["checkpoints"][0][1], 2, "two changes ago: {listed}");

        checkpoint(r#"{"action":"revert","label":"banheiros"}"#);
        assert_eq!(ids(&s), before, "the same walls, with the same ids");
        // And the checkpoint is still there to come back to again.
        let listed = checkpoint(r#"{"action":"list"}"#);
        assert_eq!(listed["checkpoints"][0][0], "banheiros", "{listed}");
    }
}
