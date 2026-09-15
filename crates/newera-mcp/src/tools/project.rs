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
    #[tool(description = "Rename the project and/or set the compass (north).")]
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
        {
            let c = doc.home().compass.clone();
            commands.push(Command::SetCompass {
                compass: Compass {
                    center: p.compass_at.unwrap_or(c.center),
                    diameter: p.compass_d.unwrap_or(c.diameter),
                    north_degrees: p.north.unwrap_or(c.north_degrees),
                    visible: p.compass_visible.unwrap_or(c.visible),
                    ..c.clone()
                },
            });
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
    #[tool(description = "Undo the last change, whoever made it.")]
    pub(crate) fn undo(&self) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        doc.undo().map_err(core)?;
        Ok(ok(&doc, &[]))
    }
    #[tool(description = "Redo the last undone change.")]
    pub(crate) fn redo(&self) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        doc.redo().map_err(core)?;
        Ok(ok(&doc, &[]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::CreateParams;
    use crate::tools::server;

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
}
