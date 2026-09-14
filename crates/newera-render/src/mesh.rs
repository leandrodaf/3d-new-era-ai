//! Turns the home into a triangle mesh for the 3D view.
//!
//! Plan coordinates `(x, y)` in centimeters map to world `(x, height, y)` in
//! meters, with Y up. Every vertex carries texture coordinates measured in
//! tiles of its material, so patterns and images keep their real size.

use bytemuck::{Pod, Zeroable};
use glam::{Vec3, Vec4};
use newera_core::{ElementId, Furniture, Home, LevelId, Material, Point2, Wall, WallCut};

/// Elements drawn highlighted.
pub type Selection = std::collections::BTreeSet<ElementId>;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    /// Linear color and opacity.
    pub color: [f32; 4],
    /// Texture coordinates in tiles.
    pub uv: [f32; 2],
    /// 0 plain, `1..` procedural pattern, [`IMAGE_BASE`]`+n` image layer `n`.
    pub kind: u32,
}

/// Material kinds at or above this sample image layer `kind - IMAGE_BASE`.
pub const IMAGE_BASE: u32 = 100;

#[derive(Debug, Default)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    /// Triangles drawn after the opaque ones, blended (glass, curtains).
    pub transparent: Vec<u32>,
    /// Image files referenced by materials, in layer order.
    pub images: Vec<String>,
}

const CM_TO_M: f32 = 0.01;
const WALL_COLOR: [f32; 3] = [0.92, 0.91, 0.88];
const REVEAL_COLOR: [f32; 3] = [0.86, 0.85, 0.82];
const SELECTED_COLOR: [f32; 3] = [0.35, 0.62, 0.95];
const FLOOR_COLOR: [f32; 3] = [0.76, 0.64, 0.50];
const CEILING_COLOR: [f32; 3] = [0.95, 0.95, 0.94];
const GROUND_COLOR: [f32; 3] = [0.55, 0.62, 0.52];
/// Room ceilings sit this far below the storey height so they don't fight
/// with the underside of the slab above.
const CEILING_GAP: f64 = 0.5;

#[allow(clippy::cast_possible_truncation)] // centimeters fit comfortably in f32
fn to_world(p: Point2, height_cm: f64) -> Vec3 {
    Vec3::new(p.x as f32, height_cm as f32, p.y as f32) * CM_TO_M
}

/// Colors are used as stored, like Sweet Home 3D does: the sRGB target then
/// brightens them, which matches how its renders look.
fn srgb_to_linear([r, g, b]: [u8; 3]) -> [f32; 3] {
    [r, g, b].map(|c| f32::from(c) / 255.0)
}

/// How a face is painted: a color, a material kind and the planar mapping
/// from world position (meters) to tile coordinates.
#[derive(Debug, Clone, Copy)]
struct Surface {
    color: [f32; 3],
    kind: u32,
    u: Vec4,
    v: Vec4,
}

impl Surface {
    fn plain(color: [f32; 3]) -> Self {
        Self {
            color,
            kind: 0,
            u: Vec4::ZERO,
            v: Vec4::ZERO,
        }
    }

    /// A material mapped on the plane spanned by `across` (horizontal, unit)
    /// and `up` (unit), both in world axes; `origin` is where tiles start.
    #[allow(clippy::cast_possible_truncation)]
    fn material(
        mesh: &mut Mesh,
        material: &Material,
        fallback: [f32; 3],
        across: Vec3,
        up: Vec3,
        origin: Vec3,
    ) -> Self {
        let kind = if let Some(image) = &material.image {
            IMAGE_BASE + mesh.image_layer(image)
        } else {
            material.pattern.map_or(0, newera_core::Pattern::index)
        };
        let color = match (material.color, &material.image) {
            (Some(color), _) => srgb_to_linear(color),
            (None, Some(_)) => [1.0; 3],
            (None, None) => material
                .pattern
                .map_or(fallback, |p| srgb_to_linear(p.default_color())),
        };
        let [w, h] = material.tile_size();
        let angle = material.angle.to_radians() as f32;
        let (sin, cos) = angle.sin_cos();
        let along = across * cos + up * sin;
        let rise = up * cos - across * sin;
        // World meters → tile units.
        let scale_u = 100.0 / w as f32;
        let scale_v = 100.0 / h as f32;
        let axis = |dir: Vec3, scale: f32| {
            let d = dir * scale;
            d.extend(-d.dot(origin))
        };
        Self {
            color,
            kind,
            u: axis(along, scale_u),
            v: axis(rise, scale_v),
        }
    }

    fn uv(&self, p: Vec3) -> [f32; 2] {
        let h = p.extend(1.0);
        [self.u.dot(h), self.v.dot(h)]
    }

    fn vertex(&self, position: Vec3, normal: Vec3) -> Vertex {
        Vertex {
            position: position.to_array(),
            normal: normal.to_array(),
            color: [self.color[0], self.color[1], self.color[2], 1.0],
            uv: self.uv(position),
            kind: self.kind,
        }
    }
}

/// Supplies meshes for pieces with imported models; `None` falls back to the
/// catalog generator.
pub type ModelSource<'a> = &'a dyn Fn(&Furniture) -> Option<newera_catalog::Mesh>;

