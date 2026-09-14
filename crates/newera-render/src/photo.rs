//! Photo renderer: a CPU path tracer over scene meshes with sunlight from
//! the home's location and time, light sources from furniture, soft
//! indirect light and translucent glass. Uses every core.

use glam::{Vec3, Vec4};
use image::{Rgba, RgbaImage};

use crate::camera::View;
use crate::mesh::Mesh;
use crate::raster::{Images, albedo};

/// A small spherical light (a lamp's source).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointLight {
    pub position: Vec3,
    /// Radiant intensity per color channel.
    pub intensity: Vec3,
    pub radius: f32,
}

/// The thread-shareable part of the options.
struct Lighting {
    bounces: u32,
    sun: Option<(Vec3, Vec3)>,
    sky: Vec3,
    lights: Vec<PointLight>,
}

/// Parameters of a photo.
pub struct PhotoOptions<'a> {
    pub width: u32,
    pub height: u32,
    pub view: View,
    /// Samples per pixel.
    pub samples: u32,
    /// Light bounces after the first hit.
    pub bounces: u32,
    /// Unit vector toward the sun and its radiance; `None` at night.
    pub sun: Option<(Vec3, Vec3)>,
    /// Radiance of the sky seen through openings.
    pub sky: Vec3,
    pub lights: Vec<PointLight>,
    pub exposure: f32,
    pub load_image: &'a dyn Fn(&str) -> Option<RgbaImage>,
}

impl std::fmt::Debug for PhotoOptions<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PhotoOptions")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("samples", &self.samples)
            .field("lights", &self.lights.len())
            .finish_non_exhaustive()
    }
}

/// Sun position for a moment and place: `(azimuth, elevation)` in degrees,
/// azimuth clockwise from north. Simplified solar ephemeris (good to a
/// fraction of a degree, plenty for daylight).
pub fn sun_position(time_ms: i64, latitude: f64, longitude: f64) -> (f64, f64) {
    #[allow(clippy::cast_precision_loss)]
    let days = time_ms as f64 / 86_400_000.0 + 2_440_587.5 - 2_451_545.0;
    let rad = f64::to_radians;
    let g = rad(357.529 + 0.985_600_28 * days);
    let q = 280.459 + 0.985_647_36 * days;
    let l = rad(q + 1.915 * g.sin() + 0.020 * (2.0 * g).sin());
    let e = rad(23.439 - 0.000_000_36 * days);
    let right_ascension = (e.cos() * l.sin()).atan2(l.cos());
    let declination = (e.sin() * l.sin()).asin();
    let sidereal = (18.697_374_558 + 24.065_709_824_419_08 * days).rem_euclid(24.0) * 15.0;
    let hour_angle = rad(sidereal + longitude) - right_ascension;
    let lat = rad(latitude);
    let elevation =
        (lat.sin() * declination.sin() + lat.cos() * declination.cos() * hour_angle.cos()).asin();
    let azimuth =
        (-hour_angle.sin()).atan2(declination.tan() * lat.cos() - lat.sin() * hour_angle.cos());
    (
        azimuth.to_degrees().rem_euclid(360.0),
        elevation.to_degrees(),
    )
}

#[derive(Clone, Copy)]
struct Bounds {
    min: Vec3,
    max: Vec3,
}

impl Bounds {
    const EMPTY: Self = Self {
        min: Vec3::splat(f32::MAX),
        max: Vec3::splat(f32::MIN),
    };

    fn grow(&mut self, p: Vec3) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }

    fn hit(&self, origin: Vec3, inv: Vec3, max_t: f32) -> bool {
        let t1 = (self.min - origin) * inv;
        let t2 = (self.max - origin) * inv;
        let near = t1.min(t2).max_element();
        let far = t1.max(t2).min_element();
        near <= far && far >= 0.0 && near <= max_t
    }
}

