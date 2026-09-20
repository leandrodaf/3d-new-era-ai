//! Minimal mesh building blocks for procedural furniture.
//!
//! Local frame, in centimeters: origin at the center of the footprint on the
//! floor, `x` along the width, `y` up, `z` along the depth with the front of
//! the piece towards `+z` (matching the plan's local `+y`).

/// Linear RGB in `0..=1`.
pub type Rgb = [f32; 3];

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<Rgb>,
    /// Procedural surfaces that accept the piece's global finish. Missing
    /// entries mean true; fixtures can retain their own ceramic/metal finish.
    pub finishable: Vec<bool>,
    pub indices: Vec<u32>,
    /// Texture coordinates from the model file; empty or one per vertex.
    pub uvs: Vec<[f32; 2]>,
    /// Material of each vertex (index into `materials`); empty when the
    /// mesh only has vertex colors.
    pub vertex_materials: Vec<u16>,
    pub materials: Vec<MeshMaterial>,
}

/// Surface of an imported model.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshMaterial {
    pub name: String,
    pub color: Rgb,
    /// 1 opaque, 0 invisible.
    pub alpha: f32,
    /// Image file for the diffuse color, absolute or relative to the model.
    pub texture: Option<std::path::PathBuf>,
    pub shininess: f32,
}

/// Window and door glass of the procedural models.
pub const GLASS: [u8; 3] = [168, 206, 226];

/// Whether a procedural vertex color is window glass: renderers let light
/// through it.
pub fn is_glass(c: Rgb) -> bool {
    let g = rgb(GLASS);
    c.iter().zip(g).all(|(a, b)| (a - b).abs() < 0.002)
}

/// Converts an sRGB byte color to linear-ish floats used by the renderers.
pub fn rgb(c: [u8; 3]) -> Rgb {
    c.map(|v| f32::from(v) / 255.0)
}

/// Mixes a color towards black (`amount < 0`) or white (`amount > 0`).
pub fn shade(c: Rgb, amount: f32) -> Rgb {
    if amount >= 0.0 {
        c.map(|v| v + (1.0 - v) * amount)
    } else {
        c.map(|v| v * (1.0 + amount))
    }
}

