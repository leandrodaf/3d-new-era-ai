//! Software renderer for scene meshes: z-buffered, perspective-correct,
//! supersampled, with the same materials and lighting as the GPU view.

use glam::{Mat4, Vec3, Vec4};
use image::{Rgba, RgbaImage};

use crate::mesh::{IMAGE_BASE, Mesh, Vertex};
use crate::patterns;

/// Everything a software render needs besides the mesh.
pub struct RenderOptions<'a> {
    pub width: u32,
    pub height: u32,
    pub view_proj: Mat4,
    pub sky: [u8; 3],
    /// Samples per pixel along each axis (2 means 4 samples per pixel).
    pub supersample: u32,
    /// Resolves image paths referenced by the mesh.
    pub load_image: &'a dyn Fn(&str) -> Option<RgbaImage>,
    /// Leave uncovered pixels transparent instead of painting the sky.
    pub transparent: bool,
    /// Darken creases and silhouettes so same-colored parts stay readable.
    pub outlines: bool,
    /// Paint back faces with this color instead of skipping them: in a
    /// section, the inside of what the cut opened shows as solid.
    pub cut_color: Option<[u8; 3]>,
}

impl std::fmt::Debug for RenderOptions<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderOptions")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("supersample", &self.supersample)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy)]
struct ClipVertex {
    clip: Vec4,
    normal: Vec3,
    color: Vec4,
    uv: [f32; 2],
}

impl ClipVertex {
    fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        Self {
            clip: a.clip + (b.clip - a.clip) * t,
            normal: a.normal + (b.normal - a.normal) * t,
            color: a.color + (b.color - a.color) * t,
            uv: [
                a.uv[0] + (b.uv[0] - a.uv[0]) * t,
                a.uv[1] + (b.uv[1] - a.uv[1]) * t,
            ],
        }
    }
}

/// Keeps the part of a triangle in front of the near plane (`z >= 0` in
/// clip space for a 0..1 depth range).
fn clip_near(poly: &[ClipVertex; 3]) -> Vec<ClipVertex> {
    let mut out = Vec::with_capacity(4);
    for i in 0..3 {
        let (a, b) = (&poly[i], &poly[(i + 1) % 3]);
        let (da, db) = (a.clip.z, b.clip.z);
        if da >= 0.0 {
            out.push(*a);
        }
        if (da >= 0.0) != (db >= 0.0) {
            out.push(ClipVertex::lerp(a, b, da / (da - db)));
        }
    }
    out
}

struct Target {
    width: usize,
    height: usize,
    color: Vec<Vec3>,
    depth: Vec<f32>,
    /// Face normal and view distance of the opaque surface at each sample.
    normal: Vec<Vec3>,
    distance: Vec<f32>,
}

pub(crate) struct Images<'a> {
    pub(crate) loaded: Vec<Option<RgbaImage>>,
    pub(crate) _source: std::marker::PhantomData<&'a ()>,
}

impl Images<'_> {
    pub(crate) fn sample(&self, layer: usize, u: f32, v: f32) -> Vec3 {
        let Some(Some(image)) = self.loaded.get(layer) else {
            return Vec3::splat(0.7);
        };
        let (w, h) = (image.width().max(1), image.height().max(1));
        #[allow(clippy::cast_precision_loss)]
        let (x, y) = (
            (u - u.floor()) * w as f32 - 0.5,
            (v - v.floor()) * h as f32 - 0.5,
        );
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let texel = |dx: f32, dy: f32| {
            let px = ((x0 + dx) as i64).rem_euclid(i64::from(w)) as u32;
            let py = ((y0 + dy) as i64).rem_euclid(i64::from(h)) as u32;
            let Rgba([r, g, b, _]) = *image.get_pixel(px, py);
            Vec3::new(f32::from(r), f32::from(g), f32::from(b)) / 255.0
        };
        let top = texel(0.0, 0.0).lerp(texel(1.0, 0.0), fx);
        let bottom = texel(0.0, 1.0).lerp(texel(1.0, 1.0), fx);
        top.lerp(bottom, fy)
    }
}

fn light_dir() -> Vec3 {
    Vec3::new(-0.4, -1.0, -0.3).normalize()
}

/// Surface color before lighting: vertex color times pattern or texture.
pub(crate) fn albedo(
    kind: u32,
    color: Vec4,
    uv: [f32; 2],
    pixel: f32,
    images: &Images<'_>,
) -> Vec3 {
    color.truncate() * detail(kind, uv, pixel, images)
}

/// What a texture image or pattern contributes on top of the flat color
/// (white when there is none).
pub(crate) fn detail(kind: u32, uv: [f32; 2], pixel: f32, images: &Images<'_>) -> Vec3 {
    if kind >= IMAGE_BASE {
        images.sample((kind - IMAGE_BASE) as usize, uv[0], -uv[1])
    } else if kind > 0 {
        Vec3::splat(patterns::shade(kind, uv[0], uv[1], pixel))
    } else {
        Vec3::ONE
    }
}