struct Node {
    bounds: Bounds,
    /// Leaf: first triangle; inner: index of the right child (left is next).
    start: u32,
    /// Triangles in a leaf; 0 for inner nodes.
    count: u32,
}

struct Scene<'a> {
    mesh: &'a Mesh,
    tris: Vec<[u32; 3]>,
    transparent: Vec<bool>,
    nodes: Vec<Node>,
    images: Images<'a>,
}

impl Scene<'_> {
    fn corner(&self, i: u32) -> Vec3 {
        Vec3::from(self.mesh.vertices[i as usize].position)
    }

    fn build(&mut self) {
        let mut order: Vec<u32> = (0..u32::try_from(self.tris.len()).unwrap_or(0)).collect();
        let centroids: Vec<Vec3> = self
            .tris
            .iter()
            .map(|t| (self.corner(t[0]) + self.corner(t[1]) + self.corner(t[2])) / 3.0)
            .collect();
        let bounds: Vec<Bounds> = self
            .tris
            .iter()
            .map(|t| {
                let mut b = Bounds::EMPTY;
                for &i in t {
                    b.grow(self.corner(i));
                }
                b
            })
            .collect();
        let mut nodes = Vec::with_capacity(self.tris.len() * 2 / 3 + 1);
        split(&mut nodes, &mut order, 0, &centroids, &bounds);
        self.tris = order.iter().map(|&i| self.tris[i as usize]).collect();
        self.transparent = order
            .iter()
            .map(|&i| self.transparent[i as usize])
            .collect();
        self.nodes = nodes;
    }

    /// Nearest hit: `(t, triangle, u, v)`.
    fn intersect(&self, origin: Vec3, dir: Vec3, max_t: f32) -> Option<(f32, usize, f32, f32)> {
        if self.nodes.is_empty() {
            return None;
        }
        let inv = dir.recip();
        let mut best: Option<(f32, usize, f32, f32)> = None;
        let mut stack = [0u32; 64];
        let mut top = 1;
        while top > 0 {
            top -= 1;
            let index = stack[top];
            let node = &self.nodes[index as usize];
            let limit = best.map_or(max_t, |b| b.0);
            if !node.bounds.hit(origin, inv, limit) {
                continue;
            }
            if node.count > 0 {
                for k in node.start..node.start + node.count {
                    let t = self.tris[k as usize];
                    if let Some((d, u, v)) = triangle_hit(
                        origin,
                        dir,
                        self.corner(t[0]),
                        self.corner(t[1]),
                        self.corner(t[2]),
                    ) && d > 1e-4
                        && d < best.map_or(max_t, |b| b.0)
                    {
                        best = Some((d, k as usize, u, v));
                    }
                }
            } else if top + 2 <= stack.len() {
                stack[top] = node.start;
                stack[top + 1] = index + 1;
                top += 2;
            }
        }
        best
    }

    /// Light passing from `origin` along `dir` for `distance`: 0 when
    /// blocked, reduced by glass.
    fn transmittance(&self, origin: Vec3, dir: Vec3, distance: f32) -> f32 {
        let mut from = origin;
        let mut left = distance;
        let mut through = 1.0;
        for _ in 0..4 {
            let Some((t, k, _, _)) = self.intersect(from, dir, left) else {
                return through;
            };
            if !self.transparent[k] {
                return 0.0;
            }
            let alpha = self.mesh.vertices[self.tris[k][0] as usize].color[3];
            through *= 1.0 - alpha.clamp(0.0, 1.0) * 0.85;
            from += dir * (t + 1e-3);
            left -= t + 1e-3;
            if left <= 0.0 {
                return through;
            }
        }
        through
    }
}

