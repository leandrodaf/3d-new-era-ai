//! Contact sheets of every catalog item seen from its front (+y, top row of
//! each cell) and from its back (bottom row), for auditing which way each
//! model faces. `cargo run -p newera-render --example catalog_faces -- <dir>`.

#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]

use std::fmt::Write as _;

use glam::Vec3;
use newera_core::{FurnitureId, Home, Point2};
use newera_render::{View, render_home};

const CELL: u32 = 220;
const PER_SHEET: usize = 8;

fn view(piece: &newera_core::Furniture, yaw: f32) -> View {
    let (w, d, h) = (
        piece.width as f32 / 100.0,
        piece.depth as f32 / 100.0,
        piece.height as f32 / 100.0,
    );
    let center = Vec3::new(0.0, piece.elevation as f32 / 100.0 + h / 2.0, 0.0);
    let radius = (w * w + d * d + h * h).sqrt() / 2.0;
    let fov_y = 40f32.to_radians();
    let distance = radius / (fov_y / 2.0).sin() * 1.1;
    let (yaw, pitch) = (yaw.to_radians(), 22f32.to_radians());
    let dir = Vec3::new(
        yaw.cos() * pitch.cos(),
        pitch.sin(),
        yaw.sin() * pitch.cos(),
    );
    View {
        eye: center + dir * distance,
        target: center,
        fov_y,
        ortho: None,
        near: None,
    }
}

fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).expect("output dir"));
    std::fs::create_dir_all(&out).unwrap();
    let items = newera_catalog::CATALOG;
    let mut index = String::new();
    for (sheet, chunk) in items.chunks(PER_SHEET).enumerate() {
        let mut image =
            image::RgbaImage::from_pixel(CELL * PER_SHEET as u32, CELL * 2, image::Rgba([255; 4]));
        for (i, item) in chunk.iter().enumerate() {
            let piece = item.instantiate(FurnitureId(1), Point2::new(0.0, 0.0));
            let mut home = Home::default();
            home.furniture.push(piece.clone());
            // 70°: seen from +y (the front), a little to the right; 250°: from behind.
            for (row, yaw) in [(0u32, 70.0f32), (1, 250.0)] {
                let cell = render_home(&home, &view(&piece, yaw), CELL, CELL, None);
                image::imageops::overlay(
                    &mut image,
                    &cell,
                    i64::from(CELL * i as u32),
                    i64::from(CELL * row),
                );
            }
            let _ = writeln!(
                index,
                "sheet {sheet:02} col {i}: {} ({:?})",
                item.id, item.category
            );
        }
        image
            .save(out.join(format!("sheet-{sheet:02}.png")))
            .unwrap();
    }
    std::fs::write(out.join("index.txt"), index).unwrap();
}
