//! Layout checks that make a plan usable, not just drawable: pieces that
//! overlap, pieces stuck in walls and doors that cannot open.

use geo::{Area, BooleanOps, Coord, LineString, Polygon};

use crate::furniture::Furniture;
use crate::geometry::Point2;
use crate::home::Home;
use crate::ids::{FurnitureId, WallId};

/// Something worth fixing in a layout.
#[derive(Debug, Clone, PartialEq)]
pub enum Issue {
    /// Two pieces occupy the same space (same floor area and height range).
    Overlap(FurnitureId, FurnitureId),
    /// A piece that is not a door or window goes through a wall.
    InWall(FurnitureId, WallId),
    /// A piece sits where a door leaf swings.
    BlocksDoor { door: FurnitureId, by: FurnitureId },
    /// A piece is outside every room (only reported when rooms exist).
    OutsideRooms(FurnitureId),
}

/// Area below which a contact is ignored (touching pieces are fine), cm².
const MIN_OVERLAP: f64 = 25.0;
/// Pieces this thin (rugs, mats) never collide.
const FLAT: f64 = 2.0;

fn polygon(points: &[Point2]) -> Polygon<f64> {
    let mut coords: Vec<Coord<f64>> = points.iter().map(|p| Coord { x: p.x, y: p.y }).collect();
    if let Some(first) = coords.first().copied() {
        coords.push(first);
    }
    Polygon::new(LineString::new(coords), vec![])
}

fn overlap_area(a: &Polygon<f64>, b: &Polygon<f64>) -> f64 {
    a.intersection(b).unsigned_area()
}

fn heights_overlap(a: &Furniture, b: &Furniture) -> bool {
    a.elevation < b.elevation + b.height && b.elevation < a.elevation + a.height
}

/// Floor area swept by a hinged door leaf, as a polygon (quarter discs).
pub fn door_swing(door: &Furniture) -> Option<Vec<Point2>> {
    let opening = door.opening.as_ref()?;
    if opening.sliding || opening.kind != crate::furniture::OpeningKind::Door {
        return None;
    }
    let leaves = f64::from(opening.leaves.clamp(1, 2));
    let radius = door.width / leaves;
    let front = door.depth / 2.0;
    let quarter = |hinge_x: f64, toward: f64| -> Vec<(f64, f64)> {
        let mut pts = vec![(hinge_x, front)];
        for i in 0..=8 {
            let a = f64::from(i) / 8.0 * std::f64::consts::FRAC_PI_2;
            pts.push((
                hinge_x + toward * radius * a.cos(),
                front + radius * a.sin(),
            ));
        }
        pts
    };
    let half = door.width / 2.0;
    let local: Vec<(f64, f64)> = if opening.leaves >= 2 {
        let mut pts = quarter(-half, 1.0);
        pts.extend(quarter(half, -1.0).into_iter().rev());
        pts
    } else if opening.hinge_right {
        quarter(half, -1.0)
    } else {
        quarter(-half, 1.0)
    };
    Some(local.into_iter().map(|p| door.to_plan(p)).collect())
}

