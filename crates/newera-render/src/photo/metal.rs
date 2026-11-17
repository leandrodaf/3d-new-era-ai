//! Apple Silicon backend for the shared photo renderer.
#![allow(clippy::cast_precision_loss)]

use super::{
    AUTO_EXPOSURE_KEY, Mesh, PhotoOptions, RgbaImage, Surface, Vec3, finish_photo, luminance,
};
use newera_metal::{Lamp, Params, Pixel, Scene};

const SHADER: &str = include_str!("photo.metal");
const ROWS_PER_PASS: u32 = 32;

pub(super) fn render(mesh: &Mesh, options: &PhotoOptions<'_>) -> Option<RgbaImage> {
    let width = options.width.max(1);
    let height = options.height.max(1);
    let forward = (options.view.target - options.view.eye).normalize();
    let right = forward.cross(Vec3::Y).normalize_or(Vec3::X);
    let up = right.cross(forward);
    let tan = (options.view.fov_y / 2.0).tan();
    let (sun_direction, sun_color) = options.sun.unwrap_or_default();
    let mut params = Params {
        eye: options.view.eye.extend(0.0).to_array(),
        forward: forward.extend(0.0).to_array(),
        right: right.extend(tan * width as f32 / height as f32).to_array(),
        up: up.extend(tan).to_array(),
        sky: options.sky.extend(0.0).to_array(),
        sun_direction: sun_direction.extend(0.0).to_array(),
        sun_color: sun_color.extend(0.0).to_array(),
        width,
        height,
        first_row: 0,
        rows: height,
        samples: options.samples.max(1),
        bounces: options.bounces,
        opaque_triangles: 0,
        light_count: 0,
        squeeze: 0.0,
        coarse: 0,
        padding: [0; 2],
    };
    let lamps: Vec<Lamp> = options
        .lights
        .iter()
        .map(|lamp| {
            let (u, v) = lamp.panel.unwrap_or_default();
            let area = 4.0 * u.length() * v.length();
            let p = lamp.position;
            let i = lamp.intensity;
            [
                p.x,
                p.y,
                p.z,
                lamp.radius,
                i.x,
                i.y,
                i.z,
                lamp.half_angle,
                u.x,
                u.y,
                u.z,
                area,
                v.x,
                v.y,
                v.z,
                lamp.clearance,
            ]
        })
        .collect();
    let textures = crate::raster::metal_textures(mesh, options.load_image);
    if newera_core::progress::cancelled() {
        return Some(RgbaImage::new(1, 1));
    }
    let textures = textures?;
    let mut scene = Scene::new(
        SHADER,
        bytemuck::cast_slice(&mesh.vertices),
        &mesh.indices,
        &mesh.transparent,
        &textures,
        &lamps,
    )?;
    drop(textures);
    if newera_core::progress::cancelled() {
        return Some(RgbaImage::new(1, 1));
    }

    let coarse = Params {
        width: 24,
        height: 18,
        rows: 18,
        samples: 1,
        coarse: 1,
        ..params
    };
    let mut exposure_pixels = vec![[0.0; 12]; 24 * 18];
    if !scene.render(coarse, &mut exposure_pixels) {
        return None;
    }
    let log_mean = exposure_pixels
        .iter()
        .map(|pixel| (luminance(rgb(pixel, 0)) + 1e-4).ln())
        .sum::<f32>()
        / exposure_pixels.len() as f32;
    params.squeeze = options.exposure * (AUTO_EXPOSURE_KEY / log_mean.exp()).clamp(0.3, 4000.0);

    let mut pixels = Vec::with_capacity((width as usize).checked_mul(height as usize)?);
    let mut batch = vec![[0.0; 12]; width as usize * height.min(ROWS_PER_PASS) as usize];
    newera_core::progress::step("Renderizando imagem", 0, u64::from(height));
    for row in (0..height).step_by(ROWS_PER_PASS as usize) {
        if newera_core::progress::cancelled() {
            return Some(RgbaImage::new(1, 1));
        }
        params.first_row = row;
        params.rows = (height - row).min(ROWS_PER_PASS);
        let output = &mut batch[..width as usize * params.rows as usize];
        if !scene.render(params, output) {
            return None;
        }
        pixels.extend(output.iter().map(|p| {
            (
                rgb(p, 0),
                Surface {
                    albedo: rgb(p, 4),
                    normal: rgb(p, 8),
                    depth: p[11],
                },
            )
        }));
        newera_core::progress::step(
            "Renderizando imagem",
            u64::from(row + params.rows),
            u64::from(height),
        );
    }
    if newera_core::progress::cancelled() {
        return Some(RgbaImage::new(1, 1));
    }
    // Wide passes (8 and 16 pixel strides) leave a grid around bright fixtures.
    let image = finish_photo(
        &pixels,
        width as usize,
        height as usize,
        options.exposure,
        3,
    );
    eprintln!("New Era photo: Metal");
    Some(image)
}

fn rgb(pixel: &Pixel, offset: usize) -> Vec3 {
    Vec3::new(pixel[offset], pixel[offset + 1], pixel[offset + 2])
}
