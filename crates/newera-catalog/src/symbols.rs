//! Architectural plan symbols, drawn in the piece's local plan frame (cm):
//! origin at the center, `x` along the width, `y` along the depth, front of
//! the piece towards `+y`.

use newera_core::{Furniture, OpeningKind};

use crate::{Model, find};

/// A drawable part of a symbol.
#[derive(Debug, Clone, PartialEq)]
pub enum SymbolShape {
    /// Filled polygon; `detail` fills use a slightly darker tone.
    Fill {
        points: Vec<(f64, f64)>,
        detail: bool,
    },
    /// Polyline; `strong` lines are thicker (outlines, door leaves).
    Line {
        points: Vec<(f64, f64)>,
        closed: bool,
        strong: bool,
    },
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<(f64, f64)> {
    vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
}

fn ellipse(
    cx: f64,
    cy: f64,
    rx: f64,
    ry: f64,
    from: f64,
    to: f64,
    segments: u32,
) -> Vec<(f64, f64)> {
    (0..=segments)
        .map(|i| {
            let a = from + (to - from) * f64::from(i) / f64::from(segments);
            (cx + rx * a.cos(), cy + ry * a.sin())
        })
        .collect()
}

struct Sym {
    shapes: Vec<SymbolShape>,
}

impl Sym {
    fn fill(&mut self, points: Vec<(f64, f64)>, detail: bool) {
        self.shapes.push(SymbolShape::Fill { points, detail });
    }
    fn line(&mut self, points: Vec<(f64, f64)>, closed: bool, strong: bool) {
        self.shapes.push(SymbolShape::Line {
            points,
            closed,
            strong,
        });
    }
    fn boxed(&mut self, points: Vec<(f64, f64)>, detail: bool) {
        self.fill(points.clone(), detail);
        self.line(points, true, false);
    }
}

/// Conventional symbol of a technical point, drawn at a readable size
/// (at least 32 cm across) whatever the physical size of the device.
fn point_symbol(s: &mut Sym, symbol: crate::PointSymbol, size: f64) {
    use crate::PointSymbol as P;
    let r = size.max(32.0) / 2.0;
    let circle = |cx: f64, cy: f64, radius: f64| {
        ellipse(cx, cy, radius, radius, 0.0, std::f64::consts::TAU, 28)
    };
    let triangle = vec![(0.0, -r), (r * 0.9, r * 0.6), (-r * 0.9, r * 0.6)];
    let square = rect(-r, -r, r, r);
    match symbol {
        P::OutletLow => {
            s.fill(triangle.clone(), false);
            s.line(triangle, true, true);
        }
        P::OutletMid => {
            s.fill(triangle.clone(), false);
            s.fill(vec![(0.0, -r), (0.0, r * 0.6), (-r * 0.9, r * 0.6)], true);
            s.line(triangle, true, true);
        }
        P::OutletHigh => {
            s.fill(triangle.clone(), true);
            s.line(triangle, true, true);
        }
        P::DataOutlet => {
            s.fill(triangle.clone(), false);
            s.line(triangle, true, true);
            s.line(vec![(-r * 0.5, r * 0.1), (r * 0.5, r * 0.1)], false, true);
        }
        P::NetworkOutlet => {
            // A triangle with "R" strokes inside: the network point.
            s.fill(triangle.clone(), false);
            s.line(triangle, true, true);
            s.line(
                vec![
                    (-r * 0.25, r * 0.45),
                    (-r * 0.25, -r * 0.2),
                    (r * 0.2, -r * 0.2),
                    (r * 0.2, r * 0.1),
                    (-r * 0.25, r * 0.1),
                    (r * 0.25, r * 0.45),
                ],
                false,
                true,
            );
        }
        P::TvOutlet => {
            // A triangle with a coaxial ring.
            s.fill(triangle.clone(), false);
            s.line(triangle, true, true);
            s.line(circle(0.0, r * 0.1, r * 0.3), true, true);
            s.fill(circle(0.0, r * 0.1, r * 0.1), true);
        }
        P::WifiPoint => {
            // A ceiling disc with two arcs of a signal.
            s.fill(circle(0.0, 0.0, r), false);
            s.line(circle(0.0, 0.0, r), true, true);
            for k in [0.35, 0.65] {
                s.line(
                    ellipse(
                        0.0,
                        r * 0.35,
                        r * k,
                        r * k,
                        std::f64::consts::PI * 1.2,
                        std::f64::consts::PI * 1.8,
                        10,
                    ),
                    false,
                    true,
                );
            }
            s.fill(circle(0.0, r * 0.35, r * 0.1), true);
        }
        P::TelecomPanel => {
            // A panel with a grid of ports.
            let panel = rect(-r * 1.2, -r * 0.45, r * 1.2, r * 0.45);
            s.fill(panel.clone(), false);
            s.line(panel.clone(), true, true);
            for x in [-r * 0.6, 0.0, r * 0.6] {
                s.line(
                    rect(x - r * 0.18, -r * 0.18, x + r * 0.18, r * 0.18),
                    true,
                    false,
                );
            }
        }
        P::Switch1 => {
            s.fill(circle(0.0, 0.0, r * 0.45), true);
            s.line(circle(0.0, 0.0, r * 0.45), true, true);
        }
        P::Switch2 => {
            for x in [-r * 0.5, r * 0.5] {
                s.fill(circle(x, 0.0, r * 0.4), true);
                s.line(circle(x, 0.0, r * 0.4), true, true);
            }
        }
        P::Switch3Way => {
            s.fill(circle(0.0, 0.0, r * 0.45), true);
            s.line(circle(0.0, 0.0, r * 0.45), true, true);
            s.line(vec![(-r, r * 0.8), (r, -r * 0.8)], false, true);
        }
        P::LightCeiling => {
            s.fill(circle(0.0, 0.0, r), false);
            s.line(circle(0.0, 0.0, r), true, true);
            let k = r * 0.7;
            s.line(vec![(-k, -k), (k, k)], false, true);
            s.line(vec![(-k, k), (k, -k)], false, true);
        }
        P::LightWall => {
            let half = ellipse(
                0.0,
                0.0,
                r,
                r,
                std::f64::consts::PI,
                std::f64::consts::TAU,
                16,
            );
            s.fill(half.clone(), false);
            s.line(half, true, true);
            s.line(
                vec![(-r * 0.6, -r * 0.6), (r * 0.6, -r * 0.2)],
                false,
                false,
            );
        }
        P::Panel => {
            let panel = rect(-r * 1.2, -r * 0.45, r * 1.2, r * 0.45);
            s.fill(panel.clone(), false);
            s.fill(
                vec![
                    (-r * 1.2, -r * 0.45),
                    (r * 1.2, -r * 0.45),
                    (r * 1.2, r * 0.45),
                ],
                true,
            );
            s.line(panel, true, true);
        }
        P::AirConditioner => {
            s.fill(square.clone(), false);
            s.line(square, true, true);
            s.line(vec![(-r, r), (r, -r)], false, true);
            s.fill(vec![(-r, -r), (r, -r), (-r, r)], true);
        }
        P::ShowerPoint => {
            s.fill(circle(0.0, 0.0, r), false);
            s.line(circle(0.0, 0.0, r), true, true);
            let t = vec![(0.0, -r * 0.6), (r * 0.5, r * 0.4), (-r * 0.5, r * 0.4)];
            s.fill(t.clone(), true);
        }
        P::Doorbell => {
            let small = rect(-r * 0.6, -r * 0.6, r * 0.6, r * 0.6);
            s.fill(small.clone(), false);
            s.line(small, true, true);
            s.fill(circle(0.0, 0.0, r * 0.3), true);
        }
        P::SmartRelay => {
            // A small box with a zigzag: the relay hidden in a box.
            let small = rect(-r * 0.6, -r * 0.6, r * 0.6, r * 0.6);
            s.fill(small.clone(), false);
            s.line(small, true, true);
            s.line(
                vec![
                    (-r * 0.4, r * 0.3),
                    (-r * 0.1, -r * 0.3),
                    (r * 0.1, r * 0.3),
                    (r * 0.4, -r * 0.3),
                ],
                false,
                true,
            );
        }
        P::SmartSwitch => {
            // A switch with a signal arc over it.
            s.fill(circle(0.0, r * 0.2, r * 0.4), true);
            s.line(circle(0.0, r * 0.2, r * 0.4), true, true);
            s.line(
                ellipse(
                    0.0,
                    r * 0.2,
                    r * 0.8,
                    r * 0.8,
                    std::f64::consts::PI * 1.2,
                    std::f64::consts::PI * 1.8,
                    10,
                ),
                false,
                true,
            );
        }
        P::Dimmer => {
            // A switch crossed by a rising arrow.
            s.fill(circle(0.0, 0.0, r * 0.45), false);
            s.line(circle(0.0, 0.0, r * 0.45), true, true);
            s.line(vec![(-r * 0.8, r * 0.8), (r * 0.8, -r * 0.8)], false, true);
            s.fill(
                vec![
                    (r * 0.8, -r * 0.8),
                    (r * 0.35, -r * 0.65),
                    (r * 0.65, -r * 0.35),
                ],
                true,
            );
        }
        P::PresenceSensor => {
            // A disc with an eye: the ceiling sensor.
            s.fill(circle(0.0, 0.0, r * 0.8), false);
            s.line(circle(0.0, 0.0, r * 0.8), true, true);
            s.line(
                ellipse(0.0, 0.0, r * 0.55, r * 0.3, 0.0, std::f64::consts::TAU, 20),
                true,
                true,
            );
            s.fill(circle(0.0, 0.0, r * 0.15), true);
        }
        P::SmartLock => {
            // A plate with a keypad.
            let plate = rect(-r * 0.45, -r, r * 0.45, r);
            s.fill(plate.clone(), false);
            s.line(plate, true, true);
            for y in [-r * 0.5, 0.0, r * 0.5] {
                s.fill(circle(0.0, y, r * 0.12), true);
            }
        }
        P::ColdWater => {
            s.fill(circle(0.0, 0.0, r * 0.6), true);
            s.line(circle(0.0, 0.0, r), true, true);
        }
        P::HotWater => {
            s.fill(circle(0.0, 0.0, r), false);
            s.fill(
                ellipse(
                    0.0,
                    0.0,
                    r,
                    r,
                    std::f64::consts::FRAC_PI_2,
                    std::f64::consts::FRAC_PI_2 * 3.0,
                    14,
                ),
                true,
            );
            s.line(circle(0.0, 0.0, r), true, true);
        }
        P::Sewer => {
            s.fill(square.clone(), false);
            s.line(square, true, true);
            s.line(circle(0.0, 0.0, r * 0.55), true, true);
        }
        P::FloorDrain => {
            s.fill(square.clone(), false);
            s.line(square, true, true);
            for k in [-0.5, 0.0, 0.5] {
                s.line(vec![(-r * 0.8, r * k), (r * 0.8, r * k)], false, false);
            }
        }
        P::VentGrille => {
            // A plate with louvre slats.
            let plate = rect(-r, -r * 0.5, r, r * 0.5);
            s.fill(plate.clone(), false);
            s.line(plate, true, true);
            for k in [-0.5, 0.0, 0.5] {
                s.line(
                    vec![(-r * 0.8, r * k * 0.8), (r * 0.8, r * k * 0.8)],
                    false,
                    false,
                );
            }
        }
        P::DryDrain => {
            // A square with a single bar: no trap.
            s.fill(square.clone(), false);
            s.line(square, true, true);
            s.line(vec![(-r * 0.8, 0.0), (r * 0.8, 0.0)], false, true);
        }
        P::LinearDrain => {
            // A long grille across the piece's own width.
            let bar = rect(-r * 2.2, -r * 0.35, r * 2.2, r * 0.35);
            s.fill(bar.clone(), false);
            s.line(bar, true, true);
            for k in [-1.6, -0.8, 0.0, 0.8, 1.6] {
                s.line(vec![(r * k, -r * 0.25), (r * k, r * 0.25)], false, false);
            }
        }
        P::RainDrain => {
            s.fill(circle(0.0, 0.0, r), false);
            s.line(circle(0.0, 0.0, r), true, true);
            s.line(vec![(-r * 0.6, -r * 0.6), (r * 0.6, r * 0.6)], false, true);
            s.line(vec![(-r * 0.6, r * 0.6), (r * 0.6, -r * 0.6)], false, true);
        }
        P::Valve => {
            let bow = vec![(-r, -r * 0.6), (r, r * 0.6), (r, -r * 0.6), (-r, r * 0.6)];
            s.fill(vec![(-r, -r * 0.6), (0.0, 0.0), (-r, r * 0.6)], true);
            s.fill(vec![(r, -r * 0.6), (0.0, 0.0), (r, r * 0.6)], true);
            s.line(bow, true, true);
        }
        P::GreaseTrap => {
            s.fill(square.clone(), false);
            s.line(square, true, true);
            s.line(vec![(-r, -r), (r, r)], false, false);
            s.line(vec![(-r, r), (r, -r)], false, false);
        }
        P::InspectionBox => {
            s.fill(square.clone(), false);
            s.line(square, true, true);
            s.line(rect(-r * 0.7, -r * 0.7, r * 0.7, r * 0.7), true, false);
        }
        P::VentPipe => {
            // A ring with a cross: the vent stack seen from above.
            s.line(circle(0.0, 0.0, r * 0.7), true, true);
            s.line(vec![(-r, 0.0), (r, 0.0)], false, true);
            s.line(vec![(0.0, -r), (0.0, r)], false, true);
        }
        P::WaterMeter => {
            s.fill(circle(0.0, 0.0, r), false);
            s.line(circle(0.0, 0.0, r), true, true);
            s.line(vec![(-r, 0.0), (r, 0.0)], false, true);
        }
        P::Gas => {
            let diamond = vec![(0.0, -r), (r, 0.0), (0.0, r), (-r, 0.0)];
            s.fill(diamond.clone(), true);
            s.line(diamond, true, true);
        }
    }
}

/// Plan symbol of a piece at its size, in its local frame.
pub fn plan_symbol(piece: &Furniture) -> Vec<SymbolShape> {
    let (w, d) = (piece.width, piece.depth);
    let (hw, hd) = (w / 2.0, d / 2.0);
    let model = find(&piece.catalog).map_or(Model::Box, |i| i.model);
    let mut s = Sym { shapes: Vec::new() };
    if let Some(newera_core::SolidShape::Outline(points)) = &piece.shape {
        // Follow the box when the piece was resized.
        let (lo, hi) = points
            .iter()
            .fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| {
                (
                    [lo[0].min(p[0]), lo[1].min(p[1])],
                    [hi[0].max(p[0]), hi[1].max(p[1])],
                )
            });
        let fit = |v: f64, k: usize, size: f64| {
            let span = hi[k] - lo[k];
            if span > 1e-9 {
                (v - lo[k]) / span * size - size / 2.0
            } else {
                0.0
            }
        };
        let outline: Vec<(f64, f64)> = points
            .iter()
            .map(|p| (fit(p[0], 0, w), fit(p[1], 1, d)))
            .collect();
        s.fill(outline.clone(), false);
        s.line(outline, true, true);
        return s.shapes;
    }

