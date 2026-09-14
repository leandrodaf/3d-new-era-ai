//! The home as a 3D scene: triangle meshes shared by the GPU view, and a
//! software renderer so servers and agents can see the home in 3D without
//! a graphics card.

mod camera;
pub mod mesh;
mod patterns;
mod raster;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub use camera::View;
pub use mesh::{IMAGE_BASE, Mesh, ModelSource, Selection, Vertex};
pub use raster::{RenderOptions, render};

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