impl Mesh {
    pub fn from_home(home: &Home, selection: &Selection, models: ModelSource<'_>) -> Self {
        let mut mesh = Self::default();
        mesh.add_ground(home);
        // Show the viewable storeys up to the one being edited (in elevation,
        // then layout order), so its inside stays visible.
        let levels: Vec<Option<LevelId>> = if home.levels.is_empty() {
            vec![None]
        } else {
            let sorted = home.sorted_levels();
            let current = home.current_level();
            let upto = sorted
                .iter()
                .position(|l| Some(l.id) == current)
                .unwrap_or(sorted.len().saturating_sub(1));
            let shown = if home.environment.all_levels_visible {
                sorted.len()
            } else {
                upto + 1
            };
            sorted
                .iter()
                .take(shown)
                .filter(|l| l.viewable)
                .map(|l| Some(l.id))
                .collect()
        };
        for level in levels {
            let view = home.level_view(level);
            let base = home.elevation_of(level);
            let storey = level.and_then(|id| home.level(id));
            let slab = storey.map_or(0.0, |l| l.floor_thickness);
            for (order, shape) in newera_core::floor_shapes(home, level)
                .into_iter()
                .enumerate()
            {
                let material = home
                    .rooms
                    .iter()
                    .find(|r| r.id == shape.room)
                    .and_then(|r| r.floor_material.clone());
                // Floors listed later sit a hair higher, so overlapping rooms
                // (a pool over a deck) show the one drawn on top, without flicker.
                #[allow(clippy::cast_precision_loss)]
                let lift = order as f64 * 0.05;
                mesh.add_floor(
                    &shape,
                    base + lift,
                    if base > 0.0 { slab } else { 0.0 },
                    material.as_ref(),
                );
            }
            let ceiling_height = storey.map_or_else(
                || view.walls.iter().map(|w| w.height).fold(0.0, f64::max),
                |l| l.height,
            );
            if ceiling_height > 0.0 {
                for room in view
                    .rooms
                    .iter()
                    .filter(|r| r.ceiling_visible && r.points.len() >= 3)
                {
                    mesh.add_ceiling(
                        &room.points,
                        base + ceiling_height - CEILING_GAP,
                        room.ceiling_material.as_ref(),
                    );
                }
            }
            let cuts = view.wall_cuts();
            for ((wall, outline), wall_cuts) in
                view.walls.iter().zip(view.wall_outlines()).zip(&cuts)
            {
                let selected = selection.contains(&ElementId::Wall(wall.id));
                mesh.add_wall(&outline, wall, wall_cuts, base, selected);
            }
            for line in view.polylines.iter().filter(|l| {
                l.elevation.is_some()
                    && l.discipline
                        .is_none_or(|d| !home.hidden_disciplines.contains(&d))
            }) {
                mesh.add_polyline(line, base);
            }
            for top in &view.furniture {
                let highlight = selection.contains(&ElementId::Furniture(top.id));
                if top
                    .discipline
                    .is_some_and(|d| home.hidden_disciplines.contains(&d))
                {
                    continue;
                }
                for piece in top.visible_leaves() {
                    let local = models(piece).unwrap_or_else(|| newera_catalog::piece_mesh(piece));
                    mesh.add_piece(piece, &local, base, highlight);
                }
            }
            let shown = |d: Option<newera_core::Discipline>| {
                d.is_none_or(|d| !home.hidden_disciplines.contains(&d))
            };
            for label in view
                .labels
                .iter()
                .filter(|l| l.pitch.is_some() && shown(l.discipline))
            {
                mesh.add_label(label, base);
            }
            for dimension in view
                .dimensions
                .iter()
                .filter(|d| d.visible_in_3d && shown(d.discipline))
            {
                mesh.add_dimension(dimension, base);
            }
        }
        mesh
    }

    /// Text lying on the plane through `origin` (cm) spanned by `right` and
    /// `up` (unit world vectors), seen from both sides.
    #[allow(clippy::cast_possible_truncation)]
    fn add_text(
        &mut self,
        text: &str,
        size: f64,
        align: f32,
        origin: Vec3,
        (right, up): (Vec3, Vec3),
        color: [f32; 3],
    ) {
        let surface = Surface::plain(color);
        let normal = right.cross(up).normalize_or_zero();
        // Both faces float a hair off the plane so they never fight.
        let lift = normal * 0.05;
        for [a, b, c] in crate::text3d::text_triangles(text, size as f32, align) {
            let at = |p: [f32; 2], side: f32| {
                (origin + right * p[0] + up * p[1] + lift * side) * CM_TO_M
            };
            // Earcut winding varies: orient each triangle to its face.
            let front = (at(b, 0.0) - at(a, 0.0))
                .cross(at(c, 0.0) - at(a, 0.0))
                .dot(normal)
                > 0.0;
            let (b, c) = if front { (b, c) } else { (c, b) };
            self.add_triangle([at(a, 1.0), at(b, 1.0), at(c, 1.0)], normal, &surface);
            self.add_triangle([at(a, -1.0), at(c, -1.0), at(b, -1.0)], -normal, &surface);
        }
    }

    /// A label shown in 3D: at its elevation, turned by its angle and
    /// tilted by its pitch (0 lying on the floor, 90 standing).
    #[allow(clippy::cast_possible_truncation)]
    fn add_label(&mut self, label: &newera_core::Label, base: f64) {
        let pitch = label.pitch.unwrap_or(0.0).to_radians() as f32;
        let angle = label.angle.to_radians() as f32;
        // Plan x → world x, plan y (down on screen) → world z.
        let right = Vec3::new(angle.cos(), 0.0, angle.sin());
        let plan_up = Vec3::new(angle.sin(), 0.0, -angle.cos());
        let up = plan_up * pitch.cos() + Vec3::Y * pitch.sin();
        let origin = Vec3::new(
            label.position.x as f32,
            (base + label.elevation) as f32,
            label.position.y as f32,
        );
        let align = match label.align {
            newera_core::TextAlign::Left => 0.0,
            newera_core::TextAlign::Center => 0.5,
            newera_core::TextAlign::Right => 1.0,
        };
        let color = srgb_to_linear(label.color.unwrap_or([0, 0, 0]));
        self.add_text(&label.text, label.size, align, origin, (right, up), color);
    }

