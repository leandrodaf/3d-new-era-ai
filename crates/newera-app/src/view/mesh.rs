//! Turns the 2D home model into a triangle mesh for the 3D view.
//!
//! Plan coordinates `(x, y)` in centimeters map to world `(x, height, y)` in
//! meters, with Y up.

use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use newera_core::{Home, Point2, Room, Wall, WallId};

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
        for wall in &home.walls {
            let color = if Some(wall.id) == selected {
                SELECTED_WALL_COLOR
            } else {
                WALL_COLOR
            };
            mesh.add_wall(wall, color);
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

    /// Floors are triangulated as a fan, which is exact for convex rooms.
    /// Concave rooms need ear clipping — tracked in the roadmap.
    fn add_room_floor(&mut self, room: &Room) {
        if room.points.len() < 3 {
            return;
        }
        let base = self.next_index();
        // Plan (x, y) maps to world (x, z), which mirrors the winding: a
        // polygon with positive shoelace area would face down. Flip it.
        let flip = signed_area(&room.points) > 0.0;
        for p in &room.points {
            self.vertices.push(Vertex {
                position: to_world(*p, 0.0).to_array(),
                normal: Vec3::Y.to_array(),
                color: FLOOR_COLOR,
            });
        }
        let n = u32::try_from(room.points.len()).expect("room point count fits in u32");
        for i in 1..n - 1 {
            if flip {
                self.indices.extend([base, base + i + 1, base + i]);
            } else {
                self.indices.extend([base, base + i, base + i + 1]);
            }
        }
    }

    /// A wall is an extruded rectangle: 4 sides plus top.
    fn add_wall(&mut self, wall: &Wall, color: [f32; 3]) {
        let start = to_world(wall.start, 0.0);
        let end = to_world(wall.end, 0.0);
        let along = (end - start).normalize_or_zero();
        if along == Vec3::ZERO {
            return;
        }
        #[allow(clippy::cast_possible_truncation)]
        let (half, height) = (
            wall.thickness as f32 * CM_TO_M / 2.0,
            wall.height as f32 * CM_TO_M,
        );
        let side = Vec3::new(-along.z, 0.0, along.x); // perpendicular on the floor
        let up = Vec3::Y * height;

        let (s_l, s_r) = (start + side * half, start - side * half);
        let (e_l, e_r) = (end + side * half, end - side * half);

        self.add_quad([s_l, e_l, e_l + up, s_l + up], side, color);
        self.add_quad([e_r, s_r, s_r + up, e_r + up], -side, color);
        self.add_quad([s_r, s_l, s_l + up, s_r + up], -along, color);
        self.add_quad([e_l, e_r, e_r + up, e_l + up], along, color);
        self.add_quad([s_l + up, e_l + up, e_r + up, s_r + up], Vec3::Y, color);
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

fn signed_area(points: &[Point2]) -> f64 {
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(p, q)| p.x * q.y - q.x * p.y)
        .sum::<f64>()
        / 2.0
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

    #[test]
    fn wall_produces_five_faces() {
        let mut home = Home::default();
        let id = home.new_wall_id();
        home.walls.push(Wall::new(
            id,
            Point2::new(0.0, 0.0),
            Point2::new(400.0, 0.0),
        ));
        let mesh = Mesh::from_home(&home, None);
        assert_eq!(mesh.vertices.len(), 4 + 5 * 4);
        assert!(
            mesh.indices
                .iter()
                .all(|&i| (i as usize) < mesh.vertices.len())
        );
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
        home.walls.push(Wall::new(
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
