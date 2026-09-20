use super::{ModelCache, TopViews, asset_versions::AssetVersions};
use newera_core::{Furniture, vfs};
use std::path::PathBuf;

struct Assets(PathBuf);
impl Assets {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "newera-cache-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn mount(&self, name: &str, bytes: &[u8]) {
        vfs::mount(&self.0, [(name.into(), bytes.to_vec())]);
    }
    fn piece() -> Furniture {
        Furniture {
            model: Some("test.obj".into()),
            width: 80.0,
            depth: 60.0,
            height: 20.0,
            ..Furniture::default()
        }
    }
}
impl Drop for Assets {
    fn drop(&mut self) {
        vfs::unmount(&self.0);
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
const OBJ: &[u8] =
    b"mtllib test.mtl\nv -1 0 -1\nv 1 0 -1\nv 1 0 1\nv -1 0 1\nusemtl body\nf 1 4 3\nf 1 3 2\n";
const RED: &[u8] = b"newmtl body\nKd 1 0 0\n";
const BLUE: &[u8] = b"newmtl body\nKd 0 0 1\n";

#[test]
fn missing_models_recover_and_material_replacements_change_the_mesh() {
    let assets = Assets::new();
    let piece = Assets::piece();
    let cache = ModelCache::default();
    assert!(cache.piece_model(&piece, Some(&assets.0)).is_none());
    assets.mount("test.obj", OBJ);
    assets.mount("test.mtl", RED);
    let red = cache.piece_model(&piece, Some(&assets.0)).unwrap();
    assets.mount("test.mtl", BLUE);
    let blue = cache.piece_model(&piece, Some(&assets.0)).unwrap();
    assert_eq!(red.positions, blue.positions);
    assert_ne!(red.colors, blue.colors);
    assert_eq!(
        blue.colors,
        cache.piece_model(&piece, Some(&assets.0)).unwrap().colors
    );
}

#[test]
fn dependency_changes_track_missing_and_retargeted_textures() {
    let assets = Assets::new();
    let path = assets.0.join("test.obj");
    assets.mount("test.obj", OBJ);
    let mut versions = AssetVersions::default();
    let missing = versions.get(&path);
    assets.mount("test.mtl", b"newmtl body\nmap_Kd a.png\n");
    let material = versions.get(&path);
    assert_ne!(missing, material);
    assets.mount("a.png", b"red");
    let image = versions.get(&path);
    assert_ne!(material, image);
    assets.mount("a.png", b"blu");
    assert_ne!(image, versions.get(&path));
    assets.mount("test.mtl", b"newmtl body\nmap_Kd b.png\n");
    let retargeted = versions.get(&path);
    assets.mount("a.png", b"unused");
    assert_eq!(retargeted, versions.get(&path));
    assets.mount("b.png", b"new");
    assert_ne!(retargeted, versions.get(&path));
}

#[test]
fn gltf_external_buffers_and_images_invalidate() {
    let assets = Assets::new();
    assets.mount(
        "model.gltf",
        br#"{"buffers":[{"uri":"mesh.bin"}],"images":[{"uri":"image.png"}]}"#,
    );
    let path = assets.0.join("model.gltf");
    let mut versions = AssetVersions::default();
    let first = versions.get(&path);
    assets.mount("mesh.bin", b"geometry");
    let buffer = versions.get(&path);
    assert_ne!(first, buffer);
    assets.mount("image.png", b"image");
    assert_ne!(buffer, versions.get(&path));
}

#[test]
fn persistent_png_is_replaced_after_material_or_renderer_change() {
    let assets = Assets::new();
    assets.mount("test.obj", OBJ);
    assets.mount("test.mtl", RED);
    let piece = Assets::piece();
    let views = TopViews::new(assets.0.join("png"), Some(assets.0.clone()), true);
    let red = views.image_for(&piece).unwrap();
    let red_pixels = image::open(&red).unwrap().to_rgba8();
    assert!(red_pixels.pixels().any(|p| p[3] > 0 && p[0] > p[2]));
    assert_eq!(Some(red.clone()), views.image_for(&piece));
    assets.mount("test.mtl", BLUE);
    let blue = views.image_for(&piece).unwrap();
    assert_ne!(red, blue);
    let blue_pixels = image::open(&blue).unwrap().to_rgba8();
    assert!(blue_pixels.pixels().any(|p| p[3] > 0 && p[2] > p[0]));
    assert_ne!(red_pixels, blue_pixels);
    let mut rebuilt = TopViews::new(assets.0.join("png"), Some(assets.0.clone()), true);
    rebuilt.source_revision = "a different renderer build";
    assert_ne!(Some(blue), rebuilt.image_for(&piece));
    let mut moved = piece.clone();
    moved.position.x = 500.0;
    moved.name = "renamed".into();
    assert_eq!(rebuilt.image_for(&piece), rebuilt.image_for(&moved));
}

#[test]
fn native_files_recover_and_same_length_edits_reload_materials() {
    let assets = Assets::new();
    let piece = Assets::piece();
    let cache = ModelCache::default();
    assert!(cache.piece_model(&piece, Some(&assets.0)).is_none());
    std::fs::write(assets.0.join("test.obj"), OBJ).unwrap();
    let mtl = assets.0.join("test.mtl");
    std::fs::write(&mtl, RED).unwrap();
    let red = cache.piece_model(&piece, Some(&assets.0)).unwrap();
    let old = std::fs::metadata(&mtl).unwrap().modified().unwrap();
    std::fs::write(&mtl, BLUE).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&mtl)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(old + std::time::Duration::from_secs(2)))
        .unwrap();
    let blue = cache.piece_model(&piece, Some(&assets.0)).unwrap();
    assert_ne!(red.colors, blue.colors);
}

