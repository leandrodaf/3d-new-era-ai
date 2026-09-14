//! View matrices shared by the GPU view and the software renderer. World
//! space is meters with Y up; plan `(x, y)` cm maps to world `(x, height, y)`.

use glam::{Mat4, Vec3};
use newera_core::{Camera, Home};

/// A ready-to-render point of view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub eye: Vec3,
    pub target: Vec3,
    /// Vertical field of view, radians.
    pub fov_y: f32,
}

impl View {
    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        let view = glam::camera::rh::view::look_at_mat4(self.eye, self.target, Vec3::Y);
        let proj = glam::camera::rh::proj::directx::perspective(
            self.fov_y.clamp(0.05, 3.0),
            aspect,
            0.02,
            500.0,
        );
        proj * view
    }

    /// Looking through a visitor camera (horizontal field of view in degrees).
    #[allow(clippy::cast_possible_truncation)]
    pub fn from_camera(camera: &Camera, aspect: f32) -> Self {
        let eye = Vec3::new(camera.x as f32, camera.z as f32, camera.y as f32) * 0.01;
        let (dx, dy) = camera.direction();
        let pitch = camera.pitch.to_radians() as f32;
        let direction = Vec3::new(
            dx as f32 * pitch.cos(),
            -pitch.sin(),
            dy as f32 * pitch.cos(),
        );
        let horizontal = (camera.fov.to_radians() as f32).clamp(0.1, 3.0);
        let fov_y = 2.0 * ((horizontal / 2.0).tan() / aspect.max(0.01)).atan();
        Self {
            eye,
            target: eye + direction,
            fov_y,
        }
    }

    /// An aerial three-quarter view framing the home, turned by `yaw` and
    /// raised by `pitch` (degrees).
    #[allow(clippy::cast_possible_truncation)]
    pub fn aerial(home: &Home, yaw: f32, pitch: f32) -> Self {
        let (center, size) =
            home.building_bounds()
                .map_or((Vec3::new(0.0, 1.0, 0.0), 10.0), |(min, max)| {
                    (
                        Vec3::new(
                            ((min.x + max.x) / 200.0) as f32,
                            1.0,
                            ((min.y + max.y) / 200.0) as f32,
                        ),
                        (min.distance(max) / 100.0) as f32,
                    )
                });
        let (yaw, pitch) = (yaw.to_radians(), pitch.to_radians().clamp(0.05, 1.5));
        let distance = (size * 1.15).clamp(4.0, 150.0);
        let direction = Vec3::new(
            yaw.cos() * pitch.cos(),
            pitch.sin(),
            yaw.sin() * pitch.cos(),
        );
        Self {
            eye: center + direction * distance,
            target: center,
            fov_y: 45f32.to_radians(),
        }
    }
}