    if let Some(opening) = piece.opening.as_ref().filter(|o| !o.sashes.is_empty()) {
        // Explicit leaves: each turns around its axis, drawn open at its end
        // angle with the arc its edge sweeps.
        s.line(rect(-hw, -hd, hw, hd), true, false);
        for sash in &opening.sashes {
            let (ax, ay) = (-hw + sash.x_axis * w, -hd + sash.y_axis * d);
            let radius = sash.width * w;
            let (start, end) = (sash.start_angle.to_radians(), sash.end_angle.to_radians());
            // Sash angles turn counter-clockwise with y pointing to the back.
            s.line(
                vec![(ax, ay), (ax + radius * end.cos(), ay - radius * end.sin())],
                false,
                true,
            );
            s.line(
                ellipse(ax, ay, radius, -radius, start, end, 18),
                false,
                false,
            );
        }
        return s.shapes;
    }

    if let Some(opening) = &piece.opening {
        match opening.kind {
            OpeningKind::Door if opening.sliding => {
                let leaf = w / f64::from(opening.leaves.max(1));
                for i in 0..opening.leaves.max(1) {
                    let x0 = -hw + f64::from(i) * leaf;
                    let y = if i % 2 == 0 { -d * 0.15 } else { d * 0.15 };
                    s.boxed(rect(x0 - 2.0, y - 1.5, x0 + leaf + 2.0, y + 1.5), true);
                }
            }
            OpeningKind::Door => {
                let leaves = opening.leaves.clamp(1, 2);
                let leaf = w / f64::from(leaves);
                let hinges: Vec<(f64, f64)> = if leaves == 2 {
                    vec![(-hw, 1.0), (hw, -1.0)]
                } else if opening.hinge_right {
                    vec![(hw, -1.0)]
                } else {
                    vec![(-hw, 1.0)]
                };
                for (hx, toward) in hinges {
                    // Leaf opened at 90°, and the arc its edge sweeps.
                    s.line(vec![(hx, hd), (hx, hd + leaf)], false, true);
                    let arc = ellipse(
                        hx,
                        hd,
                        leaf,
                        leaf,
                        std::f64::consts::FRAC_PI_2,
                        if toward > 0.0 {
                            0.0
                        } else {
                            std::f64::consts::PI
                        },
                        18,
                    );
                    s.line(arc, false, false);
                }
            }
            OpeningKind::Window => {
                for y in [-hd, 0.0, hd] {
                    s.line(vec![(-hw, y), (hw, y)], false, y == 0.0);
                }
            }
            OpeningKind::Passage => {}
        }
        // Jambs across the wall thickness.
        s.line(vec![(-hw, -hd), (-hw, hd)], false, true);
        s.line(vec![(hw, -hd), (hw, hd)], false, true);
        return s.shapes;
    }