#[test]
fn image_overrides_and_changed_model_geometry_invalidate_views() {
    let assets = Assets::new();
    assets.mount("test.obj", OBJ);
    assets.mount("test.mtl", RED);
    let mut piece = Assets::piece();
    let views = TopViews::new(assets.0.join("png"), Some(assets.0.clone()), true);
    let cache = ModelCache::default();
    let original = cache.piece_model(&piece, Some(&assets.0)).unwrap();
    assets.mount(
        "test.obj",
        &String::from_utf8_lossy(OBJ)
            .replace("v 1 0 1", "v 0 0 1")
            .into_bytes(),
    );
    let changed = cache.piece_model(&piece, Some(&assets.0)).unwrap();
    assert_ne!(original.positions, changed.positions);
    for material_override in [false, true] {
        let texture = newera_core::Material {
            image: Some("finish.png".into()),
            ..Default::default()
        };
        if material_override {
            piece.texture = None;
            piece.materials.push(newera_core::ModelMaterial {
                name: "body".into(),
                key: None,
                color: None,
                texture: Some(texture),
                shininess: None,
            });
        } else {
            piece.texture = Some(texture);
        }
        for color in [[255, 0, 0, 255], [0, 0, 255, 255]] {
            let pixels = image::RgbaImage::from_pixel(2, 2, image::Rgba(color));
            let mut bytes = std::io::Cursor::new(Vec::new());
            pixels
                .write_to(&mut bytes, image::ImageFormat::Png)
                .unwrap();
            let before = views.key(&piece).unwrap();
            assets.mount("finish.png", &bytes.into_inner());
            let after = views.key(&piece).unwrap();
            assert_ne!(before, after);
            assert!(
                views.produce(&piece, before).is_none(),
                "an outdated queued version cannot publish a PNG"
            );
            assert!(views.image_for(&piece).is_some());
        }
        // Separate the second scenario from the first scenario's images.
        std::fs::remove_dir_all(&views.dir).unwrap();
        views.inner.lock().unwrap().done.clear();
    }
}

#[test]
fn background_queue_discards_outdated_work_and_becomes_idle() {
    let assets = Assets::new();
    assets.mount("test.obj", OBJ);
    assets.mount("test.mtl", RED);
    let piece = Assets::piece();
    let views = TopViews::in_background(assets.0.join("png"), Some(assets.0.clone()), true);
    let stale = views.key(&piece).unwrap();
    {
        // Hold the cache so the worker cannot start loading until the edit is done.
        let mut cache = views.inner.lock().unwrap();
        cache.queued.insert(stale);
        views
            .background
            .as_ref()
            .unwrap()
            .send((stale, piece.clone()))
            .unwrap();
        assets.mount("test.mtl", BLUE);
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while views.generation() == 0 {
        assert!(
            std::time::Instant::now() < deadline,
            "worker never completed"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(!views.busy());
    assert!(!views.inner.lock().unwrap().done.contains_key(&stale));
    assert!(!views.dir.join(format!("{stale:016x}.png")).exists());
    let path = loop {
        if let Some(path) = views.image_for(&piece) {
            break path;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "updated image never completed"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    assert!(!views.busy());
    assert!(
        image::open(path)
            .unwrap()
            .to_rgba8()
            .pixels()
            .any(|p| p[3] > 0 && p[2] > p[0])
    );
}

#[test]
fn cache_publication_hides_partial_writes_and_cleans_failed_encodes() {
    use std::io::Write;
    let assets = Assets::new();
    let path = assets.0.join("atomic.png");
    let expected = image::RgbaImage::from_pixel(8, 8, image::Rgba([12, 34, 56, 255]));
    let mut encoded = std::io::Cursor::new(Vec::new());
    expected
        .write_to(&mut encoded, image::ImageFormat::Png)
        .unwrap();
    let encoded = encoded.into_inner();
    super::publish_cache_file(&path, |file| {
        file.write_all(&encoded[..12])?;
        assert!(
            !path.exists(),
            "another provider must not discover a partial PNG"
        );
        file.write_all(&encoded[12..])
    })
    .unwrap();
    assert_eq!(image::open(&path).unwrap().to_rgba8(), expected);
    let error = super::publish_cache_file(&path, |file| {
        file.write_all(b"partial replacement")?;
        assert_eq!(image::open(&path).unwrap().to_rgba8(), expected);
        Err(std::io::Error::other("injected encoding failure"))
    });
    assert!(error.is_err());
    assert_eq!(image::open(&path).unwrap().to_rgba8(), expected);
    assert!(!std::fs::read_dir(&assets.0).unwrap().any(|entry| {
        entry
            .unwrap()
            .path()
            .extension()
            .is_some_and(|ext| ext == "tmp")
    }));
}