    /// A dimension line in 3D: measured points at their elevations, the line
    /// offset in the plane tilted by the dimension pitch, ticks and length.
    #[allow(clippy::cast_possible_truncation)]
    fn add_dimension(&mut self, dimension: &newera_core::Dimension, base: f64) {
        let at = |p: Point2, h: f64| Vec3::new(p.x as f32, (base + h) as f32, p.y as f32);
        let a = at(dimension.start, dimension.elevation[0]);
        let b = at(dimension.end, dimension.elevation[1]);
        let Some(along) = (b - a).try_normalize() else {
            return;
        };
        let pitch = dimension.pitch.to_radians() as f32;
        // Left of start → end on the plan, then tilted up around the line.
        let left = Vec3::new(along.z, 0.0, -along.x)
            .try_normalize()
            .unwrap_or(Vec3::X);
        let side = (left * pitch.cos() + Vec3::Y * pitch.sin()).normalize();
        let offset = side * dimension.offset as f32;
        let (a2, b2) = (a + offset, b + offset);
        let color = srgb_to_linear(dimension.color.unwrap_or([0, 0, 0]));
        let surface = Surface::plain(color);
        let normal = along.cross(side).normalize_or_zero();
        let mut stroke = |p: Vec3, q: Vec3, width: f32| {
            let Some(dir) = (q - p).try_normalize() else {
                return;
            };
            let across = dir.cross(normal).normalize_or_zero() * width / 2.0;
            let lift = normal * 0.05;
            let quad = |s: f32| {
                [p - across, q - across, q + across, p + across].map(|c| (c + lift * s) * CM_TO_M)
            };
            let [p0, p1, p2, p3] = quad(1.0);
            let facing = (p1 - p0).cross(p3 - p0).dot(normal) > 0.0;
            let front = if facing {
                [p0, p1, p2, p3]
            } else {
                [p3, p2, p1, p0]
            };
            self.add_quad(front, normal, &surface);
            let [q0, q1, q2, q3] = quad(-1.0);
            let back = if facing {
                [q3, q2, q1, q0]
            } else {
                [q0, q1, q2, q3]
            };
            self.add_quad(back, -normal, &surface);
        };
        let width = 0.8;
        let overshoot = side * 8.0 * (dimension.offset as f32).signum();
        if dimension.offset.abs() > 1e-6 {
            let start = side * 4.0 * (dimension.offset as f32).signum();
            stroke(a + start, a2 + overshoot, width);
            stroke(b + start, b2 + overshoot, width);
        }
        stroke(a2, b2, width);
        let tick = (along + side).normalize() * dimension.end_mark as f32 / 2.0;
        stroke(a2 - tick, a2 + tick, width * 1.6);
        stroke(b2 - tick, b2 + tick, width * 1.6);
        // Length above the line, never read backwards from the plan.
        let size = dimension.style.as_ref().map_or(18.0, |s| s.size);
        let (right, up) = if along.x < -1e-4 || (along.x.abs() <= 1e-4 && along.z > 0.0) {
            (-along, -side)
        } else {
            (along, side)
        };
        let text = newera_core::LengthUnit::Centimeter.format_dimension(a.distance(b).into());
        let origin = (a2 + b2) / 2.0 + up * 3.0;
        self.add_text(&text, size, 0.5, origin, (right, up), color);
    }

    /// One piece alone at the origin, unturned, standing on the ground.
    pub fn piece_alone(piece: &Furniture, local: &newera_catalog::Mesh) -> Self {
        let mut mesh = Self::default();
        let mut alone = piece.clone();
        alone.position = Point2::new(0.0, 0.0);
        alone.angle = 0.0;
        alone.elevation = 0.0;
        mesh.add_piece(&alone, local, 0.0, false);
        mesh
    }

    /// Removes the ground plane (the first quad), for exports.
    pub fn drop_ground(&mut self) {
        if self.vertices.len() < 4 || self.indices.len() < 6 {
            return;
        }
        self.vertices.drain(..4);
        self.indices.drain(..6);
        for i in self.indices.iter_mut().chain(self.transparent.iter_mut()) {
            *i -= 4;
        }
    }

    fn image_layer(&mut self, path: &str) -> u32 {
        let index = self
            .images
            .iter()
            .position(|p| p == path)
            .unwrap_or_else(|| {
                self.images.push(path.to_owned());
                self.images.len() - 1
            });
        u32::try_from(index).expect("image count fits in u32")
    }

    fn horizontal(&mut self, material: Option<&Material>, fallback: [f32; 3]) -> Surface {
        match material {
            Some(m) => Surface::material(self, m, fallback, Vec3::X, Vec3::Z, Vec3::ZERO),
            None => Surface::plain(fallback),
        }
    }

