//! MCP tool surface. Each tool is a thin adapter over [`crate::edit`] or
//! `newera-core`; all of them share the document the editor is showing.
//!
//! One module per domain, each with its own router, its params and its
//! tests. Where a tool lives:
//!
//! | module | tools |
//! |---|---|
//! | [`read`] | `home`, `materials`, `catalog` |
//! | [`elements`] | `create`, `update`, `delete`, `move`, `edit_walls` |
//! | [`furniture`] | `place`, `arrange` |
//! | [`joinery`] | `joinery`, `cut_list` |
//! | [`cabinets`] | `cabinet_run`, `embed` |
//! | [`roof`] | `fit_roof` |
//! | [`lighting`] | `lighting`, `edit_lighting` |
//! | [`render`] | `render_plan`, `show_plan`, `render_3d`, `render_photo`, `export` |
//! | [`cameras`] | `cameras`, `edit_cameras`, `video`, `edit_video` |
//! | [`measure`] | `measure` |
//! | [`check`] | `layout`, `ergonomics`, `accept` |
//! | [`annotations`] | `annotations`, `edit_annotations`, `disciplines`, `edit_disciplines` |
//! | [`background`] | `background`, `edit_background` |
//! | [`levels`] | `levels`, `edit_levels` |
//! | [`project`] | `file`, `edit_home`, `history`, `edit_history`, `sessions`, `plugins`, `run_plugin`, `variants`, `edit_variants` |
//! | [`feedback`] | `feedback` |
//! | [`electrical`] | `electrical`, `edit_electrical` |
//! | [`plumbing`] | `plumbing`, `edit_plumbing` |
//! | [`rules`] | `rules` |
//! | [`reply`] | no tools: what every write needs to answer |
//!
//! A new tool goes in the domain it belongs to, and its router joins
//! `parts` in [`NewEraMcp::new`]. Two domains claiming one name would
//! overwrite in silence, so the count is asserted there and the whole
//! surface is frozen by `tool_surface_matches_snapshot`.

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
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
pub(crate) mod rules;

const INSTRUCTIONS: &str = "\
Home design editor, live in the user's window. Units: cm. Plan axes: x right, y down. \
Points are [x,y]. Id prefixes: w wall, r room, d dimension, t label, f furniture/door/window, lv storey. \
All kinds share one id counter, and composite pieces (roofs, joinery, cabinet runs) also number their \
parts, so ids have gaps: use the ids a reply returns, never guess the next one. \
Reads omit defaults (wall t=15 h=250). Writes reply `ok rev=N [ids=...]`; don't re-read \
unless needed. Every change is one undoable step. \
Tools, by what they are for — a read is a noun, the tool that changes it is edit_<noun>: \
project: home, file, edit_home, history, edit_history, variants, edit_variants, levels, edit_levels, sessions; \
drawing: create, update, delete, move, edit_walls, place, arrange, catalog, materials, measure, fit_roof, \
background, edit_background; \
joinery: joinery, cabinet_run, embed, cut_list; \
reviews: layout, ergonomics, electrical, plumbing, lighting, accept, rules; \
projects on the plan: edit_electrical, edit_plumbing, edit_lighting, annotations, edit_annotations, disciplines, \
edit_disciplines; \
images and files: render_plan, render_3d, render_photo, show_plan, cameras, edit_cameras, video, edit_video, export; \
plugins, run_plugin, feedback. \
Furniture has a front (seat, doors, foot of the bed; catalog names it): place facing= says which \
way it looks, or wall=<id> puts its back on a wall; layout lists pieces turned to face a wall as backwards. \
Spots, panels and pendants are kept on the ceiling for you: a pendant takes elev (shade height) or h (drop). \
render_plan shows the plan to you. A project can hold several plan versions; tools act on the active one. \
Finishes are short strings: `#rrggbb` paint, a pattern like `tiles #ffffff 60x60 r45` \
(tint, tile cm, rotation) or `img:path 90x90`; `none` clears. \
When a tool answers less than you asked, makes you take a detour, or leads you to a wrong conclusion \
before the right one, report it with feedback as it happens, with the whole case (the call, the literal \
reply, what was true, what it cost, the change that would help and what must not get worse) — then carry on.";

