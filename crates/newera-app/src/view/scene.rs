//! Native 3D viewport: renders the home into an offscreen wgpu texture that
//! egui then shows as an image. Owning the render pass keeps depth testing
//! and MSAA independent from egui's own pass.

use bytemuck::{Pod, Zeroable};
use eframe::egui;
use eframe::egui_wgpu::RenderState;
use eframe::wgpu;
use glam::{Mat4, Vec3};
use newera_core::{ElementId, Home};

use super::plan::Selection;

use newera_render::{Mesh, Vertex};

const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SAMPLES: u32 = 4;
const SKY: wgpu::Color = wgpu::Color {
    r: 0.62,
    g: 0.76,
    b: 0.90,
    a: 1.0,
};

/// Orbit camera around a target point, in meters.
#[derive(Debug, Clone, Copy)]
pub(crate) struct OrbitCamera {
    target: Vec3,
    yaw: f32,
    pitch: f32,
    distance: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            yaw: -0.8,
            pitch: 0.6,
            distance: 18.0,
        }
    }
}

impl OrbitCamera {
    fn eye(&self) -> Vec3 {
        let dir = Vec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        );
        self.target + dir * self.distance
    }

    fn view_proj(&self, aspect: f32) -> Mat4 {
        let view = glam::camera::rh::view::look_at_mat4(self.eye(), self.target, Vec3::Y);
        // wgpu uses a 0..1 depth range, same as DirectX.
        let proj =
            glam::camera::rh::proj::directx::perspective(45f32.to_radians(), aspect, 0.05, 500.0);
        proj * view
    }

    fn orbit(&mut self, delta: egui::Vec2) {
        self.yaw += delta.x * 0.008;
        self.pitch = (self.pitch + delta.y * 0.008).clamp(0.05, 1.5);
    }

    fn zoom(&mut self, scroll: f32) {
        self.closer((-scroll * 0.002).exp());
    }

    /// Straight to a factor, the way a pinch reports it: above 1.0 the
    /// fingers spread and the house comes closer.
    fn closer(&mut self, factor: f32) {
        self.distance = (self.distance / factor).clamp(1.0, 200.0);
    }

    /// Turns around the house without changing the height of the eye.
    fn turn(&mut self, radians: f32) {
        self.yaw += radians;
    }

    /// Moves the target on the ground plane, relative to the view direction.
    fn pan(&mut self, delta: egui::Vec2) {
        let forward = Vec3::new(self.yaw.cos(), 0.0, self.yaw.sin());
        let right = forward.cross(Vec3::Y);
        let speed = self.distance * 0.0015;
        self.target += (right * -delta.x + forward * -delta.y) * speed;
    }

    fn look_at_home(&mut self, home: &Home) {
        if let Some((min, max)) = home.building_bounds() {
            #[allow(clippy::cast_possible_truncation)]
            let center = Vec3::new(
                ((min.x + max.x) / 200.0) as f32,
                1.0,
                ((min.y + max.y) / 200.0) as f32,
            );
            #[allow(clippy::cast_possible_truncation)]
            let size = (min.distance(max) / 100.0) as f32;
            self.target = center;
            self.distance = (size * 1.4).clamp(4.0, 150.0);
        }
    }
}

/// Eye-level camera walking through the home (a stored point of view).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Visitor {
    pub(crate) camera: newera_core::Camera,
}

impl Visitor {
    #[allow(clippy::cast_possible_truncation)]
    fn eye(&self) -> Vec3 {
        let c = &self.camera;
        Vec3::new(c.x as f32, c.z as f32, c.y as f32) * 0.01
    }

    #[allow(clippy::cast_possible_truncation)]
    fn direction(&self) -> Vec3 {
        let (dx, dy) = self.camera.direction();
        let pitch = self.camera.pitch.to_radians() as f32;
        Vec3::new(
            dx as f32 * pitch.cos(),
            -pitch.sin(),
            dy as f32 * pitch.cos(),
        )
    }

