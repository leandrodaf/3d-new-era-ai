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
    /// Orthographic instead of perspective: half the visible height, meters.
    pub ortho: Option<f32>,
    /// Near clipping distance, meters: sections cut away what is closer.
    pub near: Option<f32>,
}

/// Sides of the building seen head-on, for elevations and sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// From the plan bottom (+y) looking up the plan.
    Front,
    /// From the plan top (-y).
    Back,
    /// From the plan left (-x).
    Left,
    /// From the plan right (+x).
    Right,
    /// From above, north up.
    Top,
}

impl View {
    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        let forward = (self.target - self.eye).normalize_or_zero();
        // Looking straight down, "up" on screen is the top of the plan.
        let up = if forward.y.abs() > 0.99 {
            -Vec3::Z
        } else {
            Vec3::Y
        };
        let view = glam::camera::rh::view::look_at_mat4(self.eye, self.target, up);
        let near = self.near.unwrap_or(0.02).max(0.001);
        let proj = match self.ortho {
            Some(half) => {
                let half = half.max(0.01);
                glam::camera::rh::proj::directx::orthographic(
                    -half * aspect,
                    half * aspect,
                    -half,
                    half,
                    near,
                    near + 1000.0,
                )
            }
            None => glam::camera::rh::proj::directx::perspective(
                self.fov_y.clamp(0.05, 3.0),
                aspect,
                near,
                500.0,
            ),
        };
        proj * view
    }

    /// A product shot of a piece: perspective, looking at the box `local`
    /// (`(min, max)` cm in the piece's model frame — x across, y up, z to
    /// its front) from `side` of the piece itself — its front, back, left,
    /// right, above, or a three-quarter view for `None` — at `zoom` times
    /// the distance that just fits it. The same call frames the same view
    /// every time, however the piece stands in the plan.
    #[must_use]
    pub fn product(
        home: &Home,
        piece: &newera_core::Furniture,
        local: ([f32; 3], [f32; 3]),
        side: Option<Side>,
        zoom: f32,
    ) -> Self {
        let fov_y = 35f32.to_radians();
        let (min, max) = local;
        let center: [f32; 3] = std::array::from_fn(|k| f32::midpoint(min[k], max[k]));
        let size: [f32; 3] = std::array::from_fn(|k| max[k] - min[k]);
        let plan = piece.to_plan((f64::from(center[0]), f64::from(center[2])));
        let floor = home.elevation_of(piece.level) + piece.elevation;
        #[allow(clippy::cast_possible_truncation)]
        let target = Vec3::new(
            plan.x as f32,
            (floor + f64::from(center[1])) as f32,
            plan.y as f32,
        ) / 100.0;
        // The piece's own axes on the plan: its front and its right.
        let axis = |local: (f64, f64)| {
            let (o, p) = (piece.to_plan((0.0, 0.0)), piece.to_plan(local));
            #[allow(clippy::cast_possible_truncation)]
            Vec3::new((p.x - o.x) as f32, 0.0, (p.y - o.y) as f32).normalize_or_zero()
        };
        let (front, right) = (axis((0.0, 1.0)), axis((1.0, 0.0)));
        let direction = match side {
            Some(Side::Front) => front + Vec3::Y * 0.12,
            Some(Side::Back) => -front + Vec3::Y * 0.12,
            Some(Side::Left) => -right + Vec3::Y * 0.12,
            Some(Side::Right) => right + Vec3::Y * 0.12,
            Some(Side::Top) => Vec3::Y + front * 0.02,
            None => front + right * 0.7 + Vec3::Y * 0.55,
        }
        .normalize_or_zero();
        let radius = (size[0] * size[0] + size[1] * size[1] + size[2] * size[2]).sqrt() / 200.0;
        let distance = (radius.max(0.02) / (fov_y / 2.0).sin() * zoom.clamp(0.2, 10.0)).max(0.05);
        Self {
            eye: target + direction * distance,
            target,
            fov_y,
            ortho: None,
            near: None,
        }
    }

    /// A head-on orthographic view of the whole building from `side`, for an
    /// image of `aspect`. With `cut` (plan cm along the view direction, or
    /// height for `Top`) everything in front of that plane is cut away.
    #[allow(clippy::cast_possible_truncation)]
    pub fn orthographic(home: &Home, side: Side, aspect: f32, cut: Option<f64>) -> Self {
        let top = building_top(home).max(100.0) as f32 / 100.0;
        let (min, max) = home
            .building_bounds()
            .map_or(((-5.0, -5.0), (5.0, 5.0)), |(a, b)| {
                (
                    (a.x as f32 / 100.0, a.y as f32 / 100.0),
                    (b.x as f32 / 100.0, b.y as f32 / 100.0),
                )
            });
        let center = Vec3::new(
            f32::midpoint(min.0, max.0),
            top / 2.0,
            f32::midpoint(min.1, max.1),
        );
        let (wx, wz) = (max.0 - min.0, max.1 - min.1);
        let far = 200.0;
        let (dir, across, tall, depth) = match side {
            Side::Front => (Vec3::Z, wx, top, wz),
            Side::Back => (-Vec3::Z, wx, top, wz),
            Side::Left => (-Vec3::X, wz, top, wx),
            Side::Right => (Vec3::X, wz, top, wx),
            Side::Top => (Vec3::Y, wx, wz, top),
        };
        let _ = depth;
        let half = (tall / 2.0).max(across / 2.0 / aspect.max(0.01)) * 1.08;
        let eye = center + dir * far;
        let near = cut.map(|c| {
            let c = c as f32 / 100.0;
            match side {
                Side::Front => eye.z - c,
                Side::Back => c - eye.z,
                Side::Left => c - eye.x,
                Side::Right => eye.x - c,
                Side::Top => eye.y - c,
            }
            .max(0.001)
        });
        Self {
            eye,
            target: center,
            fov_y: 45f32.to_radians(),
            ortho: Some(half),
            near,
        }
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
            ortho: None,
            near: None,
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
            ortho: None,
            near: None,
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

#[cfg(test)]
mod product_tests {
    use super::*;

    #[test]
    fn a_product_shot_looks_at_the_piece_from_its_own_front() {
        let home = Home::default();
        // A piece at (300, 200) turned so its front looks to −x.
        let piece = newera_core::Furniture {
            position: newera_core::Point2::new(300.0, 200.0),
            angle: 90.0,
            width: 60.0,
            depth: 70.0,
            height: 80.0,
            ..newera_core::Furniture::default()
        };
        let whole = ([-30.0, 0.0, -35.0], [30.0, 80.0, 35.0]);
        let front = View::product(&home, &piece, whole, Some(Side::Front), 1.0);
        assert!(
            (front.target - Vec3::new(3.0, 0.4, 2.0)).length() < 1e-4,
            "{:?}",
            front.target
        );
        assert!(front.eye.x < front.target.x - 0.5, "{front:?}");
        assert!((front.eye.z - front.target.z).abs() < 1e-3, "{front:?}");
        let back = View::product(&home, &piece, whole, Some(Side::Back), 1.0);
        assert!(back.eye.x > back.target.x + 0.5, "{back:?}");
        // A part: framed on its own box, closer.
        let arm = View::product(
            &home,
            &piece,
            ([20.0, 40.0, -35.0], [30.0, 60.0, 35.0]),
            Some(Side::Front),
            1.0,
        );
        assert!((arm.eye - arm.target).length() < (front.eye - front.target).length());
        // Moved, the same shot moves with it.
        let moved = newera_core::Furniture {
            position: newera_core::Point2::new(800.0, 200.0),
            ..piece
        };
        let again = View::product(&home, &moved, whole, Some(Side::Front), 1.0);
        assert!(((again.eye - again.target) - (front.eye - front.target)).length() < 1e-4);
    }
}
