//! Software renders of the project given by `NEWERA_SH3D_SAMPLE`:
//! `target/screenshots/soft-*.png`.

use newera_core::Document;
use newera_render::{View, render_home};

#[test]
#[ignore = "needs NEWERA_SH3D_SAMPLE"]
fn render_stored_cameras_in_software() {
    let path = std::env::var("NEWERA_SH3D_SAMPLE").expect("NEWERA_SH3D_SAMPLE");
    let mut doc = Document::default();
    newera_sh3d::open_file(&mut doc, std::path::Path::new(&path)).unwrap();
    let assets = doc.asset_dir();
    let home = doc.home();
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/screenshots");
    std::fs::create_dir_all(&out).unwrap();
    for (i, camera) in home.cameras.stored.iter().take(2).enumerate() {
        let started = std::time::Instant::now();
        let image = render_home(
            home,
            &View::from_camera(camera, 640.0 / 480.0),
            640,
            480,
            assets.as_deref(),
        );
        println!("camera {i}: {:?}", started.elapsed());
        image.save(out.join(format!("soft-camera{i}.png"))).unwrap();
    }
    let image = render_home(
        home,
        &View::aerial(home, -60.0, 50.0),
        800,
        600,
        assets.as_deref(),
    );
    image.save(out.join("soft-aerial.png")).unwrap();
}