    #[allow(clippy::cast_possible_truncation)]
    fn view_proj(&self, aspect: f32) -> Mat4 {
        let eye = self.eye();
        let view = glam::camera::rh::view::look_at_mat4(eye, eye + self.direction(), Vec3::Y);
        // Stored fields of view are horizontal.
        let horizontal = (self.camera.fov.to_radians() as f32).clamp(0.1, 3.0);
        let vertical = 2.0 * ((horizontal / 2.0).tan() / aspect).atan();
        let proj = glam::camera::rh::proj::directx::perspective(vertical, aspect, 0.02, 500.0);
        proj * view
    }

    fn look(&mut self, delta: egui::Vec2) {
        self.camera.yaw += f64::from(delta.x) * 0.3;
        self.camera.pitch = (self.camera.pitch + f64::from(delta.y) * 0.3).clamp(-85.0, 85.0);
    }

    fn walk(&mut self, forward: f64, side: f64) {
        let (dx, dy) = self.camera.direction();
        self.camera.x += dx * forward - dy * side;
        self.camera.y += dy * forward + dx * side;
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    light_dir: [f32; 3],
    /// Sunlight strength 0–1 when the sun follows the compass; negative for
    /// the default soft studio light.
    sun: f32,
}

struct GpuMesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
    /// Transparent triangles follow the opaque ones in `indices`.
    transparent_count: u32,
}

struct Targets {
    size: [u32; 2],
    msaa: wgpu::TextureView,
    depth: wgpu::TextureView,
    resolve: wgpu::TextureView,
}

/// Side of every image texture layer, in pixels.
const IMAGE_SIZE: u32 = 512;
const IMAGE_MIPS: u32 = 10;

/// GPU resources for the viewport. Created lazily on the first frame.
struct Gpu {
    pipeline: wgpu::RenderPipeline,
    transparent_pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,
    /// Image files currently uploaded, in layer order.
    images: Vec<String>,
    mesh: Option<GpuMesh>,
    targets: Option<Targets>,
    texture_id: Option<egui::TextureId>,
}

/// Direction towards the sun and its strength (0 at night) at `hour` local
/// solar time, on the day of the home's camera, from the compass location.
pub(crate) fn sun_light(home: &Home, hour: f64) -> (Vec3, f32) {
    let base = match home.cameras.top.time {
        0 => 1_789_214_400_000,
        time => time,
    };
    let time = newera_render::at_local_hour(base, hour, home.compass.longitude.unwrap_or(-46.63));
    match newera_render::sun_direction(&home.compass, time) {
        #[allow(clippy::cast_possible_truncation)]
        Some((dir, elevation)) => (dir, (elevation / 25.0).clamp(0.0, 1.0) as f32),
        None => (Vec3::Y, 0.0),
    }
}

pub(crate) struct SceneView {
    camera: OrbitCamera,
    /// When set, the view looks through this visitor instead of orbiting.
    pub(crate) visitor: Option<Visitor>,
    /// Local solar hour of the live sun; `None` keeps the soft studio light.
    pub(crate) sun_hour: Option<f64>,
    gpu: Option<Gpu>,
    /// `(document revision, selection)` the GPU mesh was built from.
    built_for: Option<(u64, Vec<ElementId>)>,
    framed_once: bool,
    /// Imported models by resolved path; `None` when a file failed to load.
    models: std::cell::RefCell<
        std::collections::HashMap<std::path::PathBuf, Option<newera_catalog::Mesh>>,
    >,
}

impl std::fmt::Debug for SceneView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SceneView")
            .field("camera", &self.camera)
            .field("built_for", &self.built_for)
            .finish_non_exhaustive()
    }
}

impl SceneView {
    pub(crate) fn new() -> Self {
        Self {
            camera: OrbitCamera::default(),
            visitor: None,
            gpu: None,
            sun_hour: None,
            built_for: None,
            framed_once: false,
            models: std::cell::RefCell::default(),
        }
    }

