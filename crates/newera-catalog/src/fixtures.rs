//! Light fixtures: recessed spots, pendants, LED panels and strips. Their
//! diffusers use [`DIFFUSER`], which photo renderers show glowing.

use newera_core::Furniture;

use crate::mesh::{Axis, Mesh, Rgb, rgb, shade};

/// Diffuser and lens color of fixtures.
pub const DIFFUSER: [u8; 3] = [255, 251, 240];

pub(crate) fn build(piece: &Furniture) -> Option<Mesh> {
    let (w, d, h) = (piece.width, piece.depth, piece.height);
    let body = rgb(piece.color.unwrap_or(crate::find(&piece.catalog)?.color));
    let lens = rgb(DIFFUSER);
    let mut m = Mesh::default();
    match piece.catalog.as_str() {
        "downlight" => downlight(&mut m, w, h, body, lens),
        "pendant" => pendant(&mut m, w, h, body, lens),
        "led-panel" => {
            m.cuboid([-w / 2.0, 0.0, -d / 2.0], [w / 2.0, h, d / 2.0], body);
            // The diffuser just below the frame, inset by the rim.
            let rim = (w.min(d) * 0.02).max(0.8);
            m.cuboid(
                [-w / 2.0 + rim, -0.05, -d / 2.0 + rim],
                [w / 2.0 - rim, 0.2, d / 2.0 - rim],
                lens,
            );
        }
        "led-strip" => {
            // Aluminum profile with its opal cover underneath.
            m.cuboid([-w / 2.0, h * 0.35, -d / 2.0], [w / 2.0, h, d / 2.0], body);
            m.cuboid(
                [-w / 2.0 + 0.3, 0.0, -d / 2.0 * 0.8],
                [w / 2.0 - 0.3, h * 0.35, d / 2.0 * 0.8],
                lens,
            );
        }
        _ => return None,
    }
    m.fit_to(w, d, h);
    Some(m)
}

fn downlight(m: &mut Mesh, w: f64, h: f64, body: Rgb, lens: Rgb) {
    let r = w / 2.0;
    // Trim ring just below the ceiling, the lens set into it and the housing
    // above, hidden in the ceiling.
    m.frustum([0.0, 0.0, 0.0], Axis::Y, 0.05, r * 0.6, r * 0.6, 24, lens);
    m.frustum([0.0, 0.05, 0.0], Axis::Y, 0.55, r, r * 0.92, 28, body);
    m.frustum(
        [0.0, 0.6, 0.0],
        Axis::Y,
        h - 0.6,
        r * 0.7,
        r * 0.7,
        20,
        shade(body, -0.3),
    );
}

fn pendant(m: &mut Mesh, w: f64, h: f64, body: Rgb, lens: Rgb) {
    let r = w / 2.0;
    let shade_h = (w * 0.75).min(h * 0.6);
    // Canopy at the ceiling, cord, dome shade and the bulb inside.
    m.frustum(
        [0.0, h - 1.5, 0.0],
        Axis::Y,
        1.5,
        r * 0.3,
        r * 0.3,
        20,
        body,
    );
    m.cylinder(
        [0.0, shade_h, 0.0],
        Axis::Y,
        h - shade_h - 1.5,
        0.35,
        rgb([30, 30, 30]),
    );
    open_cone(m, shade_h * 0.8, r, r * 0.35, body, shade(lens, -0.1));
    m.frustum(
        [0.0, shade_h * 0.8, 0.0],
        Axis::Y,
        shade_h * 0.2,
        r * 0.35,
        r * 0.12,
        20,
        body,
    );
    m.frustum(
        [0.0, shade_h * 0.12, 0.0],
        Axis::Y,
        shade_h * 0.3,
        r * 0.22,
        r * 0.12,
        16,
        lens,
    );
}

/// A cone open at both ends from `r0` at the bottom to `r1` at `height`,
/// `outside` color outside and `inside` within.
fn open_cone(m: &mut Mesh, height: f64, r0: f64, r1: f64, outside: Rgb, inside: Rgb) {
    let segments = 28;
    #[allow(clippy::cast_possible_truncation)]
    let point = |i: u32, r: f64, y: f64| {
        let a = f64::from(i) / f64::from(segments) * std::f64::consts::TAU;
        [(r * a.cos()) as f32, y as f32, (-r * a.sin()) as f32]
    };
    for i in 0..segments {
        let quad = [
            point(i, r0, 0.0),
            point(i + 1, r0, 0.0),
            point(i + 1, r1, height),
            point(i, r1, height),
        ];
        m.polygon(&quad, outside);
        let mut back = quad;
        back.reverse();
        m.polygon(&back, inside);
    }
}