/// Builds nodes depth-first: a node is followed by its left subtree; inner
/// nodes store the index of their right child in `start`.
fn split(
    nodes: &mut Vec<Node>,
    order: &mut [u32],
    offset: u32,
    centroids: &[Vec3],
    bounds: &[Bounds],
) {
    let mut b = Bounds::EMPTY;
    let mut c = Bounds::EMPTY;
    for &i in order.iter() {
        b.grow(bounds[i as usize].min);
        b.grow(bounds[i as usize].max);
        c.grow(centroids[i as usize]);
    }
    let index = nodes.len();
    nodes.push(Node {
        bounds: b,
        start: offset,
        count: u32::try_from(order.len()).unwrap_or(0),
    });
    if order.len() <= 4 {
        return;
    }
    let extent = c.max - c.min;
    let axis = if extent.x >= extent.y && extent.x >= extent.z {
        0
    } else if extent.y >= extent.z {
        1
    } else {
        2
    };
    if extent[axis] < 1e-6 {
        return;
    }
    let mid = order.len() / 2;
    order.select_nth_unstable_by(mid, |a, b| {
        centroids[*a as usize][axis].total_cmp(&centroids[*b as usize][axis])
    });
    let (left, right) = order.split_at_mut(mid);
    nodes[index].count = 0;
    split(nodes, left, offset, centroids, bounds);
    nodes[index].start = u32::try_from(nodes.len()).unwrap_or(0);
    split(
        nodes,
        right,
        offset + u32::try_from(mid).unwrap_or(0),
        centroids,
        bounds,
    );
}

fn triangle_hit(origin: Vec3, dir: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<(f32, f32, f32)> {
    let (e1, e2) = (b - a, c - a);
    let p = dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-9 {
        return None;
    }
    let inv = 1.0 / det;
    let s = origin - a;
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = dir.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    Some((e2.dot(q) * inv, u, v))
}

/// Small fast random numbers (xorshift64*).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        // The top 24 bits fit an f32 exactly.
        let bits = u32::try_from(self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40).unwrap_or(0);
        #[allow(clippy::cast_precision_loss)]
        let unit = bits as f32 / 16_777_216.0;
        unit
    }
}

fn to_linear(c: Vec3) -> Vec3 {
    c.powf(2.2)
}

/// Cosine-weighted direction around `n`.
fn hemisphere(n: Vec3, rng: &mut Rng) -> Vec3 {
    let (r1, r2) = (rng.next(), rng.next());
    let phi = std::f32::consts::TAU * r1;
    let r = r2.sqrt();
    let (x, y, z) = (r * phi.cos(), r * phi.sin(), (1.0 - r2).sqrt());
    let helper = if n.x.abs() > 0.9 { Vec3::Y } else { Vec3::X };
    let t = helper.cross(n).normalize();
    let b = n.cross(t);
    (t * x + b * y + n * z).normalize()
}

fn aces(x: Vec3) -> Vec3 {
    let (a, b, c, d, e) = (2.51, 0.03, 2.43, 0.59, 0.14);
    ((x * (x * a + b)) / (x * (x * c + d) + e)).clamp(Vec3::ZERO, Vec3::ONE)
}

#[allow(clippy::too_many_lines)]
/// What the camera ray first sees, to guide denoising.
#[derive(Clone, Copy, Default)]
struct Surface {
    albedo: Vec3,
    normal: Vec3,
    depth: f32,
}

fn luminance(c: Vec3) -> f32 {
    c.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}

