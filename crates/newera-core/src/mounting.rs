//! Where a wall-mounted point can go: into the masonry, never into glass.
//!
//! An outlet, a switch, a network or water point is set in a box inside the
//! wall. A glass wall has nothing to set it in, and neither has the span of
//! a door or a window at the height it takes — an outlet under a window sill
//! is fine, one on the pane is not.

use crate::furniture::{Furniture, OpeningKind};
use crate::geometry::Point2;
use crate::home::Home;
use crate::materials::WallFamily;

/// How far off a wall's centerline a point still sits in it, beyond half
/// its thickness, cm.
const IN_WALL: f64 = 12.0;

/// Whether a catalog piece is set into a wall.
pub fn wall_mounted(catalog: &str) -> bool {
    matches!(
        catalog,
        "outlet-low"
            | "outlet-mid"
            | "outlet-high"
            | "switch"
            | "switch-double"
            | "switch-3way"
            | "network-outlet"
            | "data-outlet"
            | "tv-outlet"
            | "doorbell"
            | "electrical-panel"
            | "telecom-panel"
            | "light-wall"
            | "ac-point"
            | "shower-point"
            | "smart-switch"
            | "dimmer"
            | "smart-lock"
            | "cold-water"
            | "hot-water"
            | "gas-point"
            | "valve"
    )
}

fn distance_to_segment(p: Point2, a: Point2, b: Point2) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = (dx * dx + dy * dy).max(1e-9);
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
    Point2::new(a.x + t * dx, a.y + t * dy).distance(p)
}

