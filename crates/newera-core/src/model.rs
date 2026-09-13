use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::geometry::{Point2, polygon_area};
use crate::joins::wall_outlines;

/// Error returned when parsing an id such as `"w12"` fails.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid {kind} id `{raw}` (expected e.g. `{prefix}12`)")]
pub struct ParseIdError {
    kind: &'static str,
    prefix: &'static str,
    raw: String,
}

macro_rules! id_type {
    ($(#[$meta:meta])* $name:ident, prefix = $prefix:literal, kind = $kind:literal) => {
        $(#[$meta])*
        ///
        #[doc = concat!("Short and stable: rendered as `", $prefix, "<n>`, which keeps MCP payloads cheap for AI agents. Ids are never reused.")]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub u64);

        impl $name {
            pub const PREFIX: &'static str = $prefix;
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}{}", $prefix, self.0)
            }
        }

        impl FromStr for $name {
            type Err = ParseIdError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                s.strip_prefix($prefix)
                    .and_then(|n| n.parse().ok())
                    .map(Self)
                    .ok_or_else(|| ParseIdError { kind: $kind, prefix: $prefix, raw: s.to_owned() })
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(self)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let raw = Cow::<str>::deserialize(deserializer)?;
                raw.parse().map_err(serde::de::Error::custom)
            }
        }

        impl JsonSchema for $name {
            fn schema_name() -> Cow<'static, str> {
                stringify!($name).into()
            }

            fn json_schema(_: &mut SchemaGenerator) -> Schema {
                json_schema!({
                    "type": "string",
                    "pattern": concat!("^", $prefix, "[0-9]+$"),
                })
            }
        }
    };
}

id_type!(
    /// Identifier of a [`Wall`].
    WallId, prefix = "w", kind = "wall"
);
id_type!(
    /// Identifier of a [`Room`].
    RoomId, prefix = "r", kind = "room"
);

/// A straight wall on the floor plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Wall {
    pub id: WallId,
    pub start: Point2,
    pub end: Point2,
    /// Thickness in centimeters.
    pub thickness: f64,
    /// Height in centimeters.
    pub height: f64,
}

impl Wall {
    pub const DEFAULT_THICKNESS: f64 = 15.0;
    pub const DEFAULT_HEIGHT: f64 = 250.0;

    pub fn new(id: WallId, start: Point2, end: Point2) -> Self {
        Self {
            id,
            start,
            end,
            thickness: Self::DEFAULT_THICKNESS,
            height: Self::DEFAULT_HEIGHT,
        }
    }

    pub fn length(&self) -> f64 {
        self.start.distance(self.end)
    }
}

/// A named floor area delimited by a polygon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Room {
    pub id: RoomId,
    pub name: String,
    pub points: Vec<Point2>,
}

impl Room {
    /// Floor area in square centimeters.
    pub fn area(&self) -> f64 {
        polygon_area(&self.points)
    }
}

/// The compass rose drawn on the plan. Its north direction will also drive
/// sun position for lighting and renders.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Compass {
    /// Center on the plan, in centimeters.
    pub center: Point2,
    /// Diameter in centimeters.
    pub diameter: f64,
    /// Clockwise angle from the plan's "up" (-Y) to geographic north, in degrees.
    pub north_degrees: f64,
    pub visible: bool,
}

impl Default for Compass {
    fn default() -> Self {
        // Same defaults as Sweet Home 3D.
        Self {
            center: Point2::new(-100.0, 50.0),
            diameter: 100.0,
            north_degrees: 0.0,
            visible: true,
        }
    }
}

/// The whole project being edited.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Home {
    pub name: String,
    pub walls: Vec<Wall>,
    pub rooms: Vec<Room>,
    #[serde(default)]
    pub compass: Compass,
    /// Next number handed out for any id. Monotonic, so ids an agent saw
    /// earlier never point to a different element later.
    #[serde(default)]
    next_id: u64,
}

impl Default for Home {
    fn default() -> Self {
        Self {
            name: "Nova casa".to_owned(),
            walls: Vec::new(),
            rooms: Vec::new(),
            compass: Compass::default(),
            next_id: 1,
        }
    }
}

impl Home {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    pub fn wall(&self, id: WallId) -> Option<&Wall> {
        self.walls.iter().find(|w| w.id == id)
    }

    pub fn room(&self, id: RoomId) -> Option<&Room> {
        self.rooms.iter().find(|r| r.id == id)
    }

    pub fn new_wall_id(&mut self) -> WallId {
        WallId(self.allocate_id())
    }

    pub fn new_room_id(&mut self) -> RoomId {
        RoomId(self.allocate_id())
    }

    fn allocate_id(&mut self) -> u64 {
        // Also guards against files whose counter lags behind their elements.
        let max_used = self
            .walls
            .iter()
            .map(|w| w.id.0)
            .chain(self.rooms.iter().map(|r| r.id.0))
            .max()
            .unwrap_or(0);
        let id = self.next_id.max(max_used + 1);
        self.next_id = id + 1;
        id
    }

    /// Floor outline of every wall with corners joined, in `walls` order.
    pub fn wall_outlines(&self) -> Vec<Vec<Point2>> {
        wall_outlines(&self.walls)
    }

    /// Axis-aligned bounds `(min, max)` of every wall and room point.
    pub fn bounds(&self) -> Option<(Point2, Point2)> {
        let points = self
            .walls
            .iter()
            .flat_map(|w| [w.start, w.end])
            .chain(self.rooms.iter().flat_map(|r| r.points.iter().copied()));
        points.fold(None, |acc, p| {
            let (min, max) = acc.unwrap_or((p, p));
            Some((
                Point2::new(min.x.min(p.x), min.y.min(p.y)),
                Point2::new(max.x.max(p.x), max.y.max(p.y)),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_short_strings() {
        assert_eq!(WallId(12).to_string(), "w12");
        assert_eq!("r7".parse::<RoomId>().unwrap(), RoomId(7));
        assert!("w7".parse::<RoomId>().is_err());
        assert_eq!(serde_json::to_string(&WallId(3)).unwrap(), r#""w3""#);
    }

    #[test]
    fn allocated_ids_never_collide_with_existing_elements() {
        let mut home = Home::default();
        home.walls.push(Wall::new(
            WallId(40),
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 0.0),
        ));
        assert_eq!(home.new_wall_id(), WallId(41));
        assert_eq!(home.new_room_id(), RoomId(42));
    }
}