fn radiance(
    scene: &Scene<'_>,
    options: &Lighting,
    mut origin: Vec3,
    mut dir: Vec3,
    rng: &mut Rng,
    first: &mut Option<Surface>,
) -> Vec3 {
    let mut throughput = Vec3::ONE;
    let mut total = Vec3::ZERO;
    let mut depth = 0;
    let mut pass_through = 0;
    loop {
        let Some((t, k, u, v)) = scene.intersect(origin, dir, 1e4) else {
            let sun_disc = options.sun.map_or(Vec3::ZERO, |(sun_dir, sun)| {
                if depth == 0 && dir.dot(sun_dir) > 0.9995 {
                    sun * 0.02
                } else {
                    Vec3::ZERO
                }
            });
            if first.is_none() {
                *first = Some(Surface {
                    albedo: Vec3::ONE,
                    normal: -dir,
                    depth: 1e4,
                });
            }
            return total + throughput * (options.sky + sun_disc);
        };
        let tri = scene.tris[k];
        let w = 1.0 - u - v;
        let [va, vb, vc] = tri.map(|i| &scene.mesh.vertices[i as usize]);
        let point = origin + dir * t;
        let mut normal =
            (Vec3::from(va.normal) * w + Vec3::from(vb.normal) * u + Vec3::from(vc.normal) * v)
                .normalize_or_zero();
        if normal == Vec3::ZERO {
            normal = (Vec3::from(vb.position) - Vec3::from(va.position))
                .cross(Vec3::from(vc.position) - Vec3::from(va.position))
                .normalize_or_zero();
        }
        if normal.dot(dir) > 0.0 {
            normal = -normal;
        }
        let color = Vec4::from(va.color) * w + Vec4::from(vb.color) * u + Vec4::from(vc.color) * v;
        let uv = [
            va.uv[0] * w + vb.uv[0] * u + vc.uv[0] * v,
            va.uv[1] * w + vb.uv[1] * u + vc.uv[1] * v,
        ];
        let base =
            to_linear(albedo(va.kind, color, uv, 0.002, &scene.images)).min(Vec3::splat(0.95));

        // Glass lets light through.
        if scene.transparent[k] && pass_through < 6 && rng.next() > color.w.clamp(0.0, 1.0) * 0.6 {
            origin = point + dir * 1e-3;
            throughput *= Vec3::ONE.lerp(base, 0.15);
            pass_through += 1;
            continue;
        }

        if first.is_none() {
            *first = Some(Surface {
                albedo: base,
                normal,
                depth: (point - origin).length() + t * 0.0,
            });
        }
        let at = point + normal * 1e-3;
        // Sunlight.
        if let Some((sun_dir, sun)) = options.sun {
            let cos = normal.dot(sun_dir);
            if cos > 0.0 {
                let visible = scene.transmittance(at, sun_dir, 1e4);
                total += throughput * base * sun * (cos * visible / std::f32::consts::PI);
            }
        }
        // One lamp, picked in proportion to how much it could light this
        // point (intensity over squared distance, facing it).
        let weights: Vec<f32> = options
            .lights
            .iter()
            .map(|l| {
                let to = l.position - at;
                if normal.dot(to) <= 0.0 {
                    0.0
                } else {
                    luminance(l.intensity) / to.length_squared().max(0.04)
                }
            })
            .collect();
        let weight_sum: f32 = weights.iter().sum();
        if weight_sum > 0.0 {
            let mut target = rng.next() * weight_sum;
            let mut pick = weights.len() - 1;
            for (i, w) in weights.iter().enumerate() {
                if target < *w {
                    pick = i;
                    break;
                }
                target -= w;
            }
            let probability = weights[pick] / weight_sum;
            let light = options.lights[pick];
            let jitter = hemisphere(Vec3::Y, rng) * light.radius;
            let to_light = light.position + jitter - at;
            let distance = to_light.length();
            let l = to_light / distance.max(1e-4);
            let cos = normal.dot(l);
            if cos > 0.0 {
                let visible = scene.transmittance(at, l, (distance - light.radius).max(0.0));
                total += throughput
                    * base
                    * light.intensity
                    * (cos * visible
                        / probability.max(1e-6)
                        / (distance * distance).max(0.04)
                        / std::f32::consts::PI);
            }
        }

        if depth >= options.bounces {
            return total;
        }
        throughput *= base;
        if depth >= 2 {
            let survive = throughput.max_element().clamp(0.05, 0.95);
            if rng.next() > survive {
                return total;
            }
            throughput /= survive;
        }
        origin = at;
        dir = hemisphere(normal, rng);
        depth += 1;
    }
}

