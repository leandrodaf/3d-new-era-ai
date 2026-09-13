//! MCP tools.
//!
//! Token budget is a design constraint here, not an afterthought:
//! - ids are short (`w12`, `r3`) and points are `[x, y]` pairs;
//! - reads use a compact view that omits default values;
//! - writes answer with one line (`ok rev=7 ids=w8,w9`) instead of echoing state;
//! - batch-friendly tools (polyline walls, multi-delete) replace chatty calls.

use newera_core::{
    Command, Compass, CoreError, Home, Point2, Room, RoomId, SharedDocument, Wall, WallId,
};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::{ErrorData, ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

const INSTRUCTIONS: &str = "\
Home design editor. Units: centimeters. Plan axes: x right, y down. \
Points are [x,y]. Ids: w=wall, r=room. Every change is undoable and shows \
live in the user's editor. Start with get_home. Writes reply `ok rev=N ids=...`; \
re-read only when you need fresh state.";

/// The MCP server. Cheap to clone: it only holds a handle to the document.
#[derive(Debug, Clone)]
pub struct NewEraMcp {
    document: SharedDocument,
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct CreateWallsParams {
    /// Polyline vertices; N points create N-1 connected walls.
    points: Vec<Point2>,
    /// Also connect the last point back to the first.
    #[serde(default)]
    closed: bool,
    /// Thickness in cm (default 15).
    thickness: Option<f64>,
    /// Height in cm (default 250).
    height: Option<f64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct DeleteParams {
    /// Ids to delete, e.g. `["w3","r1"]`. Applied as one undoable step.
    ids: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct CreateRoomParams {
    name: String,
    /// Floor polygon, at least 3 points.
    points: Vec<Point2>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct RenameHomeParams {
    name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SetCompassParams {
    /// Clockwise degrees from plan up (-y) to north.
    north_degrees: Option<f64>,
    center: Option<Point2>,
    /// Diameter in cm.
    diameter: Option<f64>,
    visible: Option<bool>,
}

#[tool_router]
impl NewEraMcp {
    pub fn new(document: SharedDocument) -> Self {
        Self {
            document,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "Compact home state. Walls: {id,a,b} plus t(thickness)/h(height) only when not 15/250. Rooms: {id,name,pts,m2}."
    )]
    fn get_home(&self) -> String {
        let doc = self.document.read();
        compact_home(doc.home(), doc.revision()).to_string()
    }

    #[tool(description = "Create connected walls along a polyline in one undoable step.")]
    fn create_walls(
        &self,
        Parameters(params): Parameters<CreateWallsParams>,
    ) -> Result<String, ErrorData> {
        if params.points.len() < 2 {
            return Err(invalid("at least 2 points are required"));
        }
        let mut points = params.points;
        if params.closed && points.len() > 2 {
            points.push(points[0]);
        }

        let mut doc = self.document.write();
        let walls: Vec<Wall> = points
            .windows(2)
            .map(|pair| Wall {
                thickness: params.thickness.unwrap_or(Wall::DEFAULT_THICKNESS),
                height: params.height.unwrap_or(Wall::DEFAULT_HEIGHT),
                ..Wall::new(doc.new_wall_id(), pair[0], pair[1])
            })
            .collect();
        let ids: Vec<String> = walls.iter().map(|w| w.id.to_string()).collect();
        let commands = walls.into_iter().map(Command::add_wall).collect();
        doc.execute(Command::Batch { commands }).map_err(to_error)?;
        Ok(ok(doc.revision(), &ids))
    }

    #[tool(description = "Create a named room from a floor polygon.")]
    fn create_room(
        &self,
        Parameters(params): Parameters<CreateRoomParams>,
    ) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        let room = Room {
            id: doc.new_room_id(),
            name: params.name,
            points: params.points,
        };
        let ids = [room.id.to_string()];
        doc.execute(Command::add_room(room)).map_err(to_error)?;
        Ok(ok(doc.revision(), &ids))
    }

    #[tool(description = "Delete walls and/or rooms by id in one undoable step.")]
    fn delete(&self, Parameters(params): Parameters<DeleteParams>) -> Result<String, ErrorData> {
        let commands = params
            .ids
            .iter()
            .map(|raw| {
                if let Ok(id) = raw.parse::<WallId>() {
                    Ok(Command::RemoveWall { id })
                } else if let Ok(id) = raw.parse::<RoomId>() {
                    Ok(Command::RemoveRoom { id })
                } else {
                    Err(invalid(format!("unknown id `{raw}`")))
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.execute(Command::Batch { commands })
    }

    #[tool(description = "Rename the project.")]
    fn rename_home(
        &self,
        Parameters(params): Parameters<RenameHomeParams>,
    ) -> Result<String, ErrorData> {
        self.execute(Command::RenameHome { name: params.name })
    }

    #[tool(description = "Update the compass (north direction). Omitted fields keep their value.")]
    fn set_compass(
        &self,
        Parameters(params): Parameters<SetCompassParams>,
    ) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        let current = doc.home().compass;
        let compass = Compass {
            center: params.center.unwrap_or(current.center),
            diameter: params.diameter.unwrap_or(current.diameter),
            north_degrees: params.north_degrees.unwrap_or(current.north_degrees),
            visible: params.visible.unwrap_or(current.visible),
        };
        doc.execute(Command::SetCompass { compass })
            .map_err(to_error)?;
        Ok(ok(doc.revision(), &[]))
    }

    #[tool(description = "Undo the last change, whoever made it.")]
    fn undo(&self) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        doc.undo().map_err(to_error)?;
        Ok(ok(doc.revision(), &[]))
    }

    #[tool(description = "Redo the last undone change.")]
    fn redo(&self) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        doc.redo().map_err(to_error)?;
        Ok(ok(doc.revision(), &[]))
    }
}

impl NewEraMcp {
    fn execute(&self, command: Command) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        doc.execute(command).map_err(to_error)?;
        Ok(ok(doc.revision(), &[]))
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

fn ok(revision: u64, ids: &[String]) -> String {
    if ids.is_empty() {
        format!("ok rev={revision}")
    } else {
        format!("ok rev={revision} ids={}", ids.join(","))
    }
}

fn invalid(message: impl Into<String>) -> ErrorData {
    ErrorData::invalid_params(message.into(), None)
}

#[allow(clippy::needless_pass_by_value)] // used as `map_err(to_error)`
fn to_error(err: CoreError) -> ErrorData {
    invalid(err.to_string())
}

/// Rounds to 0.1 cm and drops the fractional part when it is zero, so `800`
/// is sent instead of `800.0`.
#[allow(clippy::cast_possible_truncation)]
fn num(value: f64) -> Value {
    let rounded = (value * 10.0).round() / 10.0;
    if rounded.fract() == 0.0 && rounded.abs() < 9e15 {
        json!(rounded as i64)
    } else {
        json!(rounded)
    }
}

fn point(p: Point2) -> Value {
    json!([num(p.x), num(p.y)])
}

pub(crate) fn compact_home(home: &Home, revision: u64) -> Value {
    let walls: Vec<Value> = home
        .walls
        .iter()
        .map(|w| {
            let mut v = json!({ "id": w.id.to_string(), "a": point(w.start), "b": point(w.end) });
            if (w.thickness - Wall::DEFAULT_THICKNESS).abs() > f64::EPSILON {
                v["t"] = num(w.thickness);
            }
            if (w.height - Wall::DEFAULT_HEIGHT).abs() > f64::EPSILON {
                v["h"] = num(w.height);
            }
            v
        })
        .collect();
    let rooms: Vec<Value> = home
        .rooms
        .iter()
        .map(|r| {
            json!({
                "id": r.id.to_string(),
                "name": r.name,
                "pts": r.points.iter().copied().map(point).collect::<Vec<_>>(),
                "m2": (r.area() / 100.0).round() / 100.0,
            })
        })
        .collect();

    let mut out = json!({ "rev": revision, "name": home.name, "walls": walls, "rooms": rooms });
    if home.compass != Compass::default() {
        out["north"] = num(home.compass.north_degrees);
    }
    out
}

#[cfg(test)]
mod tests {
    use newera_core::Document;

    use super::*;

    fn walls(server: &NewEraMcp, points: &[[f64; 2]], closed: bool) -> String {
        server
            .create_walls(Parameters(CreateWallsParams {
                points: points.iter().copied().map(Point2::from).collect(),
                closed,
                thickness: None,
                height: None,
            }))
            .unwrap()
    }

    #[test]
    fn compact_view_omits_defaults_and_rounds() {
        let mut home = Home::default();
        let id = home.new_wall_id();
        home.walls.push(Wall::new(
            id,
            Point2::new(0.0, 0.0),
            Point2::new(800.04, 0.0),
        ));
        assert_eq!(
            compact_home(&home, 3).to_string(),
            r#"{"name":"Nova casa","rev":3,"rooms":[],"walls":[{"a":[0,0],"b":[800,0],"id":"w1"}]}"#
        );
    }

    #[test]
    fn create_walls_replies_with_one_line() {
        let server = NewEraMcp::new(SharedDocument::new(Document::default()));
        let reply = walls(&server, &[[0.0, 0.0], [400.0, 0.0], [400.0, 300.0]], true);
        assert_eq!(reply, "ok rev=1 ids=w1,w2,w3");
    }

    #[test]
    fn delete_accepts_mixed_ids_atomically() {
        let server = NewEraMcp::new(SharedDocument::new(Document::default()));
        walls(&server, &[[0.0, 0.0], [400.0, 0.0]], false);
        let err = server
            .delete(Parameters(DeleteParams {
                ids: vec!["w1".into(), "r99".into()],
            }))
            .unwrap_err();
        assert!(err.message.contains("r99"));
        assert_eq!(server.document.read().home().walls.len(), 1, "rolled back");
    }
}
