use glam::{Mat4, Vec3};
use image::RgbaImage;

use crate::{Mesh, raster::metal_textures};

const SHADER: &str = concat!(
    include_str!("../photo/photo.metal"),
    include_str!("video.metal")
);

pub(super) struct Renderer<'a> {
    mesh: &'a Mesh,
    video: newera_metal::Video,
    size: (u32, u32),
    clear: Vec<(f32, [u32; 3])>,
    sorted: Vec<u32>,
}

impl<'a> Renderer<'a> {
    pub(super) fn new(
        mesh: &'a Mesh,
        size: (u32, u32),
        load: &dyn Fn(&str) -> Option<RgbaImage>,
    ) -> Option<Self> {
        let textures = metal_textures(mesh, load)?;
        let video = newera_metal::Video::new(
            SHADER,
            bytemuck::cast_slice(&mesh.vertices),
            &mesh.indices,
            mesh.transparent.len(),
            &textures,
            size,
        )?;
        Some(Self {
            mesh,
            video,
            size,
            clear: Vec::new(),
            sorted: Vec::new(),
        })
    }

    pub(super) fn frame(&mut self, view: Mat4, sky: [u8; 3]) -> Option<RgbaImage> {
        // Match the CPU renderer's back-to-front order for blended surfaces.
        self.clear.clear();
        for ids in self.mesh.transparent.as_chunks::<3>().0 {
            let depth = ids.iter().try_fold(f32::MIN, |depth, &i| {
                let v = self.mesh.vertices.get(i as usize)?;
                Some(depth.max((view * Vec3::from(v.position).extend(1.0)).w))
            })?;
            self.clear.push((depth, *ids));
        }
        self.clear.sort_by(|a, b| b.0.total_cmp(&a.0));
        self.sorted.clear();
        self.sorted
            .extend(self.clear.iter().flat_map(|(_, triangle)| triangle));
        let mut image = RgbaImage::new(self.size.0, self.size.1);
        let sky = sky.map(|v| f32::from(v) / 255.0);
        self.video
            .frame(&view.to_cols_array(), &sky, &self.sorted, image.as_mut())
            .then_some(image)
    }
}
