//! MCP tool surface. Each tool is a thin adapter over [`crate::edit`] or
//! `newera-core`; all of them share the document the editor is showing.
//!
//! One module per domain, each with its own router, its params and its
//! tests. Where a tool lives:
//!
//! | module | tools |
//! |---|---|
//! | [`read`] | `get_home`, `materials`, `catalog` |
//! | [`elements`] | `create`, `update`, `delete`, `move`, `split_wall`, `merge_walls` |
//! | [`furniture`] | `place`, `arrange` |
//! | [`joinery`] | `joinery`, `cut_list` |
//! | [`cabinets`] | `cabinet_run`, `embed` |
//! | [`roof`] | `fit_roof` |
//! | [`lighting`] | `lighting` |
//! | [`render`] | `render_plan`, `render_3d`, `render_photo`, `export_plan` |
//! | [`cameras`] | `cameras`, `video` |
//! | [`measure`] | `measure` |
//! | [`check`] | `check_layout`, `ergonomics` |
//! | [`annotations`] | `annotations`, `disciplines` |
//! | [`background`] | `set_background`, `trace_background` |
//! | [`levels`] | `levels` |
//! | [`project`] | `save_home`, `open_home`, `new_home`, `set_home`, `undo`, `redo`, `checkpoint`, `sessions`, `plugins`, `variants` |
//! | [`feedback`] | `feedback` |
//! | [`electrical`] | `electrical` |
//! | [`plumbing`] | `plumbing` |
//! | [`reply`] | no tools: what every write needs to answer |
//!
//! A new tool goes in the domain it belongs to, and its router joins
//! `parts` in [`NewEraMcp::new`]. Two domains claiming one name would
//! overwrite in silence, so the count is asserted there and the whole
//! surface is frozen by `tool_surface_is_unchanged`.

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::{ServerHandler, tool_handler};

use newera_core::SharedDocument;

mod annotations;
mod background;
mod cabinets;
mod cameras;
mod check;
mod electrical;
mod elements;
mod feedback;
mod furniture;
mod joinery;
mod levels;
mod lighting;
mod measure;
#[cfg(not(target_arch = "wasm32"))]
mod native_job;
mod plumbing;
mod project;
mod read;
mod render;
mod reply;
mod roof;

const INSTRUCTIONS: &str = "\
Home design editor, live in the user's window. Units: cm. Plan axes: x right, y down. \
Points are [x,y]. Id prefixes: w wall, r room, d dimension, t label, f furniture/door/window, lv storey. \
All kinds share one id counter, and composite pieces (roofs, joinery, cabinet runs) also number their \
parts, so ids have gaps: use the ids a reply returns, never guess the next one. \
Reads omit defaults (wall t=15 h=250). Writes reply `ok rev=N [ids=...]`; don't re-read \
unless needed. Every change is one undoable step. Use render_plan to check visually. \
A project can hold several plan versions (variants tool); tools act on the active one. \
Finishes are short strings: `#rrggbb` paint, a pattern like `tiles #ffffff 60x60 r45` \
(tint, tile cm, rotation) or `img:path 90x90`; `none` clears. Wall types and patterns: materials tool. \
When a tool answers less than you asked, makes you take a detour, or leads you to a wrong conclusion \
before the right one, report it with the feedback tool as it happens, with the whole case (the call, the literal \
reply, what was true, what it cost, the change that would help and what must not get worse) — then carry on.";

/// The MCP server. Cheap to clone: it only holds a handle to the document.
#[derive(Debug, Clone)]
pub struct NewEraMcp {
    document: SharedDocument,
    tool_router: ToolRouter<Self>,
}

/// The MCP server, assembled from one router per domain.
///
/// A new tool means a `#[tool]` in a domain module and its router in
/// `parts`. Two domains claiming one name would overwrite in silence — the
/// merge is a map insert — so the count is checked here, and the whole
/// surface is frozen by `tool_surface_is_unchanged`.
impl NewEraMcp {
    pub fn new(document: SharedDocument) -> Self {
        let parts = [
            Self::levels_router(),
            Self::measure_router(),
            Self::check_router(),
            Self::background_router(),
            Self::roof_router(),
            Self::lighting_router(),
            Self::annotations_router(),
            Self::cameras_router(),
            Self::project_router(),
            Self::read_router(),
            Self::render_router(),
            Self::joinery_router(),
            Self::cabinets_router(),
            Self::furniture_router(),
            Self::elements_router(),
            Self::feedback_router(),
            Self::electrical_router(),
            Self::plumbing_router(),
        ];
        let expected: usize = parts.iter().map(|r| r.map.len()).sum();
        let mut tool_router = parts
            .into_iter()
            .fold(ToolRouter::new(), |all, part| all + part);
        debug_assert_eq!(
            tool_router.map.len(),
            expected,
            "two domains registered the same tool name"
        );
        for route in tool_router.map.values_mut() {
            let mut schema = serde_json::Value::Object((*route.attr.input_schema).clone());
            crate::schema::compact(&mut schema);
            if let serde_json::Value::Object(map) = schema {
                route.attr.input_schema = std::sync::Arc::new(map);
            }
        }
        Self {
            document,
            tool_router,
        }
    }

    /// The tools this server offers, with their schemas — the same list the
    /// handshake hands out, for whoever has no handshake to ask.
    pub fn tools(&self) -> Vec<rmcp::model::Tool> {
        self.tool_router.list_all()
    }
}