type V3 = [f32; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(v: V3) -> V3 {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len < 1e-12 { v } else { v.map(|c| c / len) }
}

#[allow(clippy::cast_possible_truncation)]
fn f(v: f64) -> f32 {
    v as f32
}

/// Axis of a cylinder or cone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Mesh {
    pub(crate) fn protect_finish_since(&mut self, start: usize) {
        self.finishable.resize(start, true);
        self.finishable.resize(self.positions.len(), false);
    }
    fn next(&self) -> u32 {
        u32::try_from(self.positions.len()).expect("mesh fits in u32 indices")
    }

    /// Adds a planar polygon (convex, counter-clockwise seen from its front).
    ///
    /// # Panics
    ///
    /// If the mesh outgrows `u32` indices.
    pub fn polygon(&mut self, corners: &[V3], color: Rgb) {
        if corners.len() < 3 {
            return;
        }
        let normal = normalize(cross(
            sub(corners[1], corners[0]),
            sub(corners[2], corners[0]),
        ));
        let base = self.next();
        for c in corners {
            self.positions.push(*c);
            self.normals.push(normal);
            self.colors.push(color);
        }
        let n = u32::try_from(corners.len()).expect("small polygon");
        for i in 1..n - 1 {
            self.indices.extend([base, base + i, base + i + 1]);
        }
    }

    /// Axis-aligned box from `min` to `max` (cm), all six faces outward.
    pub fn cuboid(&mut self, min: [f64; 3], max: [f64; 3], color: Rgb) {
        let [x0, y0, z0] = min.map(f);
        let [x1, y1, z1] = max.map(f);
        if x1 - x0 <= 0.0 || y1 - y0 <= 0.0 || z1 - z0 <= 0.0 {
            return;
        }
        let top = shade(color, 0.06);
        let bottom = shade(color, -0.2);
        self.polygon(
            &[[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]],
            color,
        ); // front +z
        self.polygon(
            &[[x1, y0, z0], [x0, y0, z0], [x0, y1, z0], [x1, y1, z0]],
            color,
        ); // back -z
        self.polygon(
            &[[x1, y0, z1], [x1, y0, z0], [x1, y1, z0], [x1, y1, z1]],
            color,
        ); // right +x
        self.polygon(
            &[[x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]],
            color,
        ); // left -x
        self.polygon(
            &[[x0, y1, z1], [x1, y1, z1], [x1, y1, z0], [x0, y1, z0]],
            top,
        ); // top +y
        self.polygon(
            &[[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]],
            bottom,
        ); // bottom -y
    }

    /// Box given by its center on the floor plane (x, z), bottom `y0`, size.
    pub fn block(&mut self, center: [f64; 2], y0: f64, size: [f64; 3], color: Rgb) {
        let [w, h, d] = size;
        self.cuboid(
            [center[0] - w / 2.0, y0, center[1] - d / 2.0],
            [center[0] + w / 2.0, y0 + h, center[1] + d / 2.0],
            color,
        );
    }

    /// Frustum (cylinder when radii match) along `axis`, from `base` for `length`.
    #[allow(clippy::too_many_arguments)]
    pub fn frustum(
        &mut self,
        base: [f64; 3],
        axis: Axis,
        length: f64,
        r0: f64,
        r1: f64,
        segments: u32,
        color: Rgb,
    ) {
        let segments = segments.max(3);
        let point = |t: f64, r: f64, along: f64| -> V3 {
            let a = t * std::f64::consts::TAU;
            let (u, v) = (r * a.cos(), r * a.sin());
            match axis {
                Axis::Y => [f(base[0] + u), f(base[1] + along), f(base[2] - v)],
                Axis::X => [f(base[0] + along), f(base[1] + u), f(base[2] + v)],
                Axis::Z => [f(base[0] + u), f(base[1] + v), f(base[2] + along)],
            }
        };
        for i in 0..segments {
            let (t0, t1) = (
                f64::from(i) / f64::from(segments),
                f64::from(i + 1) / f64::from(segments),
            );
            let quad = [
                point(t0, r0, 0.0),
                point(t1, r0, 0.0),
                point(t1, r1, length),
                point(t0, r1, length),
            ];
            if r1 <= 1e-9 {
                self.polygon(&[quad[0], quad[1], quad[2]], color);
            } else {
                self.polygon(&quad, color);
            }
        }
        let cap = |t_radius: f64, along: f64, reverse: bool| -> Vec<V3> {
            let mut pts: Vec<V3> = (0..segments)
                .map(|i| point(f64::from(i) / f64::from(segments), t_radius, along))
                .collect();
            if reverse {
                pts.reverse();
            }
            pts
        };
        if r1 > 1e-9 {
            self.polygon(&cap(r1, length, false), shade(color, 0.06));
        }
        if r0 > 1e-9 {
            self.polygon(&cap(r0, 0.0, true), shade(color, -0.2));
        }
    }

    pub fn cylinder(&mut self, base: [f64; 3], axis: Axis, length: f64, radius: f64, color: Rgb) {
        self.frustum(base, axis, length, radius, radius, 20, color);
    }

    pub fn append(&mut self, other: &Self) {
        if !self.finishable.is_empty() || !other.finishable.is_empty() {
            self.finishable.resize(self.positions.len(), true);
            self.finishable.extend(
                (0..other.positions.len())
                    .map(|i| other.finishable.get(i).copied().unwrap_or(true)),
            );
        }
        let base = self.next();
        let had_extra = !self.uvs.is_empty() || !self.vertex_materials.is_empty();
        let has_extra = !other.uvs.is_empty() || !other.vertex_materials.is_empty();
        if had_extra || has_extra {
            let n = self.positions.len();
            self.uvs.resize(n, [0.0, 0.0]);
            self.vertex_materials.resize(n, u16::MAX);
            let material_base = u16::try_from(self.materials.len()).unwrap_or(u16::MAX);
            self.materials.extend(other.materials.iter().cloned());
            let m = other.positions.len();
            if other.uvs.len() == m {
                self.uvs.extend_from_slice(&other.uvs);
            } else {
                self.uvs.resize(n + m, [0.0, 0.0]);
            }
            if other.vertex_materials.len() == m {
                self.vertex_materials
                    .extend(other.vertex_materials.iter().map(|&k| {
                        if k == u16::MAX {
                            k
                        } else {
                            k.saturating_add(material_base)
                        }
                    }));
            } else {
                self.vertex_materials.resize(n + m, u16::MAX);
            }
        }
        self.positions.extend_from_slice(&other.positions);
        self.normals.extend_from_slice(&other.normals);
        self.colors.extend_from_slice(&other.colors);
        self.indices.extend(other.indices.iter().map(|i| i + base));
    }

    /// Material of a vertex, if the mesh has materials.
    pub fn material_of(&self, vertex: usize) -> Option<&MeshMaterial> {
        self.vertex_materials
            .get(vertex)
            .and_then(|&k| self.materials.get(usize::from(k)))
    }

    /// Applies a 3×3 rotation (rows) to positions and normals.
    #[allow(clippy::cast_possible_truncation)]
    pub fn rotate(&mut self, rows: [[f64; 3]; 3]) {
        let m = rows.map(|r| r.map(|v| v as f32));
        let apply = |p: [f32; 3]| -> [f32; 3] {
            std::array::from_fn(|r| m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2])
        };
        for p in &mut self.positions {
            *p = apply(*p);
        }
        for n in &mut self.normals {
            *n = normalize(apply(*n));
        }
        // A mirroring rotation flips triangle winding.
        let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
        if det < 0.0 {
            for tri in self.indices.chunks_mut(3) {
                tri.swap(1, 2);
            }
        }
    }

    /// Axis-aligned bounds `(min, max)` in cm.
    pub fn bounds(&self) -> Option<(V3, V3)> {
        self.positions.iter().fold(None, |acc, p| {
            let (mut min, mut max) = acc.unwrap_or((*p, *p));
            for k in 0..3 {
                min[k] = min[k].min(p[k]);
                max[k] = max[k].max(p[k]);
            }
            Some((min, max))
        })
    }

    /// Scales and shifts the mesh so its bounds become exactly
    /// `[-w/2, w/2] × [0, h] × [-d/2, d/2]`.
    pub fn fit_to(&mut self, width: f64, depth: f64, height: f64) {
        let Some((min, max)) = self.bounds() else {
            return;
        };
        let size = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
        let target = [f(width), f(height), f(depth)];
        let scale: V3 = std::array::from_fn(|k| {
            if size[k] > 1e-9 {
                target[k] / size[k]
            } else {
                1.0
            }
        });
        for p in &mut self.positions {
            p[0] = (p[0] - min[0]) * scale[0] - target[0] / 2.0;
            p[1] = (p[1] - min[1]) * scale[1];
            p[2] = (p[2] - min[2]) * scale[2] - target[2] / 2.0;
        }
        // Non-uniform scaling bends normals by the inverse scale.
        for n in &mut self.normals {
            *n = normalize([n[0] / scale[0], n[1] / scale[1], n[2] / scale[2]]);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn combining_meshes_preserves_protected_finishes_and_default_body_faces() {
        let mut body = Mesh::default();
        body.cuboid([0.0; 3], [1.0; 3], [0.5; 3]);
        let count = body.positions.len();
        let plain = body.clone();
        let mut fixture = body.clone();
        fixture.protect_finish_since(0);
        body.append(&fixture);
        body.append(&plain);
        assert_eq!(body.finishable.len(), count * 3);
        assert!(body.finishable[..count].iter().all(|v| *v));
        assert!(body.finishable[count..count * 2].iter().all(|v| !*v));
        assert!(body.finishable[count * 2..].iter().all(|v| *v));
    }

    pub(crate) fn assert_outward(mesh: &Mesh) {
        for tri in mesh.indices.chunks(3) {
            let [a, b, c] = [0, 1, 2].map(|k| mesh.positions[tri[k] as usize]);
            let geometric = cross(sub(b, a), sub(c, a));
            let declared = mesh.normals[tri[0] as usize];
            let dot = geometric[0] * declared[0]
                + geometric[1] * declared[1]
                + geometric[2] * declared[2];
            assert!(dot >= -1e-3, "triangle faces against its normal");
        }
    }

    #[test]
    fn cuboid_and_cylinder_are_closed_and_outward() {
        let mut m = Mesh::default();
        m.cuboid([-10.0, 0.0, -5.0], [10.0, 30.0, 5.0], [0.5; 3]);
        m.cylinder([40.0, 0.0, 0.0], Axis::Y, 20.0, 4.0, [0.5; 3]);
        m.frustum([0.0, 50.0, 0.0], Axis::Z, 10.0, 5.0, 0.0, 12, [0.5; 3]);
        assert_outward(&m);
        let (min, max) = m.bounds().unwrap();
        assert!(min[1].abs() < 1e-6);
        assert!((max[0] - 44.0).abs() < 1e-4);
    }

    #[test]
    fn fit_to_matches_target_bounds() {
        let mut m = Mesh::default();
        m.cuboid([3.0, 1.0, 2.0], [5.0, 9.0, 7.0], [0.5; 3]);
        m.fit_to(100.0, 60.0, 80.0);
        let (min, max) = m.bounds().unwrap();
        assert!((min[0] + 50.0).abs() < 1e-3 && (max[0] - 50.0).abs() < 1e-3);
        assert!(min[1].abs() < 1e-3 && (max[1] - 80.0).abs() < 1e-3);
        assert!((min[2] + 30.0).abs() < 1e-3 && (max[2] - 30.0).abs() < 1e-3);
    }
}