    /// A floor: its top face at `base`, and for upper storeys a slab of
    /// `thickness` below it, whose underside is the ceiling of the room below.
    fn add_floor(
        &mut self,
        shape: &newera_core::FloorShape,
        base: f64,
        thickness: f64,
        material: Option<&Material>,
    ) {
        let top = self.horizontal(material, FLOOR_COLOR);
        let under = Surface::plain(CEILING_COLOR);
        let edge = Surface::plain(WALL_COLOR);
        for tri in &shape.triangles {
            let mut t = *tri;
            if newera_core::signed_area(&t) > 0.0 {
                t.swap(1, 2);
            }
            let corners = t.map(|p| to_world(p, base));
            self.add_triangle(corners, Vec3::Y, &top);
            if thickness > 0.0 {
                let below = [t[0], t[2], t[1]].map(|p| to_world(p, base - thickness));
                self.add_triangle(below, -Vec3::Y, &under);
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
                        &edge,
                    );
                }
            }
        }
    }

    /// A room ceiling, visible from below only.
    fn add_ceiling(&mut self, points: &[Point2], height: f64, material: Option<&Material>) {
        let surface = self.horizontal(material, CEILING_COLOR);
        let down: Vec<Point2> = up_facing(points).into_iter().rev().collect();
        let base = self.next_index();
        for p in &down {
            self.vertices
                .push(surface.vertex(to_world(*p, height), -Vec3::Y));
        }
        for [a, b, c] in newera_core::triangulate(&down) {
            let idx = |i: usize| base + u32::try_from(i).expect("index fits in u32");
            // `triangulate` keeps the input winding, which now faces down.
            self.indices.extend([idx(a), idx(b), idx(c)]);
        }
    }

    fn add_triangle(&mut self, corners: [Vec3; 3], normal: Vec3, surface: &Surface) {
        let base = self.next_index();
        for corner in corners {
            self.vertices.push(surface.vertex(corner, normal));
        }
        self.indices.extend([base, base + 1, base + 2]);
    }

    fn add_ground(&mut self, home: &Home) {
        let (min, max) = home
            .building_bounds()
            .unwrap_or((Point2::new(-500.0, -500.0), Point2::new(500.0, 500.0)));
        let margin = 1000.0;
        let a = to_world(Point2::new(min.x - margin, min.y - margin), -1.0);
        let c = to_world(Point2::new(max.x + margin, max.y + margin), -1.0);
        let b = Vec3::new(c.x, a.y, a.z);
        let d = Vec3::new(a.x, a.y, c.z);
        let mut grass = Surface::plain(GROUND_COLOR);
        grass.kind = newera_core::Pattern::Grass.index();
        grass.u = Vec4::new(0.5, 0.0, 0.0, 0.0);
        grass.v = Vec4::new(0.0, 0.0, 0.5, 0.0);
        self.add_quad([a, d, c, b], Vec3::Y, &grass);
    }

    /// Which side of the wall a plan point is on: `Some(true)` left of
    /// `start → end`, `Some(false)` right, `None` on the centerline (end caps).
    fn side_of(wall: &Wall, centerline: &[Point2], p: Point2) -> Option<bool> {
        let mut best: Option<(f64, f64)> = None;
        for seg in centerline.windows(2) {
            let (a, b) = (seg[0], seg[1]);
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let len2 = (dx * dx + dy * dy).max(1e-12);
            let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
            let q = Point2::new(a.x + dx * t, a.y + dy * t);
            let dist = p.distance(q);
            let cross = dx * (p.y - a.y) - dy * (p.x - a.x);
            if best.is_none_or(|(d, _)| dist < d) {
                best = Some((dist, cross / len2.sqrt()));
            }
        }
        let (_, signed) = best?;
        // Left of +x is -y in plan axes: a negative cross product.
        (signed.abs() > wall.thickness * 0.25).then_some(signed < 0.0)
    }

    fn wall_surface(
        &mut self,
        wall: &Wall,
        left: bool,
        p: Point2,
        q: Point2,
        selected: bool,
    ) -> Surface {
        if selected {
            return Surface::plain(SELECTED_COLOR);
        }
        let material = if left {
            &wall.left_side
        } else {
            &wall.right_side
        };
        let Some(material) = material else {
            return Surface::plain(WALL_COLOR);
        };
        // The viewer's right on an outward face is its `p → q` direction.
        let across = (to_world(q, 0.0) - to_world(p, 0.0)).normalize_or(Vec3::X);
        // Anchor tiles at the wall start so split faces line up.
        Surface::material(
            self,
            material,
            WALL_COLOR,
            across,
            Vec3::Y,
            to_world(wall.start, 0.0),
        )
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
        selected: bool,
    ) {
        if outline.len() < 3 {
            return;
        }
        let plain = Surface::plain(if selected { SELECTED_COLOR } else { WALL_COLOR });
        let centerline = wall.centerline();
        let points = up_facing(outline);
        let (bottom, top) = (base, base + wall.height);
        let len = wall.start.distance(wall.end).max(1e-9);
        let dir = (
            (wall.end.x - wall.start.x) / len,
            (wall.end.y - wall.start.y) / len,
        );
        let along = |p: Point2| (p.x - wall.start.x) * dir.0 + (p.y - wall.start.y) * dir.1;
        // Sloping walls rise linearly from `height` to `height_at_end`.
        let rise = wall.height_at_end.map_or(0.0, |end| end - wall.height);
        let top_at = |p: Point2| top + rise * (along(p) / len).clamp(0.0, 1.0);
        let mut baseboards: Vec<(Point2, Point2, newera_core::Baseboard)> = Vec::new();

        let n = points.len();
        for k in 0..n {
            let (p, q) = (points[k], points[(k + 1) % n]);
            let edge_len = p.distance(q);
            if edge_len < 1e-6 {
                continue;
            }
            let mid = Point2::new(p.x.midpoint(q.x), p.y.midpoint(q.y));
            let side = Self::side_of(wall, &centerline, mid);
            let surface = match side {
                Some(left) => self.wall_surface(wall, left, p, q, selected),
                None => plain,
            };
            let parallel_edge = ((q.x - p.x) * dir.1 - (q.y - p.y) * dir.0).abs() / edge_len < 1e-3;
            if let Some(left) = side
                && parallel_edge
                && let Some(board) = if left {
                    &wall.left_baseboard
                } else {
                    &wall.right_baseboard
                }
            {
                baseboards.push((p, q, board.clone()));
            }
            let parallel = ((q.x - p.x) * dir.1 - (q.y - p.y) * dir.0).abs() / edge_len < 1e-3;
            if cuts.is_empty() || !parallel || wall.is_arc() {
                self.add_side_sloped(p, q, bottom, top_at(p), top_at(q), &surface);
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
                let (a0, a1) = (at(s0), at(s1));
                if let Some(cut) = cuts.iter().find(|c| c.from <= mid && c.to >= mid) {
                    if base + cut.bottom > bottom {
                        self.add_side(a0, a1, bottom, base + cut.bottom, &surface);
                    }
                    if base + cut.top < top_at(a0).min(top_at(a1)) {
                        self.add_side_sloped(
                            a0,
                            a1,
                            base + cut.top,
                            top_at(a0),
                            top_at(a1),
                            &surface,
                        );
                    }
                } else {
                    self.add_side_sloped(a0, a1, bottom, top_at(a0), top_at(a1), &surface);
                }
            }
        }
        if rise.abs() < 1e-9 {
            self.add_cap(&points, top, &plain);
        } else {
            let heights: Vec<f64> = points.iter().map(|p| top_at(*p)).collect();
            self.add_cap_heights(&points, &heights, &plain);
        }
        if !wall.is_arc() {
            for cut in cuts {
                self.add_reveals(wall, dir, cut, base);
            }
        }
        for (p, q, board) in baseboards {
            self.add_baseboard(p, q, &board, base, cuts, wall, along(p), along(q));
        }
    }

    /// A skirting board in front of a wall face `p → q` (outward winding),
    /// interrupted where doors reach the floor.
    #[allow(clippy::too_many_arguments)]
    fn add_baseboard(
        &mut self,
        p: Point2,
        q: Point2,
        board: &newera_core::Baseboard,
        base: f64,
        cuts: &[WallCut],
        _wall: &Wall,
        a: f64,
        b: f64,
    ) {
        let surface = match &board.material {
            Some(m) => Surface::plain(srgb_to_linear(m.base_color([240, 240, 235]))),
            None => Surface::plain(REVEAL_COLOR),
        };
        let (lo, hi) = (a.min(b), a.max(b));
        let mut spans = vec![(lo, hi)];
        for cut in cuts.iter().filter(|c| c.bottom < board.height) {
            spans = spans
                .into_iter()
                .flat_map(|(s0, s1)| {
                    let mut out = Vec::new();
                    if cut.from > s0 {
                        out.push((s0, cut.from.min(s1)));
                    }
                    if cut.to < s1 {
                        out.push((cut.to.max(s0), s1));
                    }
                    out
                })
                .filter(|(s0, s1)| s1 - s0 > 0.5)
                .collect();
        }
        let at = |s: f64| {
            let t = if (b - a).abs() < 1e-9 {
                0.0
            } else {
                (s - a) / (b - a)
            };
            Point2::new(p.x + (q.x - p.x) * t, p.y + (q.y - p.y) * t)
        };
        let len = p.distance(q).max(1e-9);
        // Outward normal of the face in plan axes (matches `add_side`).
        let (out_x, out_y) = (-(q.y - p.y) / len, (q.x - p.x) / len);
        let (bottom, top) = (base, base + board.height);
        for (s0, s1) in spans {
            let (mut f0, mut f1) = (at(s0), at(s1));
            if a > b {
                std::mem::swap(&mut f0, &mut f1);
            }
            let shift = |pt: Point2| {
                Point2::new(
                    pt.x + out_x * board.thickness,
                    pt.y + out_y * board.thickness,
                )
            };
            let (g0, g1) = (shift(f0), shift(f1));
            self.add_side(g0, g1, bottom, top, &surface);
            self.add_side(f0, g0, bottom, top, &surface);
            self.add_side(g1, f1, bottom, top, &surface);
            let cap = up_facing(&[f0, g0, g1, f1]);
            self.add_cap(&cap, top, &surface);
        }
    }

    /// Inner faces of a hole: jambs, sill and lintel.
    fn add_reveals(&mut self, wall: &Wall, dir: (f64, f64), cut: &WallCut, base: f64) {
        let reveal = Surface::plain(REVEAL_COLOR);
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
                &reveal,
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
                &reveal,
            );
        }
    }

    /// Vertical face over plan edge `p → q` (up-facing winding, so outward).
    /// A polyline shown in 3D: a flat strip of its width at its elevation,
    /// visible from above and below.
    fn add_polyline(&mut self, line: &newera_core::Polyline, base: f64) {
        let height = base + line.elevation.unwrap_or(0.0);
        let surface = Surface::plain(srgb_to_linear(line.color));
        let half = line.thickness.max(0.5) / 2.0;
        let mut points = line.points.clone();
        if line.closed && points.len() > 2 {
            points.push(points[0]);
        }
        for pair in points.windows(2) {
            let (p, q) = (pair[0], pair[1]);
            let len = p.distance(q);
            if len < 1e-6 {
                continue;
            }
            let (nx, ny) = (-(q.y - p.y) / len * half, (q.x - p.x) / len * half);
            let corners = [
                Point2::new(p.x + nx, p.y + ny),
                Point2::new(q.x + nx, q.y + ny),
                Point2::new(q.x - nx, q.y - ny),
                Point2::new(p.x - nx, p.y - ny),
            ];
            let up = up_facing(&corners);
            self.add_cap(&up, height, &surface);
            let down: Vec<Point2> = up.iter().rev().copied().collect();
            let start = self.next_index();
            for c in &down {
                self.vertices
                    .push(surface.vertex(to_world(*c, height - 0.1), -Vec3::Y));
            }
            self.indices
                .extend([start, start + 1, start + 2, start, start + 2, start + 3]);
        }
    }

    /// Vertical face whose top slopes from `top_p` to `top_q`.
    fn add_side_sloped(
        &mut self,
        p: Point2,
        q: Point2,
        bottom: f64,
        top_p: f64,
        top_q: f64,
        surface: &Surface,
    ) {
        let (pb, qb) = (to_world(p, bottom), to_world(q, bottom));
        let (pt, qt) = (to_world(p, top_p), to_world(q, top_q));
        let normal = (qb - pb).cross(Vec3::Y).normalize_or_zero();
        self.add_quad([pb, qb, qt, pt], normal, surface);
    }

    /// Cap with a height per point (sloping wall tops).
    fn add_cap_heights(&mut self, points: &[Point2], heights: &[f64], surface: &Surface) {
        let base = self.next_index();
        for (p, h) in points.iter().zip(heights) {
            self.vertices
                .push(surface.vertex(to_world(*p, *h), Vec3::Y));
        }
        for [a, b, c] in newera_core::triangulate(points) {
            let idx = |i: usize| base + u32::try_from(i).expect("index fits in u32");
            self.indices.extend([idx(a), idx(b), idx(c)]);
        }
    }

    fn add_side(&mut self, p: Point2, q: Point2, bottom: f64, top: f64, surface: &Surface) {
        let (pb, qb) = (to_world(p, bottom), to_world(q, bottom));
        let (pt, qt) = (to_world(p, top), to_world(q, top));
        let normal = (qb - pb).cross(Vec3::Y).normalize_or_zero();
        self.add_quad([pb, qb, qt, pt], normal, surface);
    }

    /// Places a catalog or imported mesh in the world.
    ///
    /// Looks follow the piece: its color replaces every material, else its
    /// texture covers the whole model, else each model material applies,
    /// with per-name overrides.
    fn add_piece(
        &mut self,
        piece: &Furniture,
        local: &newera_catalog::Mesh,
        floor: f64,
        highlight: bool,
    ) {
        // Tilts turn the model around its center: pitch around the width
        // axis, roll around the depth axis.
        #[allow(clippy::cast_possible_truncation)]
        let tilt = (piece.pitch != 0.0 || piece.roll != 0.0).then(|| {
            glam::Mat3::from_rotation_x(piece.pitch.to_radians() as f32)
                * glam::Mat3::from_rotation_z(piece.roll.to_radians() as f32)
        });
        #[allow(clippy::cast_possible_truncation)]
        let middle = piece.height as f32 / 2.0;
        let world = |p: [f32; 3]| {
            let p = tilt.map_or(p, |m| {
                (m * (Vec3::from(p) - Vec3::Y * middle) + Vec3::Y * middle).to_array()
            });
            let plan = piece.to_plan((f64::from(p[0]), f64::from(p[2])));
            to_world(plan, floor + piece.elevation + f64::from(p[1]))
        };
        let piece_color = piece.color.map(srgb_to_linear);
        let piece_texture = piece.texture.as_ref().filter(|t| t.image.is_some());
        // A pattern finish (wood, stone, marble…) laid on every face.
        let piece_pattern = piece
            .texture
            .as_ref()
            .filter(|t| t.image.is_none())
            .and_then(|t| t.pattern.map(|p| (t, p)));
        #[allow(clippy::cast_possible_truncation)]
        let opacity = piece.opacity.map_or(1.0, |o| o.clamp(0.0, 1.0) as f32);
        let texture_layer = piece_texture
            .map(|t| IMAGE_BASE + self.image_layer(t.image.as_deref().unwrap_or_default()));
        // Materials whose vertices carry texture coordinates.
        let mut has_uv = vec![false; local.materials.len()];
        for (k, uv) in local.uvs.iter().enumerate() {
            if (uv[0] != 0.0 || uv[1] != 0.0)
                && let Some(flag) = local
                    .vertex_materials
                    .get(k)
                    .and_then(|&m| has_uv.get_mut(usize::from(m)))
            {
                *flag = true;
            }
        }
        // Material overrides and texture layers, resolved once per material:
        // `(color, layer, alpha, planar tile size)`. An override texture on a
        // material without texture coordinates is laid flat at its real size.
        let looks: Vec<Look> = local
            .materials
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let over = piece.materials.iter().find(|o| o.name == m.name);
                let color = over.and_then(|o| o.color).map(srgb_to_linear);
                let over_texture = over
                    .and_then(|o| o.texture.as_ref())
                    .filter(|t| t.image.is_some());
                let texture = over_texture
                    .and_then(|t| t.image.clone())
                    .or_else(|| m.texture.as_ref().map(|p| p.display().to_string()));
                let layer = if color.is_some() {
                    None
                } else {
                    texture.map(|t| IMAGE_BASE + self.image_layer(&t))
                };
                let planar = over_texture
                    .filter(|_| !has_uv.get(i).copied().unwrap_or(false))
                    .map(newera_core::Material::tile_size);
                (color, layer, m.alpha, planar)
            })
            .collect();
        let planar_uv = |position: &[f32; 3], normal: &[f32; 3], [w, h]: [f64; 2]| {
            let n = normal.map(f32::abs);
            let (u, v) = if n[1] >= n[0] && n[1] >= n[2] {
                (position[0], position[2])
            } else if n[0] >= n[2] {
                (position[2], position[1])
            } else {
                (position[0], position[1])
            };
            #[allow(clippy::cast_possible_truncation)]
            [u / w as f32, v / h as f32]
        };

        let base = self.next_index();
        let mut vertex_alpha = Vec::with_capacity(local.positions.len());
        for (k, (position, normal)) in local.positions.iter().zip(&local.normals).enumerate() {
            let tip = world([
                position[0] + normal[0],
                position[1] + normal[1],
                position[2] + normal[2],
            ]);
            let at = world(*position);
            let world_normal = (tip - at).normalize_or_zero();
            let raw = local.colors.get(k).copied().unwrap_or([0.8; 3]);
            let look = local
                .vertex_materials
                .get(k)
                .and_then(|&m| looks.get(usize::from(m)));
            let alpha = look.map_or(1.0, |l| l.2).min(opacity);
            let (mut color, mut kind, mut uv) = (raw, 0, [0.0, 0.0]);
            if let Some(c) = piece_color
                && piece_pattern.is_none()
            {
                color = c;
            } else if let Some((texture, pattern)) = piece_pattern {
                uv = planar_uv(position, normal, texture.tile_size());
                color = srgb_to_linear(
                    texture
                        .color
                        .or(piece.color)
                        .unwrap_or_else(|| pattern.default_color()),
                );
                kind = pattern.index();
            } else if let (Some(layer), Some(texture)) = (texture_layer, piece_texture) {
                // Planar mapping on the face's dominant axis, at the texture's real size.
                uv = planar_uv(position, normal, texture.tile_size());
                color = [1.0; 3];
                kind = layer;
            } else if let Some((over, layer, _, planar)) = look {
                if let Some(c) = over {
                    color = *c;
                } else if let Some(layer) = layer {
                    color = [1.0; 3];
                    kind = *layer;
                    uv = match planar {
                        Some(size) => planar_uv(position, normal, *size),
                        // The shader flips v, matching OBJ's bottom-up convention.
                        None => local.uvs.get(k).copied().unwrap_or([0.0, 0.0]),
                    };
                }
            }
            if highlight {
                color = std::array::from_fn(|i| color[i] * 0.55 + 0.45 * SELECTED_COLOR[i]);
                kind = 0;
            }
            vertex_alpha.push(alpha);
            self.vertices.push(Vertex {
                position: at.to_array(),
                normal: world_normal.to_array(),
                color: [color[0], color[1], color[2], alpha],
                uv,
                kind,
            });
        }
        // Mirroring flips handedness; reorder triangles so faces keep
        // pointing outward.
        for tri in local.indices.chunks(3) {
            let order = if piece.mirrored {
                [tri[0], tri[2], tri[1]]
            } else {
                [tri[0], tri[1], tri[2]]
            };
            let clear = order
                .iter()
                .any(|&i| vertex_alpha.get(i as usize).is_some_and(|a| *a < 0.99));
            let target = if clear {
                &mut self.transparent
            } else {
                &mut self.indices
            };
            target.extend(order.map(|i| base + i));
        }
    }

    /// Horizontal face at `height` (cm); `points` must already face up.
    fn add_cap(&mut self, points: &[Point2], height: f64, surface: &Surface) {
        let base = self.next_index();
        for p in points {
            self.vertices
                .push(surface.vertex(to_world(*p, height), Vec3::Y));
        }
        for [a, b, c] in newera_core::triangulate(points) {
            let idx = |i: usize| base + u32::try_from(i).expect("index fits in u32");
            self.indices.extend([idx(a), idx(b), idx(c)]);
        }
    }

    /// Adds a quad, fixing the corner order so it faces `normal`.
    fn add_facing(&mut self, mut corners: [Vec3; 4], normal: Vec3, surface: &Surface) {
        let geometric = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
        if geometric.dot(normal) < 0.0 {
            corners.reverse();
        }
        self.add_quad(corners, normal, surface);
    }

    /// Adds a quad; corners must be counter-clockwise when seen from `normal`.
    fn add_quad(&mut self, corners: [Vec3; 4], normal: Vec3, surface: &Surface) {
        let base = self.next_index();
        for corner in corners {
            self.vertices.push(surface.vertex(corner, normal));
        }
        self.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn next_index(&self) -> u32 {
        u32::try_from(self.vertices.len()).expect("mesh vertex count fits in u32")
    }
}

