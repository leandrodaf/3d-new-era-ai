//! A PNG of a project, without the app: the short loop for anything you have
//! to *look* at — floors, walls, joins, materials, cameras.
//!
//! Building the editor after a change in the core takes about a minute and a
//! half; this stops at `newera-render`, which is a quarter of it, and writes
//! the picture straight to a file.
//!
//! ```text
//! cargo run -p newera-render --example shot -- web/demo.newera /tmp/shot.png cam=2
//! cargo run -p newera-render --example shot -- web/demo.newera /tmp/shot.png aerial yaw=45 pitch=35
//! cargo run -p newera-render --example shot -- web/demo.newera /tmp/shot.png top w=1200 h=900
//! ```
//!
//! `view` is `cam=<i>` (a stored point of view), `aerial` (default) or one of
//! `front|back|left|right|top`; `walls=down` or `walls=cutaway` brings the
//! walls down to show the rooms.

use std::path::{Path, PathBuf};

use newera_core::{Document, Point2};
use newera_render::{Cutaway, Side, View};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let positional: Vec<&String> = args.iter().filter(|a| !a.contains('=')).collect();
    let value = |key: &str| {
        args.iter()
            .find_map(|a| a.strip_prefix(&format!("{key}=")))
            .map(str::to_owned)
    };
    let number = |key: &str, fallback: f32| {
        value(key).map_or(fallback, |v| {
            v.parse().unwrap_or_else(|_| panic!("{key} wants a number"))
        })
    };

    let file = PathBuf::from(positional.first().map_or("web/demo.newera", |s| s.as_str()));
    let out = PathBuf::from(positional.get(1).map_or("shot.png", |s| s.as_str()));
    let size = |key: &str, fallback: u32| {
        value(key).map_or(fallback, |v| {
            v.parse().unwrap_or_else(|_| panic!("{key} wants a number"))
        })
    };
    let (w, h) = (size("w", 900), size("h", 600));

    let mut document = Document::new(newera_core::Home::new("shot"));
    let opened = newera_sh3d::open_file(&mut document, &file)
        .unwrap_or_else(|e| panic!("cannot open {}: {e}", file.display()));
    for warning in opened.warnings {
        eprintln!("warning: {warning}");
    }
    let home = document.home().clone();

    #[allow(clippy::cast_precision_loss)]
    let aspect = w as f32 / h as f32;
    let view = match positional.get(2).map(|s| s.as_str()) {
        Some(side @ ("front" | "back" | "left" | "right" | "top")) => {
            let side = match side {
                "front" => Side::Front,
                "back" => Side::Back,
                "left" => Side::Left,
                "right" => Side::Right,
                _ => Side::Top,
            };
            View::orthographic(
                &home,
                side,
                aspect,
                value("cut").and_then(|c| c.parse().ok()),
            )
        }
        _ => match value("cam") {
            Some(index) => {
                let index: usize = index.parse().expect("cam wants a number");
                let camera = home
                    .cameras
                    .stored
                    .get(index)
                    .unwrap_or_else(|| panic!("no stored camera {index}"));
                eprintln!("camera {index}: {}", camera.name.as_deref().unwrap_or("—"));
                View::from_camera(camera, aspect)
            }
            None => View::aerial_zoom(
                &home,
                number("yaw", 60.0),
                number("pitch", 40.0),
                number("zoom", 1.0),
            ),
        },
    };

    let cutaway = match value("walls").as_deref() {
        Some("down") => Some(Cutaway::all(&home)),
        Some("cutaway") => {
            let toward = view.target - view.eye;
            Some(Cutaway::facing(
                &home,
                Point2::new(f64::from(toward.x), f64::from(toward.z)),
            ))
        }
        _ => None,
    };

    let assets = document.asset_dir();
    let image = newera_render::render_home_cut(
        &home,
        &view,
        cutaway.as_ref(),
        w,
        h,
        assets.as_deref().map(Path::new),
    );
    image.save(&out).expect("the picture is written");
    println!("{}", out.display());
}