/// Renders a photo of the mesh.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub fn render_photo(mesh: &Mesh, options: &PhotoOptions<'_>) -> RgbaImage {
    let mut tris: Vec<[u32; 3]> = Vec::new();
    let mut transparent = Vec::new();
    for (list, clear) in [(&mesh.indices, false), (&mesh.transparent, true)] {
        for t in list.as_chunks::<3>().0 {
            tris.push(*t);
            transparent.push(clear);
        }
    }
    let mut scene = Scene {
        mesh,
        tris,
        transparent,
        nodes: Vec::new(),
        images: Images {
            loaded: mesh
                .images
                .iter()
                .map(|p| (options.load_image)(p))
                .collect(),
            _source: std::marker::PhantomData,
        },
    };
    scene.build();

    let (w, h) = (
        options.width.max(1) as usize,
        options.height.max(1) as usize,
    );
    let aspect = w as f32 / h as f32;
    let forward = (options.view.target - options.view.eye).normalize();
    let right = forward.cross(Vec3::Y).normalize_or(Vec3::X);
    let up = right.cross(forward);
    let tan = (options.view.fov_y / 2.0).tan();
    let samples = options.samples.max(1);

    let lighting = Lighting {
        bounces: options.bounces,
        sun: options.sun,
        sky: options.sky,
        lights: options.lights.clone(),
    };
    let (eye, exposure) = (options.view.eye, options.exposure);
    let mut pixels = vec![(Vec3::ZERO, Surface::default()); w * h];
    // Browsers (wasm32) have no threads: work on the calling one there.
    let threads = if cfg!(target_arch = "wasm32") {
        1
    } else {
        std::thread::available_parallelism().map_or(4, std::num::NonZero::get)
    };
    let rows_per = h.div_ceil(threads);
    let work = |chunk_index: usize, chunk: &mut [(Vec3, Surface)]| {
        {
            let (scene, lighting) = (&scene, &lighting);
            {
                for (i, pixel) in chunk.iter_mut().enumerate() {
                    let (x, y) = (i % w, chunk_index * rows_per + i / w);
                    let mut rng =
                        Rng(((y as u64) << 32 | x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
                    let mut sum = Vec3::ZERO;
                    let mut surface = Surface::default();
                    for _ in 0..samples {
                        let sx = ((x as f32 + rng.next()) / w as f32 * 2.0 - 1.0) * tan * aspect;
                        let sy = (1.0 - (y as f32 + rng.next()) / h as f32 * 2.0) * tan;
                        let dir = (forward + right * sx + up * sy).normalize();
                        let mut first = None;
                        let value = radiance(scene, lighting, eye, dir, &mut rng, &mut first);
                        // Clamp fireflies from rare paths.
                        sum += value.min(Vec3::splat(20.0));
                        let f = first.unwrap_or_default();
                        surface.albedo += f.albedo;
                        surface.normal += f.normal;
                        surface.depth += f.depth;
                    }
                    let n = samples as f32;
                    *pixel = (
                        sum / n,
                        Surface {
                            albedo: surface.albedo / n,
                            normal: surface.normal.normalize_or_zero(),
                            depth: surface.depth / n,
                        },
                    );
                }
            }
        }
    };
    if threads == 1 {
        work(0, &mut pixels);
    } else {
        std::thread::scope(|s| {
            for (chunk_index, chunk) in pixels.chunks_mut(rows_per * w).enumerate() {
                let work = &work;
                s.spawn(move || work(chunk_index, chunk));
            }
        });
    }

    let denoised = denoise(&pixels, w, h);
    // Auto exposure: bring the scene's average brightness to a mid gray.
    let log_mean = denoised
        .iter()
        .map(|c| (luminance(*c) + 1e-4).ln())
        .sum::<f32>()
        / denoised.len().max(1) as f32;
    let auto = (0.2 / log_mean.exp()).clamp(0.3, 30.0);
    let mut image = RgbaImage::new(w as u32, h as u32);
    for (i, value) in denoised.iter().enumerate() {
        let mapped = aces(*value * exposure * auto).powf(1.0 / 2.2) * 255.0;
        image.put_pixel(
            (i % w) as u32,
            (i / w) as u32,
            Rgba([mapped.x as u8, mapped.y as u8, mapped.z as u8, 255]),
        );
    }
    image
}

/// Edge-aware à-trous filter on lighting (radiance divided by albedo), so
/// textures stay sharp while noise in light and shadow is smoothed.
#[allow(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss
)]
fn denoise(pixels: &[(Vec3, Surface)], w: usize, h: usize) -> Vec<Vec3> {
    let albedo: Vec<Vec3> = pixels
        .iter()
        .map(|(_, s)| s.albedo.max(Vec3::splat(0.03)))
        .collect();
    let mut light: Vec<Vec3> = pixels
        .iter()
        .zip(&albedo)
        .map(|((c, _), a)| *c / *a)
        .collect();
    let kernel = [1.0 / 16.0, 0.25, 3.0 / 8.0, 0.25, 1.0 / 16.0];
    for pass in 0..5 {
        let step = 1i64 << pass;
        let source = light.clone();
        for y in 0..h {
            for x in 0..w {
                let center = y * w + x;
                let (c, s) = (source[center], pixels[center].1);
                let cl = luminance(c);
                let (mut sum, mut weight) = (Vec3::ZERO, 0.0f32);
                for (j, ky) in kernel.iter().enumerate() {
                    let yy = y as i64 + (j as i64 - 2) * step;
                    if yy < 0 || yy >= h as i64 {
                        continue;
                    }
                    for (i, kx) in kernel.iter().enumerate() {
                        let xx = x as i64 + (i as i64 - 2) * step;
                        if xx < 0 || xx >= w as i64 {
                            continue;
                        }
                        let index = yy as usize * w + xx as usize;
                        let (q, qs) = (source[index], pixels[index].1);
                        let normal = s.normal.dot(qs.normal).max(0.0).powi(32);
                        let depth = (-(s.depth - qs.depth).abs()
                            / (0.05 * s.depth.max(0.5) * step as f32))
                            .exp();
                        let lum = (-(cl - luminance(q)).abs() / (0.6 * (cl + 0.05))).exp();
                        let wgt = kx * ky * normal * depth * lum;
                        sum += q * wgt;
                        weight += wgt;
                    }
                }
                light[center] = if weight > 1e-6 { sum / weight } else { c };
            }
        }
    }
    light.iter().zip(&albedo).map(|(l, a)| *l * *a).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noon_sun_is_high_and_midnight_below_the_horizon() {
        // 2026-06-21 12:00 local solar time at the equator's longitude 0.
        let noon = 1_782_043_200_000;
        let (_, elevation) = sun_position(noon, 0.0, 0.0);
        assert!(elevation > 60.0, "{elevation}");
        let (_, night) = sun_position(noon + 12 * 3_600_000, 0.0, 0.0);
        assert!(night < 0.0, "{night}");
        // São Paulo, morning: sun to the east (azimuth between 45° and 135°).
        let morning = 1_789_214_400_000 - 3 * 3_600_000; // 09:00 UTC ≈ 06:00 local
        let (azimuth, _) = sun_position(morning + 3 * 3_600_000, -23.5, -46.6);
        assert!((45.0..135.0).contains(&azimuth), "{azimuth}");
    }

    #[test]
    fn triangles_are_hit_from_either_side() {
        let (a, b, c) = (Vec3::ZERO, Vec3::X, Vec3::Y);
        let hit = triangle_hit(Vec3::new(0.2, 0.2, 1.0), -Vec3::Z, a, b, c).unwrap();
        assert!((hit.0 - 1.0).abs() < 1e-5);
        assert!(triangle_hit(Vec3::new(0.2, 0.2, -1.0), Vec3::Z, a, b, c).is_some());
        assert!(triangle_hit(Vec3::new(2.0, 2.0, 1.0), -Vec3::Z, a, b, c).is_none());
    }
}
