//! Room detection: the enclosed space around a point, bounded by the inner
//! faces of the walls (like double-clicking inside a room in a CAD tool).

use geo::{Contains, Coord, LineString, Polygon};

use crate::elements::Wall;
use crate::geometry::{Point2, signed_area};
use crate::joins::wall_outlines;

/// Polygon of the space enclosed by walls that contains `point`, or `None`
/// when the point is not fully surrounded (open walls, outside, or on a wall).
pub fn detect_room(walls: &[Wall], point: Point2) -> Option<Vec<Point2>> {
    let polygons: Vec<Polygon<f64>> = wall_outlines(walls)
        .into_iter()
        .filter(|outline| outline.len() >= 3 && signed_area(outline).abs() > 1e-6)
        .map(|outline| Polygon::new(ring(&outline), vec![]))
        .collect();
    if polygons.is_empty() {
        return None;
    }
    let union = geo::unary_union(&polygons);
    let target = geo::Point::new(point.x, point.y);

    // Holes in the union of all wall footprints are exactly the enclosed rooms.
    // Nested holes can't happen (a hole can't contain walls of the same
    // polygon), but separate buildings can, so pick the smallest match.
    union
        .iter()
        .flat_map(|poly| poly.interiors().iter())
        .filter(|hole| Polygon::new((*hole).clone(), vec![]).contains(&target))
        .map(simplify)
        .min_by(|a, b| signed_area(a).abs().total_cmp(&signed_area(b).abs()))
}

fn ring(points: &[Point2]) -> LineString<f64> {
    let mut coords: Vec<Coord<f64>> = points.iter().map(|p| Coord { x: p.x, y: p.y }).collect();
    coords.push(coords[0]);
    LineString::new(coords)
}

/// Ring to open polygon, dropping duplicate and collinear vertices and
/// rounding to 0.01 cm to hide floating-point noise from the boolean ops.
fn simplify(ring: &LineString<f64>) -> Vec<Point2> {
    let round = |v: f64| (v * 100.0).round() / 100.0;
    let mut points: Vec<Point2> = ring
        .coords()
        .map(|c| Point2::new(round(c.x), round(c.y)))
        .collect();
    if points.len() > 1 && points.first() == points.last() {
        points.pop();
    }
    points.dedup();
    let mut changed = true;
    while changed && points.len() > 3 {
        changed = false;
        let n = points.len();
        if let Some(i) = (0..n).find(|&i| {
            let (a, b, c) = (points[(i + n - 1) % n], points[i], points[(i + 1) % n]);
            let cross = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
            cross.abs() < 1e-6 * a.distance(c).max(1.0)
        }) {
            points.remove(i);
            changed = true;
        }
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::polygon_area;
    use crate::ids::WallId;

    fn walls_from(polyline: &[(f64, f64)], closed: bool, thickness: f64) -> Vec<Wall> {
        let mut points: Vec<Point2> = polyline.iter().map(|&(x, y)| Point2::new(x, y)).collect();
        if closed {
            points.push(points[0]);
        }
        points
            .windows(2)
            .enumerate()
            .map(|(i, pair)| Wall {
                thickness,
                ..Wall::new(WallId(i as u64 + 1), pair[0], pair[1])
            })
            .collect()
    }

    #[test]
    fn detects_inner_faces_of_a_closed_square() {
        let walls = walls_from(
            &[(0.0, 0.0), (400.0, 0.0), (400.0, 300.0), (0.0, 300.0)],
            true,
            20.0,
        );
        let room = detect_room(&walls, Point2::new(200.0, 150.0)).expect("room");
        assert_eq!(room.len(), 4);
        assert!(
            (polygon_area(&room) - 380.0 * 280.0).abs() < 0.1,
            "{}",
            polygon_area(&room)
        );
    }

    #[test]
    fn open_walls_or_outside_points_detect_nothing() {
        let open = walls_from(
            &[(0.0, 0.0), (400.0, 0.0), (400.0, 300.0), (0.0, 300.0)],
            false,
            20.0,
        );
        assert!(detect_room(&open, Point2::new(200.0, 150.0)).is_none());
        let closed = walls_from(
            &[(0.0, 0.0), (400.0, 0.0), (400.0, 300.0), (0.0, 300.0)],
            true,
            20.0,
        );
        assert!(detect_room(&closed, Point2::new(900.0, 150.0)).is_none());
    }

    #[test]
    fn partition_wall_splits_the_space_even_without_shared_endpoints() {
        let mut walls = walls_from(
            &[(0.0, 0.0), (600.0, 0.0), (600.0, 400.0), (0.0, 400.0)],
            true,
            20.0,
        );
        // Partition touching the outer walls' middles (T junctions by overlap).
        walls.push(Wall {
            thickness: 10.0,
            ..Wall::new(
                WallId(99),
                Point2::new(250.0, 0.0),
                Point2::new(250.0, 400.0),
            )
        });
        let left = detect_room(&walls, Point2::new(100.0, 200.0)).expect("left room");
        let right = detect_room(&walls, Point2::new(400.0, 200.0)).expect("right room");
        assert!(
            (polygon_area(&left) - 235.0 * 380.0).abs() < 0.1,
            "{}",
            polygon_area(&left)
        );
        assert!(
            (polygon_area(&right) - 335.0 * 380.0).abs() < 0.1,
            "{}",
            polygon_area(&right)
        );
    }
}
