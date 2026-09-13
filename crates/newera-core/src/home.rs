use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::elements::{BackgroundImage, Compass, Dimension, Element, Label, Room, Wall};
use crate::error::{CoreError, CoreResult};
use crate::furniture::{Furniture, WallCut, wall_cuts};
use crate::geometry::Point2;
use crate::ids::{DimensionId, ElementId, FurnitureId, LabelId, RoomId, WallId};
use crate::joins::wall_outlines;

/// The whole project being edited.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Home {
    pub name: String,
    #[serde(default)]
    pub walls: Vec<Wall>,
    #[serde(default)]
    pub rooms: Vec<Room>,
    #[serde(default)]
    pub dimensions: Vec<Dimension>,
    #[serde(default)]
    pub labels: Vec<Label>,
    #[serde(default)]
    pub furniture: Vec<Furniture>,
    #[serde(default)]
    pub compass: Compass,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<BackgroundImage>,
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
            dimensions: Vec::new(),
            labels: Vec::new(),
            furniture: Vec::new(),
            compass: Compass::default(),
            background: None,
            next_id: 1,
        }
    }
}

/// Generates typed accessors, id allocation and the generic element
/// operations for every element collection.
macro_rules! collections {
    ($($variant:ident, $ty:ident, $id:ident, $field:ident, $get:ident, $new_id:ident;)+) => {
        impl Home {
            $(
                pub fn $get(&self, id: $id) -> Option<&$ty> {
                    self.$field.iter().find(|e| e.id == id)
                }

                pub fn $new_id(&mut self) -> $id {
                    $id(self.allocate_id())
                }
            )+

            fn max_used_id(&self) -> u64 {
                [$(self.$field.iter().map(|e| e.id.0).max().unwrap_or(0)),+]
                    .into_iter()
                    .max()
                    .unwrap_or(0)
            }

            pub fn contains(&self, id: ElementId) -> bool {
                self.element(id).is_some()
            }

            /// A copy of the element with this id.
            pub fn element(&self, id: ElementId) -> Option<Element> {
                match id {
                    $(ElementId::$variant(id) => self.$get(id).cloned().map(Element::$variant),)+
                }
            }

            /// All elements, in a stable order (by kind, then insertion).
            pub fn elements(&self) -> impl Iterator<Item = Element> + '_ {
                std::iter::empty()
                    $(.chain(self.$field.iter().cloned().map(Element::$variant)))+
            }

            pub(crate) fn insert(&mut self, element: Element, index: Option<usize>) -> CoreResult<()> {
                element.validate()?;
                let id = element.id();
                if self.contains(id) {
                    return Err(CoreError::AlreadyExists(id));
                }
                match element {
                    $(Element::$variant(e) => {
                        let at = index.unwrap_or(self.$field.len()).min(self.$field.len());
                        self.$field.insert(at, e);
                    })+
                }
                Ok(())
            }

            /// Removes an element, returning it with the index it had.
            pub(crate) fn remove(&mut self, id: ElementId) -> CoreResult<(Element, usize)> {
                match id {
                    $(ElementId::$variant(typed) => {
                        let at = self.$field.iter().position(|e| e.id == typed).ok_or(CoreError::NotFound(id))?;
                        Ok((Element::$variant(self.$field.remove(at)), at))
                    })+
                }
            }

            /// Replaces an element with the same id, returning the previous one.
            pub(crate) fn replace(&mut self, element: Element) -> CoreResult<Element> {
                element.validate()?;
                let id = element.id();
                match element {
                    $(Element::$variant(e) => {
                        let slot = self.$field.iter_mut().find(|x| x.id == e.id).ok_or(CoreError::NotFound(id))?;
                        Ok(Element::$variant(std::mem::replace(slot, e)))
                    })+
                }
            }
        }
    };
}

collections! {
    Wall, Wall, WallId, walls, wall, new_wall_id;
    Room, Room, RoomId, rooms, room, new_room_id;
    Dimension, Dimension, DimensionId, dimensions, dimension, new_dimension_id;
    Label, Label, LabelId, labels, label, new_label_id;
    Furniture, Furniture, FurnitureId, furniture, piece, new_furniture_id;
}

impl Home {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    fn allocate_id(&mut self) -> u64 {
        // Also guards against files whose counter lags behind their elements.
        let id = self.next_id.max(self.max_used_id() + 1);
        self.next_id = id + 1;
        id
    }

    /// Floor outline of every wall with corners joined, in `walls` order.
    pub fn wall_outlines(&self) -> Vec<Vec<Point2>> {
        wall_outlines(&self.walls)
    }

    /// Door and window holes in each wall, in `walls` order.
    pub fn wall_cuts(&self) -> Vec<Vec<WallCut>> {
        wall_cuts(&self.walls, &self.furniture)
    }

    /// Axis-aligned bounds `(min, max)` of the drawing.
    pub fn bounds(&self) -> Option<(Point2, Point2)> {
        let points = self
            .walls
            .iter()
            .flat_map(Wall::centerline)
            .chain(self.rooms.iter().flat_map(|r| r.points.iter().copied()))
            .chain(self.dimensions.iter().flat_map(|d| [d.start, d.end]))
            .chain(self.labels.iter().map(|l| l.position))
            .chain(self.furniture.iter().flat_map(Furniture::footprint));
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
    fn allocated_ids_never_collide_with_existing_elements() {
        let mut home = Home::default();
        home.walls.push(Wall::new(
            WallId(40),
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 0.0),
        ));
        assert_eq!(home.new_wall_id(), WallId(41));
        assert_eq!(home.new_room_id(), RoomId(42));
        assert_eq!(home.new_label_id(), LabelId(43));
    }

    #[test]
    fn generic_operations_round_trip() {
        let mut home = Home::default();
        let id = home.new_label_id();
        let label = Label {
            id,
            text: "Entrada".into(),
            position: Point2::new(10.0, 20.0),
            size: Label::DEFAULT_SIZE,
            angle: 0.0,
        };
        home.insert(label.clone().into(), None).unwrap();
        assert_eq!(home.element(id.into()), Some(Element::Label(label.clone())));
        assert!(matches!(
            home.insert(label.clone().into(), None),
            Err(CoreError::AlreadyExists(_))
        ));
        let (removed, index) = home.remove(id.into()).unwrap();
        assert_eq!((removed, index), (Element::Label(label), 0));
        assert!(matches!(
            home.remove(id.into()),
            Err(CoreError::NotFound(_))
        ));
    }
}
