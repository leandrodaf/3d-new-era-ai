//! Plan viewport: maps plan centimeters to screen points.

use eframe::egui::{Pos2, Rect, Vec2};
use newera_core::Point2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Camera {
    /// Plan point (cm) shown at the center of the viewport.
    pub(crate) center: Point2,
    /// Screen points per centimeter.
    pub(crate) zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            center: Point2::new(400.0, 300.0),
            zoom: 0.8,
        }
    }
}

impl Camera {
    pub(crate) const MIN_ZOOM: f32 = 0.02;
    pub(crate) const MAX_ZOOM: f32 = 40.0;

    #[allow(clippy::cast_possible_truncation)]
    pub(crate) fn to_screen(self, rect: Rect, p: Point2) -> Pos2 {
        rect.center()
            + Vec2::new(
                ((p.x - self.center.x) as f32) * self.zoom,
                ((p.y - self.center.y) as f32) * self.zoom,
            )
    }

    pub(crate) fn to_world(self, rect: Rect, p: Pos2) -> Point2 {
        let d = (p - rect.center()) / self.zoom;
        Point2::new(
            self.center.x + f64::from(d.x),
            self.center.y + f64::from(d.y),
        )
    }

    /// Plan distance (cm) covered by `px` screen points.
    pub(crate) fn cm(&self, px: f32) -> f64 {
        f64::from(px / self.zoom)
    }

    /// Zooms by `factor`, keeping the plan point under `anchor` still.
    pub(crate) fn zoom_at(&mut self, rect: Rect, anchor: Pos2, factor: f32) {
        let before = self.to_world(rect, anchor);
        self.zoom = (self.zoom * factor).clamp(Self::MIN_ZOOM, Self::MAX_ZOOM);
        let after = self.to_world(rect, anchor);
        self.center.x += before.x - after.x;
        self.center.y += before.y - after.y;
    }

    pub(crate) fn pan(&mut self, delta: Vec2) {
        self.center.x -= f64::from(delta.x / self.zoom);
        self.center.y -= f64::from(delta.y / self.zoom);
    }

    /// Frames `min..max` inside `rect` with some margin.
    #[allow(clippy::cast_possible_truncation)]
    pub(crate) fn fit(&mut self, rect: Rect, min: Point2, max: Point2) {
        let (w, h) = ((max.x - min.x).max(100.0), (max.y - min.y).max(100.0));
        let zoom = (f64::from(rect.width()) / w).min(f64::from(rect.height()) / h) * 0.85;
        self.zoom = (zoom as f32).clamp(Self::MIN_ZOOM, Self::MAX_ZOOM);
        self.center = Point2::new(min.x.midpoint(max.x), min.y.midpoint(max.y));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_and_world_are_inverse_and_zoom_keeps_anchor() {
        let rect = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(400.0, 300.0));
        let mut cam = Camera::default();
        let p = Point2::new(123.0, -45.0);
        let back = cam.to_world(rect, cam.to_screen(rect, p));
        assert!((back.x - p.x).abs() < 1e-3 && (back.y - p.y).abs() < 1e-3);

        let anchor = Pos2::new(50.0, 60.0);
        let before = cam.to_world(rect, anchor);
        cam.zoom_at(rect, anchor, 3.0);
        let after = cam.to_world(rect, anchor);
        assert!((before.x - after.x).abs() < 1e-3 && (before.y - after.y).abs() < 1e-3);
    }
}