// The macro generates async trait methods that resolve immediately.
#[allow(clippy::unused_async_trait_impl)]
#[tool_handler(router = self.tool_router)]
impl ServerHandler for NewEraMcp {
    async fn call_tool(
        &self,
        request: rmcp::model::CallToolRequestParams,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::CallToolResponse, rmcp::ErrorData> {
        #[cfg(not(target_arch = "wasm32"))]
        if matches!(
            request.name.as_ref(),
            "render_photo" | "render_plan" | "render_3d" | "video"
        ) {
            return native_job::run(self.clone(), request, context).await;
        }
        let call = rmcp::handler::server::tool::ToolCallContext::new(self, request, context);
        self.tool_router.call(call).await
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new("3d-new-era-ai", env!("CARGO_PKG_VERSION"))
                    .with_title("3D New Era AI"),
            )
            .with_instructions(INSTRUCTIONS)
    }
}

/// The sources behind a reply, resolved once: `{code: [title, tier, url]}`.
/// A reply cites a standard by its short code and pays for the title here,
/// not inside every sentence. Named `sources`, not `refs`: the annotations
/// tool already calls its room reference tags `refs`, and one word with two
/// meanings on the same surface costs an agent a wrong guess.
/// Orphaned acceptances as rows, each with the live finding that probably
/// took its place: the same thing — the part after the rule — under
/// another key, when there is one. `[key, reason]` or `[key, reason, successor]`.
pub(crate) fn orphan_rows(
    orphaned: Vec<(String, String)>,
    live: &[String],
) -> Vec<serde_json::Value> {
    let tail = |k: &str| k.rsplit(':').next().unwrap_or(k).to_owned();
    let after_rule = |k: &str| k.split_once(':').map(|(_, rest)| rest.to_owned());
    orphaned
        .into_iter()
        .map(|(key, why)| {
            let successor = live
                .iter()
                .find(|l| {
                    **l != key && after_rule(l).is_some() && after_rule(l) == after_rule(&key)
                })
                .or_else(|| {
                    live.iter().find(|l| {
                        **l != key
                            && tail(l) == tail(&key)
                            && l.split(':').next() == key.split(':').next()
                    })
                });
            match successor {
                Some(s) => serde_json::json!([key, why, s]),
                None => serde_json::json!([key, why]),
            }
        })
        .collect()
}

pub(crate) fn sources(codes: &[&str]) -> serde_json::Value {
    let map: serde_json::Map<String, serde_json::Value> = codes
        .iter()
        .filter_map(|c| newera_core::standards::standard(c))
        .map(|r| {
            (
                r.code.to_owned(),
                serde_json::json!([r.title, r.tier.letter(), r.url]),
            )
        })
        .collect();
    serde_json::Value::Object(map)
}

#[cfg(test)]
fn server() -> NewEraMcp {
    NewEraMcp::new(SharedDocument::new(newera_core::Document::default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool_surface() -> std::collections::BTreeMap<String, serde_json::Value> {
        server()
            .tool_router
            .list_all()
            .into_iter()
            .map(|tool| (tool.name.to_string(), serde_json::to_value(tool).unwrap()))
            .collect()
    }

    /// Compare complete schemas and descriptions, including equal-length
    /// changes. Separate assertions identify the tool whose contract changed.
    #[test]
    fn tool_surface_matches_snapshot() {
        let expected: std::collections::BTreeMap<String, serde_json::Value> =
            serde_json::from_str(include_str!("../../tests/fixtures/tool-surface.json")).unwrap();
        let actual = tool_surface();
        assert_eq!(
            actual.keys().collect::<Vec<_>>(),
            expected.keys().collect::<Vec<_>>(),
            "the set of tools changed; review tests/fixtures/tool-surface.json"
        );
        for (name, tool) in actual {
            assert_eq!(
                tool, expected[&name],
                "MCP contract changed for {name}; review tests/fixtures/tool-surface.json"
            );
        }
    }

    /// Explicit maintenance command, never run by normal tests or CI.
    #[test]
    #[ignore = "rewrites the tool contract snapshot; review the resulting git diff"]
    fn update_tool_surface_snapshot() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/tool-surface.json");
        let json = serde_json::to_string_pretty(&tool_surface()).unwrap();
        std::fs::write(path, format!("{json}\n")).unwrap();
    }

    #[test]
    #[ignore = "prints the size of the tool list"]
    fn tool_list_size() {
        let tools = server().tool_router.list_all();
        let json = serde_json::to_string(&tools).unwrap();
        println!(
            "{} tools, {} bytes (~{} tokens)",
            tools.len(),
            json.len(),
            json.len() / 4
        );
        let mut sizes: Vec<(usize, String)> = tools
            .iter()
            .map(|t| (serde_json::to_string(t).unwrap().len(), t.name.to_string()))
            .collect();
        sizes.sort();
        for (n, name) in sizes.iter().rev() {
            println!("{n:6} {name}");
        }
        let update = tools.iter().find(|t| t.name == "update").unwrap();
        println!("{}", serde_json::to_string(&update.input_schema).unwrap());
    }

    #[test]
    fn an_orphan_points_to_the_finding_that_took_its_place() {
        let live = vec![
            "nbr15575g:f807:livres-frente".to_owned(),
            "plumb:vent-far:f1601".to_owned(),
        ];
        let rows = super::orphan_rows(
            vec![
                ("-:f807:livres-frente".to_owned(), "motivo".to_owned()),
                ("plumb:vent:f1601".to_owned(), "outro".to_owned()),
                ("plumb:grease".to_owned(), "prédio".to_owned()),
            ],
            &live,
        );
        assert_eq!(rows[0][2], "nbr15575g:f807:livres-frente");
        assert_eq!(rows[1][2], "plumb:vent-far:f1601");
        assert!(rows[2].get(2).is_none());
    }
}
