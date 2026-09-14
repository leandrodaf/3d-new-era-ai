//! The home as a 3D scene: triangle meshes shared by the GPU view, and a
//! software renderer so servers and agents can see the home in 3D without
//! a graphics card.

mod camera;
pub mod export;
pub mod mesh;
mod patterns;
pub mod photo;
mod raster;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub use camera::View;
pub use mesh::{IMAGE_BASE, Mesh, ModelSource, Selection, Vertex};
pub use raster::{RenderOptions, render};

/// Top-view images of pieces for the plan, rendered once and cached as PNG
/// files. Safe to share across threads.
#[derive(Debug, Clone)]
pub struct TopViews {
    inner: std::sync::Arc<std::sync::Mutex<TopViewCache>>,
    dir: PathBuf,
    assets: Option<PathBuf>,
    /// Also replace catalog symbols (otherwise only imported models).
    pub all: bool,
    /// Render missing images on a worker thread instead of blocking.
    background: Option<std::sync::mpsc::Sender<newera_core::Furniture>>,
    /// Bumped whenever a background image becomes ready.
    generation: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

#[derive(Debug, Default)]
struct TopViewCache {
    models: ModelCache,
    done: HashMap<u64, Option<String>>,
    queued: std::collections::HashSet<u64>,
}

impl TopViews {
    /// A provider that renders missing images right away (servers, exports).
    pub fn new(dir: PathBuf, assets: Option<PathBuf>, all: bool) -> Self {
        Self {
            inner: std::sync::Arc::default(),
            dir,
            assets,
            all,
            background: None,
            generation: std::sync::Arc::default(),
        }
    }