    if let Model::Point(symbol) = model {
        point_symbol(&mut s, symbol, w.max(d));
        return s.shapes;
    }

    let round = matches!(
        model,
        Model::Table { round: true }
            | Model::Stool
            | Model::Plant
            | Model::Tree
            | Model::Lamp
            | Model::Column { round: true }
            | Model::Pool { oval: true }
    );
    let outline = if round {
        ellipse(0.0, 0.0, hw, hd, 0.0, std::f64::consts::TAU, 40)
    } else {
        rect(-hw, -hd, hw, hd)
    };
    // Rugs show the floor through: outline and fringe only.
    if model == Model::Rug {
        s.line(
            rect(
                -hw + 6.0_f64.min(hw / 4.0),
                -hd + 6.0_f64.min(hd / 4.0),
                hw - 6.0_f64.min(hw / 4.0),
                hd - 6.0_f64.min(hd / 4.0),
            ),
            true,
            false,
        );
    } else {
        s.fill(outline.clone(), false);
    }

    match model {
        Model::Sofa { seats } => {
            let arm = (w * 0.1).clamp(10.0, 22.0);
            let back = (d * 0.22).clamp(12.0, 25.0);
            s.boxed(rect(-hw, -hd, hw, -hd + back), true);
            s.boxed(rect(-hw, -hd, -hw + arm, hd), true);
            s.boxed(rect(hw - arm, -hd, hw, hd), true);
            let slot = (w - 2.0 * arm) / f64::from(seats.max(1));
            for i in 1..seats.max(1) {
                let x = -hw + arm + f64::from(i) * slot;
                s.line(vec![(x, -hd + back), (x, hd)], false, false);
            }
        }
        Model::Bed { pillows } => {
            let head = (d * 0.04).clamp(4.0, 8.0);
            s.boxed(rect(-hw, -hd, hw, -hd + head), true);
            let n = pillows.max(1);
            let slot = (w - 20.0) / f64::from(n);
            for i in 0..n {
                let x0 = -hw + 10.0 + f64::from(i) * slot + 3.0;
                s.line(
                    rect(x0, -hd + head + 6.0, x0 + slot - 6.0, -hd + head + 42.0),
                    true,
                    false,
                );
            }
            // Folded duvet.
            let fold = -hd + d * 0.33;
            s.line(vec![(-hw, fold), (hw, fold)], false, false);
            s.line(vec![(-hw, fold + 12.0), (hw, fold + 12.0)], false, false);
        }
        Model::Chair | Model::OfficeChair => {
            s.boxed(rect(-hw, -hd, hw, -hd + (d * 0.15).max(4.0)), true);
        }
        Model::Cabinet { doors, .. } if doors >= 2 && d > 50.0 => {
            // Wardrobe: hanging rod.
            s.line(vec![(-hw + 5.0, 0.0), (hw - 5.0, 0.0)], false, false);
        }
        Model::Shelf { .. } => {
            s.line(vec![(-hw, -hd + 2.0), (hw, -hd + 2.0)], false, false);
        }
        Model::Fridge => {
            s.line(vec![(-hw, hd - 4.0), (hw, hd - 4.0)], false, false);
        }
        Model::Cooktop => {
            for (sx, sy, r) in [
                (-0.22, -0.2, 7.0),
                (0.22, -0.2, 5.5),
                (-0.22, 0.2, 5.5),
                (0.22, 0.2, 8.5),
            ] {
                let r = f64::min(r, w * 0.12);
                s.line(
                    ellipse(sx * w, sy * d, r, r, 0.0, std::f64::consts::TAU, 20),
                    true,
                    false,
                );
            }
        }
        Model::SinkBowl => {
            s.boxed(rect(-hw + 2.5, -hd + 2.5, hw - 2.5, hd - 2.5), true);
            s.line(
                ellipse(0.0, 0.0, 2.5, 2.5, 0.0, std::f64::consts::TAU, 12),
                true,
                false,
            );
        }
        Model::Stove => {
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let r = (w * 0.09).min(7.0);
                s.line(
                    ellipse(
                        sx * w * 0.22,
                        sy * d * 0.2,
                        r,
                        r,
                        0.0,
                        std::f64::consts::TAU,
                        20,
                    ),
                    true,
                    false,
                );
            }
        }
        Model::SinkCounter | Model::Basin => {
            let (bw, bd) = ((w * 0.45).min(60.0), (d * 0.6).min(45.0));
            if model == Model::Basin {
                let r = (w.min(d) * 0.3).min(22.0);
                s.boxed(
                    ellipse(0.0, 0.0, r * 1.2, r, 0.0, std::f64::consts::TAU, 28),
                    true,
                );
            } else {
                s.boxed(rect(-bw / 2.0, -bd / 2.0, bw / 2.0, bd / 2.0), true);
            }
            s.line(
                ellipse(
                    0.0,
                    -bd / 2.0 - 4.0,
                    2.0,
                    2.0,
                    0.0,
                    std::f64::consts::TAU,
                    10,
                ),
                true,
                false,
            );
        }
        Model::Appliance { round_door } => {
            if round_door {
                let r = (w.min(d) * 0.35).min(25.0);
                s.line(
                    ellipse(0.0, 0.0, r, r, 0.0, std::f64::consts::TAU, 28),
                    true,
                    false,
                );
            }
        }
        Model::Toilet => {
            let tank = (d * 0.25).clamp(12.0, 20.0);
            s.boxed(rect(-hw, -hd, hw, -hd + tank), true);
            let cy = -hd + tank + (d - tank) / 2.0;
            s.boxed(
                ellipse(
                    0.0,
                    cy,
                    hw * 0.85,
                    (d - tank) / 2.0 * 0.95,
                    0.0,
                    std::f64::consts::TAU,
                    28,
                ),
                true,
            );
        }
        Model::Shower => {
            s.line(vec![(-hw, -hd), (hw, hd)], false, false);
            s.line(vec![(-hw, hd), (hw, -hd)], false, false);
            s.line(
                ellipse(0.0, 0.0, 3.0, 3.0, 0.0, std::f64::consts::TAU, 12),
                true,
                false,
            );
        }
        Model::Bathtub => {
            s.boxed(rect(-hw + 8.0, -hd + 8.0, hw - 8.0, hd - 8.0), true);
            s.line(
                ellipse(hw - 20.0, 0.0, 2.5, 2.5, 0.0, std::f64::consts::TAU, 12),
                true,
                false,
            );
        }
        Model::Stairs { steps } => {
            let n = steps.max(2);
            let run = d / f64::from(n);
            for i in 1..n {
                let y = hd - f64::from(i) * run;
                s.line(vec![(-hw, y), (hw, y)], false, false);
            }
            // Arrow pointing up the stairs (towards -y).
            s.line(vec![(0.0, hd - run / 2.0), (0.0, -hd + run)], false, true);
            s.line(
                vec![
                    (-w * 0.15, -hd + run + w * 0.15),
                    (0.0, -hd + run),
                    (w * 0.15, -hd + run + w * 0.15),
                ],
                false,
                true,
            );
        }
        Model::Plant | Model::Tree => {
            for k in [0.6, 0.3] {
                s.line(
                    ellipse(0.0, 0.0, hw * k, hd * k, 0.0, std::f64::consts::TAU, 24),
                    true,
                    false,
                );
            }
        }
        Model::Lamp => {
            s.line(
                ellipse(0.0, 0.0, hw * 0.3, hd * 0.3, 0.0, std::f64::consts::TAU, 16),
                true,
                false,
            );
        }
        Model::Car => {
            s.line(
                rect(-w * 0.22, -hd + 16.0, w * 0.25, hd - 16.0),
                true,
                false,
            );
            s.line(
                vec![
                    (w * 0.25, -hd + 16.0),
                    (w * 0.33, -hd + 22.0),
                    (w * 0.33, hd - 22.0),
                    (w * 0.25, hd - 16.0),
                ],
                false,
                false,
            );
        }
        Model::Desk => {
            let drawer = (w * 0.35).min(45.0);
            s.line(vec![(hw - drawer, -hd), (hw - drawer, hd)], false, false);
        }
        Model::Hood => {
            // Above the cut line, like a wall cabinet, plus the duct.
            let duct = (w * 0.3).min(30.0) / 2.0;
            s.line(vec![(-hw, hd - 3.0), (hw, hd - 3.0)], false, false);
            s.line(vec![(-duct, -hd + 3.0), (duct, -hd + 3.0)], false, false);
        }
        Model::TvStand
        | Model::Tv
        | Model::Counter
        | Model::WallCabinet
        | Model::Microwave
        | Model::Oven => {
            s.line(vec![(-hw, hd - 3.0), (hw, hd - 3.0)], false, false);
        }
        Model::Table { round: false } => {
            s.line(rect(-hw + 5.0, -hd + 5.0, hw - 5.0, hd - 5.0), true, false);
        }
        Model::Pool { oval: false } => {
            let coping = 30.0_f64.min(w / 6.0).min(d / 6.0);
            s.boxed(
                rect(-hw + coping, -hd + coping, hw - coping, hd - coping),
                true,
            );
        }
        Model::Pool { oval: true } => {
            let coping = 30.0_f64.min(w / 6.0).min(d / 6.0);
            s.boxed(
                ellipse(
                    0.0,
                    0.0,
                    hw - coping,
                    hd - coping,
                    0.0,
                    std::f64::consts::TAU,
                    48,
                ),
                true,
            );
        }
        Model::DiningSet { chairs } => {
            let chair = 45.0_f64.min(d * 0.3);
            let tw = w - 2.0 * chair * 0.6;
            s.boxed(rect(-tw / 2.0, -hd + chair, tw / 2.0, hd - chair), false);
            let per_side = u32::from(chairs.max(2)) / 2;
            for i in 0..per_side {
                let x = -tw / 2.0 + tw * (f64::from(i) + 0.5) / f64::from(per_side);
                let half = chair / 2.2;
                s.line(
                    rect(x - half, -hd + 3.0, x + half, -hd + chair - 3.0),
                    true,
                    false,
                );
                s.line(
                    rect(x - half, hd - chair + 3.0, x + half, hd - 3.0),
                    true,
                    false,
                );
            }
        }
        Model::SofaL => {
            let seat_d = (d * 0.55).min(95.0);
            let chaise = (w * 0.3).max(70.0).min(w * 0.5);
            let back = 22.0_f64.min(seat_d * 0.25);
            s.line(vec![(-hw, -hd + back), (hw, -hd + back)], false, false);
            s.line(
                vec![
                    (-hw, -hd + seat_d),
                    (hw - chaise, -hd + seat_d),
                    (hw - chaise, hd),
                ],
                false,
                false,
            );
        }
        Model::Railing | Model::Fence | Model::GlassPanel => {
            s.line(vec![(-hw, 0.0), (hw, 0.0)], false, true);
        }
        Model::Lounger => {
            s.line(
                vec![(-hw, -hd + d * 0.3), (hw, -hd + d * 0.3)],
                false,
                false,
            );
        }
        Model::Grill => {
            s.line(rect(-w * 0.35, -hd, w * 0.35, -hd + 10.0), true, false);
        }
        Model::Corrugated => {
            let waves = (w / 18.0).round().max(2.0);
            for i in 1..crate::count(waves) {
                let x = -hw + w * f64::from(i) / waves;
                s.line(vec![(x, -hd), (x, hd)], false, false);
            }
        }
        _ => {}
    }
    s.line(outline, true, true);
    s.shapes
}