/// How one model material is painted: `(color, image layer, alpha, planar
/// tile size)`.
type Look = (Option<[f32; 3]>, Option<u32>, f32, Option<[f64; 2]>);

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
mod wall_detail_tests {
    use super::*;

    #[test]
    fn sloping_walls_and_baseboards() {
        let mut home = Home::default();
        let id = home.new_wall_id();
        let mut wall = Wall::new(id, Point2::new(0.0, 0.0), Point2::new(400.0, 0.0));
        wall.height_at_end = Some(350.0);
        wall.left_side = None;
        wall.left_baseboard = Some(newera_core::Baseboard {
            thickness: 1.5,
            height: 10.0,
            material: None,
        });
        home.walls.push(wall);
        let mesh = Mesh::from_home(&home, &Selection::new(), &|_| None);
        let max_y = mesh
            .vertices
            .iter()
            .map(|v| v.position[1])
            .fold(f32::MIN, f32::max);
        assert!((max_y - 3.5).abs() < 1e-4, "top rises to 350 cm: {max_y}");
        for tri in mesh.indices.chunks(3) {
            let [a, b, c] = [0, 1, 2].map(|k| Vec3::from(mesh.vertices[tri[k] as usize].position));
            let declared = Vec3::from(mesh.vertices[tri[0] as usize].normal);
            assert!(
                (b - a).cross(c - a).dot(declared) >= -1e-9,
                "faces point outward"
            );
        }
        // The baseboard stands in front of the left face (plan -y, beyond the
        // 7.5 cm half thickness).
        let front = mesh
            .vertices
            .iter()
            .filter(|v| (v.position[1] - 0.10).abs() < 1e-4)
            .map(|v| v.position[2])
            .fold(f32::MAX, f32::min);
        assert!(
            (front + 0.09).abs() < 1e-3,
            "baseboard 1.5 cm proud of the face: {front}"
        );
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

#[cfg(test)]
mod annotation_tests {
    use newera_core::{Dimension, DimensionId, Label, LabelId};

    use super::*;

    fn build(home: &Home) -> Mesh {
        Mesh::from_home(home, &Selection::new(), &|_| None)
    }

    fn extent(mesh: &Mesh, from: usize) -> (Vec3, Vec3) {
        mesh.vertices[from..].iter().fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(lo, hi), v| (lo.min(v.position.into()), hi.max(v.position.into())),
        )
    }

    #[test]
    fn labels_with_pitch_and_3d_dimensions_become_geometry() {
        let mut home = Home::default();
        home.labels.push(Label {
            id: LabelId(1),
            text: "Plan only".into(),
            ..Label::default()
        });
        home.dimensions.push(Dimension {
            id: DimensionId(2),
            end: Point2::new(300.0, 0.0),
            ..Dimension::default()
        });
        let ground = build(&home).vertices.len();
        assert_eq!(ground, 4, "plan-only annotations stay out of 3D");

        // Standing label 150 cm up, facing -z.
        home.labels[0].pitch = Some(90.0);
        home.labels[0].elevation = 150.0;
        home.labels[0].size = 40.0;
        let mesh = build(&home);
        assert!(mesh.vertices.len() > ground + 100);
        let (lo, hi) = extent(&mesh, ground);
        assert!((lo.y - 1.5).abs() < 0.15 && hi.y > 1.7, "{lo} {hi}");
        assert!(
            (hi.z - lo.z).abs() < 0.01,
            "standing text is vertical: {lo} {hi}"
        );
        assert!(mesh.indices.chunks(3).all(|t| {
            let [a, b, c] = [0, 1, 2].map(|k| Vec3::from(mesh.vertices[t[k] as usize].position));
            (b - a)
                .cross(c - a)
                .dot(mesh.vertices[t[0] as usize].normal.into())
                >= -1e-9
        }));

        // Lying label and a 3D dimension lifted 100 cm with 50 cm offset.
        home.labels[0].pitch = Some(0.0);
        let label_only = build(&home).vertices.len();
        home.dimensions[0].visible_in_3d = true;
        home.dimensions[0].elevation = [100.0, 100.0];
        home.dimensions[0].offset = 50.0;
        let mesh = build(&home);
        let (lo, hi) = extent(&mesh, label_only);
        assert!(mesh.vertices.len() > label_only + 50);
        assert!(
            lo.x < 0.05 && hi.x > 2.95,
            "spans the measured length: {lo} {hi}"
        );
        assert!(
            (lo.y - 1.0).abs() < 0.01 && (hi.y - 1.0).abs() < 0.01,
            "{lo} {hi}"
        );
        // Offset to the left of start → end on the plan is -y, world -z.
        assert!(lo.z < -0.5, "{lo}");
    }
}

#[cfg(test)]
mod material_tests {
    use newera_catalog::{Mesh as ModelMesh, MeshMaterial};
    use newera_core::{Material, ModelMaterial};

