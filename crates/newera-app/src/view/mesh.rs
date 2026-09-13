//! Turns the 2D home model into a triangle mesh for the 3D view.
//!
//! Plan coordinates `(x, y)` in centimeters map to world `(x, height, y)` in
//! meters, with Y up.

use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use newera_core::{Home, Point2, Room, WallId};

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(crate) struct Vertex {
    pub(crate) position: [f32; 3],
    pub(crate) normal: [f32; 3],
    pub(crate) color: [f32; 3],
}

#[derive(Debug, Default)]
pub(crate) struct Mesh {
    pub(crate) vertices: Vec<Vertex>,
    pub(crate) indices: Vec<u32>,
}

const CM_TO_M: f32 = 0.01;
const WALL_COLOR: [f32; 3] = [0.92, 0.91, 0.88];
const SELECTED_WALL_COLOR: [f32; 3] = [0.35, 0.62, 0.95];
const FLOOR_COLOR: [f32; 3] = [0.76, 0.64, 0.50];
const GROUND_COLOR: [f32; 3] = [0.55, 0.62, 0.52];

#[allow(clippy::cast_possible_truncation)] // centimeters fit comfortably in f32
fn to_world(p: Point2, height_cm: f64) -> Vec3 {
    Vec3::new(p.x as f32, height_cm as f32, p.y as f32) * CM_TO_M
}

impl Mesh {
    pub(crate) fn from_home(home: &Home, selected: Option<WallId>) -> Self {
        let mut mesh = Self::default();
        mesh.add_ground(home);
        for room in &home.rooms {
            mesh.add_room_floor(room);
        }
        for (wall, outline) in home.walls.iter().zip(home.wall_outlines()) {
            let color = if Some(wall.id) == selected {
                SELECTED_WALL_COLOR
            } else {
                WALL_COLOR
            };
            mesh.add_prism(&outline, 0.0, wall.height, color);
        }
        mesh
    }

    fn add_ground(&mut self, home: &Home) {
        let (min, max) = home
            .bounds()
            .unwrap_or((Point2::new(-500.0, -500.0), Point2::new(500.0, 500.0)));
        let margin = 1000.0;
        let a = to_world(Point2::new(min.x - margin, min.y - margin), -1.0);
        let c = to_world(Point2::new(max.x + margin, max.y + margin), -1.0);
        let b = Vec3::new(c.x, a.y, a.z);
        let d = Vec3::new(a.x, a.y, c.z);
        self.add_quad([a, d, c, b], Vec3::Y, GROUND_COLOR);
    }

    fn add_room_floor(&mut self, room: &Room) {
        let points = up_facing(&room.points);
        self.add_cap(&points, 0.0, FLOOR_COLOR);
    }

    /// Extrudes a floor polygon from `bottom` to `top` (cm): side faces plus a
    /// top cap. Works for any simple polygon, including joined wall outlines.
    fn add_prism(&mut self, outline: &[Point2], bottom: f64, top: f64, color: [f32; 3]) {
        if outline.len() < 3 {
            return;
        }
        let points = up_facing(outline);
        let n = points.len();
        for k in 0..n {
            let (p, q) = (points[k], points[(k + 1) % n]);
            if p.distance(q) < 1e-6 {
                continue;
            }
            let (pb, qb) = (to_world(p, bottom), to_world(q, bottom));
            let (pt, qt) = (to_world(p, top), to_world(q, top));
            // With up-facing winding, (q - p) x Y points out of the polygon.
            let normal = (qb - pb).cross(Vec3::Y).normalize_or_zero();
            self.add_quad([pb, qb, qt, pt], normal, color);
        }
        self.add_cap(&points, top, color);
    }

    /// Horizontal face at `height` (cm); `points` must already face up.
    fn add_cap(&mut self, points: &[Point2], height: f64, color: [f32; 3]) {
        let base = self.next_index();
        for p in points {
            self.vertices.push(Vertex {
                position: to_world(*p, height).to_array(),
                normal: Vec3::Y.to_array(),
                color,
            });
        }
        for [a, b, c] in newera_core::triangulate(points) {
            let idx = |i: usize| base + u32::try_from(i).expect("index fits in u32");
            self.indices.extend([idx(a), idx(b), idx(c)]);
        }
    }

