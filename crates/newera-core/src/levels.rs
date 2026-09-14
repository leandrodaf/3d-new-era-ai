//! Floors of multi-storey houses: room floors with stair holes.

use geo::{BooleanOps, Coord, LineString, Polygon, TriangulateEarcut};

use crate::geometry::Point2;
use crate::home::Home;
use crate::ids::{LevelId, RoomId};

/// A floor area ready to render: outline, holes and triangles (plan cm).
#[derive(Debug, Clone, PartialEq)]
pub struct FloorShape {
    /// Room this floor belongs to.
    pub room: RoomId,
    pub exterior: Vec<Point2>,
    pub holes: Vec<Vec<Point2>>,
    pub triangles: Vec<[Point2; 3]>,
}

fn ring(points: &[Point2]) -> LineString<f64> {
    let mut coords: Vec<Coord<f64>> = points.iter().map(|p| Coord { x: p.x, y: p.y }).collect();
    if let Some(first) = coords.first().copied() {
        coords.push(first);
    }
    LineString::new(coords)
}

fn open_ring(line: &LineString<f64>) -> Vec<Point2> {
    let mut pts: Vec<Point2> = line.coords().map(|c| Point2::new(c.x, c.y)).collect();
    if pts.len() > 1 && pts.first() == pts.last() {
        pts.pop();
    }
    pts
}

/// Footprints of stairs that climb through the floor of `level`: stairs on a
/// lower storey whose top reaches this storey's floor.
pub fn stair_holes(home: &Home, level: Option<LevelId>) -> Vec<Vec<Point2>> {
    let floor = home.elevation_of(level);
    home.furniture
        .iter()
        .filter(|f| f.visible && f.is_stairs())
        .filter(|f| {
            let base = home.elevation_of(f.level);
            base < floor - 1.0 && base + f.elevation + f.height >= floor - 1.0
        })
        .map(|f| f.footprint().to_vec())
        .collect()
}

/// Floor shapes of every visible room floor on `level`, with stair holes cut out.
pub fn floor_shapes(home: &Home, level: Option<LevelId>) -> Vec<FloorShape> {
    let holes: Vec<Polygon<f64>> = stair_holes(home, level)
        .iter()
        .map(|h| Polygon::new(ring(h), vec![]))
        .collect();
    home.rooms
        .iter()
        .filter(|r| r.floor_visible && r.points.len() >= 3 && home.on_level(r.level, level))
        .flat_map(|room| {
            let mut shape = geo::MultiPolygon::new(vec![Polygon::new(ring(&room.points), vec![])]);
            for hole in &holes {
                shape = shape.difference(hole);
            }
            let room_id = room.id;
            shape.into_iter().map(move |poly| {
                let raw = poly.earcut_triangles_raw();
                let vertex = |i: usize| {
                    let [x, y] = raw.vertices[i];
                    Point2::new(x, y)
                };
                let triangles = raw
                    .triangle_indices
                    .chunks(3)
                    .map(|t| [vertex(t[0]), vertex(t[1]), vertex(t[2])])
                    .collect();
                FloorShape {
                    room: room_id,
                    exterior: open_ring(poly.exterior()),
                    holes: poly.interiors().iter().map(open_ring).collect(),
                    triangles,
                }
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::{Level, Room};
    use crate::furniture::Furniture;
    use crate::geometry::polygon_area;
    use crate::ids::FurnitureId;

    #[test]
    fn stairs_cut_a_hole_in_the_floor_above_only() {
        let mut home = Home::default();
        let ground = Level {
            id: LevelId(1),
            name: "Térreo".into(),
            elevation: 0.0,
            height: 250.0,
            floor_thickness: 12.0,
            ..Default::default()
        };
        let upper = Level {
            id: LevelId(2),
            name: "1º".into(),
            elevation: 262.0,
            height: 250.0,
            floor_thickness: 12.0,
            ..Default::default()
        };
        home.levels = vec![ground, upper];
        let square = |id, level| {
            let mut r = Room::new(
                RoomId(id),
                "Sala",
                vec![
                    Point2::new(0.0, 0.0),
                    Point2::new(500.0, 0.0),
                    Point2::new(500.0, 400.0),
                    Point2::new(0.0, 400.0),
                ],
            );
            r.level = level;
            r
        };
        home.rooms = vec![square(10, None), square(11, Some(LevelId(2)))];
        home.furniture.push(Furniture {
            id: FurnitureId(20),
            catalog: "stairs".into(),
            name: "Escada".into(),
            position: Point2::new(250.0, 200.0),
            elevation: 0.0,
            angle: 0.0,
            width: 90.0,
            depth: 300.0,
            height: 270.0,
            mirrored: false,
            color: None,
            opening: None,
            model: None,
            visible: true,
            level: None,
            ..Default::default()
        });

        let ground_floor = floor_shapes(&home, Some(LevelId(1)));
        assert_eq!(ground_floor.len(), 1);
        assert!(ground_floor[0].holes.is_empty());

        let upper_floor = floor_shapes(&home, Some(LevelId(2)));
        assert_eq!(upper_floor.len(), 1);
        assert_eq!(upper_floor[0].holes.len(), 1, "stairs open the upper floor");
        let area: f64 = upper_floor[0]
            .triangles
            .iter()
            .map(|t| polygon_area(t))
            .sum();
        assert!(
            (area - (500.0 * 400.0 - 90.0 * 300.0)).abs() < 1e-3,
            "{area}"
        );
    }
}