/// The arguments of a read that takes none. Anything given is refused by
/// name: a write's argument sent to its read would otherwise be dropped in
/// silence, and the agent would think it had changed something.
#[derive(Debug, Default, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Nothing {}

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
/// surface is frozen by `tool_surface_matches_snapshot`.
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
            Self::rules_router(),
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
            crate::hints::apply(&mut route.attr);
            crate::output::apply(&mut route.attr);
            crate::app::link(&mut route.attr);
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

    /// The first argument `name` was given that its schema does not declare,
    /// as the sentence an agent needs to fix its call. `None` for a tool that
    /// is not there: that is the router's to say.
    pub(crate) fn unknown_argument(&self, name: &str, args: &serde_json::Value) -> Option<String> {
        let route = self.tool_router.map.get(name)?;
        let schema = serde_json::Value::Object((*route.attr.input_schema).clone());
        crate::args::unknown(&schema, args)
    }
}

/// A call the tool refused, as a result the agent reads and acts on — not a
/// protocol error, which a client may show to nobody. The reason is the one
/// the tool gave, with what to do instead.
fn refused(message: impl Into<String>) -> rmcp::model::CallToolResponse {
    rmcp::model::CallToolResponse::Complete(rmcp::model::CallToolResult::error(vec![
        rmcp::model::ContentBlock::text(message.into()),
    ]))
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
        let args = serde_json::Value::Object(request.arguments.clone().unwrap_or_default());
        if let Some(why) = self.unknown_argument(&request.name, &args) {
            return Ok(refused(why));
        }
        let known = self.tool_router.map.contains_key(request.name.as_ref());
        #[cfg(not(target_arch = "wasm32"))]
        let answer = if matches!(
            request.name.as_ref(),
            "render_photo" | "render_plan" | "render_3d" | "edit_video"
        ) {
            native_job::run(self.clone(), request, context).await
        } else {
            let call = rmcp::handler::server::tool::ToolCallContext::new(self, request, context);
            self.tool_router.call(call).await
        };
        #[cfg(target_arch = "wasm32")]
        let answer = {
            let call = rmcp::handler::server::tool::ToolCallContext::new(self, request, context);
            self.tool_router.call(call).await
        };
        // Arguments that do not fit, and a tool that refuses what it was
        // asked, are answered inside the result (the protocol's own advice),
        // so the agent sees why and tries again. An unknown tool stays a
        // protocol error.
        let answer = match answer {
            Err(e) if known && e.code == rmcp::model::ErrorCode::INVALID_PARAMS => {
                Ok(refused(e.message.into_owned()))
            }
            other => other,
        };
        answer.map(|response| match response {
            rmcp::model::CallToolResponse::Complete(mut result) => {
                crate::output::structure(&mut result);
                rmcp::model::CallToolResponse::Complete(result)
            }
            other => other,
        })
    }

    async fn list_resources(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::ListResourcesResult, rmcp::ErrorData> {
        serde_json::from_value(crate::app::resource_list())
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))
    }

    async fn read_resource(
        &self,
        request: rmcp::model::ReadResourceRequestParams,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::ReadResourceResponse, rmcp::ErrorData> {
        let found = crate::app::read(&request.uri).ok_or_else(|| {
            rmcp::ErrorData::resource_not_found(
                format!(
                    "no resource {}; resources/list names the ones there are",
                    request.uri
                ),
                None,
            )
        })?;
        serde_json::from_value(found)
            .map(rmcp::model::ReadResourceResponse::Complete)
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))
    }

    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_server_info(
            Implementation::new("3d-new-era-ai", env!("CARGO_PKG_VERSION"))
                .with_title("3D New Era AI")
                .with_description(crate::output::DESCRIPTION)
                .with_website_url(crate::output::WEBSITE)
                .with_icons(crate::output::icons()),
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

/// The sources behind a reply, resolved once, with the letter each one is
/// worth **at `at`** — the same standard obliges here and informs there, so a
/// citation without its place is a citation that cannot be trusted.
pub(crate) fn sources(codes: &[&str], at: &newera_core::Place) -> serde_json::Value {
    let map: serde_json::Map<String, serde_json::Value> = codes
        .iter()
        .filter_map(|c| newera_core::standards::standard(c))
        .map(|r| {
            (
                r.code.to_owned(),
                serde_json::json!([r.title, r.force(at).letter(), r.url]),
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
