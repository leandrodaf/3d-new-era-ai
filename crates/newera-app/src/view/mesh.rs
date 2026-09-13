//! Turns the home into a triangle mesh for the 3D view.
//!
//! Plan coordinates `(x, y)` in centimeters map to world `(x, height, y)` in
//! meters, with Y up.

use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use newera_core::{ElementId, Furniture, Home, LevelId, Point2, Wall, WallCut};

use super::plan::Selection;

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
const REVEAL_COLOR: [f32; 3] = [0.86, 0.85, 0.82];
const SELECTED_COLOR: [f32; 3] = [0.35, 0.62, 0.95];
const FLOOR_COLOR: [f32; 3] = [0.76, 0.64, 0.50];
const CEILING_COLOR: [f32; 3] = [0.95, 0.95, 0.94];
const GROUND_COLOR: [f32; 3] = [0.55, 0.62, 0.52];

#[allow(clippy::cast_possible_truncation)] // centimeters fit comfortably in f32
fn to_world(p: Point2, height_cm: f64) -> Vec3 {
    Vec3::new(p.x as f32, height_cm as f32, p.y as f32) * CM_TO_M
}

/// Supplies meshes for pieces with imported models; `None` falls back to the
/// catalog generator.
pub(crate) type ModelSource<'a> = &'a dyn Fn(&Furniture) -> Option<newera_catalog::Mesh>;

impl Mesh {
    pub(crate) fn from_home(home: &Home, selection: &Selection, models: ModelSource<'_>) -> Self {
        let mut mesh = Self::default();
        mesh.add_ground(home);
        // Show the storeys up to the one being edited, so its inside stays visible.
        let current = home.elevation_of(home.current_level());
        let levels: Vec<Option<LevelId>> = if home.levels.is_empty() {
            vec![None]
        } else {
            home.sorted_levels()
                .into_iter()
                .filter(|l| l.elevation <= current + 1e-6)
                .map(|l| Some(l.id))
                .collect()
        };
        for level in levels {
            let view = home.level_view(level);
            let base = home.elevation_of(level);
            let slab = level
                .and_then(|id| home.level(id))
                .map_or(0.0, |l| l.floor_thickness);
            for shape in newera_core::floor_shapes(home, level) {
                mesh.add_floor(&shape, base, if base > 0.0 { slab } else { 0.0 });
            }
            let cuts = view.wall_cuts();
            for ((wall, outline), wall_cuts) in
                view.walls.iter().zip(view.wall_outlines()).zip(&cuts)
            {
                let color = if selection.contains(&ElementId::Wall(wall.id)) {
                    SELECTED_COLOR
                } else {
                    WALL_COLOR
                };
                mesh.add_wall(&outline, wall, wall_cuts, base, color);
            }
            for piece in view.furniture.iter().filter(|f| f.visible) {
                let local = models(piece).unwrap_or_else(|| newera_catalog::piece_mesh(piece));
                let highlight = selection.contains(&ElementId::Furniture(piece.id));
                mesh.add_piece(piece, &local, base, highlight);
            }
        }
        mesh
    }

    /// A floor: its top face at `base`, and for upper storeys a slab of
    /// `thickness` below it, whose underside is the ceiling of the room below.
    fn add_floor(&mut self, shape: &newera_core::FloorShape, base: f64, thickness: f64) {
        for tri in &shape.triangles {
            let mut t = *tri;
            if newera_core::signed_area(&t) > 0.0 {
                t.swap(1, 2);
            }
            let corners = t.map(|p| to_world(p, base));
            self.add_triangle(corners, Vec3::Y, FLOOR_COLOR);
            if thickness > 0.0 {
                let below = [t[0], t[2], t[1]].map(|p| to_world(p, base - thickness));
                self.add_triangle(below, -Vec3::Y, CEILING_COLOR);
            }
        }
        if thickness > 0.0 {
            // Outer edges face out, hole edges face into the hole.
            for (ring, outward) in std::iter::once((&shape.exterior, true))
                .chain(shape.holes.iter().map(|h| (h, false)))
            {
                let mut points = up_facing(ring);
                if !outward {
                    points.reverse();
                }
                let n = points.len();
                for k in 0..n {
                    self.add_side(
                        points[k],
                        points[(k + 1) % n],
                        base - thickness,
                        base,
                        WALL_COLOR,
                    );
                }
            }
        }
    }

