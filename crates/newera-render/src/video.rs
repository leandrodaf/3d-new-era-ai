//! Videos along a camera path: keyframes are interpolated with a
//! Catmull-Rom spline at the path speed, each frame is rasterized and the
//! frames are packed as Motion-JPEG in an AVI file (plays in VLC, mpv,
//! ffmpeg and most desktop players).

use std::io::Write;
use std::path::Path;

use image::RgbaImage;
use newera_core::{Camera, Home};

use crate::{Mesh, ModelCache, RenderOptions, Selection, View, render};

/// Turning speed used to time keyframes that mostly rotate, degrees/s.
const TURN_DEG_PER_S: f64 = 45.0;

/// Unwraps headings so consecutive keyframes differ by at most 180°.
fn unwrapped_yaws(keys: &[Camera]) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::with_capacity(keys.len());
    for key in keys {
        let yaw = match out.last() {
            Some(&prev) => prev + (key.yaw - prev + 180.0).rem_euclid(360.0) - 180.0,
            None => key.yaw,
        };
        out.push(yaw);
    }
    out
}

/// Seconds each segment between keyframes lasts at `speed` m/s.
pub fn segment_durations(keys: &[Camera], speed: f64) -> Vec<f64> {
    let yaws = unwrapped_yaws(keys);
    keys.windows(2)
        .zip(yaws.windows(2))
        .map(|(pair, yaw)| {
            let (a, b) = (&pair[0], &pair[1]);
            let meters =
                ((b.x - a.x).powi(2) + (b.y - a.y).powi(2) + (b.z - a.z).powi(2)).sqrt() / 100.0;
            let turn = (yaw[1] - yaw[0])
                .abs()
                .max((b.pitch - a.pitch).abs())
                .max((b.fov - a.fov).abs());
            (meters / speed.max(0.01))
                .max(turn / TURN_DEG_PER_S)
                .max(0.2)
        })
        .collect()
}

fn catmull_rom(p0: f64, p1: f64, p2: f64, p3: f64, t: f64) -> f64 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * (2.0 * p1
        + (p2 - p0) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t3)
}

/// The cameras of every frame of a video along `keys` at `fps` and `speed`
/// m/s. One keyframe gives a single frame; none gives no frames.
pub fn interpolate_path(keys: &[Camera], fps: u32, speed: f64) -> Vec<Camera> {
    match keys {
        [] => return Vec::new(),
        [only] => return vec![only.clone()],
        _ => {}
    }
    let yaws = unwrapped_yaws(keys);
    let durations = segment_durations(keys, speed);
    let total: f64 = durations.iter().sum();
    let fps = f64::from(fps.max(1));
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let frames = ((total * fps).round() as usize).max(2);
    let last = keys.len() - 1;
    (0..frames)
        .map(|frame| {
            #[allow(clippy::cast_precision_loss)]
            let mut t = frame as f64 / (frames - 1) as f64 * total;
            let mut segment = 0;
            while segment < durations.len() - 1 && t > durations[segment] {
                t -= durations[segment];
                segment += 1;
            }
            let u = (t / durations[segment]).clamp(0.0, 1.0);
            let index = |offset: isize| {
                let i = isize::try_from(segment).unwrap_or(0) + offset;
                usize::try_from(i.clamp(0, isize::try_from(last).unwrap_or(0))).unwrap_or(0)
            };
            let (i0, i1, i2, i3) = (index(-1), index(0), index(1), index(2));
            let spline = |f: &dyn Fn(usize) -> f64| catmull_rom(f(i0), f(i1), f(i2), f(i3), u);
            let a = &keys[i1];
            let b = &keys[i2];
            #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
            let time = a.time + ((b.time - a.time) as f64 * u) as i64;
            Camera {
                name: None,
                x: spline(&|i| keys[i].x),
                y: spline(&|i| keys[i].y),
                z: spline(&|i| keys[i].z),
                yaw: spline(&|i| yaws[i]).rem_euclid(360.0),
                pitch: spline(&|i| keys[i].pitch),
                fov: spline(&|i| keys[i].fov),
                time,
                lens: a.lens.clone(),
                renderer: a.renderer.clone(),
            }
        })
        .collect()
}