/// Why `piece` cannot be set where it stands, if it cannot: in a glass wall,
/// or in the span of a door, a window or an open passage at its height.
/// Pieces not set into walls, or standing in no wall, are never refused.
pub fn blocked(home: &Home, piece: &Furniture) -> Option<String> {
    if !wall_mounted(&piece.catalog) {
        return None;
    }
    let view = home.level_view(home.current_level());
    let wall = view
        .walls
        .iter()
        .filter(|w| !w.is_arc())
        .map(|w| (w, distance_to_segment(piece.position, w.start, w.end)))
        .filter(|(w, d)| *d <= w.thickness / 2.0 + IN_WALL)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(w, _)| w);
    let (lo, hi) = (piece.elevation, piece.elevation + piece.height);
    let Some(wall) = wall else {
        // In no wall: set on a glass piece (a shower screen, a railing, a
        // see-through partition) at its height is set on nothing.
        return view
            .furniture
            .iter()
            .flat_map(Furniture::flatten)
            .filter(|f| f.id != piece.id && !f.is_opening())
            .filter(|f| {
                matches!(f.catalog.as_str(), "shower-glass" | "glass-railing")
                    || f.opacity.is_some_and(|o| o < 0.6)
            })
            .find(|f| {
                let mut near = (*f).clone();
                near.width += 2.0 * IN_WALL;
                near.depth += 2.0 * IN_WALL;
                near.contains(piece.position) && lo < f.elevation + f.height && hi > f.elevation
            })
            .map(|f| {
                format!(
                    "{} {} está sobre o vidro de {} ({}): não há onde embutir a caixa; leve-o a uma parede.",
                    piece.name, piece.id, f.name, f.id
                )
            });
    };
    let family = wall
        .wall_type
        .as_deref()
        .and_then(crate::materials::wall_type)
        .map(|t| t.family);
    if family == Some(WallFamily::Glass) {
        return Some(format!(
            "{} {} está numa parede de vidro ({}): não há onde embutir a caixa; leve-o a uma parede de alvenaria ou drywall.",
            piece.name, piece.id, wall.id
        ));
    }
    view.furniture
        .iter()
        .flat_map(Furniture::flatten)
        .filter(|f| f.id != piece.id)
        .find_map(|f| {
            let kind = f.opening.as_ref()?.kind;
            let mut span = f.clone();
            span.depth = span.depth.max(wall.thickness + 2.0 * IN_WALL);
            let top = if kind == OpeningKind::Passage {
                f64::MAX
            } else {
                f.elevation + f.height
            };
            let overlaps = lo < top && hi > f.elevation;
            (span.contains(piece.position) && overlaps).then(|| {
                let what = match kind {
                    OpeningKind::Door => "no vão da porta",
                    OpeningKind::Window => "sobre o vidro da janela",
                    OpeningKind::Passage => "no vão aberto",
                };
                format!(
                    "{} {} está {what} {} ({}): não há parede ali para a caixa; mova-o para o lado do vão, ou abaixo do peitoril.",
                    piece.name, piece.id, f.name, f.id
                )
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::Wall;
    use crate::furniture::Opening;
    use crate::ids::{FurnitureId, WallId};

    fn outlet(elevation: f64, at: (f64, f64)) -> Furniture {
        Furniture {
            id: FurnitureId(10),
            catalog: "outlet-low".into(),
            name: "Tomada".into(),
            position: Point2::new(at.0, at.1),
            elevation,
            width: 10.0,
            depth: 4.0,
            height: 10.0,
            ..Furniture::default()
        }
    }

    #[test]
    fn a_point_on_glass_or_in_an_opening_span_is_refused_and_under_a_sill_is_not() {
        let mut home = Home::default();
        home.walls.push(Wall::new(
            WallId(1),
            Point2::new(0.0, 0.0),
            Point2::new(400.0, 0.0),
        ));
        let mut window = Furniture {
            id: FurnitureId(20),
            catalog: "window".into(),
            name: "Janela".into(),
            position: Point2::new(100.0, 0.0),
            elevation: 110.0,
            width: 120.0,
            depth: 15.0,
            height: 100.0,
            ..Furniture::default()
        };
        window.opening = Some(Opening {
            kind: OpeningKind::Window,
            ..Opening::default()
        });
        let mut door = Furniture {
            id: FurnitureId(21),
            catalog: "door".into(),
            name: "Porta".into(),
            position: Point2::new(300.0, 0.0),
            width: 80.0,
            depth: 15.0,
            height: 210.0,
            ..Furniture::default()
        };
        door.opening = Some(Opening::default());
        home.furniture = vec![window, door];

        assert!(
            blocked(&home, &outlet(130.0, (100.0, 3.0))).is_some(),
            "on the pane"
        );
        assert!(
            blocked(&home, &outlet(30.0, (100.0, 3.0))).is_none(),
            "under the sill"
        );
        assert!(
            blocked(&home, &outlet(30.0, (300.0, 3.0))).is_some(),
            "in the door"
        );
        assert!(
            blocked(&home, &outlet(30.0, (200.0, 3.0))).is_none(),
            "on the wall between"
        );
        assert!(
            blocked(&home, &outlet(30.0, (200.0, 150.0))).is_none(),
            "in no wall at all"
        );

        // A glass wall takes nothing.
        home.walls[0].apply_type(
            crate::materials::WALL_TYPES
                .iter()
                .find(|t| t.family == WallFamily::Glass)
                .unwrap(),
        );
        let why = blocked(&home, &outlet(30.0, (200.0, 3.0))).unwrap();
        assert!(why.contains("vidro"), "{why}");
        // A glass railing standing free takes nothing either.
        let mut open = Home::default();
        open.furniture.push(Furniture {
            id: FurnitureId(30),
            catalog: "glass-railing".into(),
            name: "Guarda-corpo".into(),
            position: Point2::new(0.0, 0.0),
            width: 200.0,
            depth: 2.0,
            height: 110.0,
            ..Furniture::default()
        });
        assert!(blocked(&open, &outlet(30.0, (50.0, 4.0))).is_some());
        assert!(blocked(&open, &outlet(30.0, (50.0, 80.0))).is_none());
        // A ceiling light is not set into walls.
        let mut light = outlet(270.0, (200.0, 3.0));
        light.catalog = "light-ceiling".into();
        assert!(blocked(&home, &light).is_none());
    }
}