    /// Frames the whole home on the next frame.
    /// The current point of view, for photos and exports.
    pub(crate) fn current_view(&self) -> newera_render::View {
        match &self.visitor {
            Some(visitor) => newera_render::View::from_camera(&visitor.camera, 4.0 / 3.0),
            None => newera_render::View {
                eye: self.camera.eye(),
                target: self.camera.target,
                fov_y: 45f32.to_radians(),
                ortho: None,
                near: None,
            },
        }
    }

    /// `(direction the light travels, sun strength)` for the shader.
    fn light(&self, home: &Home) -> (Vec3, f32) {
        let Some(hour) = self.sun_hour else {
            return (Vec3::new(-0.4, -1.0, -0.3).normalize(), -1.0);
        };
        let (dir, strength) = sun_light(home, hour);
        (-dir, strength)
    }

    pub(crate) fn request_frame(&mut self) {
        self.framed_once = false;
    }

    pub(crate) fn ui(
        &mut self,
        ui: &mut egui::Ui,
        render_state: Option<&RenderState>,
        home: &Home,
        revision: u64,
        selection: &Selection,
        project: Option<&std::path::Path>,
    ) {
        let Some(rs) = render_state else {
            ui.centered_and_justified(|ui| {
                ui.label(crate::i18n::tr("Visualização 3D requer o backend wgpu."))
            });
            return;
        };

        let rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        self.handle_input(ui, &response, home);

        let ppp = ui.ctx().pixels_per_point();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let size = [
            ((rect.width() * ppp).round() as u32).max(1),
            ((rect.height() * ppp).round() as u32).max(1),
        ];

        if !self.framed_once && home.bounds().is_some() {
            self.camera.look_at_home(home);
            self.framed_once = true;
        }

        let light = self.light(home);
        let gpu = self.gpu.get_or_insert_with(|| Gpu::new(&rs.device));
        let key = (revision, selection.iter().copied().collect::<Vec<_>>());
        if self.built_for.as_ref() != Some(&key) {
            let models = |piece: &newera_core::Furniture| {
                let path = newera_core::resolve_asset(project, piece.model.as_deref()?);
                let mut cache = self.models.borrow_mut();
                let mut mesh = cache
                    .entry(path.clone())
                    .or_insert_with(|| match newera_catalog::load_model(&path) {
                        Ok(model) => Some(model.mesh),
                        Err(err) => {
                            tracing::warn!("cannot load model {}: {err}", path.display());
                            None
                        }
                    })
                    .clone()?;
                mesh.rotate(piece.model_transform.rotation);
                mesh.fit_to(piece.width, piece.depth, piece.height);
                Some(mesh)
            };
            let mesh = Mesh::from_home(home, selection, &models);
            gpu.upload_images(rs, &mesh.images, project);
            gpu.upload_mesh(rs, &mesh);
            self.built_for = Some(key);
        }
        #[allow(clippy::cast_precision_loss)]
        let aspect = size[0] as f32 / size[1] as f32;
        let view_proj = match &self.visitor {
            Some(visitor) => visitor.view_proj(aspect),
            None => self.camera.view_proj(aspect),
        };
        gpu.render(rs, size, view_proj, light);

        if let Some(texture_id) = gpu.texture_id {
            ui.painter().image(
                texture_id,
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
        ui.painter().text(
            rect.left_bottom() + egui::vec2(8.0, -8.0),
            egui::Align2::LEFT_BOTTOM,
            if self.visitor.is_some() {
                crate::i18n::tr("Visitante — arraste: olhar · W/A/S/D ou setas: andar · Scroll: avançar · Esc: visão aérea")
            } else {
                crate::i18n::tr("Arraste ou dois dedos: girar · Shift: mover · Pinça ou scroll: zoom · F: enquadrar")
            },
            egui::FontId::proportional(11.0),
            egui::Color32::from_black_alpha(160),
        );
    }

    fn handle_input(&mut self, ui: &egui::Ui, response: &egui::Response, home: &Home) {
        if let Some(visitor) = &mut self.visitor {
            if response.dragged_by(egui::PointerButton::Primary)
                || response.dragged_by(egui::PointerButton::Secondary)
            {
                visitor.look(response.drag_delta());
            }
            if response.hovered() {
                // Walking with the hands: two fingers up and down walk, left
                // and right step aside, and the keys do what they always did.
                let hands = crate::view::gesture::Gesture::read(ui);
                let (forward, side) = ui.input(|i| {
                    let key = |k| if i.key_down(k) { 1.0 } else { 0.0 };
                    (
                        f64::from(hands.glide.y + hands.wheel) * 0.5
                            + (key(egui::Key::W) + key(egui::Key::ArrowUp)
                                - key(egui::Key::S)
                                - key(egui::Key::ArrowDown))
                                * 4.0,
                        f64::from(-hands.glide.x) * 0.5
                            + (key(egui::Key::D) + key(egui::Key::ArrowRight)
                                - key(egui::Key::A)
                                - key(egui::Key::ArrowLeft))
                                * 4.0,
                    )
                });
                if forward != 0.0 || side != 0.0 {
                    visitor.walk(forward, side);
                    ui.ctx().request_repaint();
                }
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.visitor = None;
                }
            }
            return;
        }
        let panning = response.dragged_by(egui::PointerButton::Middle)
            || (response.dragged_by(egui::PointerButton::Primary)
                && ui.input(|i| i.modifiers.shift));
        if panning {
            self.camera.pan(response.drag_delta());
        } else if response.dragged_by(egui::PointerButton::Primary)
            || response.dragged_by(egui::PointerButton::Secondary)
        {
            self.camera.orbit(response.drag_delta());
        }
        if response.hovered() {
            // Two fingers turn the house around, as they would a model held
            // in the hands; with Shift they slide it; the pinch comes closer
            // and a twist spins it on the spot.
            let hands = crate::view::gesture::Gesture::read(ui);
            if !hands.is_idle() {
                if hands.wheel != 0.0 {
                    self.camera.zoom(hands.wheel);
                }
                if let Some(pinch) = hands.pinch() {
                    self.camera.closer(pinch);
                }
                if hands.twist != 0.0 {
                    self.camera.turn(-hands.twist);
                }
                if hands.glide != egui::Vec2::ZERO {
                    if ui.input(|i| i.modifiers.shift) {
                        self.camera.pan(hands.glide);
                    } else {
                        self.camera.orbit(hands.glide * 0.6);
                    }
                }
            }
            if ui.input(|i| i.key_pressed(egui::Key::F)) {
                self.camera.look_at_home(home);
            }
        }
    }
}

impl Gpu {
    fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("scene image sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 8,
            ..Default::default()
        });
        let bind_group =
            Self::bind_images(device, &bind_group_layout, &uniforms, &sampler, None, &[]);

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            ..Default::default()
        });

        let make_pipeline = |label: &str, transparent: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        step_mode: wgpu::VertexStepMode::Vertex,
                        array_stride: std::mem::size_of::<Vertex>() as u64,
                        attributes: &wgpu::vertex_attr_array![
                            0 => Float32x3,
                            1 => Float32x3,
                            2 => Float32x4,
                            3 => Float32x2,
                            4 => Uint32,
                        ],
                    })],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: COLOR_FORMAT,
                        blend: transparent.then_some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    // Glass panes are often single-sided; show both faces.
                    cull_mode: (!transparent).then_some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(!transparent),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: SAMPLES,
                    ..Default::default()
                },
                multiview_mask: None,
                cache: None,
            })
        };
        let pipeline = make_pipeline("scene pipeline", false);
        let transparent_pipeline = make_pipeline("scene transparent pipeline", true);

        Self {
            pipeline,
            transparent_pipeline,
            uniforms,
            layout: bind_group_layout,
            sampler,
            bind_group,
            images: Vec::new(),
            mesh: None,
            targets: None,
            texture_id: None,
        }
    }

    /// Builds the bind group with a texture array holding `images` (decoded
    /// RGBA, one per layer); an empty list binds a single white layer.
    fn bind_images(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        uniforms: &wgpu::Buffer,
        sampler: &wgpu::Sampler,
        queue: Option<&wgpu::Queue>,
        images: &[image::RgbaImage],
    ) -> wgpu::BindGroup {
        // GL backends (WebGL) guess the view from the layer count: one layer
        // means plain 2D and multiples of six mean cube maps, and sampling the
        // array then reads black. A spare layer avoids both.
        let mut layers = u32::try_from(images.len().max(2)).expect("layer count fits in u32");
        if layers % 6 == 0 {
            layers += 1;
        }
        let (size, mips) = if images.is_empty() {
            (1, 1)
        } else {
            (IMAGE_SIZE, IMAGE_MIPS)
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene images"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: layers,
            },
            mip_level_count: mips,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        if let Some(queue) = queue {
            for (layer, image) in images.iter().enumerate() {
                let mut level_image = image.clone();
                for mip in 0..mips {
                    let side = (size >> mip).max(1);
                    if mip > 0 {
                        level_image = image::imageops::resize(
                            image,
                            side,
                            side,
                            image::imageops::FilterType::Triangle,
                        );
                    }
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &texture,
                            mip_level: mip,
                            origin: wgpu::Origin3d {
                                x: 0,
                                y: 0,
                                z: u32::try_from(layer).expect("layer fits in u32"),
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
                        level_image.as_raw(),
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(4 * side),
                            rows_per_image: Some(side),
                        },
                        wgpu::Extent3d {
                            width: side,
                            height: side,
                            depth_or_array_layers: 1,
                        },
                    );
                }
            }
        }
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene bind group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    /// Uploads material images when the set of files changed. Unreadable
    /// files become a neutral gray layer so the rest still renders.
    fn upload_images(
        &mut self,
        rs: &RenderState,
        images: &[String],
        project: Option<&std::path::Path>,
    ) {
        if self.images == images {
            return;
        }
        let decoded: Vec<image::RgbaImage> = images
            .iter()
            .map(|file| {
                let path = newera_core::resolve_asset(project, file);
                match newera_core::vfs::read(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|b| newera_core::images::decode(&b).map_err(|e| e.to_string()))
                {
                    Ok(img) => image::imageops::resize(
                        &img.to_rgba8(),
                        IMAGE_SIZE,
                        IMAGE_SIZE,
                        image::imageops::FilterType::Triangle,
                    ),
                    Err(err) => {
                        tracing::warn!("cannot load texture {}: {err}", path.display());
                        #[cfg(target_arch = "wasm32")]
                        web_sys::console::warn_1(
                            &format!("cannot load texture {}: {err}", path.display()).into(),
                        );
                        image::RgbaImage::from_pixel(
                            IMAGE_SIZE,
                            IMAGE_SIZE,
                            image::Rgba([180, 180, 180, 255]),
                        )
                    }
                }
            })
            .collect();
        self.bind_group = Self::bind_images(
            &rs.device,
            &self.layout,
            &self.uniforms,
            &self.sampler,
            Some(&rs.queue),
            &decoded,
        );
        self.images = images.to_vec();
    }

    fn upload_mesh(&mut self, rs: &RenderState, mesh: &Mesh) {
        // WebGPU may reject mapped-at-creation buffers even below the device's
        // max_buffer_size. Upload through the queue instead: no JS mapping of
        // the whole scene and no second copy concatenating opaque/glass indices.
        let vertex_bytes: &[u8] = bytemuck::cast_slice(&mesh.vertices);
        let opaque_bytes: &[u8] = bytemuck::cast_slice(&mesh.indices);
        let glass_bytes: &[u8] = bytemuck::cast_slice(&mesh.transparent);
        let buffer = |label, size: u64, usage| {
            rs.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size.max(4),
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let vertices = buffer(
            "scene vertices",
            vertex_bytes.len() as u64,
            wgpu::BufferUsages::VERTEX,
        );
        let indices = buffer(
            "scene indices",
            (opaque_bytes.len() + glass_bytes.len()) as u64,
            wgpu::BufferUsages::INDEX,
        );
        let upload = |buffer: &wgpu::Buffer, offset: u64, bytes: &[u8]| {
            // Bound each browser-side transfer, including after large MCP edits.
            for (i, chunk) in bytes.chunks(64 * 1024).enumerate() {
                rs.queue
                    .write_buffer(buffer, offset + (i * 64 * 1024) as u64, chunk);
            }
        };
        upload(&vertices, 0, vertex_bytes);
        upload(&indices, 0, opaque_bytes);
        upload(&indices, opaque_bytes.len() as u64, glass_bytes);
        self.mesh = Some(GpuMesh {
            vertices,
            indices,
            index_count: u32::try_from(mesh.indices.len()).expect("index count fits in u32"),
            transparent_count: u32::try_from(mesh.transparent.len())
                .expect("index count fits in u32"),
        });
    }

    fn ensure_targets(&mut self, rs: &RenderState, size: [u32; 2]) {
        if self.targets.as_ref().is_some_and(|t| t.size == size) {
            return;
        }
        let device = &rs.device;
        let texture = |label, format, samples, usage| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: size[0],
                        height: size[1],
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&wgpu::TextureViewDescriptor::default())
        };
        let targets = Targets {
            size,
            msaa: texture(
                "scene msaa",
                COLOR_FORMAT,
                SAMPLES,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            ),
            depth: texture(
                "scene depth",
                DEPTH_FORMAT,
                SAMPLES,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            ),
            resolve: texture(
                "scene color",
                COLOR_FORMAT,
                1,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            ),
        };

        let mut renderer = rs.renderer.write();
        match self.texture_id {
            Some(id) => renderer.update_egui_texture_from_wgpu_texture(
                device,
                &targets.resolve,
                wgpu::FilterMode::Linear,
                id,
            ),
            None => {
                self.texture_id = Some(renderer.register_native_texture(
                    device,
                    &targets.resolve,
                    wgpu::FilterMode::Linear,
                ));
            }
        }
        self.targets = Some(targets);
    }

    fn render(&mut self, rs: &RenderState, size: [u32; 2], view_proj: Mat4, light: (Vec3, f32)) {
        self.ensure_targets(rs, size);
        let (Some(targets), Some(mesh)) = (&self.targets, &self.mesh) else {
            return;
        };

        let uniforms = Uniforms {
            view_proj: view_proj.to_cols_array_2d(),
            light_dir: light.0.to_array(),
            sun: light.1,
        };
        rs.queue
            .write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&uniforms));

        let mut encoder = rs
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("scene encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &targets.msaa,
                    depth_slice: None,
                    resolve_target: Some(&targets.resolve),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(SKY),
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &targets.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, mesh.vertices.slice(..));
            pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            if mesh.transparent_count > 0 {
                pass.set_pipeline(&self.transparent_pipeline);
                pass.draw_indexed(
                    mesh.index_count..mesh.index_count + mesh.transparent_count,
                    0,
                    0..1,
                );
            }
        }
        rs.queue.submit([encoder.finish()]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_sun_follows_the_compass_and_the_hour() {
        let mut home = Home::default();
        home.compass.latitude = Some(-23.5);
        home.compass.longitude = Some(-46.6);
        let (noon, strength) = sun_light(&home, 12.0);
        assert!(noon.y > 0.7 && strength > 0.9, "{noon} {strength}");
        let (_, night) = sun_light(&home, 0.0);
        assert!(night.abs() < f32::EPSILON);
        // Morning sun rises in the east; north up on the plan puts east at +x.
        let (morning, _) = sun_light(&home, 8.0);
        assert!(morning.x > 0.5, "{morning}");
        // Turning the compass half a turn moves it to the other side.
        home.compass.north_degrees = 180.0;
        let (turned, _) = sun_light(&home, 8.0);
        assert!(turned.x < -0.5, "{turned}");
    }
}
