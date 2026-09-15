//! MCP tool surface. Each tool is a thin adapter over [`crate::edit`] or
//! `newera-core`; all of them share the document the editor is showing.

use newera_core::{Command, SharedDocument, ops};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::{ErrorData, ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::edit::{self, CreateParams, UpdateSpec};

mod annotations;
mod background;
mod cabinets;
mod cameras;
mod check;
mod furniture;
mod joinery;
mod levels;
mod lighting;
mod measure;
mod project;
mod read;
mod render;
mod reply;
mod roof;

use reply::{applied, background_scale, core, invalid, ok, on_variant, preview};

const INSTRUCTIONS: &str = "\
Home design editor, live in the user's window. Units: cm. Plan axes: x right, y down. \
Points are [x,y]. Id prefixes: w wall, r room, d dimension, t label, f furniture/door/window, lv storey. \
All kinds share one id counter, and composite pieces (roofs, joinery, cabinet runs) also number their \
parts, so ids have gaps: use the ids a reply returns, never guess the next one. \
Reads omit defaults (wall t=15 h=250). Writes reply `ok rev=N [ids=...]`; don't re-read \
unless needed. Every change is one undoable step. Use render_plan to check visually. \
A project can hold several plan versions (variants tool); tools act on the active one. \
Finishes are short strings: `#rrggbb` paint, a pattern like `tiles #ffffff 60x60 r45` \
(tint, tile cm, rotation) or `img:path 90x90`; `none` clears. Wall types and patterns: materials tool.";

/// The MCP server. Cheap to clone: it only holds a handle to the document.
#[derive(Debug, Clone)]
pub struct NewEraMcp {
    document: SharedDocument,
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct UpdateParams {
    items: Vec<UpdateSpec>,
    /// Plan version (tab) to write to; switches to it first.
    v: Option<usize>,
    /// Try it without applying: reports what would change, the clearances
    /// around every piece it touches, and which layout and ergonomics
    /// findings it would resolve or create. Nothing is written and the
    /// user's window does not move.
    dry: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct IdsParams {
    ids: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct MoveParams {
    ids: Vec<String>,
    dx: f64,
    dy: f64,
    /// Drag endpoints of walls joined to moved walls (default true).
    joined: Option<bool>,
    /// Try it without applying; see `update`.
    dry: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SplitParams {
    id: String,
    /// Split position along the wall, 0..1 (default 0.5).
    t: Option<f64>,
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
            Self::tool_router(),
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
}

#[tool_router]
impl NewEraMcp {
    #[tool(
        description = "Create walls (polylines; hs = height per point for gables), rooms (pts, or at=[x,y] to detect from walls), dims (a+b or wall id), labels, roofs (rectangle pts, gable|shed, pitch or ridge_h, eave h, overhang, gables=true closes the ends, skylights [{at,w,d}] cut glazed openings) and solids (pts outline raised by h at elev: slabs/mezzanines of any shape; or profile [[u,z]] swept from a to b: gables, ramps) in one atomic step."
    )]
    pub(crate) fn create(
        &self,
        Parameters(mut p): Parameters<CreateParams>,
    ) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        on_variant(&mut doc, p.v)?;
        if p.px {
            let bg = background_scale(&doc)?;
            p.map_points(&|q| bg.point(q));
        }
        let ids = edit::create(&mut doc, p).map_err(invalid)?;
        Ok(ok(&doc, &ids))
    }

    #[tool(
        description = "Change fields of elements by id; fields must match the element kind (e.g. furniture mat/opacity/pitch, wall h_end, room auto, polyline divider). anchor on a resize holds one face still (back/front/left/right of the piece, bottom/top, or a plan side) instead of growing around the center, so a run of joinery keeps its back on the wall. dry=true answers what it would do — changed fields, clearances around each piece it touches, findings resolved and created — without writing anything, so a size can be tried before it is applied. Otherwise the reply names what changed."
    )]
    pub(crate) fn update(
        &self,
        Parameters(p): Parameters<UpdateParams>,
    ) -> Result<String, ErrorData> {
        if p.dry.unwrap_or(false) {
            let doc = self.document.read();
            let items = p.items;
            return preview(&doc, move |scratch| {
                edit::update(scratch, items).map_err(invalid)
            });
        }
        let mut doc = self.document.write();
        on_variant(&mut doc, p.v)?;
        let before = doc.home().clone();
        edit::update(&mut doc, p.items).map_err(invalid)?;
        Ok(applied(&doc, &before))
    }

    #[tool(description = "Delete elements by id, atomically.")]
    pub(crate) fn delete(&self, Parameters(p): Parameters<IdsParams>) -> Result<String, ErrorData> {
        let ids = edit::parse_ids(&p.ids).map_err(invalid)?;
        let mut doc = self.document.write();
        let commands = ids.into_iter().map(Command::remove).collect();
        doc.execute(Command::Batch { commands }).map_err(core)?;
        Ok(ok(&doc, &[]))
    }

    #[tool(
        name = "move",
        description = "Move elements by dx,dy cm. dry=true answers what it would do without writing anything; see `update`."
    )]
    pub(crate) fn move_elements(
        &self,
        Parameters(p): Parameters<MoveParams>,
    ) -> Result<String, ErrorData> {
        let ids = edit::parse_ids(&p.ids).map_err(invalid)?;
        let joined = p.joined.unwrap_or(true);
        if p.dry.unwrap_or(false) {
            let doc = self.document.read();
            return preview(&doc, move |scratch| {
                ops::translate(scratch, &ids, p.dx, p.dy, joined).map_err(core)
            });
        }
        let mut doc = self.document.write();
        let before = doc.home().clone();
        ops::translate(&mut doc, &ids, p.dx, p.dy, joined).map_err(core)?;
        Ok(applied(&doc, &before))
    }

    #[tool(description = "Split a wall into two joined walls at t (0..1).")]
    pub(crate) fn split_wall(
        &self,
        Parameters(p): Parameters<SplitParams>,
    ) -> Result<String, ErrorData> {
        let id = p.id.parse().map_err(|e| invalid(format!("{e}")))?;
        let mut doc = self.document.write();
        let second = ops::split_wall(&mut doc, id, p.t.unwrap_or(0.5)).map_err(core)?;
        Ok(ok(&doc, &[second.to_string()]))
    }
}

