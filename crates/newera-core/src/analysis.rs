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
/// How far a piece may press into a wall face and still count as against it, cm.
const WALL_TOLERANCE: f64 = 2.0;
/// Pieces this thin (rugs, mats) never collide.
const FLAT: f64 = 2.0;

fn polygon(points: &[Point2]) -> Polygon<f64> {
    let mut coords: Vec<Coord<f64>> = points.iter().map(|p| Coord { x: p.x, y: p.y }).collect();
    if let Some(first) = coords.first().copied() {
        coords.push(first);
    }
    Polygon::new(LineString::new(coords), vec![])
}

fn heights_overlap(a: &Furniture, b: &Furniture) -> bool {
    let ((a0, a1), (b0, b1)) = (a.height_range(), b.height_range());
    a0 < b1 && b0 < a1
}

fn centroid(polygon: &Polygon<f64>) -> Option<Point2> {
    geo::Centroid::centroid(polygon).map(|c| Point2::new(c.x(), c.y()))
}

/// Height of a wall's top above the floor at a plan point.
fn wall_top_at(wall: &crate::elements::Wall, p: Point2) -> f64 {
    let len = wall.start.distance(wall.end).max(1e-9);
    let t = (((p.x - wall.start.x) * (wall.end.x - wall.start.x)
        + (p.y - wall.start.y) * (wall.end.y - wall.start.y))
        / (len * len))
        .clamp(0.0, 1.0);
    wall.height + (wall.height_at_end.unwrap_or(wall.height) - wall.height) * t
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
    // Pieces inside groups are checked one by one; pieces of the same group
    // (roof slopes, a table and its chairs) are meant to touch.
    let mut pieces: Vec<&Furniture> = Vec::new();
    let mut groups: Vec<usize> = Vec::new();
    for (g, top) in home.furniture.iter().enumerate() {
        for leaf in top.visible_leaves() {
            pieces.push(leaf);
            groups.push(g);
        }
    }
    let footprints: Vec<Polygon<f64>> = pieces
        .iter()
        .map(|f| polygon(&f.projected_footprint()))
        .collect();

    for (i, a) in pieces.iter().enumerate() {
        if a.is_opening() || a.height <= FLAT {
            continue;
        }
        for (j, b) in pieces.iter().enumerate().skip(i + 1) {
            if b.is_opening()
                || b.height <= FLAT
                || groups[i] == groups[j]
                || !heights_overlap(a, b)
            {
                continue;
            }
            let shared = footprints[i].intersection(&footprints[j]);
            if shared.unsigned_area() <= MIN_OVERLAP {
                continue;
            }
            // Tilted pieces only collide if they are at the same height there.
            let meet = shared.iter().next().and_then(centroid).is_none_or(|at| {
                a.underside_at(at) < b.top_at(at) && b.underside_at(at) < a.top_at(at)
            });
            if meet {
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
            if outline.len() < 3 {
                continue;
            }
            let shared = footprints[i].intersection(&polygon(outline));
            if shared.unsigned_area() <= MIN_OVERLAP {
                continue;
            }
            // Resting against a wall is not being in it: only count pieces that
            // go more than a couple of centimeters into it.
            let depth = shared
                .iter()
                .filter_map(geo::MinimumRotatedRect::minimum_rotated_rect)
                .map(|r| {
                    let c: Vec<_> = r.exterior().coords().copied().collect();
                    let a = (c[1].x - c[0].x).hypot(c[1].y - c[0].y);
                    let b = (c[2].x - c[1].x).hypot(c[2].y - c[1].y);
                    a.min(b)
                })
                .fold(0.0, f64::max);
            if depth <= WALL_TOLERANCE {
                continue;
            }
            // Over the shared area, is the piece below the wall top? Tilted
            // pieces (rafters, roof slopes) are measured right there.
            let Some(at) = shared.iter().next().and_then(centroid) else {
                continue;
            };
            if piece.underside_at(at) < wall_top_at(wall, at) - 1.0 {
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
            // Below the door's sill (footings under a raised floor) is out of its way.
            if piece.is_opening()
                || piece.height <= FLAT
                || piece.height_range().1 <= door.elevation + 1.0
            {
                continue;
            }
            let shared = swing.intersection(&footprints[i]);
            let high_enough = shared
                .iter()
                .next()
                .and_then(centroid)
                .is_some_and(|at| piece.underside_at(at) >= door.elevation + door.height);
            if shared.unsigned_area() > MIN_OVERLAP && !high_enough {
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
    fn pieces_against_a_wall_or_below_a_raised_door_are_fine() {
        let mut home = room_home();
        // A wardrobe pressing 1 cm into the top wall's face: against it, not in it.
        let face = home.walls[0].thickness / 2.0;
        home.furniture
            .push(piece(30, (250.0, face - 1.0 + 30.0), (180.0, 60.0, 220.0)));
        // 5 cm in: that one is in the wall.
        home.furniture
            .push(piece(31, (100.0, face - 5.0 + 20.0), (60.0, 40.0, 80.0)));
        let mut door = piece(32, (0.0, 0.0), (80.0, 15.0, 210.0));
        door.opening = Some(Opening::default());
        door.elevation = 55.0;
        let left_wall = home.walls[3].clone();
        align_to_wall(&mut door, &left_wall, 200.0);
        home.furniture.push(door);
        // A footing under the raised floor, right in the swing.
        home.furniture
            .push(piece(33, (40.0, 180.0), (40.0, 40.0, 50.0)));
        let issues = check_layout(&home);
        assert!(
            !issues
                .iter()
                .any(|i| matches!(i, Issue::InWall(FurnitureId(30), _))),
            "{issues:?}"
        );
        assert!(
            issues
                .iter()
                .any(|i| matches!(i, Issue::InWall(FurnitureId(31), _))),
            "{issues:?}"
        );
        assert!(
            !issues.iter().any(|i| matches!(
                i,
                Issue::BlocksDoor {
                    by: FurnitureId(33),
                    ..
                }
            )),
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

#[cfg(test)]
mod tilt_tests {
    use super::*;
    use crate::elements::Wall;
    use crate::furniture::{Opening, align_to_wall, wall_cuts};
    use crate::ids::WallId;

    #[test]
    fn openings_fit_sloping_walls_and_raised_slopes_do_not_collide() {
        // Gable wall rising from 1 cm to 675 cm over 300 cm.
        let mut wall = Wall::new(WallId(1), Point2::new(0.0, 0.0), Point2::new(300.0, 0.0));
        wall.height = 1.0;
        wall.height_at_end = Some(675.0);
        let mut door = Furniture {
            id: FurnitureId(2),
            catalog: "door".into(),
            width: 80.0,
            depth: 15.0,
            height: 210.0,
            opening: Some(Opening::default()),
            ..Furniture::default()
        };
        align_to_wall(&mut door, &wall, 200.0);
        let cuts = wall_cuts(std::slice::from_ref(&wall), &[door.clone()]);
        let cut = &cuts[0][0];
        // At 160 cm along, the wall is ~360 cm high: the whole door fits.
        assert!((cut.top - 210.0).abs() < 1e-9, "{cut:?}");

        // A roof slope over the room: 45°, centered 300 cm up.
        let slope = Furniture {
            id: FurnitureId(3),
            catalog: "box".into(),
            position: Point2::new(0.0, 0.0),
            width: 400.0,
            depth: 400.0,
            height: 12.0,
            elevation: 300.0,
            pitch: -45.0,
            ..Furniture::default()
        };
        let low = slope.underside_at(Point2::new(0.0, -100.0));
        let high = slope.underside_at(Point2::new(0.0, 100.0));
        assert!(high > low + 150.0, "{low} {high}");
        let fp = slope.projected_footprint();
        let depth = fp[0].distance(fp[3]);
        assert!(depth < 400.0 * 0.8, "projected depth {depth}");
        let (bottom, top) = slope.height_range();
        assert!(bottom < 200.0 && top > 450.0, "{bottom} {top}");
        // A table under the high end of the slope does not collide with it.
        let table = Furniture {
            id: FurnitureId(4),
            catalog: "box".into(),
            position: Point2::new(0.0, 100.0),
            width: 60.0,
            depth: 60.0,
            height: 75.0,
            ..Furniture::default()
        };
        let mut home = Home::default();
        home.furniture = vec![slope, table];
        assert!(check_layout(&home).is_empty(), "{:?}", check_layout(&home));
    }
}