/// Keyframes circling the building at `height` cm, looking at its center:
/// a ready-made aerial tour.
pub fn orbit_path(home: &Home, height: f64, points: usize) -> Vec<Camera> {
    let (center, radius) = home
        .building_bounds()
        .map_or(((0.0, 0.0), 1000.0), |(min, max)| {
            (
                (f64::midpoint(min.x, max.x), f64::midpoint(min.y, max.y)),
                min.distance(max) * 0.75,
            )
        });
    let points = points.max(3);
    let pitch = (height - 100.0).atan2(radius).to_degrees();
    (0..=points)
        .map(|i| {
            #[allow(clippy::cast_precision_loss)]
            let angle = i as f64 / points as f64 * std::f64::consts::TAU;
            let (x, y) = (
                center.0 + radius * angle.cos(),
                center.1 + radius * angle.sin(),
            );
            // Camera::direction is (-sin yaw, cos yaw); face the center.
            let yaw = (-(center.0 - x)).atan2(center.1 - y).to_degrees();
            Camera {
                name: Some(format!("Órbita {}", i + 1)),
                x,
                y,
                z: height,
                yaw,
                pitch,
                fov: 63.0,
                ..Camera::default()
            }
        })
        .collect()
}

/// Rasterizes `cameras` one by one, building the home mesh only once.
pub fn render_frames(
    home: &Home,
    cameras: &[Camera],
    width: u32,
    height: u32,
    assets: Option<&Path>,
    mut on_frame: impl FnMut(usize, RgbaImage),
) {
    let cache = ModelCache::default();
    let models = |piece: &newera_core::Furniture| cache.piece_model(piece, assets);
    let mesh = Mesh::from_home(home, &Selection::new(), &models);
    #[allow(clippy::cast_precision_loss)]
    let aspect = width.max(1) as f32 / height.max(1) as f32;
    let textures = std::cell::RefCell::new(std::collections::HashMap::new());
    let load = |file: &str| {
        textures
            .borrow_mut()
            .entry(file.to_owned())
            .or_insert_with(|| {
                image::open(newera_core::resolve_asset(assets, file))
                    .ok()
                    .map(|i| {
                        image::imageops::resize(
                            &i.to_rgba8(),
                            256,
                            256,
                            image::imageops::FilterType::Triangle,
                        )
                    })
            })
            .clone()
    };
    for (i, camera) in cameras.iter().enumerate() {
        let view = View::from_camera(camera, aspect);
        let image = render(
            &mesh,
            &RenderOptions {
                width,
                height,
                view_proj: view.view_proj(aspect),
                sky: home.environment.sky_color,
                supersample: 2,
                load_image: &load,
                transparent: false,
                outlines: true,
                cut_color: None,
            },
        );
        on_frame(i, image);
    }
}

