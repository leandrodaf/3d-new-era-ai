// Copy to crates/newera-render/examples/metal_demo.rs before building.
use std::{path::PathBuf, time::Instant};

use newera_core::{Document, Home};
use newera_render::{PhotoQuality, View, at_local_hour, photo_home, video};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("expected output directory")?,
    );
    std::fs::create_dir_all(&output)?;
    let mut document = Document::new(Home::new("Metal review"));
    newera_sh3d::open_file(&mut document, &PathBuf::from("web/demo.newera"))?;
    let home = document.home();
    let assets = document.asset_dir();
    let orbit = video::orbit_path(home, 600.0, 8);
    let start = home.cameras.stored.first().unwrap_or(&orbit[0]).clone();
    let mut end = start.clone();
    end.x -= 80.0;
    end.yaw -= 8.0;
    let size = (640, 360);
    let view = View::from_camera(&start, 16.0 / 9.0);
    let time = at_local_hour(
        1_789_214_400_000,
        10.0,
        home.compass.longitude.unwrap_or(-46.63),
    );

    let since = Instant::now();
    photo_home(
        home,
        &view,
        time,
        size.0,
        size.1,
        assets.as_deref(),
        PhotoQuality::Draft,
    )
    .save(output.join("photo.png"))?;
    println!("photo_seconds={:.4}", since.elapsed().as_secs_f64());
    let since = Instant::now();
    let result = video::render_video(
        home,
        &[start, end],
        12,
        0.8,
        size,
        assets.as_deref(),
        &output.join("video.avi"),
        |_, _| {},
    )?;
    println!(
        "video_seconds={:.4} frames={} bytes={}",
        since.elapsed().as_secs_f64(),
        result.frames,
        result.bytes
    );
    Ok(())
}
