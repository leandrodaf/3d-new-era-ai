//! Carry finish ownership through procedural construction, including shaded
//! colors and nested parts. A body finish must not repaint fixed glass,
//! ceramic, metal, bedding, or a luminaire's diffuser.

use crate::mesh::{self, Axis, Mesh, Rgb};

#[derive(Clone, Copy)]
pub(crate) struct Paint {
    color: Rgb,
    finishable: bool,
}

impl Paint {
    pub(crate) fn body(color: Rgb) -> Self {
        Self {
            color,
            finishable: true,
        }
    }
}

/// A material supplied by the generator, independent of the body finish.
/// Primary surfaces with a fixed default color opt in via `Paint::body`.
pub(crate) fn rgb(color: [u8; 3]) -> Paint {
    Paint {
        color: mesh::rgb(color),
        finishable: false,
    }
}

pub(crate) fn shade(paint: Paint, amount: f32) -> Paint {
    Paint {
        color: mesh::shade(paint.color, amount),
        ..paint
    }
}

#[derive(Default)]
pub(crate) struct PaintedMesh(Mesh);

impl PaintedMesh {
    fn paint(&mut self, paint: Paint, draw: impl FnOnce(&mut Mesh, Rgb)) {
        let start = self.0.positions.len();
        draw(&mut self.0, paint.color);
        if !paint.finishable {
            self.0.protect_finish_since(start);
        }
    }

    pub(crate) fn polygon(&mut self, corners: &[[f32; 3]], paint: Paint) {
        self.paint(paint, |mesh, color| mesh.polygon(corners, color));
    }

    pub(crate) fn cuboid(&mut self, min: [f64; 3], max: [f64; 3], paint: Paint) {
        self.paint(paint, |mesh, color| mesh.cuboid(min, max, color));
    }

    pub(crate) fn block(&mut self, center: [f64; 2], y0: f64, size: [f64; 3], paint: Paint) {
        self.paint(paint, |mesh, color| mesh.block(center, y0, size, color));
    }

    pub(crate) fn cylinder(
        &mut self,
        base: [f64; 3],
        axis: Axis,
        length: f64,
        radius: f64,
        paint: Paint,
    ) {
        self.paint(paint, |mesh, color| {
            mesh.cylinder(base, axis, length, radius, color);
        });
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn frustum(
        &mut self,
        base: [f64; 3],
        axis: Axis,
        length: f64,
        r0: f64,
        r1: f64,
        segments: u32,
        paint: Paint,
    ) {
        self.paint(paint, |mesh, color| {
            mesh.frustum(base, axis, length, r0, r1, segments, color);
        });
    }

    pub(crate) fn fit(mut self, width: f64, depth: f64, height: f64) -> Mesh {
        self.0.fit_to(width, depth, height);
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_colors_keep_different_finish_ownership_after_shading() {
        let fixed = rgb([150, 154, 160]);
        let body = Paint::body(mesh::rgb([150, 154, 160]));
        let mut m = PaintedMesh::default();
        m.cuboid([0.0; 3], [1.0; 3], shade(fixed, 0.1));
        m.cuboid([0.0; 3], [1.0; 3], shade(body, 0.1));
        let m = m.fit(10.0, 10.0, 10.0);
        let half = m.positions.len() / 2;
        assert_eq!(&m.colors[..half], &m.colors[half..]);
        assert!((0..half).all(|i| !m.finishable.get(i).copied().unwrap_or(true)));
        assert!((half..m.positions.len()).all(|i| m.finishable.get(i).copied().unwrap_or(true)));
    }
}