/// Packs JPEG frames into a Motion-JPEG AVI.
///
/// # Errors
/// Only I/O errors of `out`.
pub fn write_mjpeg_avi(
    out: &mut impl Write,
    frames: &[Vec<u8>],
    width: u32,
    height: u32,
    fps: u32,
) -> std::io::Result<()> {
    fn chunk(buf: &mut Vec<u8>, id: [u8; 4], data: &[u8]) {
        buf.extend_from_slice(&id);
        buf.extend_from_slice(&u32::try_from(data.len()).unwrap_or(u32::MAX).to_le_bytes());
        buf.extend_from_slice(data);
        if data.len() % 2 == 1 {
            buf.push(0);
        }
    }
    fn list(buf: &mut Vec<u8>, kind: [u8; 4], body: &[u8]) {
        buf.extend_from_slice(b"LIST");
        buf.extend_from_slice(
            &u32::try_from(body.len() + 4)
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        buf.extend_from_slice(&kind);
        buf.extend_from_slice(body);
    }
    let le = |values: &[u32]| {
        values
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<u8>>()
    };
    let count = u32::try_from(frames.len()).unwrap_or(u32::MAX);
    let fps = fps.max(1);
    let largest = frames.iter().map(Vec::len).max().unwrap_or(0);
    let largest = u32::try_from(largest).unwrap_or(u32::MAX);

    let mut avih = le(&[
        1_000_000 / fps,
        largest.saturating_mul(fps),
        0,
        0x10, // AVIF_HASINDEX
        count,
        0,
        1,
        largest,
        width,
        height,
    ]);
    avih.extend_from_slice(&[0; 16]);

    let mut strh = Vec::with_capacity(56);
    strh.extend_from_slice(b"vidsMJPG");
    strh.extend_from_slice(&le(&[0]));
    strh.extend_from_slice(&[0; 4]); // priority, language
    strh.extend_from_slice(&le(&[0, 1, fps, 0, count, largest, u32::MAX, 0]));
    for v in [
        0u16,
        0,
        u16::try_from(width).unwrap_or(u16::MAX),
        u16::try_from(height).unwrap_or(u16::MAX),
    ] {
        strh.extend_from_slice(&v.to_le_bytes());
    }

    let mut strf = le(&[40, width, height]);
    strf.extend_from_slice(&1u16.to_le_bytes());
    strf.extend_from_slice(&24u16.to_le_bytes());
    strf.extend_from_slice(b"MJPG");
    strf.extend_from_slice(&le(&[width * height * 3, 0, 0, 0, 0]));

    let mut strl = Vec::new();
    chunk(&mut strl, *b"strh", &strh);
    chunk(&mut strl, *b"strf", &strf);
    let mut hdrl = Vec::new();
    chunk(&mut hdrl, *b"avih", &avih);
    list(&mut hdrl, *b"strl", &strl);

    let mut movi = Vec::new();
    let mut index = Vec::with_capacity(frames.len() * 16);
    for frame in frames {
        // Offsets are relative to the `movi` fourcc.
        let offset = u32::try_from(movi.len() + 4).unwrap_or(u32::MAX);
        chunk(&mut movi, *b"00dc", frame);
        index.extend_from_slice(b"00dc");
        index.extend_from_slice(&le(&[
            0x10,
            offset,
            u32::try_from(frame.len()).unwrap_or(u32::MAX),
        ]));
    }

    let mut body = Vec::with_capacity(hdrl.len() + movi.len() + index.len() + 64);
    body.extend_from_slice(b"AVI ");
    list(&mut body, *b"hdrl", &hdrl);
    list(&mut body, *b"movi", &movi);
    chunk(&mut body, *b"idx1", &index);
    out.write_all(b"RIFF")?;
    out.write_all(&u32::try_from(body.len()).unwrap_or(u32::MAX).to_le_bytes())?;
    out.write_all(&body)
}

/// Encodes a frame as JPEG at `quality` (1–100).
pub fn jpeg(image: &RgbaImage, quality: u8) -> Vec<u8> {
    let rgb = image::DynamicImage::ImageRgba8(image.clone()).to_rgb8();
    let mut out = Vec::new();
    let mut encoder =
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality.clamp(1, 100));
    let _ = encoder.encode_image(&rgb);
    out
}

/// Summary of a finished video.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VideoInfo {
    pub frames: usize,
    pub seconds: f64,
    pub bytes: usize,
}

/// Renders a video of `home` along its camera path (or `keys`) to an AVI.
/// `progress` receives `(done, total)` after each frame.
///
/// # Errors
/// When there are fewer than two keyframes or the file can't be written.
#[allow(clippy::too_many_arguments)]
pub fn render_video(
    home: &Home,
    keys: &[Camera],
    fps: u32,
    speed: f64,
    size: (u32, u32),
    assets: Option<&Path>,
    file: &Path,
    mut progress: impl FnMut(usize, usize),
) -> Result<VideoInfo, String> {
    if keys.len() < 2 {
        return Err("the camera path needs at least 2 points".into());
    }
    let (width, height) = (size.0.max(16) & !1, size.1.max(16) & !1);
    let cameras = interpolate_path(keys, fps, speed);
    let total = cameras.len();
    let mut frames = Vec::with_capacity(total);
    render_frames(home, &cameras, width, height, assets, |i, image| {
        frames.push(jpeg(&image, 88));
        progress(i + 1, total);
    });
    let mut bytes = Vec::new();
    write_mjpeg_avi(&mut bytes, &frames, width, height, fps).map_err(|e| e.to_string())?;
    std::fs::write(file, &bytes).map_err(|e| format!("{}: {e}", file.display()))?;
    #[allow(clippy::cast_precision_loss)]
    Ok(VideoInfo {
        frames: total,
        seconds: total as f64 / f64::from(fps.max(1)),
        bytes: bytes.len(),
    })
}

#[cfg(test)]
mod tests {
    use newera_core::{Point2, Wall};

    use super::*;

    fn cam(x: f64, y: f64, yaw: f64) -> Camera {
        Camera {
            x,
            y,
            z: 170.0,
            yaw,
            pitch: 0.0,
            fov: 63.0,
            ..Camera::default()
        }
    }