    use super::*;

    #[test]
    fn override_textures_without_coordinates_are_laid_flat_at_real_size() {
        // A 100 × 50 cm top, one material, no texture coordinates in the file.
        let model = ModelMesh {
            positions: vec![
                [0.0, 4.0, 0.0],
                [100.0, 4.0, 0.0],
                [100.0, 4.0, 50.0],
                [0.0, 4.0, 50.0],
            ],
            normals: vec![[0.0, 1.0, 0.0]; 4],
            colors: vec![[0.06; 3]; 4],
            indices: vec![0, 2, 1, 0, 3, 2],
            uvs: vec![[0.0, 0.0]; 4],
            vertex_materials: vec![0; 4],
            materials: vec![MeshMaterial {
                name: "stone".into(),
                color: [0.06; 3],
                alpha: 1.0,
                texture: None,
                shininess: 0.0,
            }],
        };
        let mut piece = Furniture {
            width: 100.0,
            depth: 50.0,
            height: 4.0,
            ..Furniture::default()
        };
        piece.materials.push(ModelMaterial {
            name: "stone".into(),
            key: None,
            color: None,
            texture: Some(Material {
                image: Some("marble.png".into()),
                tile: Some([25.0, 25.0]),
                ..Material::default()
            }),
            shininess: None,
        });
        let mesh = Mesh::piece_alone(&piece, &model);
        let us: Vec<f32> = mesh.vertices.iter().map(|v| v.uv[0]).collect();
        let span = us.iter().copied().fold(f32::MIN, f32::max)
            - us.iter().copied().fold(f32::MAX, f32::min);
        // 100 cm across 25 cm tiles: four repetitions, textured with the image.
        assert!((span - 4.0).abs() < 1e-3, "{us:?}");
        assert!(mesh.vertices.iter().all(|v| v.kind == IMAGE_BASE));
        assert_eq!(mesh.images, ["marble.png"]);
    }
}
