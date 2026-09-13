use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{CoreError, CoreResult};
use crate::model::{Compass, Home, Room, RoomId, Wall, WallId};

/// A reversible change to a [`Home`].
///
/// Applying a command returns its inverse, which is what makes undo/redo work
/// for every client. Commands are serializable so they can travel over MCP,
/// HTTP or be recorded in a macro/script later.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    AddWall {
        wall: Wall,
        /// Position to insert at; used internally to restore order on undo.
        #[serde(default, skip)]
        index: Option<usize>,
    },
    UpdateWall {
        wall: Wall,
    },
    RemoveWall {
        id: WallId,
    },
    AddRoom {
        room: Room,
        #[serde(default, skip)]
        index: Option<usize>,
    },
    RemoveRoom {
        id: RoomId,
    },
    RenameHome {
        name: String,
    },
    SetCompass {
        compass: Compass,
    },
    /// Several commands applied atomically: either all succeed or none does.
    Batch {
        commands: Vec<Command>,
    },
}

impl Command {
    pub fn add_wall(wall: Wall) -> Self {
        Self::AddWall { wall, index: None }
    }

    pub fn add_room(room: Room) -> Self {
        Self::AddRoom { room, index: None }
    }

    /// Applies the command and returns the command that reverts it.
    ///
    /// On error the home is left untouched.
    pub(crate) fn apply(self, home: &mut Home) -> CoreResult<Self> {
        match self {
            Self::AddWall { wall, index } => {
                validate_wall(&wall)?;
                if home.wall(wall.id).is_some() {
                    return Err(CoreError::InvalidGeometry(format!(
                        "wall {} already exists",
                        wall.id
                    )));
                }
                let id = wall.id;
                let index = index.unwrap_or(home.walls.len()).min(home.walls.len());
                home.walls.insert(index, wall);
                Ok(Self::RemoveWall { id })
            }
            Self::UpdateWall { wall } => {
                validate_wall(&wall)?;
                let slot = home
                    .walls
                    .iter_mut()
                    .find(|w| w.id == wall.id)
                    .ok_or(CoreError::WallNotFound(wall.id))?;
                let previous = std::mem::replace(slot, wall);
                Ok(Self::UpdateWall { wall: previous })
            }
            Self::RemoveWall { id } => {
                let index = home
                    .walls
                    .iter()
                    .position(|w| w.id == id)
                    .ok_or(CoreError::WallNotFound(id))?;
                let wall = home.walls.remove(index);
                Ok(Self::AddWall {
                    wall,
                    index: Some(index),
                })
            }
            Self::AddRoom { room, index } => {
                validate_room(&room)?;
                if home.room(room.id).is_some() {
                    return Err(CoreError::InvalidGeometry(format!(
                        "room {} already exists",
                        room.id
                    )));
                }
                let id = room.id;
                let index = index.unwrap_or(home.rooms.len()).min(home.rooms.len());
                home.rooms.insert(index, room);
                Ok(Self::RemoveRoom { id })
            }
            Self::RemoveRoom { id } => {
                let index = home
                    .rooms
                    .iter()
                    .position(|r| r.id == id)
                    .ok_or(CoreError::RoomNotFound(id))?;
                let room = home.rooms.remove(index);
                Ok(Self::AddRoom {
                    room,
                    index: Some(index),
                })
            }
            Self::RenameHome { name } => {
                let previous = std::mem::replace(&mut home.name, name);
                Ok(Self::RenameHome { name: previous })
            }
            Self::SetCompass { compass } => {
                if !(compass.center.is_finite()
                    && compass.diameter > 0.0
                    && compass.north_degrees.is_finite())
                {
                    return Err(CoreError::InvalidGeometry(
                        "compass needs a finite center and angle and a positive diameter".into(),
                    ));
                }
                let previous = std::mem::replace(&mut home.compass, compass);
                Ok(Self::SetCompass { compass: previous })
            }
            Self::Batch { commands } => {
                let mut inverses = Vec::with_capacity(commands.len());
                for command in commands {
                    match command.apply(home) {
                        Ok(inverse) => inverses.push(inverse),
                        Err(err) => {
                            // Roll back what was already applied, newest first.
                            for inverse in inverses.into_iter().rev() {
                                inverse
                                    .apply(home)
                                    .expect("inverse of an applied command must apply");
                            }
                            return Err(err);
                        }
                    }
                }
                inverses.reverse();
                Ok(Self::Batch { commands: inverses })
            }
        }
    }
}

fn validate_wall(wall: &Wall) -> CoreResult<()> {
    if !wall.start.is_finite() || !wall.end.is_finite() {
        return Err(CoreError::InvalidGeometry(
            "wall points must be finite".into(),
        ));
    }
    if wall.length() < 1.0 {
        return Err(CoreError::InvalidGeometry(
            "wall must be at least 1 cm long".into(),
        ));
    }
    if !(wall.thickness > 0.0 && wall.height > 0.0) {
        return Err(CoreError::InvalidGeometry(
            "wall thickness and height must be positive".into(),
        ));
    }
    Ok(())
}

fn validate_room(room: &Room) -> CoreResult<()> {
    if room.points.len() < 3 {
        return Err(CoreError::InvalidGeometry(
            "room needs at least 3 points".into(),
        ));
    }
    if room.points.iter().any(|p| !p.is_finite()) {
        return Err(CoreError::InvalidGeometry(
            "room points must be finite".into(),
        ));
    }
    Ok(())
}
