//! Native 3D viewport: renders the home into an offscreen wgpu texture that
//! egui then shows as an image. Owning the render pass keeps depth testing
//! and MSAA independent from egui's own pass.

use bytemuck::{Pod, Zeroable};
use eframe::egui;
use eframe::egui_wgpu::RenderState;
use eframe::wgpu::{self, util::DeviceExt};
use glam::{Mat4, Vec3};
use newera_core::{Home, WallId};

use super::mesh::{Mesh, Vertex};

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
        self.distance = (self.distance * (-scroll * 0.002).exp()).clamp(1.0, 200.0);
    }

    /// Moves the target on the ground plane, relative to the view direction.
    fn pan(&mut self, delta: egui::Vec2) {
        let forward = Vec3::new(self.yaw.cos(), 0.0, self.yaw.sin());
        let right = forward.cross(Vec3::Y);
        let speed = self.distance * 0.0015;
        self.target += (right * -delta.x + forward * -delta.y) * speed;
    }

    fn look_at_home(&mut self, home: &Home) {
        if let Some((min, max)) = home.bounds() {
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

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    light_dir: [f32; 3],
    _pad: f32,
}

struct GpuMesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
}

struct Targets {
    size: [u32; 2],
    msaa: wgpu::TextureView,
    depth: wgpu::TextureView,
    resolve: wgpu::TextureView,
}

/// GPU resources for the viewport. Created lazily on the first frame.
struct Gpu {
    pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    mesh: Option<GpuMesh>,
    targets: Option<Targets>,
    texture_id: Option<egui::TextureId>,
}

pub(crate) struct SceneView {
    camera: OrbitCamera,
    gpu: Option<Gpu>,
    /// `(document revision, selected wall)` the GPU mesh was built from.
    built_for: Option<(u64, Option<WallId>)>,
    framed_once: bool,
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
            gpu: None,
            built_for: None,
            framed_once: false,
        }
    }

    pub(crate) fn frame_home(&mut self, home: &Home) {
        self.camera.look_at_home(home);
    }

    pub(crate) fn ui(
        &mut self,
        ui: &mut egui::Ui,
        render_state: Option<&RenderState>,
        home: &Home,
        revision: u64,
        selected: Option<WallId>,
    ) {
        let Some(rs) = render_state else {
            ui.centered_and_justified(|ui| ui.label("Visualização 3D requer o backend wgpu."));
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

        if !self.framed_once && !home.walls.is_empty() {
            self.camera.look_at_home(home);
            self.framed_once = true;
        }

        let gpu = self.gpu.get_or_insert_with(|| Gpu::new(&rs.device));
        if self.built_for != Some((revision, selected)) {
            gpu.upload_mesh(&rs.device, &Mesh::from_home(home, selected));
            self.built_for = Some((revision, selected));
        }
        gpu.render(rs, size, &self.camera);

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
            "Arraste: girar · Shift/botão do meio: mover · Scroll: zoom · F: enquadrar",
            egui::FontId::proportional(11.0),
            egui::Color32::from_black_alpha(160),
        );
    }

    fn handle_input(&mut self, ui: &egui::Ui, response: &egui::Response, home: &Home) {
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
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.camera.zoom(scroll);
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
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene bind group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            ..Default::default()
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("scene pipeline"),
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
                        2 => Float32x3,
                    ],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: COLOR_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
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
        });

        Self {
            pipeline,
            uniforms,
            bind_group,
            mesh: None,
            targets: None,
            texture_id: None,
        }
    }

    fn upload_mesh(&mut self, device: &wgpu::Device, mesh: &Mesh) {
        self.mesh = Some(GpuMesh {
            vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("scene vertices"),
                contents: bytemuck::cast_slice(&mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("scene indices"),
                contents: bytemuck::cast_slice(&mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
            index_count: u32::try_from(mesh.indices.len()).expect("index count fits in u32"),
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

    fn render(&mut self, rs: &RenderState, size: [u32; 2], camera: &OrbitCamera) {
        self.ensure_targets(rs, size);
        let (Some(targets), Some(mesh)) = (&self.targets, &self.mesh) else {
            return;
        };

        #[allow(clippy::cast_precision_loss)]
        let aspect = size[0] as f32 / size[1] as f32;
        let uniforms = Uniforms {
            view_proj: camera.view_proj(aspect).to_cols_array_2d(),
            light_dir: Vec3::new(-0.4, -1.0, -0.3).normalize().to_array(),
            _pad: 0.0,
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
        }
        rs.queue.submit([encoder.finish()]);
    }
}
