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

/// Plan symbol of a piece at its size, in its local frame.
pub fn plan_symbol(piece: &Furniture) -> Vec<SymbolShape> {
    let (w, d) = (piece.width, piece.depth);
    let (hw, hd) = (w / 2.0, d / 2.0);
    let model = find(&piece.catalog).map_or(Model::Box, |i| i.model);
    let mut s = Sym { shapes: Vec::new() };

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

    let round = matches!(
        model,
        Model::Table { round: true }
            | Model::Stool
            | Model::Plant
            | Model::Tree
            | Model::Lamp
            | Model::Column { round: true }
    );
    let outline = if round {
        ellipse(0.0, 0.0, hw, hd, 0.0, std::f64::consts::TAU, 40)
    } else {
        rect(-hw, -hd, hw, hd)
    };
    s.fill(outline.clone(), false);

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
        Model::TvStand | Model::Tv | Model::Counter | Model::WallCabinet | Model::Microwave => {
            s.line(vec![(-hw, hd - 3.0), (hw, hd - 3.0)], false, false);
        }
        Model::Table { round: false } => {
            s.line(rect(-hw + 5.0, -hd + 5.0, hw - 5.0, hd - 5.0), true, false);
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