/// Albedo times light, exactly like the GPU fragment shader.
fn shade(
    kind: u32,
    color: Vec4,
    uv: [f32; 2],
    pixel: f32,
    normal: Vec3,
    images: &Images<'_>,
) -> Vec3 {
    let albedo = albedo(kind, color, uv, pixel, images);
    let n = normal.normalize_or_zero();
    let diffuse = n.dot(-light_dir()).max(0.0);
    let ambient = 0.86 + (1.0 - 0.86) * (n.y * 0.5 + 0.5);
    (albedo * (ambient + diffuse * 0.22)).min(Vec3::ONE)
}

fn vertex(v: &Vertex, view_proj: Mat4) -> ClipVertex {
    ClipVertex {
        clip: view_proj * Vec3::from(v.position).extend(1.0),
        normal: Vec3::from(v.normal),
        color: Vec4::from(v.color),
        uv: v.uv,
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn raster_triangle(
    target: &mut Target,
    tri: [ClipVertex; 3],
    kind: u32,
    transparent: bool,
    images: &Images<'_>,
    cut: Option<Vec3>,
) {
    let (w, h) = (target.width as f32, target.height as f32);
    let screen = tri.map(|v| {
        let inv_w = 1.0 / v.clip.w;
        let ndc = v.clip.truncate() * inv_w;
        (
            (ndc.x * 0.5 + 0.5) * w,
            (1.0 - (ndc.y * 0.5 + 0.5)) * h,
            ndc.z,
            inv_w,
        )
    });
    let [(x0, y0, z0, q0), (x1, y1, z1, q1), (x2, y2, z2, q2)] = screen;
    // Signed area in screen space (y down): front faces are negative here.
    let area = (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0);
    let back = !transparent && area > 0.0;
    if area.abs() < 1e-9 || (back && cut.is_none()) {
        return;
    }
    let min_x = x0.min(x1).min(x2).floor().max(0.0) as usize;
    let max_x = (x0.max(x1).max(x2).ceil() as usize).min(target.width);
    let min_y = y0.min(y1).min(y2).floor().max(0.0) as usize;
    let max_y = (y0.max(y1).max(y2).ceil() as usize).min(target.height);
    if min_x >= max_x || min_y >= max_y {
        return;
    }
    let weights = |px: f32, py: f32| {
        let e0 = ((x2 - x1) * (py - y1) - (y2 - y1) * (px - x1)) / area;
        let e1 = ((x0 - x2) * (py - y2) - (y0 - y2) * (px - x2)) / area;
        (e0, e1, 1.0 - e0 - e1)
    };
    let perspective = |b: (f32, f32, f32)| {
        let (p0, p1, p2) = (b.0 * q0, b.1 * q1, b.2 * q2);
        let sum = p0 + p1 + p2;
        (p0 / sum, p1 / sum, p2 / sum)
    };
    let uv_at = |p: (f32, f32, f32)| {
        [
            tri[0].uv[0] * p.0 + tri[1].uv[0] * p.1 + tri[2].uv[0] * p.2,
            tri[0].uv[1] * p.0 + tri[1].uv[1] * p.1 + tri[2].uv[1] * p.2,
        ]
    };
    for py in min_y..max_y {
        for px in min_x..max_x {
            let (sx, sy) = (px as f32 + 0.5, py as f32 + 0.5);
            let b = weights(sx, sy);
            if b.0 < -1e-5 || b.1 < -1e-5 || b.2 < -1e-5 {
                continue;
            }
            let z = b.0 * z0 + b.1 * z1 + b.2 * z2;
            let index = py * target.width + px;
            if !(0.0..=1.0).contains(&z) || z >= target.depth[index] {
                continue;
            }
            let p = perspective(b);
            let uv = uv_at(p);
            let pixel = if kind > 0 && kind < IMAGE_BASE {
                let next = uv_at(perspective(weights(sx + 1.0, sy)));
                (next[0] - uv[0]).hypot(next[1] - uv[1])
            } else {
                0.0
            };
            let normal = tri[0].normal * p.0 + tri[1].normal * p.1 + tri[2].normal * p.2;
            let color = tri[0].color * p.0 + tri[1].color * p.1 + tri[2].color * p.2;
            let lit = match (back, cut) {
                (true, Some(poche)) => poche,
                _ => shade(kind, color, uv, pixel, normal, images),
            };
            if transparent {
                let alpha = color.w.clamp(0.0, 1.0);
                target.color[index] = target.color[index].lerp(lit, alpha);
            } else {
                target.color[index] = lit;
                target.depth[index] = z;
                target.normal[index] = normal.normalize_or_zero();
                target.distance[index] = 1.0 / (b.0 * q0 + b.1 * q1 + b.2 * q2);
            }
        }
    }
}

/// Darkens samples where the surface turns sharply or jumps in depth.
fn outline(target: &mut Target, ss: usize) {
    let (w, h) = (target.width, target.height);
    let reach = ss.max(1);
    let mut edge = vec![false; w * h];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let (z, n, d) = (target.depth[i], target.normal[i], target.distance[i]);
            let differs = |j: usize| {
                let (zj, nj, dj) = (target.depth[j], target.normal[j], target.distance[j]);
                if z.is_finite() != zj.is_finite() {
                    return true;
                }
                if !z.is_finite() {
                    return false;
                }
                let jump = if d.is_finite() && dj.is_finite() && (d - 1.0).abs() > 1e-6 {
                    (d - dj).abs() > 0.03 * d.min(dj)
                } else {
                    (z - zj).abs() > 0.004
                };
                jump || n.dot(nj) < 0.8
            };
            edge[i] =
                (x + reach < w && differs(i + reach)) || (y + reach < h && differs(i + reach * w));
        }
    }
    let ink = Vec3::splat(0.22);
    for (color, e) in target.color.iter_mut().zip(edge) {
        if e {
            *color = color.lerp(ink, 0.55);
        }
    }
}

