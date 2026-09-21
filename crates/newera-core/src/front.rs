//! Which way each catalog piece is meant to stand.
//!
//! Every catalog generator builds its front on the piece's local `+y`, so at
//! `angle` 0 the front looks down the plan (+y) and the back up it (−y). What
//! that front *is* — the seat of a sofa, the foot of a bed, the doors of a
//! wardrobe — and whether its back belongs on a wall is written here, per
//! catalog id, so an agent placing one does not have to guess from a box.

use crate::Point2;

/// How a piece relates to the walls around it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stance {
    /// The same from every side, or no side that matters: a rug, a round
    /// table, a lamp, a column, a drain. Any angle is right.
    Any,
    /// Has a front, and stands anywhere: an armchair turned to the TV, a
    /// chair at a table, an island with its doors to the cooktop.
    Free,
    /// Has a front, and its back belongs on a wall: the headboard of a bed,
    /// the back of a wardrobe, the cistern of a toilet.
    Wall,
}

impl Stance {
    pub fn name(self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::Free => "free",
            Self::Wall => "wall",
        }
    }

    /// Whether the piece has a front at all.
    pub fn has_front(self) -> bool {
        self != Self::Any
    }
}

/// The front of a catalog piece, named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Front {
    pub stance: Stance,
    /// What faces the room: the side people use (local `+y`).
    pub front: &'static str,
    /// What is on the other side (local `−y`).
    pub back: &'static str,
}

const fn wall(front: &'static str, back: &'static str) -> Front {
    Front {
        stance: Stance::Wall,
        front,
        back,
    }
}

const fn free(front: &'static str, back: &'static str) -> Front {
    Front {
        stance: Stance::Free,
        front,
        back,
    }
}

const ANY: Front = Front {
    stance: Stance::Any,
    front: "",
    back: "",
};

/// Every catalog id with a front, and what it is. Ids not listed have none
/// ([`Stance::Any`]); `newera-catalog` checks that every item it has is
/// either here or meant to be absent.
const FRONTS: &[(&str, Front)] = &[
    // Living
    ("sofa-3", wall("seat", "backrest")),
    ("sofa-2", wall("seat", "backrest")),
    ("sofa-l", wall("seat and chaise", "backrest")),
    ("armchair", free("seat", "backrest")),
    ("tv-stand", wall("doors", "back panel")),
    ("tv", wall("screen", "back")),
    ("bookcase", wall("shelves", "back panel")),
    // Dining
    ("chair", free("seat", "backrest")),
    ("sideboard", wall("doors", "back panel")),
    // Kitchen
    ("fridge", wall("doors", "back")),
    ("stove", wall("oven door and knobs", "back")),
    ("sink-counter", wall("doors", "back")),
    ("base-cabinet", wall("door", "back")),
    ("wall-cabinet", wall("door", "back")),
    ("hood", wall("front", "back")),
    ("dishwasher", wall("door", "back")),
    ("microwave", wall("door", "back")),
    ("oven", wall("door", "back")),
    ("kitchen-island", free("doors", "overhang side")),
    // Bedroom
    ("bed-double", wall("foot", "headboard")),
    ("bed-queen", wall("foot", "headboard")),
    ("bed-king", wall("foot", "headboard")),
    ("bed-single", wall("foot", "headboard")),
    ("nightstand", wall("drawers", "back")),
    ("wardrobe", wall("doors", "back")),
    ("dresser", wall("drawers", "back")),
    // Bathroom
    ("toilet", wall("seat", "cistern")),
    ("basin-cabinet", wall("doors", "back")),
    ("shower", wall("glass entry", "tiled walls")),
    // Laundry
    ("washer", wall("door", "back")),
    ("dryer", wall("door", "back")),
    ("laundry-sink", wall("door", "back")),
    // Office
    ("desk", wall("drawers and knee space", "back")),
    ("office-chair", free("seat", "backrest")),
    // Structure
    ("stairs", free("first step", "top landing")),
    // Outdoor
    ("lounger", free("foot rest", "backrest")),
    ("grill", wall("firebox opening", "back wall")),
];

/// The front of the catalog piece `catalog`; [`Stance::Any`] when it has
/// none or the id is not a catalog one.
pub fn of(catalog: &str) -> Front {
    FRONTS
        .iter()
        .find(|(id, _)| *id == catalog)
        .map_or(ANY, |(_, front)| *front)
}

/// Every catalog id this module gives a front, for the catalog's own check.
pub fn listed() -> impl Iterator<Item = &'static str> {
    FRONTS.iter().map(|(id, _)| *id)
}

/// The clockwise `angle` that turns a piece's front toward `dir` on the plan
/// (x right, y down). `+y` is 0, `−x` is 90, `−y` is 180, `+x` is 270.
pub fn angle_facing(dir: Point2) -> f64 {
    let a = (-dir.x).atan2(dir.y).to_degrees().rem_euclid(360.0);
    // Keep round numbers round: 270, not 269.99999999999997.
    let r = a.round();
    let a = if (a - r).abs() < 1e-9 { r } else { a };
    if a >= 360.0 { a - 360.0 } else { a }
}

/// Where a piece at `angle` looks, as a unit vector on the plan.
pub fn front_vector(angle: f64) -> Point2 {
    let (sin, cos) = angle.to_radians().sin_cos();
    Point2::new(-sin, cos)
}

/// A side of the plan, `+x`, `-x`, `+y` or `-y`, as a unit vector.
pub fn side_vector(side: &str) -> Option<Point2> {
    Some(match side.trim() {
        "+x" | "x" | "right" | "east" => Point2::new(1.0, 0.0),
        "-x" | "left" | "west" => Point2::new(-1.0, 0.0),
        "+y" | "y" | "down" | "south" => Point2::new(0.0, 1.0),
        "-y" | "up" | "north" => Point2::new(0.0, -1.0),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angles_follow_the_clockwise_plan_convention() {
        for (side, angle) in [("+y", 0.0), ("-x", 90.0), ("-y", 180.0), ("+x", 270.0)] {
            let dir = side_vector(side).unwrap();
            assert!((angle_facing(dir) - angle).abs() < 1e-9, "{side}");
            let back = front_vector(angle);
            assert!((back.x - dir.x).abs() < 1e-9 && (back.y - dir.y).abs() < 1e-9);
            // And the snapped name every read reports agrees.
            let piece = crate::Furniture {
                angle,
                ..Default::default()
            };
            assert_eq!(crate::facing(&piece), side);
        }
    }

    #[test]
    fn unlisted_ids_have_no_front() {
        assert_eq!(of("rug").stance, Stance::Any);
        assert_eq!(of("bed-double").stance, Stance::Wall);
        assert_eq!(of("bed-double").back, "headboard");
    }
}