    /// A provider that renders missing images on a worker thread; they show
    /// up once [`TopViews::generation`] changes.
    pub fn in_background(dir: PathBuf, assets: Option<PathBuf>, all: bool) -> Self {
        let mut views = Self::new(dir, assets, all);
        let (sender, receiver) = std::sync::mpsc::channel::<newera_core::Furniture>();
        let worker = views.clone();
        std::thread::spawn(move || {
            for piece in receiver {
                if let Some(key) = worker.key(&piece) {
                    let image = worker.produce(&piece, key);
                    if let Ok(mut cache) = worker.inner.lock() {
                        cache.done.insert(key, image);
                    }
                    worker
                        .generation
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            }
        });
        views.background = Some(sender);
        views
    }

    /// Changes whenever background images become ready.
    pub fn generation(&self) -> u64 {
        self.generation.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Whether images are still being rendered.
    pub fn busy(&self) -> bool {
        self.inner
            .lock()
            .is_ok_and(|c| c.queued.iter().any(|k| !c.done.contains_key(k)))
    }

    fn key(&self, piece: &newera_core::Furniture) -> Option<u64> {
        use std::hash::{Hash, Hasher};
        if piece.is_opening() || piece.discipline.is_some() || (piece.model.is_none() && !self.all)
        {
            return None;
        }
        let mut copy = piece.clone();
        copy.position = newera_core::Point2::new(0.0, 0.0);
        copy.angle = 0.0;
        copy.elevation = 0.0;
        copy.level = None;
        copy.id = newera_core::FurnitureId(0);
        copy.name.clear();
        copy.info = newera_core::PieceInfo::default();
        copy.properties.clear();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        // Serialized, the look-defining fields hash stably.
        serde_json::to_string(&copy).ok()?.hash(&mut hasher);
        self.assets.hash(&mut hasher);
        Some(hasher.finish())
    }

    /// The cached top view of a piece: rendered now, or queued when working
    /// in the background (then `None` until ready).
    pub fn image_for(&self, piece: &newera_core::Furniture) -> Option<String> {
        let key = self.key(piece)?;
        {
            let mut cache = self.inner.lock().ok()?;
            if let Some(done) = cache.done.get(&key) {
                return done.clone();
            }
            let path = self.dir.join(format!("{key:016x}.png"));
            if path.exists() {
                let found = Some(path.display().to_string());
                cache.done.insert(key, found.clone());
                return found;
            }
            if let Some(sender) = &self.background {
                if cache.queued.insert(key) {
                    let _ = sender.send(piece.clone());
                }
                return None;
            }
        }
        let image = self.produce(piece, key);
        if let Ok(mut cache) = self.inner.lock() {
            cache.done.insert(key, image.clone());
        }
        image
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn produce(&self, piece: &newera_core::Furniture, key: u64) -> Option<String> {
        let path = self.dir.join(format!("{key:016x}.png"));
        let local = {
            let cache = self.inner.lock().ok()?;
            cache.models.piece_model(piece, self.assets.as_deref())
        }
        .unwrap_or_else(|| newera_catalog::piece_mesh(piece));
        let mesh = Mesh::piece_alone(piece, &local);
        let longest = piece.width.max(piece.depth).max(1.0);
        let px_per_cm = (256.0 / longest).min(2.0);
        let (w, h) = (
            ((piece.width * px_per_cm).round() as u32).max(8),
            ((piece.depth * px_per_cm).round() as u32).max(8),
        );
        let (hw, hd, top) = (
            (piece.width / 200.0) as f32,
            (piece.depth / 200.0) as f32,
            (piece.height / 100.0) as f32,
        );
        let view = glam::camera::rh::view::look_at_mat4(
            glam::Vec3::new(0.0, top + 1.0, 0.0),
            glam::Vec3::ZERO,
            glam::Vec3::NEG_Z,
        );
        let proj = glam::camera::rh::proj::directx::orthographic(-hw, hw, -hd, hd, 0.01, top + 2.0);
        let assets = self.assets.clone();
        let load = |file: &str| {
            image::open(newera_core::resolve_asset(assets.as_deref(), file))
                .ok()
                .map(|i| {
                    image::imageops::resize(
                        &i.to_rgba8(),
                        128,
                        128,
                        image::imageops::FilterType::Triangle,
                    )
                })
        };
        let image = render(
            &mesh,
            &RenderOptions {
                width: w,
                height: h,
                view_proj: proj * view,
                sky: [255, 255, 255],
                supersample: 2,
                load_image: &load,
                transparent: true,
            },
        );
        std::fs::create_dir_all(&self.dir).ok();
        image.save(&path).ok().map(|()| path.display().to_string())
    }
}

/// Loads imported models once per file and fits them to each piece.
#[derive(Debug, Default)]
pub struct ModelCache {
    models: std::cell::RefCell<HashMap<PathBuf, Option<newera_catalog::Mesh>>>,
}

impl ModelCache {
    /// Mesh of a piece's imported model, rotated and fitted to its box;
    /// `None` when it has no model or the file can't be read.
    pub fn piece_model(
        &self,
        piece: &newera_core::Furniture,
        assets: Option<&Path>,
    ) -> Option<newera_catalog::Mesh> {
        let path = newera_core::resolve_asset(assets, piece.model.as_deref()?);
        let mut cache = self.models.borrow_mut();
        let mut mesh = cache
            .entry(path.clone())
            .or_insert_with(|| newera_catalog::load_model(&path).ok().map(|m| m.mesh))
            .clone()?;
        mesh.rotate(piece.model_transform.rotation);
        mesh.fit_to(piece.width, piece.depth, piece.height);
        Some(mesh)
    }
}

/// Exports the home's 3D model (`.glb` or `.obj`), with models and textures
/// resolved relative to `assets`. The ground plane is left out.
pub fn export_home(
    home: &newera_core::Home,
    path: &Path,
    assets: Option<&Path>,
) -> Result<(), export::ExportError> {
    let cache = ModelCache::default();
    let models = |piece: &newera_core::Furniture| cache.piece_model(piece, assets);
    let mut mesh = Mesh::from_home(home, &Selection::new(), &models);
    mesh.drop_ground();
    let images = |file: &str| {
        let path = newera_core::resolve_asset(assets, file);
        let bytes = std::fs::read(&path).ok()?;
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map_or_else(|| "png".to_owned(), str::to_ascii_lowercase);
        // Formats other than PNG/JPEG are converted to PNG.
        if matches!(ext.as_str(), "png" | "jpg" | "jpeg") {
            Some((bytes, ext))
        } else {
            let image = image::load_from_memory(&bytes).ok()?;
            let mut png = Vec::new();
            image
                .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
                .ok()?;
            Some((png, "png".to_owned()))
        }
    };
    export::export_mesh(&mesh, path, &images)
}

/// The same day as `time_ms` at `hour` local solar time for `longitude`.
pub fn at_local_hour(time_ms: i64, hour: f64, longitude: f64) -> i64 {
    const DAY: i64 = 86_400_000;
    #[allow(clippy::cast_possible_truncation)]
    let offset = ((hour - longitude / 15.0) * 3_600_000.0) as i64;
    time_ms.div_euclid(DAY) * DAY + offset
}

/// How long a photo may take: samples per pixel and bounces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhotoQuality {
    Draft,
    Good,
    Best,
}

impl PhotoQuality {
    fn budget(self) -> (u32, u32) {
        match self {
            Self::Draft => (8, 2),
            Self::Good => (48, 3),
            Self::Best => (192, 4),
        }
    }
}

/// A photo of the home: sunlight from the compass location at the camera's
/// time (`time_ms`), light from lamps and glass-filtered daylight.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
pub fn photo_home(
    home: &newera_core::Home,
    view: &View,
    time_ms: i64,
    width: u32,
    height: u32,
    assets: Option<&Path>,
    quality: PhotoQuality,
) -> image::RgbaImage {
    use glam::Vec3;
    let cache = ModelCache::default();
    let models = |piece: &newera_core::Furniture| cache.piece_model(piece, assets);
    let mesh = Mesh::from_home(home, &Selection::new(), &models);

    // Sun: azimuth is clockwise from north; the compass says where north is.
    let compass = &home.compass;
    let (azimuth, elevation) = photo::sun_position(
        time_ms,
        compass.latitude.unwrap_or(-23.55),
        compass.longitude.unwrap_or(-46.63),
    );
    let sun = (elevation > -2.0).then(|| {
        let heading = (compass.north_degrees + azimuth).to_radians();
        let el = elevation.max(0.5).to_radians();
        let dir = Vec3::new(
            (heading.sin() * el.cos()) as f32,
            el.sin() as f32,
            (-heading.cos() * el.cos()) as f32,
        )
        .normalize();
        let strength = (elevation / 20.0).clamp(0.15, 1.0) as f32;
        (dir, Vec3::new(1.0, 0.95, 0.88) * 3.2 * strength)
    });
    let daylight = sun.map_or(0.05, |(d, _)| 0.35 + 0.65 * d.y);
    let sky_color = Vec3::from(home.environment.sky_color.map(f32::from)) / 255.0;
    let sky = sky_color.powf(2.2) * 1.4 * daylight;

    // Lamps of the storeys shown.
    let mut lights = Vec::new();
    for top in &home.furniture {
        for piece in top.visible_leaves() {
            let Some(light) = &piece.light else { continue };
            let floor = home.elevation_of(piece.level);
            for source in &light.sources {
                let at = piece.to_plan((
                    (source.x - 0.5) * piece.width,
                    (source.y - 0.5) * piece.depth,
                ));
                let z = floor + piece.elevation + source.z.clamp(0.0, 1.0) * piece.height;
                let color = Vec3::from(source.color.map(f32::from)) / 255.0;
                let share = light.sources.len().max(1) as f32;
                lights.push(photo::PointLight {
                    position: Vec3::new(at.x as f32, z as f32, at.y as f32) * 0.01,
                    intensity: color.powf(2.2) * (light.power as f32 * 6.0 / share),
                    radius: 0.05,
                });
            }
        }
    }
    let (samples, bounces) = quality.budget();
    let load = |file: &str| {
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
    };
    photo::render_photo(
        &mesh,
        &photo::PhotoOptions {
            width,
            height,
            view: *view,
            samples,
            bounces,
            sun,
            sky,
            lights,
            exposure: 1.1,
            load_image: &load,
        },
    )
}

/// Renders a home from a point of view: builds the mesh, loads models and
/// textures relative to `assets`, and rasterizes it in software.
pub fn render_home(
    home: &newera_core::Home,
    view: &View,
    width: u32,
    height: u32,
    assets: Option<&Path>,
) -> image::RgbaImage {
    let cache = ModelCache::default();
    let models = |piece: &newera_core::Furniture| cache.piece_model(piece, assets);
    let mesh = Mesh::from_home(home, &Selection::new(), &models);
    #[allow(clippy::cast_precision_loss)]
    let aspect = width.max(1) as f32 / height.max(1) as f32;
    let load = |file: &str| {
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
    };
    render(
        &mesh,
        &RenderOptions {
            width,
            height,
            view_proj: view.view_proj(aspect),
            sky: home.environment.sky_color,
            supersample: 2,
            load_image: &load,
            transparent: false,
        },
    )
}

#[cfg(test)]
mod tests {
    use newera_core::{Command, Document, Point2, Room, Wall};

    use super::*;

    #[test]
    fn renders_walls_floor_and_sky() {
        let mut doc = Document::default();
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
        let mut room = Room::new(
            doc.new_room_id(),
            "Sala",
            pts.iter().map(|&(x, y)| Point2::new(x, y)).collect(),
        );
        room.floor_material = Some("wood".parse().unwrap());
        commands.push(Command::insert(room));
        doc.execute(Command::Batch { commands }).unwrap();

        let home = doc.home();
        let image = render_home(home, &View::aerial(home, -50.0, 40.0), 160, 120, None);
        assert_eq!(image.dimensions(), (160, 120));
        let sky = home.environment.sky_color;
        let top = image.get_pixel(2, 2).0;
        assert_eq!([top[0], top[1], top[2]], sky, "sky at the top corner");
        let center = image.get_pixel(80, 70).0;
        assert_ne!(
            [center[0], center[1], center[2]],
            sky,
            "the house fills the middle"
        );
        let distinct: std::collections::HashSet<[u8; 4]> = image.pixels().map(|p| p.0).collect();
        assert!(
            distinct.len() > 50,
            "shaded, textured pixels: {}",
            distinct.len()
        );
    }
}
