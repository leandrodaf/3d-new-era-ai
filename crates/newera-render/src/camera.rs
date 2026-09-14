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
    /// raised by `pitch` (degrees). `yaw` 0 looks from the east (+x of the
    /// plan), 90 from the south (bottom of the plan).
    pub fn aerial(home: &Home, yaw: f32, pitch: f32) -> Self {
        Self::aerial_zoom(home, yaw, pitch, 1.0)
    }

    /// [`View::aerial`] with the whole building — its height included —
    /// inside the frame; `zoom` above 1 moves away, below 1 comes closer.
    #[allow(clippy::cast_possible_truncation)]
    pub fn aerial_zoom(home: &Home, yaw: f32, pitch: f32, zoom: f32) -> Self {
        let fov_y = 45f32.to_radians();
        let top = building_top(home).max(100.0) as f32 / 100.0;
        let (center, radius) =
            home.building_bounds()
                .map_or((Vec3::new(0.0, 1.0, 0.0), 5.0), |(min, max)| {
                    let (w, d) = (
                        (max.x - min.x) as f32 / 100.0,
                        (max.y - min.y) as f32 / 100.0,
                    );
                    (
                        Vec3::new(
                            ((min.x + max.x) / 200.0) as f32,
                            top / 2.0,
                            ((min.y + max.y) / 200.0) as f32,
                        ),
                        (w * w + d * d + top * top).sqrt() / 2.0,
                    )
                });
        let (yaw, pitch) = (yaw.to_radians(), pitch.to_radians().clamp(0.05, 1.5));
        // The bounding sphere just fits the vertical field of view.
        let distance = (radius / (fov_y / 2.0).sin() * zoom.clamp(0.2, 10.0)).clamp(2.0, 400.0);
        let direction = Vec3::new(
            yaw.cos() * pitch.cos(),
            pitch.sin(),
            yaw.sin() * pitch.cos(),
        );
        Self {
            eye: center + direction * distance,
            target: center,
            fov_y,
        }
    }
}

/// Highest point of walls and visible furniture above the ground, cm.
fn building_top(home: &Home) -> f64 {
    let walls = home
        .walls
        .iter()
        .map(|w| home.elevation_of(w.level) + w.height.max(w.height_at_end.unwrap_or(0.0)));
    let furniture = home
        .furniture
        .iter()
        .filter(|f| f.visible)
        .map(|f| home.elevation_of(f.level) + f.elevation + f.height);
    walls.chain(furniture).fold(0.0, f64::max)
}

#[cfg(test)]
mod tests {
    use newera_core::{Point2, Wall};

    use super::*;

    #[test]
    fn aerial_view_frames_tall_buildings_whole() {
        let mut home = Home::default();
        // A 6 × 7 m A-frame: gables 675 cm high.
        for (a, b, h) in [
            ((0.0, 0.0), (600.0, 0.0), 675.0),
            ((0.0, 700.0), (600.0, 700.0), 675.0),
        ] {
            let id = home.new_wall_id();
            let mut wall = Wall::new(id, Point2::new(a.0, a.1), Point2::new(b.0, b.1));
            wall.height = h;
            home.walls.push(wall);
        }
        for yaw in [0.0, 60.0, 200.0] {
            let view = View::aerial(&home, yaw, 30.0);
            let m = view.view_proj(4.0 / 3.0);
            for corner in [(0.0, 0.0), (6.0, 0.0), (0.0, 7.0), (6.0, 7.0)] {
                for y in [0.0, 6.75] {
                    let clip = m * glam::Vec4::new(corner.0, y, corner.1, 1.0);
                    let ndc = clip.truncate() / clip.w;
                    assert!(ndc.x.abs() <= 1.0 && ndc.y.abs() <= 1.0, "yaw {yaw}: {ndc}");
                }
            }
            let far = View::aerial_zoom(&home, yaw, 30.0, 2.0);
            assert!(far.eye.distance(far.target) > view.eye.distance(view.target) * 1.9);
        }
    }
}