pub fn check_layout(home: &Home) -> Vec<Issue> {
    let mut issues = Vec::new();
    let pieces: Vec<&Furniture> = home.furniture.iter().filter(|f| f.visible).collect();
    let footprints: Vec<Polygon<f64>> = pieces.iter().map(|f| polygon(&f.footprint())).collect();

    for (i, a) in pieces.iter().enumerate() {
        if a.is_opening() || a.height <= FLAT {
            continue;
        }
        for (j, b) in pieces.iter().enumerate().skip(i + 1) {
            if b.is_opening() || b.height <= FLAT || !heights_overlap(a, b) {
                continue;
            }
            if overlap_area(&footprints[i], &footprints[j]) > MIN_OVERLAP {
                issues.push(Issue::Overlap(a.id, b.id));
            }
        }
    }

    let outlines = home.wall_outlines();
    for (i, piece) in pieces.iter().enumerate() {
        if piece.is_opening() {
            continue;
        }
        for (wall, outline) in home.walls.iter().zip(&outlines) {
            if outline.len() >= 3
                && piece.elevation < wall.height
                && overlap_area(&footprints[i], &polygon(outline)) > MIN_OVERLAP
            {
                issues.push(Issue::InWall(piece.id, wall.id));
            }
        }
    }

    for door in pieces.iter().filter(|f| f.is_opening()) {
        let Some(swing) = door_swing(door) else {
            continue;
        };
        let swing = polygon(&swing);
        for (i, piece) in pieces.iter().enumerate() {
            if piece.is_opening()
                || piece.height <= FLAT
                || piece.elevation >= door.elevation + door.height
            {
                continue;
            }
            if overlap_area(&swing, &footprints[i]) > MIN_OVERLAP {
                issues.push(Issue::BlocksDoor {
                    door: door.id,
                    by: piece.id,
                });
            }
        }
    }

    if !home.rooms.is_empty() {
        let rooms: Vec<Polygon<f64>> = home.rooms.iter().map(|r| polygon(&r.points)).collect();
        for piece in pieces.iter().filter(|f| !f.is_opening()) {
            let center = geo::Point::new(piece.position.x, piece.position.y);
            if !rooms.iter().any(|r| geo::Contains::contains(r, &center)) {
                issues.push(Issue::OutsideRooms(piece.id));
            }
        }
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::{Room, Wall};
    use crate::furniture::{Opening, align_to_wall};
    use crate::ids::RoomId;

    fn piece(id: u64, at: (f64, f64), size: (f64, f64, f64)) -> Furniture {
        Furniture {
            id: FurnitureId(id),
            catalog: "box".into(),
            name: "Caixa".into(),
            position: Point2::new(at.0, at.1),
            elevation: 0.0,
            angle: 0.0,
            width: size.0,
            depth: size.1,
            height: size.2,
            mirrored: false,
            color: None,
            opening: None,
            model: None,
            visible: true,
            level: None,
            ..Default::default()
        }
    }

    fn room_home() -> Home {
        let mut home = Home::default();
        let pts = [(0.0, 0.0), (500.0, 0.0), (500.0, 400.0), (0.0, 400.0)];
        for i in 0..4 {
            let (a, b) = (pts[i], pts[(i + 1) % 4]);
            let id = home.new_wall_id();
            home.walls
                .push(Wall::new(id, Point2::new(a.0, a.1), Point2::new(b.0, b.1)));
        }
        home.rooms.push(Room::new(
            RoomId(90),
            "Sala",
            pts.iter().map(|&(x, y)| Point2::new(x, y)).collect(),
        ));
        home
    }

    #[test]
    fn finds_overlaps_but_not_stacked_or_touching_pieces() {
        let mut home = room_home();
        home.furniture
            .push(piece(10, (200.0, 200.0), (100.0, 100.0, 75.0)));
        home.furniture
            .push(piece(11, (250.0, 200.0), (100.0, 100.0, 75.0))); // overlaps 10
        home.furniture
            .push(piece(12, (350.0, 200.0), (100.0, 100.0, 75.0))); // touches 11
        let mut shelf = piece(13, (200.0, 200.0), (100.0, 40.0, 50.0));
        shelf.elevation = 150.0; // above 10
        home.furniture.push(shelf);
        home.furniture
            .push(piece(14, (200.0, 200.0), (200.0, 140.0, 1.0))); // rug

        let issues = check_layout(&home);
        assert_eq!(
            issues,
            vec![Issue::Overlap(FurnitureId(10), FurnitureId(11))],
            "{issues:?}"
        );
    }

    #[test]
    fn finds_pieces_in_walls_blocked_doors_and_outside_rooms() {
        let mut home = room_home();
        home.furniture
            .push(piece(20, (250.0, 10.0), (100.0, 60.0, 80.0))); // pokes into top wall
        let mut door = piece(21, (0.0, 0.0), (80.0, 15.0, 210.0));
        door.opening = Some(Opening::default());
        let left_wall = home.walls[3].clone(); // (0,400) -> (0,0)
        align_to_wall(&mut door, &left_wall, 200.0);
        home.furniture.push(door);
        home.furniture
            .push(piece(22, (40.0, 180.0), (40.0, 40.0, 80.0))); // in the swing
        home.furniture
            .push(piece(23, (900.0, 900.0), (40.0, 40.0, 40.0))); // outside

        let issues = check_layout(&home);
        assert!(
            issues.contains(&Issue::InWall(FurnitureId(20), WallId(1))),
            "{issues:?}"
        );
        assert!(
            issues.contains(&Issue::OutsideRooms(FurnitureId(23))),
            "{issues:?}"
        );
        let swing_side = door_swing(&home.furniture[1]).unwrap();
        assert!(
            swing_side.iter().all(|p| p.x >= -8.0),
            "door swings into the room: {swing_side:?}"
        );
        assert!(
            issues.contains(&Issue::BlocksDoor {
                door: FurnitureId(21),
                by: FurnitureId(22)
            }),
            "{issues:?}"
        );
        assert!(
            !issues
                .iter()
                .any(|i| matches!(i, Issue::InWall(FurnitureId(21), _))),
            "doors belong in walls"
        );
    }
}