    #[test]
    fn path_passes_through_keyframes_at_the_given_speed() {
        let keys = [
            cam(0.0, 0.0, 0.0),
            cam(500.0, 0.0, 0.0),
            cam(500.0, 500.0, 0.0),
        ];
        // 10 m at 1 m/s and 10 fps: ~100 frames.
        let frames = interpolate_path(&keys, 10, 1.0);
        assert!((99..=101).contains(&frames.len()), "{}", frames.len());
        let first = &frames[0];
        let last = frames.last().unwrap();
        assert!(first.x.abs() < 1e-6 && first.y.abs() < 1e-6);
        assert!((last.x - 500.0).abs() < 1e-6 && (last.y - 500.0).abs() < 1e-6);
        let middle = &frames[frames.len() / 2];
        assert!(
            (middle.x - 500.0).abs() < 20.0 && middle.y.abs() < 20.0,
            "{middle:?}"
        );
    }

    #[test]
    fn heading_takes_the_short_way_round() {
        let keys = [cam(0.0, 0.0, 350.0), cam(0.0, 0.0, 10.0)];
        let frames = interpolate_path(&keys, 25, 1.0);
        assert!(
            frames.iter().all(|c| c.yaw >= 349.0 || c.yaw <= 11.0),
            "{:?}",
            frames.iter().map(|c| c.yaw).collect::<Vec<_>>()
        );
        // 20° at 45°/s, but never under 0.2 s.
        #[allow(clippy::cast_precision_loss)]
        let n = frames.len() as f64;
        assert!((n - 25.0 * 20.0 / 45.0).abs() <= 1.0);
    }

    #[test]
    fn orbit_faces_the_center() {
        let mut home = Home::default();
        let id = home.new_wall_id();
        home.walls.push(Wall::new(
            id,
            Point2::new(0.0, 0.0),
            Point2::new(1000.0, 0.0),
        ));
        let id = home.new_wall_id();
        home.walls.push(Wall::new(
            id,
            Point2::new(1000.0, 0.0),
            Point2::new(1000.0, 800.0),
        ));
        let keys = orbit_path(&home, 800.0, 8);
        assert_eq!(keys.len(), 9);
        for key in &keys {
            let (dx, dy) = key.direction();
            let (tx, ty) = (500.0 - key.x, 400.0 - key.y);
            let len = (tx * tx + ty * ty).sqrt();
            assert!((dx - tx / len).abs() < 1e-6 && (dy - ty / len).abs() < 1e-6);
            assert!(key.pitch > 0.0, "looks down");
        }
    }

    #[test]
    fn writes_a_playable_mjpeg_avi() {
        let mut home = Home::default();
        let id = home.new_wall_id();
        home.walls.push(Wall::new(
            id,
            Point2::new(0.0, 0.0),
            Point2::new(400.0, 0.0),
        ));
        let dir = std::env::temp_dir().join(format!("newera-video-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("tour.avi");
        let keys = [cam(200.0, 400.0, 180.0), cam(200.0, 300.0, 180.0)];
        let mut calls = 0;
        let info = render_video(&home, &keys, 10, 1.0, (65, 48), None, &file, |_, _| {
            calls += 1;
        })
        .unwrap();
        assert_eq!(calls, info.frames);
        assert_eq!(info.frames, 10);
        let bytes = std::fs::read(&file).unwrap();
        assert_eq!(bytes.len(), info.bytes);
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"AVI ");
        let riff = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        assert_eq!(riff + 8, bytes.len());
        let find = |needle: &[u8]| bytes.windows(needle.len()).position(|w| w == needle);
        assert!(find(b"MJPG").is_some());
        assert!(find(b"idx1").is_some());
        // Width was made even; the first frame is a JPEG.
        let avih = find(b"avih").unwrap() + 8;
        assert_eq!(
            u32::from_le_bytes(bytes[avih + 32..avih + 36].try_into().unwrap()),
            64
        );
        let movi = find(b"movi").unwrap();
        assert_eq!(&bytes[movi + 4..movi + 8], b"00dc");
        assert_eq!(&bytes[movi + 12..movi + 14], &[0xFF, 0xD8]);
        // The index points at the frame chunks.
        let idx = find(b"idx1").unwrap() + 8;
        let offset = u32::from_le_bytes(bytes[idx + 8..idx + 12].try_into().unwrap()) as usize;
        assert_eq!(&bytes[movi + offset..movi + offset + 4], b"00dc");
        assert!(
            render_video(&home, &keys[..1], 10, 1.0, (64, 48), None, &file, |_, _| {}).is_err()
        );
        std::fs::remove_dir_all(dir).ok();
    }
}
