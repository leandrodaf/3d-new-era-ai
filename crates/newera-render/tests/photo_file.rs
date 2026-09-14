//! Photos of the project given by `NEWERA_PHOTO_FILE` (`.newera`) at the
//! hours in `NEWERA_PHOTO_HOURS` (default `15,19`), aerial and from the
//! stored cameras: `target/screenshots/photo-file-*.png`.

use newera_render::{PhotoQuality, View, at_local_hour, photo_home};

#[test]
#[ignore = "needs NEWERA_PHOTO_FILE"]
fn photos_of_a_project_file() {
    let path = std::env::var("NEWERA_PHOTO_FILE").expect("NEWERA_PHOTO_FILE");
    let cache = std::env::temp_dir().join("newera-photo-file");
    let (project, assets) = newera_core::open_project(std::path::Path::new(&path), &cache).unwrap();
    let home = &project.variants[project.active].1;
    let hours: Vec<f64> = std::env::var("NEWERA_PHOTO_HOURS")
        .unwrap_or_else(|_| "15,19".to_owned())
        .split(',')
        .filter_map(|h| h.trim().parse().ok())
        .collect();
    let quality = match std::env::var("NEWERA_PHOTO_QUALITY").as_deref() {
        Ok("good") => PhotoQuality::Good,
        Ok("best") => PhotoQuality::Best,
        _ => PhotoQuality::Draft,
    };
    let (w, h) = (640, 480);
    let mut views = vec![("aerial".to_owned(), View::aerial(home, -60.0, 20.0))];
    for (i, camera) in home.cameras.stored.iter().enumerate() {
        views.push((format!("cam{i}"), View::from_camera(camera, 640.0 / 480.0)));
    }
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/screenshots");
    std::fs::create_dir_all(&out).unwrap();
    let longitude = home.compass.longitude.unwrap_or(-46.63);
    for hour in hours {
        let time = at_local_hour(home.cameras.top.time, hour, longitude);
        for (name, view) in &views {
            let started = std::time::Instant::now();
            let image = photo_home(home, view, time, w, h, assets.as_deref(), quality);
            println!("{name} {hour}h: {:?}", started.elapsed());
            image
                .save(out.join(format!("photo-file-{name}-{hour}h.png")))
                .unwrap();
        }
    }
}
