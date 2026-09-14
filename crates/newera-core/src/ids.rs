//! Short, typed element ids such as `w12` or `r3`.
//!
//! Ids are cheap for AI agents to read and write, never reused (they come
//! from a monotonic counter in [`crate::Home`]), and self-describing: the
//! prefix tells which kind of element an id refers to.

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Error returned when parsing an id fails.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid id `{raw}` (expected e.g. {expected})")]
pub struct ParseIdError {
    expected: &'static str,
    raw: String,
}

macro_rules! serde_via_display {
    ($name:ident, $pattern:expr, $desc:expr) => {
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
                json_schema!({ "type": "string", "pattern": $pattern, "description": $desc })
            }
        }
    };
}

macro_rules! id_types {
    ($($(#[$meta:meta])* $name:ident => $prefix:literal, $variant:ident;)+) => {
        $(
            $(#[$meta])*
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
                        .filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
                        .and_then(|n| n.parse().ok())
                        .map(Self)
                        .ok_or_else(|| ParseIdError {
                            expected: concat!("`", $prefix, "12`"),
                            raw: s.to_owned(),
                        })
                }
            }

            impl From<$name> for ElementId {
                fn from(id: $name) -> Self {
                    Self::$variant(id)
                }
            }

            serde_via_display!($name, concat!("^", $prefix, "[0-9]+$"), concat!("id with prefix `", $prefix, "`"));
        )+

        /// Id of any element in a home.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum ElementId {
            $($variant($name),)+
        }

        impl ElementId {
            /// The raw counter value, shared by all kinds.
            pub fn number(self) -> u64 {
                match self {
                    $(Self::$variant(id) => id.0,)+
                }
            }
        }

        impl fmt::Display for ElementId {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    $(Self::$variant(id) => id.fmt(f),)+
                }
            }
        }

        impl FromStr for ElementId {
            type Err = ParseIdError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                // Longest prefixes first, so `lv3` is never read as another kind.
                let mut kinds: Vec<(&str, fn(&str) -> Option<ElementId>)> = vec![
                    $(($prefix, |s| s.parse::<$name>().ok().map(ElementId::$variant)),)+
                ];
                kinds.sort_by_key(|(prefix, _)| std::cmp::Reverse(prefix.len()));
                kinds
                    .iter()
                    .filter(|(prefix, _)| s.starts_with(prefix))
                    .find_map(|(_, parse)| parse(s))
                    .ok_or_else(|| ParseIdError {
                        expected: concat!($("`", $prefix, "12` "),+),
                        raw: s.to_owned(),
                    })
            }
        }

        serde_via_display!(ElementId, "^[a-z]+[0-9]+$", "element id, e.g. w12 (wall) or r3 (room)");
    };
}

id_types! {
    /// Identifier of a [`crate::Wall`].
    WallId => "w", Wall;
    /// Identifier of a [`crate::Polyline`].
    PolylineId => "pl", Polyline;
    /// Identifier of a [`crate::Room`].
    RoomId => "r", Room;
    /// Identifier of a [`crate::Dimension`].
    DimensionId => "d", Dimension;
    /// Identifier of a [`crate::Label`].
    LabelId => "t", Label;
    /// Identifier of a [`crate::Furniture`] piece, door or window.
    FurnitureId => "f", Furniture;
    /// Identifier of a [`crate::Level`] (storey).
    LevelId => "lv", Level;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_render_and_parse_with_their_prefix() {
        assert_eq!(WallId(12).to_string(), "w12");
        assert_eq!("r7".parse::<RoomId>().unwrap(), RoomId(7));
        assert!("w7".parse::<RoomId>().is_err());
        assert!("w".parse::<WallId>().is_err());
        assert!("w-1".parse::<WallId>().is_err());
        assert_eq!(serde_json::to_string(&WallId(3)).unwrap(), r#""w3""#);
    }

    #[test]
    fn element_ids_dispatch_on_prefix() {
        assert_eq!(
            "d4".parse::<ElementId>().unwrap(),
            ElementId::Dimension(DimensionId(4))
        );
        assert_eq!(
            "t9".parse::<ElementId>().unwrap(),
            ElementId::Label(LabelId(9))
        );
        assert!("x1".parse::<ElementId>().is_err());
        let json = serde_json::to_string(&ElementId::Room(RoomId(2))).unwrap();
        assert_eq!(json, r#""r2""#);
    }
}