#[cfg(test)]
mod tests {
    use newera_core::{FurnitureId, Point2};

    use super::*;
    use crate::CATALOG;

    #[test]
    fn symbols_stay_inside_the_footprint_except_door_swings() {
        for item in CATALOG {
            let piece = item.instantiate(FurnitureId(1), Point2::new(0.0, 0.0));
            let shapes = plan_symbol(&piece);
            assert!(
                !shapes.is_empty() || item.id == "passage",
                "{} has no symbol",
                item.id
            );
            // Technical symbols keep a readable minimum size on purpose.
            if item.category.discipline().is_some() {
                continue;
            }
            let swings = item
                .opening
                .is_some_and(|o| o.kind == OpeningKind::Door && !o.sliding);
            for shape in &shapes {
                let points = match shape {
                    SymbolShape::Fill { points, .. } | SymbolShape::Line { points, .. } => points,
                };
                for (x, y) in points {
                    let reach_y = if swings {
                        piece.depth / 2.0 + piece.width
                    } else {
                        piece.depth / 2.0
                    };
                    assert!(x.abs() <= piece.width / 2.0 + 2.01, "{} x={x}", item.id);
                    assert!(
                        *y >= -piece.depth / 2.0 - 2.01 && *y <= reach_y + 0.01,
                        "{} y={y}",
                        item.id
                    );
                }
            }
        }
    }
}