    fn add_triangle(&mut self, corners: [Vec3; 3], normal: Vec3, color: [f32; 3]) {
        let base = self.next_index();
        for corner in corners {
            self.vertices.push(Vertex {
                position: corner.to_array(),
                normal: normal.to_array(),
                color,
            });
        }
        self.indices.extend([base, base + 1, base + 2]);
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

    /// Extrudes a wall outline, leaving holes where doors and windows are:
    /// long faces are split around each opening, and the hole gets jambs, a
    /// sill and a lintel.
    fn add_wall(
        &mut self,
        outline: &[Point2],
        wall: &Wall,
        cuts: &[WallCut],
        base: f64,
        color: [f32; 3],
    ) {
        if outline.len() < 3 {
            return;
        }
        let points = up_facing(outline);
        let (bottom, top) = (base, base + wall.height);
        let len = wall.start.distance(wall.end).max(1e-9);
        let dir = (
            (wall.end.x - wall.start.x) / len,
            (wall.end.y - wall.start.y) / len,
        );
        let along = |p: Point2| (p.x - wall.start.x) * dir.0 + (p.y - wall.start.y) * dir.1;

        let n = points.len();
        for k in 0..n {
            let (p, q) = (points[k], points[(k + 1) % n]);
            let edge_len = p.distance(q);
            if edge_len < 1e-6 {
                continue;
            }
            let parallel = ((q.x - p.x) * dir.1 - (q.y - p.y) * dir.0).abs() / edge_len < 1e-3;
            if cuts.is_empty() || !parallel || wall.is_arc() {
                self.add_side(p, q, bottom, top, color);
                continue;
            }
            // Split the long face at every cut boundary it crosses.
            let (a, b) = (along(p), along(q));
            let (lo, hi) = (a.min(b), a.max(b));
            let mut stops = vec![a, b];
            for cut in cuts {
                for s in [cut.from, cut.to] {
                    if s > lo + 1e-6 && s < hi - 1e-6 {
                        stops.push(s);
                    }
                }
            }
            stops.sort_by(f64::total_cmp);
            if b < a {
                stops.reverse();
            }
            let at = |s: f64| {
                let t = if (b - a).abs() < 1e-9 {
                    0.0
                } else {
                    (s - a) / (b - a)
                };
                Point2::new(p.x + (q.x - p.x) * t, p.y + (q.y - p.y) * t)
            };
            for pair in stops.windows(2) {
                let (s0, s1) = (pair[0], pair[1]);
                let mid = s0.midpoint(s1);
                match cuts.iter().find(|c| c.from <= mid && c.to >= mid) {
                    Some(cut) => {
                        if base + cut.bottom > bottom {
                            self.add_side(at(s0), at(s1), bottom, base + cut.bottom, color);
                        }
                        if base + cut.top < top {
                            self.add_side(at(s0), at(s1), base + cut.top, top, color);
                        }
                    }
                    None => self.add_side(at(s0), at(s1), bottom, top, color),
                }
            }
        }
        self.add_cap(&points, top, color);
        if !wall.is_arc() {
            for cut in cuts {
                self.add_reveals(wall, dir, cut, base);
            }
        }
    }

    /// Inner faces of a hole: jambs, sill and lintel.
    fn add_reveals(&mut self, wall: &Wall, dir: (f64, f64), cut: &WallCut, base: f64) {
        let half = wall.thickness / 2.0;
        let normal = (-dir.1, dir.0);
        let point = |s: f64, side: f64| {
            Point2::new(
                wall.start.x + dir.0 * s + normal.0 * side,
                wall.start.y + dir.1 * s + normal.1 * side,
            )
        };
        #[allow(clippy::cast_possible_truncation)]
        let along = Vec3::new(dir.0 as f32, 0.0, dir.1 as f32);
        for (s, facing) in [(cut.from, along), (cut.to, -along)] {
            let (l, r) = (point(s, half), point(s, -half));
            self.add_facing(
                [
                    to_world(l, base + cut.bottom),
                    to_world(r, base + cut.bottom),
                    to_world(r, base + cut.top),
                    to_world(l, base + cut.top),
                ],
                facing,
                REVEAL_COLOR,
            );
        }
        for (z, facing) in [(cut.bottom, Vec3::Y), (cut.top, -Vec3::Y)] {
            if z <= 0.0 || z >= wall.height {
                continue;
            }
            self.add_facing(
                [
                    to_world(point(cut.from, half), base + z),
                    to_world(point(cut.to, half), base + z),
                    to_world(point(cut.to, -half), base + z),
                    to_world(point(cut.from, -half), base + z),
                ],
                facing,
                REVEAL_COLOR,
            );
        }
    }

    /// Vertical face over plan edge `p → q` (up-facing winding, so outward).
    fn add_side(&mut self, p: Point2, q: Point2, bottom: f64, top: f64, color: [f32; 3]) {
        let (pb, qb) = (to_world(p, bottom), to_world(q, bottom));
        let (pt, qt) = (to_world(p, top), to_world(q, top));
        let normal = (qb - pb).cross(Vec3::Y).normalize_or_zero();
        self.add_quad([pb, qb, qt, pt], normal, color);
    }

    /// Places a catalog or imported mesh in the world.
    fn add_piece(
        &mut self,
        piece: &Furniture,
        local: &newera_catalog::Mesh,
        floor: f64,
        highlight: bool,
    ) {
        let base = self.next_index();
        let world = |p: [f32; 3]| {
            let plan = piece.to_plan((f64::from(p[0]), f64::from(p[2])));
            to_world(plan, floor + piece.elevation + f64::from(p[1]))
        };
        for ((position, normal), color) in local
            .positions
            .iter()
            .zip(&local.normals)
            .zip(&local.colors)
        {
            let tip = world([
                position[0] + normal[0],
                position[1] + normal[1],
                position[2] + normal[2],
            ]);
            let at = world(*position);
            let world_normal = (tip - at).normalize_or_zero();
            let color = if highlight {
                [
                    color[0] * 0.55 + 0.45 * SELECTED_COLOR[0],
                    color[1] * 0.55 + 0.45 * SELECTED_COLOR[1],
                    color[2] * 0.55 + 0.45 * SELECTED_COLOR[2],
                ]
            } else {
                *color
            };
            self.vertices.push(Vertex {
                position: at.to_array(),
                normal: world_normal.to_array(),
                color,
            });
        }
        // Mirroring flips handedness; reorder triangles so faces keep
        // pointing outward.
        for tri in local.indices.chunks(3) {
            if piece.mirrored {
                self.indices
                    .extend([base + tri[0], base + tri[2], base + tri[1]]);
            } else {
                self.indices
                    .extend([base + tri[0], base + tri[1], base + tri[2]]);
            }
        }
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

    /// Adds a quad, fixing the corner order so it faces `normal`.
    fn add_facing(&mut self, mut corners: [Vec3; 4], normal: Vec3, color: [f32; 3]) {
        let geometric = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
        if geometric.dot(normal) < 0.0 {
            corners.reverse();
        }
        self.add_quad(corners, normal, color);
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
    use newera_core::{Room, align_to_wall};

    use super::*;

    fn no_models(_: &Furniture) -> Option<newera_catalog::Mesh> {
        None
    }

    fn build(home: &Home) -> Mesh {
        Mesh::from_home(home, &Selection::new(), &no_models)
    }

    fn assert_outward(mesh: &Mesh) {
        for tri in mesh.indices.chunks(3) {
            let [a, b, c] = [0, 1, 2].map(|k| Vec3::from(mesh.vertices[tri[k] as usize].position));
            let geometric = (b - a).cross(c - a);
            let declared = Vec3::from(mesh.vertices[tri[0] as usize].normal);
            assert!(geometric.dot(declared) >= -1e-9, "triangle faces inward");
        }
    }

    fn wall_home() -> Home {
        let mut home = Home::default();
        for (a, b) in [((0.0, 0.0), (600.0, 0.0)), ((600.0, 0.0), (600.0, 400.0))] {
            let id = home.new_wall_id();
            home.walls
                .push(Wall::new(id, Point2::new(a.0, a.1), Point2::new(b.0, b.1)));
        }
        home
    }

    #[test]
    fn empty_home_still_has_ground() {
        let mesh = build(&Home::default());
        assert_eq!(mesh.vertices.len(), 4);
        assert_eq!(mesh.indices.len(), 6);
    }

    #[test]
    fn walls_rooms_and_furniture_face_outward() {
        let mut home = wall_home();
        let id = home.new_room_id();
        home.rooms.push(Room::new(
            id,
            "L",
            [
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
        ));
        for (i, item) in newera_catalog::CATALOG.iter().enumerate() {
            let id = home.new_furniture_id();
            #[allow(clippy::cast_precision_loss)]
            let mut piece = item.instantiate(id, Point2::new(100.0 + i as f64 * 7.0, 200.0));
            #[allow(clippy::cast_precision_loss)]
            let turn = i as f64;
            piece.angle = 37.0 * turn;
            piece.mirrored = i % 2 == 0;
            home.furniture.push(piece);
        }
        let mesh = build(&home);
        assert!(mesh.indices.len() > 1000);
        assert_outward(&mesh);
    }

    #[test]
    fn a_window_leaves_a_hole_in_the_wall() {
        let mut home = wall_home();
        let wall = home.walls[0].clone();
        let id = home.new_furniture_id();
        let mut window = newera_catalog::find("window")
            .unwrap()
            .instantiate(id, Point2::new(0.0, 0.0));
        align_to_wall(&mut window, &wall, 300.0);
        home.furniture.push(window);

        let mut walls_only = home.clone();
        walls_only.furniture.clear();
        let solid = build(&walls_only);
        let holed = build(&home);
        assert_outward(&holed);

        // No wall triangle may cross the middle of the hole (x=300, y=0, h=155 cm).
        let hole = Vec3::new(3.0, 1.55, 0.0);
        let hits = |mesh: &Mesh, ids: std::ops::Range<usize>| {
            mesh.indices[ids].chunks(3).any(|tri| {
                let [a, b, c] =
                    [0, 1, 2].map(|k| Vec3::from(mesh.vertices[tri[k] as usize].position));
                point_in_triangle_xy(hole, a, b, c)
            })
        };
        let wall_tris = |mesh: &Mesh| 6..mesh.indices.len();
        assert!(
            hits(&solid, wall_tris(&solid)),
            "solid wall covers the point"
        );
        // Count only wall geometry: furniture (the window frame/glass) is appended after walls.
        let wall_end =
            holed.indices.len() - newera_catalog::piece_mesh(&home.furniture[0]).indices.len();
        assert!(!hits(&holed, 6..wall_end), "wall has a hole there");
    }

    /// Whether `p` lies inside triangle abc on a vertical plane facing ±z.
    fn point_in_triangle_xy(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> bool {
        if (a.z - p.z).abs() > 0.08 || (b.z - p.z).abs() > 0.08 || (c.z - p.z).abs() > 0.08 {
            return false;
        }
        let sign = |p1: Vec3, p2: Vec3, p3: Vec3| {
            (p1.x - p3.x) * (p2.y - p3.y) - (p2.x - p3.x) * (p1.y - p3.y)
        };
        let (d1, d2, d3) = (sign(p, a, b), sign(p, b, c), sign(p, c, a));
        let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
        let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
        !(neg && pos)
    }

    #[test]
    fn wall_top_is_at_wall_height_in_meters() {
        let mesh = build(&wall_home());
        let max_y = mesh
            .vertices
            .iter()
            .map(|v| v.position[1])
            .fold(f32::MIN, f32::max);
        assert!((max_y - 2.5).abs() < 1e-5);
    }
}

#[cfg(test)]
mod level_tests {
    use newera_core::{Command, Document, Room, ops};

    use super::*;

    #[test]
    fn storeys_stack_with_outward_slabs() {
        let mut doc = Document::default();
        let square = |doc: &mut Document| {
            let pts = [(0.0, 0.0), (500.0, 0.0), (500.0, 400.0), (0.0, 400.0)];
            let mut commands: Vec<Command> = (0..4)
                .map(|i| {
                    let (a, b) = (pts[i], pts[(i + 1) % 4]);
                    Command::insert(Wall::new(
                        doc.new_wall_id(),
                        Point2::new(a.0, a.1),
                        Point2::new(b.0, b.1),
                    ))
                })
                .collect();
            commands.push(Command::insert(Room::new(
                doc.new_room_id(),
                "Sala",
                pts.iter().map(|&(x, y)| Point2::new(x, y)).collect(),
            )));
            doc.execute(Command::Batch { commands }).unwrap();
        };
        square(&mut doc);
        ops::add_level(&mut doc, None, None).unwrap();
        square(&mut doc);
        let stairs = newera_catalog::find("stairs")
            .unwrap()
            .instantiate(doc.new_furniture_id(), Point2::new(250.0, 200.0));
        doc.select_level(doc.home().base_level());
        doc.execute(Command::insert(stairs)).unwrap();
        doc.select_level(doc.home().sorted_levels().last().map(|l| l.id));

        let mesh = Mesh::from_home(doc.home(), &Selection::new(), &|_| None);
        let max_y = mesh
            .vertices
            .iter()
            .map(|v| v.position[1])
            .fold(f32::MIN, f32::max);
        assert!(
            (max_y - 5.12).abs() < 1e-3,
            "upper walls top at 262+250 cm: {max_y}"
        );
        for tri in mesh.indices.chunks(3) {
            let [a, b, c] = [0, 1, 2].map(|k| Vec3::from(mesh.vertices[tri[k] as usize].position));
            let declared = Vec3::from(mesh.vertices[tri[0] as usize].normal);
            assert!(
                (b - a).cross(c - a).dot(declared) >= -1e-9,
                "face points inward"
            );
        }
        // Selecting the ground floor hides the storey above.
        doc.select_level(doc.home().base_level());
        let ground_only = Mesh::from_home(doc.home(), &Selection::new(), &|_| None);
        let max_y = ground_only
            .vertices
            .iter()
            .map(|v| v.position[1])
            .fold(f32::MIN, f32::max);
        // Only the ground storey remains; the stairs (280 cm) are its tallest piece.
        assert!(max_y <= 2.801, "{max_y}");
    }
}