    /// Adds a quad; corners must be counter-clockwise when seen from `normal`.
    fn add_quad(&mut self, corners: [Vec3; 4], normal: Vec3, color: [f32; 3]) {
        let base = self.next_index();
        for corner in corners {
            self.vertices.push(Vertex {
                position: corner.to_array(),
                normal: normal.to_array(),
                color,
            });
        }
        self.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn next_index(&self) -> u32 {
        u32::try_from(self.vertices.len()).expect("mesh vertex count fits in u32")
    }
}

/// Plan `(x, y)` maps to world `(x, z)`, which mirrors winding: polygons with
/// a negative shoelace area are counter-clockwise when seen from above.
fn up_facing(points: &[Point2]) -> Vec<Point2> {
    if newera_core::signed_area(points) > 0.0 {
        points.iter().rev().copied().collect()
    } else {
        points.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_home_still_has_ground() {
        let mesh = Mesh::from_home(&Home::default(), None);
        assert_eq!(mesh.vertices.len(), 4);
        assert_eq!(mesh.indices.len(), 6);
    }

    fn triangle_normal(mesh: &Mesh, tri: &[u32]) -> Vec3 {
        let [a, b, c] = [0, 1, 2].map(|k| Vec3::from(mesh.vertices[tri[k] as usize].position));
        (b - a).cross(c - a)
    }

    #[test]
    fn every_triangle_agrees_with_its_vertex_normal() {
        // An L of joined walls plus a concave room: faces must point outwards.
        let mut home = Home::default();
        for (a, b) in [((0.0, 0.0), (600.0, 0.0)), ((600.0, 0.0), (600.0, 400.0))] {
            let id = home.new_wall_id();
            home.walls.push(newera_core::Wall::new(
                id,
                Point2::new(a.0, a.1),
                Point2::new(b.0, b.1),
            ));
        }
        let id = home.new_room_id();
        home.rooms.push(Room {
            id,
            name: "L".into(),
            points: [
                (0.0, 0.0),
                (600.0, 0.0),
                (600.0, 300.0),
                (300.0, 300.0),
                (300.0, 600.0),
                (0.0, 600.0),
            ]
            .iter()
            .map(|&(x, y)| Point2::new(x, y))
            .collect(),
        });
        let mesh = Mesh::from_home(&home, None);
        assert!(mesh.indices.len() > 6);
        for tri in mesh.indices.chunks(3) {
            let geometric = triangle_normal(&mesh, tri);
            let declared = Vec3::from(mesh.vertices[tri[0] as usize].normal);
            assert!(geometric.dot(declared) > 0.0, "triangle faces inward");
        }
    }

    #[test]
    fn room_floor_faces_up_for_both_windings() {
        let square = [(0.0, 0.0), (300.0, 0.0), (300.0, 300.0), (0.0, 300.0)];
        for points in [square.to_vec(), square.iter().rev().copied().collect()] {
            let room = Room {
                id: newera_core::RoomId(1),
                name: "Sala".into(),
                points: points.iter().map(|&(x, y)| Point2::new(x, y)).collect(),
            };
            let mut mesh = Mesh::default();
            mesh.add_room_floor(&room);
            for tri in mesh.indices.chunks(3) {
                let [a, b, c] =
                    [0, 1, 2].map(|k| Vec3::from(mesh.vertices[tri[k] as usize].position));
                assert!((b - a).cross(c - a).y > 0.0, "floor triangle faces down");
            }
        }
    }

    #[test]
    fn wall_top_is_at_wall_height_in_meters() {
        let mut home = Home::default();
        let id = home.new_wall_id();
        home.walls.push(newera_core::Wall::new(
            id,
            Point2::new(0.0, 0.0),
            Point2::new(400.0, 0.0),
        ));
        let mesh = Mesh::from_home(&home, None);
        let max_y = mesh
            .vertices
            .iter()
            .map(|v| v.position[1])
            .fold(f32::MIN, f32::max);
        assert!((max_y - 2.5).abs() < 1e-5);
    }
}