// The macro generates async trait methods that resolve immediately.
#[allow(clippy::unused_async_trait_impl)]
#[tool_handler(router = self.tool_router)]
impl ServerHandler for NewEraMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new("3d-new-era-ai", env!("CARGO_PKG_VERSION"))
                    .with_title("3D New Era AI"),
            )
            .with_instructions(INSTRUCTIONS)
    }
}

#[cfg(test)]
fn server() -> NewEraMcp {
    NewEraMcp::new(SharedDocument::new(newera_core::Document::default()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::cameras::CamerasParams;
    use crate::tools::furniture::PlaceParams;
    use crate::tools::read::GetHomeParams;

    #[test]
    fn a_dry_write_answers_the_question_without_touching_the_plan() {
        let s = server();
        s.create(Parameters(
            serde_json::from_str(
                r#"{"walls":[{"pts":[[0,0],[500,0],[500,400],[0,400]],"closed":true}],
                    "rooms":[{"name":"Cozinha","at":[250,200]}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        s.place(Parameters(
            serde_json::from_str(
                r#"{"items":[{"cat":"base-cabinet","at":[250,40],"w":300,"d":60,"h":90},
                             {"cat":"base-cabinet","at":[250,340],"w":300,"d":60,"h":90}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        let rev = s.document.read().revision();
        let depth = s.document.read().home().furniture[0].depth;

        // "And if the counter were 100 cm deep?" — asked, not applied.
        let dry: serde_json::Value = serde_json::from_str(
            &s.update(Parameters(UpdateParams {
                items: serde_json::from_str(r#"[{"id":"f6","d":100}]"#).unwrap(),
                v: None,
                dry: Some(true),
            }))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(dry["dry"], true, "{dry}");
        assert_eq!(dry["changed"][0]["id"], "f6", "{dry}");
        assert_eq!(dry["changed"][0]["to"]["wdh"][1], 100.0, "{dry}");
        // The corridor it would leave, measured on the copy. Depth grows
        // around the center, so the front only advances 20 cm — and the back
        // ends up inside the wall, which the dry run says before it happens.
        assert!(
            (dry["clearances"]["f6"]["+y"][0].as_f64().unwrap() - 220.0).abs() < 0.5,
            "{dry}"
        );
        assert_eq!(dry["issues_new"][0], "f6+w1", "{dry}");
        assert_eq!(
            s.document.read().revision(),
            rev,
            "a dry run writes nothing"
        );
        assert!(
            (s.document.read().home().furniture[0].depth - depth).abs() < 1e-9,
            "and changes nothing"
        );

        // Applied for real, the reply says what moved instead of only `ok`.
        let reply = s
            .update(Parameters(UpdateParams {
                items: serde_json::from_str(r#"[{"id":"f6","d":100}]"#).unwrap(),
                v: None,
                dry: None,
            }))
            .unwrap();
        assert!(reply.starts_with("ok rev="), "{reply}");
        let diff: serde_json::Value =
            serde_json::from_str(&reply[reply.find('{').expect("a diff")..]).unwrap();
        assert_eq!(diff["changed"][0]["id"], "f6", "{reply}");
        assert_eq!(diff["changed"][0]["from"]["wdh"][1], 60.0, "{reply}");
    }

    #[test]
    fn a_resize_can_hold_one_face_instead_of_growing_around_the_center() {
        let s = server();
        s.create(Parameters(
            serde_json::from_str(
                r#"{"walls":[{"pts":[[0,0],[500,0],[500,400],[0,400]],"closed":true}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        // A counter with its back on the top wall, and one turned a quarter
        // turn with its back on the left wall.
        s.place(Parameters(
            serde_json::from_str(
                r#"{"items":[{"cat":"base-cabinet","at":[250,37.5],"w":300,"d":60,"h":90},
                             {"cat":"base-cabinet","at":[37.5,250],"w":200,"d":60,"h":90,"angle":270}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        let deepen = |id: &str, anchor: &str| {
            s.update(Parameters(UpdateParams {
                items: serde_json::from_str(&format!(
                    r#"[{{"id":"{id}","d":80,"anchor":"{anchor}"}}]"#
                ))
                .unwrap(),
                v: None,
                dry: None,
            }))
            .unwrap();
        };

        deepen("f5", "back");
        let home = s.document.read();
        let counter = home.home().find_piece("f5".parse().unwrap()).unwrap();
        let (min, max) = newera_core::plan_bounds(counter);
        assert!(
            (min.y - 7.5).abs() < 0.01,
            "the back stays on the wall: {min:?}"
        );
        assert!((max.y - 87.5).abs() < 0.01, "the front advances: {max:?}");
        drop(home);

        // Turned 270°, the piece's back looks at -x: the same word holds the
        // face against the left wall, not a plan side worked out by hand.
        deepen("f6", "back");
        let home = s.document.read();
        let turned = home.home().find_piece("f6".parse().unwrap()).unwrap();
        let (min, max) = newera_core::plan_bounds(turned);
        assert_eq!(newera_core::facing(turned), "+x", "{turned:?}");
        assert!((min.x - 7.5).abs() < 0.01, "{min:?}");
        assert!((max.x - 87.5).abs() < 0.01, "{max:?}");
        drop(home);

        // Without an anchor the center is what stays, which is the old trap.
        s.update(Parameters(UpdateParams {
            items: serde_json::from_str(r#"[{"id":"f5","d":100}]"#).unwrap(),
            v: None,
            dry: None,
        }))
        .unwrap();
        let home = s.document.read();
        let counter = home.home().find_piece("f5".parse().unwrap()).unwrap();
        let (min, _) = newera_core::plan_bounds(counter);
        assert!((min.y - (-2.5)).abs() < 0.01, "{min:?}");
    }

    #[test]
    fn labels_and_dimensions_in_3d() {
        let s = server();
        let params: CreateParams = serde_json::from_str(
            r#"{"labels":[{"text":"Sala","at":[10,10],"pitch":90,"elev":150},{"text":"Plano","at":[0,0]}],
                "dims":[{"a":[0,0],"b":[300,0],"off":30,"in3d":true,"elev":250,"pitch":90}]}"#,
        )
        .unwrap();
        s.create(Parameters(params)).unwrap();
        {
            let doc = s.document.read();
            let home = doc.home();
            assert_eq!(home.labels[0].pitch, Some(90.0));
            assert!((home.labels[0].elevation - 150.0).abs() < 1e-9);
            assert_eq!(home.labels[1].pitch, None);
            let d = &home.dimensions[0];
            assert!(d.visible_in_3d && (d.elevation[1] - 250.0).abs() < 1e-9);
        }
        let ids: Vec<String> = {
            let doc = s.document.read();
            vec![
                doc.home().labels[0].id.to_string(),
                doc.home().labels[1].id.to_string(),
                doc.home().dimensions[0].id.to_string(),
            ]
        };
        let specs: Vec<UpdateSpec> = serde_json::from_str(&format!(
            r#"[{{"id":"{}","in3d":false}},{{"id":"{}","pitch":0}},{{"id":"{}","in3d":false}}]"#,
            ids[0], ids[1], ids[2]
        ))
        .unwrap();
        s.update(Parameters(UpdateParams {
            items: specs,
            v: None,
            dry: None,
        }))
        .unwrap();
        let doc = s.document.read();
        let home = doc.home();
        assert_eq!(home.labels[0].pitch, None);
        assert_eq!(home.labels[1].pitch, Some(0.0));
        assert!(!home.dimensions[0].visible_in_3d);
    }

    #[test]
    fn polylines_label_styles_and_cameras() {
        let s = server();
        let params: CreateParams = serde_json::from_str(
            r#"{"labels":[{"text":"Tomada","at":[10,10]}],
                "polylines":[{"pts":[[0,0],[100,0],[100,50]],"t":2,"color":[200,0,0],"dash":"dash","arrows":["none","delta"]}]}"#,
        )
        .unwrap();
        assert_eq!(s.create(Parameters(params)).unwrap(), "ok rev=1 ids=t1,pl2");
        let spec: UpdateSpec =
            serde_json::from_str(r#"{"id":"t1","bold":true,"align":"left","color":[0,0,255]}"#)
                .unwrap();
        s.update(Parameters(UpdateParams {
            items: vec![spec],
            v: None,
            dry: None,
        }))
        .unwrap();
        let home = s.get_home(Parameters(GetHomeParams::default())).unwrap();
        assert!(
            home.contains(r#""bold":true"#) && home.contains(r#""align":"left""#),
            "{home}"
        );
        assert!(
            home.contains(r#""dash":"dash""#) && home.contains(r#""arrows":["none","delta"]"#),
            "{home}"
        );

        let store = |name: &str| CamerasParams {
            action: Some("store".into()),
            name: Some(name.into()),
            x: Some(100.0),
            y: Some(200.0),
            ..CamerasParams::default()
        };
        s.cameras(Parameters(store("Sala"))).unwrap();
        s.cameras(Parameters(CamerasParams {
            action: Some("view".into()),
            i: Some(0),
            ..CamerasParams::default()
        }))
        .unwrap();
        let list = s.cameras(Parameters(CamerasParams::default())).unwrap();
        assert!(
            list.contains(r#""active":"visitor""#) && list.contains("Sala"),
            "{list}"
        );
        assert!(
            s.cameras(Parameters(CamerasParams {
                action: Some("view".into()),
                i: Some(5),
                ..CamerasParams::default()
            }))
            .is_err()
        );
        s.undo().unwrap();
        assert!(
            !s.document.read().home().cameras.observer_active,
            "undo restores the aerial view"
        );
    }

    /// The whole tool surface, frozen: the names an agent can call and the
    /// exact bytes of what it reads to learn them.
    ///
    /// `list_all` sorts by name, so this is stable across runs. It is the
    /// guard for moving tools between modules: a dropped router, two modules
    /// claiming one name (the merge overwrites in silence), a description
    /// left behind or a schema that missed compaction all change it.
    #[test]
    fn tool_surface_is_unchanged() {
        const NAMES: &str = "annotations,arrange,cabinet_run,cameras,catalog,check_layout,\
create,cut_list,delete,disciplines,embed,ergonomics,export_plan,fit_roof,get_home,joinery,\
levels,lighting,materials,measure,move,new_home,open_home,place,plugins,redo,render_3d,\
render_photo,render_plan,save_home,sessions,set_background,set_home,split_wall,\
trace_background,undo,update,variants,video";

        let tools = server().tool_router.list_all();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert_eq!(names.join(","), NAMES, "the set of tools changed");
        let bytes = serde_json::to_string(&tools).unwrap().len();
        assert_eq!(
            bytes, 49408,
            "a description or schema changed; this test guards a pure move"
        );
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
    fn sloping_walls_and_tilted_pieces() {
        let s = server();
        // A gable: 600 cm base rising to 675 cm in the middle.
        let params: CreateParams =
            serde_json::from_str(r#"{"walls":[{"pts":[[0,0],[300,0],[600,0]],"hs":[10,675,10]}]}"#)
                .unwrap();
        s.create(Parameters(params)).unwrap();
        {
            let doc = s.document.read();
            let walls = &doc.home().walls;
            assert_eq!(
                (walls[0].height, walls[0].height_at_end),
                (10.0, Some(675.0))
            );
            assert_eq!(
                (walls[1].height, walls[1].height_at_end),
                (675.0, Some(10.0))
            );
        }
        let bad: CreateParams =
            serde_json::from_str(r#"{"walls":[{"pts":[[0,0],[100,0]],"hs":[10]}]}"#).unwrap();
        assert!(s.create(Parameters(bad)).is_err());
        let wall = s.document.read().home().walls[0].id.to_string();
        let spec: UpdateSpec =
            serde_json::from_str(&format!(r#"{{"id":"{wall}","h_end":300}}"#)).unwrap();
        s.update(Parameters(UpdateParams {
            items: vec![spec],
            v: None,
            dry: None,
        }))
        .unwrap();
        assert_eq!(s.document.read().home().walls[0].height_at_end, Some(300.0));

        // A 400 cm rafter tilted 45°: its far end rises.
        let params: PlaceParams = serde_json::from_str(
            r#"{"items":[{"cat":"box","at":[300,300],"w":10,"d":400,"h":10,"elev":100,"pitch":45}]}"#,
        )
        .unwrap();
        s.place(Parameters(params)).unwrap();
        let doc = s.document.read();
        let piece = doc.home().furniture.last().unwrap().clone();
        assert!((piece.pitch - 45.0).abs() < 1e-9);
        let mut only = doc.home().clone();
        only.walls.clear();
        let mesh =
            newera_render::Mesh::from_home(&only, &newera_render::Selection::new(), &|_| None);
        let top = mesh
            .vertices
            .iter()
            .map(|v| v.position[1])
            .fold(f32::MIN, f32::max);
        // Centered at 105 cm, half its length at 45° adds ~141 cm.
        assert!(top > 2.3 && top < 2.6, "{top}");
    }

    #[test]
    fn dividers_and_rooms_that_follow_walls() {
        let s = server();
        let params: CreateParams = serde_json::from_str(
            r#"{"walls":[{"pts":[[0,0],[800,0],[800,400],[0,400]],"closed":true}],
                "polylines":[{"pts":[[450,0],[450,400]],"divider":true}],
                "rooms":[{"name":"Sala","at":[200,200]},{"name":"Jantar","at":[600,200]}]}"#,
        )
        .unwrap();
        s.create(Parameters(params)).unwrap();
        let (sala, jantar, line) = {
            let doc = s.document.read();
            let h = doc.home();
            assert!(h.polylines[0].room_divider && h.rooms.iter().all(|r| r.auto));
            (
                h.rooms[0].area(),
                h.rooms[1].area(),
                h.polylines[0].id.to_string(),
            )
        };
        assert!(sala > jantar + 90.0 * 300.0, "{sala} {jantar}");
        let spec: UpdateSpec =
            serde_json::from_str(&format!(r#"{{"id":"{line}","pts":[[350,0],[350,400]]}}"#))
                .unwrap();
        s.update(Parameters(UpdateParams {
            items: vec![spec],
            v: None,
            dry: None,
        }))
        .unwrap();
        let doc = s.document.read();
        let h = doc.home();
        assert!(
            h.rooms[1].area() > h.rooms[0].area(),
            "rooms followed the divider"
        );
    }

    #[test]
    fn solids_and_skylights() {
        let s = server();
        let params: CreateParams = serde_json::from_str(
            r#"{"roofs":[{"pts":[[0,0],[0,700],[600,700],[600,0]],"h":0,"ridge_h":675,"overhang":0,"gables":true,
                          "skylights":[{"at":[150,350],"w":100,"d":80}]}],
                "solids":[{"pts":[[150,400],[450,400],[300,650]],"h":15,"elev":300,"mat":"wood"},
                          {"profile":[[-100,0],[100,0],[0,150]],"a":[1000,0],"b":[1000,300]}]}"#,
        )
        .unwrap();
        s.create(Parameters(params)).unwrap();
        let doc = s.document.read();
        let home = doc.home();
        let roof = home.furniture.iter().find(|f| f.is_group()).unwrap();
        // The slope with the skylight is split around it, plus the glass and the ridge cap.
        assert_eq!(
            roof.children.len(),
            1 + 5 + 1,
            "{:?}",
            roof.children.iter().map(|c| &c.name).collect::<Vec<_>>()
        );
        assert_eq!(
            roof.children.iter().filter(|c| c.opacity.is_some()).count(),
            1
        );
        let slab = home
            .furniture
            .iter()
            .find(|f| matches!(f.shape, Some(newera_core::SolidShape::Outline(_))))
            .unwrap();
        assert!((slab.width - 300.0).abs() < 1e-9 && (slab.elevation - 300.0).abs() < 1e-9);
        let gable = home
            .furniture
            .iter()
            .find(|f| matches!(f.shape, Some(newera_core::SolidShape::Profile(_))))
            .unwrap();
        assert!((gable.depth - 300.0).abs() < 1e-9 && (gable.height - 150.0).abs() < 1e-9);
        assert!(
            (gable.position.x - 1000.0).abs() < 1e-6 && (gable.position.y - 150.0).abs() < 1e-6
        );
        let mut only = home.clone();
        only.walls.clear();
        only.furniture.retain(|f| f.shape.is_some());
        let mesh =
            newera_render::Mesh::from_home(&only, &newera_render::Selection::new(), &|_| None);
        let top = mesh
            .vertices
            .iter()
            .map(|v| v.position[1])
            .fold(f32::MIN, f32::max);
        assert!((top - 3.15).abs() < 0.01, "slab top {top}");
        drop(doc);
        let bad: CreateParams =
            serde_json::from_str(r#"{"solids":[{"profile":[[0,0],[1,0],[0,1]]}]}"#).unwrap();
        assert!(s.create(Parameters(bad)).is_err());
    }
}