/// Renders the mesh to an image.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
pub fn render(mesh: &Mesh, options: &RenderOptions<'_>) -> RgbaImage {
    let ss = options.supersample.clamp(1, 4) as usize;
    let (out_w, out_h) = (
        options.width.max(1) as usize,
        options.height.max(1) as usize,
    );
    let sky = Vec3::new(
        f32::from(options.sky[0]),
        f32::from(options.sky[1]),
        f32::from(options.sky[2]),
    ) / 255.0;
    let mut target = Target {
        width: out_w * ss,
        height: out_h * ss,
        color: vec![sky; out_w * ss * out_h * ss],
        depth: vec![f32::INFINITY; out_w * ss * out_h * ss],
        normal: vec![Vec3::ZERO; out_w * ss * out_h * ss],
        distance: vec![f32::INFINITY; out_w * ss * out_h * ss],
    };
    let images = Images {
        loaded: mesh
            .images
            .iter()
            .map(|p| (options.load_image)(p))
            .collect(),
        _source: std::marker::PhantomData,
    };
    let triangle = |tri: &[u32]| -> Option<([ClipVertex; 3], u32)> {
        let vertices = [tri[0], tri[1], tri[2]].map(|i| mesh.vertices.get(i as usize));
        let [Some(a), Some(b), Some(c)] = vertices else {
            return None;
        };
        Some(([a, b, c].map(|v| vertex(v, options.view_proj)), a.kind))
    };
    let draw = |target: &mut Target, tri: [ClipVertex; 3], kind: u32, transparent: bool| {
        let poly = clip_near(&tri);
        for k in 1..poly.len().saturating_sub(1) {
            raster_triangle(
                target,
                [poly[0], poly[k], poly[k + 1]],
                kind,
                transparent,
                &images,
                options
                    .cut_color
                    .map(|c| Vec3::from(c.map(f32::from)) / 255.0),
            );
        }
    };
    for tri in mesh.indices.as_chunks::<3>().0 {
        if let Some((verts, kind)) = triangle(tri) {
            draw(&mut target, verts, kind, false);
        }
    }
    if options.outlines {
        outline(&mut target, ss);
    }
    // Blended surfaces back to front.
    let mut clear: Vec<([ClipVertex; 3], u32, f32)> = mesh
        .transparent
        .as_chunks::<3>()
        .0
        .iter()
        .filter_map(|tri| triangle(tri))
        .map(|(verts, kind)| {
            let depth = verts.iter().map(|v| v.clip.w).fold(f32::MIN, f32::max);
            (verts, kind, depth)
        })
        .collect();
    clear.sort_by(|a, b| b.2.total_cmp(&a.2));
    for (verts, kind, _) in clear {
        draw(&mut target, verts, kind, true);
    }

    let mut image = RgbaImage::new(out_w as u32, out_h as u32);
    let samples = (ss * ss) as f32;
    for y in 0..out_h {
        for x in 0..out_w {
            let mut sum = Vec3::ZERO;
            let mut covered = 0.0_f32;
            for sy in 0..ss {
                for sx in 0..ss {
                    let index = (y * ss + sy) * target.width + x * ss + sx;
                    if options.transparent {
                        if target.depth[index].is_finite() {
                            sum += target.color[index];
                            covered += 1.0;
                        }
                    } else {
                        sum += target.color[index];
                    }
                }
            }
            let (divisor, alpha) = if options.transparent {
                (covered.max(1.0), covered / samples)
            } else {
                (samples, 1.0)
            };
            let c = (sum / divisor * 255.0)
                .round()
                .clamp(Vec3::ZERO, Vec3::splat(255.0));
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            image.put_pixel(
                x as u32,
                y as u32,
                Rgba([
                    c.x as u8,
                    c.y as u8,
                    c.z as u8,
                    (alpha * 255.0).round() as u8,
                ]),
            );
        }
    }
    image
}
