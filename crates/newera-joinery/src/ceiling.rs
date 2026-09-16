//! Plasterboard ceilings: coves (sancas) along a room's walls with a channel
//! for LED strip, and shadow gaps (tabicas) between ceiling and walls.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Output, Part, num};

/// Plasterboard thickness, cm.
const BOARD: f64 = 1.25;
const PLASTER: [u8; 3] = [246, 245, 241];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CoveType {
    /// Lowered border with a lip that stops short of the ceiling: light washes
    /// up onto the ceiling.
    #[default]
    Open,
    /// Lowered border closed up to the ceiling (no light slot).
    Closed,
    /// The middle of the ceiling is lowered; light comes out around it.
    Inverted,
}

/// A cove along a room. Sizes in cm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct CoveParams {
    /// Room outline `[[x,y],…]`, cm (filled from the room when given by id).
    pub pts: Vec<[f64; 2]>,
    #[serde(rename = "type")]
    pub kind: CoveType,
    /// Ceiling height from the floor, cm (default 260).
    pub ceiling: f64,
    /// Border width from the walls, cm (default 40).
    pub width: f64,
    /// How far the border comes down, cm (default 15).
    pub drop: f64,
    /// Opening left for the light, cm (default 8).
    pub slot: f64,
    /// LED strip in the channel (default true).
    pub led: bool,
}

impl Default for CoveParams {
    fn default() -> Self {
        Self {
            pts: Vec::new(),
            kind: CoveType::Open,
            ceiling: 260.0,
            width: 40.0,
            drop: 15.0,
            slot: 8.0,
            led: true,
        }
    }
}

/// A shadow gap along a room. Sizes in cm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ShadowGapParams {
    pub pts: Vec<[f64; 2]>,
    /// Ceiling height from the floor, cm (default 260).
    pub ceiling: f64,
    /// Gap between ceiling board and wall, cm (default 1.5).
    pub gap: f64,
    /// Depth of the recess, cm (default 3).
    pub depth: f64,
    /// Indirect LED light in the gap (default false).
    pub led: bool,
}

impl Default for ShadowGapParams {
    fn default() -> Self {
        Self {
            pts: Vec::new(),
            ceiling: 260.0,
            gap: 1.5,
            depth: 3.0,
            led: false,
        }
    }
}

fn signed_area(pts: &[[f64; 2]]) -> f64 {
    let n = pts.len();
    (0..n)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f64>()
        / 2.0
}

/// The polygon moved `distance` cm inward (mitred corners).
fn inset(pts: &[[f64; 2]], distance: f64) -> Option<Vec<[f64; 2]>> {
    let n = pts.len();
    if n < 3 {
        return None;
    }
    // With y pointing down, a positive shoelace area turns clockwise on screen:
    // the inside is to the right of each edge.
    let side = if signed_area(pts) > 0.0 { 1.0 } else { -1.0 };
    let lines: Vec<([f64; 2], [f64; 2])> = (0..n)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let len = dx.hypot(dy).max(1e-9);
            let normal = [-dy / len * side, dx / len * side];
            (
                [a[0] + normal[0] * distance, a[1] + normal[1] * distance],
                [dx / len, dy / len],
            )
        })
        .collect();
    (0..n)
        .map(|i| {
            let (p, u) = lines[(i + n - 1) % n];
            let (q, v) = lines[i];
            let cross = u[0] * v[1] - u[1] * v[0];
            if cross.abs() < 1e-9 {
                return Some(q);
            }
            let t = ((q[0] - p[0]) * v[1] - (q[1] - p[1]) * v[0]) / cross;
            Some([p[0] + u[0] * t, p[1] + u[1] * t])
        })
        .collect()
}

fn perimeter(pts: &[[f64; 2]]) -> f64 {
    (0..pts.len())
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            (b[0] - a[0]).hypot(b[1] - a[1])
        })
        .sum()
}

/// Mitred strips between two nested outlines, one per wall.
fn strips(
    name: &str,
    outer: &[[f64; 2]],
    inner: &[[f64; 2]],
    z: f64,
    height: f64,
    board: Option<&str>,
    color: [u8; 3],
) -> Vec<Part> {
    let n = outer.len();
    (0..n)
        .map(|i| {
            let j = (i + 1) % n;
            let quad = vec![outer[i], outer[j], inner[j], inner[i]];
            let (lo, hi) = quad
                .iter()
                .fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| {
                    (
                        [lo[0].min(p[0]), lo[1].min(p[1])],
                        [hi[0].max(p[0]), hi[1].max(p[1])],
                    )
                });
            let length = (outer[j][0] - outer[i][0]).hypot(outer[j][1] - outer[i][1]);
            let width = {
                let (a, b, p) = (outer[i], outer[j], inner[i]);
                ((b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])).abs()
                    / length.max(1e-9)
            };
            let mut part = Part::solid(
                &format!("{name} {}", i + 1),
                [lo[0], lo[1], z],
                [hi[0] - lo[0], hi[1] - lo[1], height],
                color,
            );
            part.board = board.map(str::to_owned);
            part.outline = Some(quad);
            part.cut = Some([length, width]);
            part
        })
        .collect()
}

fn check_room(pts: &[[f64; 2]], border: f64) -> Result<Vec<[f64; 2]>, String> {
    if pts.len() < 3 || signed_area(pts).abs() < 1.0 {
        return Err("Informe o contorno do cômodo (pts) ou o id do cômodo.".into());
    }
    let inner = inset(pts, border).ok_or("Contorno inválido.")?;
    // The border can't swallow the room.
    if signed_area(&inner).signum() != signed_area(pts).signum()
        || signed_area(&inner).abs() < 100.0
    {
        return Err(format!(
            "Uma faixa de {} cm não cabe neste cômodo; reduza width.",
            num(border)
        ));
    }
    Ok(inner)
}

pub(crate) fn cove(p: &CoveParams) -> Result<Output, String> {
    let mut notes: Vec<String> = Vec::new();
    if p.drop <= 0.0 {
        return Err("A sanca precisa de uma descida maior que zero (drop).".into());
    }
    if p.drop < 5.0 {
        notes.push(format!(
            "Descida de {} cm: abaixo de 5 cm a fita de LED aparece.",
            num(p.drop)
        ));
    }
    if p.kind == CoveType::Open && p.slot >= p.drop {
        notes.push(format!(
            "A abertura de luz de {} cm é maior que a descida de {} cm: a fita fica à vista; slot até {} a esconde.",
            num(p.slot),
            num(p.drop),
            num((p.drop - 3.0).max(0.0))
        ));
    }
    let outer = p.pts.clone();
    let inner = check_room(&outer, p.width)?;
    let lip = inset(&outer, p.width - BOARD).ok_or("Contorno inválido.")?;
    let (ceiling, drop) = (p.ceiling, p.drop);
    let board = "Gesso acartonado 12,5 mm";
    let mut parts = Vec::new();

    match p.kind {
        CoveType::Open | CoveType::Closed => {
            parts.extend(strips(
                "Placa da sanca",
                &outer,
                &inner,
                ceiling - drop,
                BOARD,
                Some(board),
                PLASTER,
            ));
            let lip_top = if p.kind == CoveType::Open {
                ceiling - p.slot
            } else {
                ceiling
            };
            parts.extend(strips(
                "Espelho da sanca",
                &lip,
                &inner,
                ceiling - drop,
                lip_top - (ceiling - drop),
                Some(board),
                PLASTER,
            ));
            if p.led && p.kind == CoveType::Open {
                let led_out = inset(&outer, p.width - BOARD - 3.0).ok_or("Contorno inválido.")?;
                let led_in = inset(&outer, p.width - BOARD - 2.0).ok_or("Contorno inválido.")?;
                let mut led = strips(
                    "Fita de LED",
                    &led_out,
                    &led_in,
                    ceiling - drop + BOARD,
                    0.3,
                    None,
                    [255, 236, 180],
                );
                parts.append(&mut led);
            }
        }
        CoveType::Inverted => {
            // The border stays at the ceiling; the middle comes down.
            let slot = inset(&outer, p.width + p.slot).ok_or("Contorno inválido.")?;
            if signed_area(&slot).signum() != signed_area(&outer).signum() {
                return Err(
                    "A sanca invertida não cabe neste cômodo; reduza width ou slot.".into(),
                );
            }
            let mut middle = Part::solid(
                "Forro rebaixado",
                [0.0, 0.0, ceiling - drop],
                [0.0, 0.0, BOARD],
                PLASTER,
            );
            let (lo, hi) = slot
                .iter()
                .fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| {
                    (
                        [lo[0].min(p[0]), lo[1].min(p[1])],
                        [hi[0].max(p[0]), hi[1].max(p[1])],
                    )
                });
            middle.at = [lo[0], lo[1], ceiling - drop];
            middle.size = [hi[0] - lo[0], hi[1] - lo[1], BOARD];
            middle.board = Some(board.into());
            middle.outline = Some(slot.clone());
            parts.push(middle);
            let rim = inset(&outer, p.width + p.slot + BOARD).ok_or("Contorno inválido.")?;
            parts.extend(strips(
                "Espelho do rebaixo",
                &slot,
                &rim,
                ceiling - drop,
                drop,
                Some(board),
                PLASTER,
            ));
            notes.push("Fita de LED voltada para baixo, escondida na borda do rebaixo.".into());
        }
    }
    let mut hardware = vec![format!(
        "{} m de perfil e tabica para a sanca",
        num(perimeter(&outer) / 100.0 * 2.0)
    )];
    if p.led && p.kind != CoveType::Closed {
        hardware.push(format!(
            "{} m de fita de LED",
            num(perimeter(&inner) / 100.0)
        ));
    }
    let size = bounds_size(&outer, ceiling);
    Ok(Output {
        parts,
        size,
        hardware,
        notes,
        extra_cuts: Vec::new(),
        name: format!(
            "Sanca {} {} cm",
            match p.kind {
                CoveType::Open => "aberta",
                CoveType::Closed => "fechada",
                CoveType::Inverted => "invertida",
            },
            num(p.width)
        ),
    })
}

pub(crate) fn shadow_gap(p: &ShadowGapParams) -> Result<Output, String> {
    let mut notes: Vec<String> = Vec::new();
    if p.gap <= 0.0 {
        return Err("A tabica precisa de uma folga maior que zero (gap).".into());
    }
    if !(0.5..=5.0).contains(&p.gap) {
        notes.push(format!(
            "Tabica de {} cm fica fora do usual (0,5 a 5 cm).",
            num(p.gap)
        ));
    }
    let outer = p.pts.clone();
    let inner = check_room(&outer, p.gap)?;
    let mut parts = strips(
        "Tabica",
        &outer,
        &inner,
        p.ceiling - p.depth,
        p.depth,
        None,
        [34, 34, 36],
    );
    if p.led {
        let led = inset(&outer, p.gap / 2.0).ok_or("Contorno inválido.")?;
        let led_in = inset(&outer, p.gap / 2.0 + 0.3).ok_or("Contorno inválido.")?;
        parts.extend(strips(
            "Fita de LED",
            &led,
            &led_in,
            p.ceiling - p.depth,
            0.3,
            None,
            [255, 236, 180],
        ));
    }
    let mut hardware = vec![format!(
        "{} m de perfil tabica",
        num(perimeter(&outer) / 100.0)
    )];
    if p.led {
        hardware.push(format!(
            "{} m de fita de LED",
            num(perimeter(&outer) / 100.0)
        ));
    }
    Ok(Output {
        parts,
        size: bounds_size(&outer, p.ceiling),
        hardware,
        notes,
        extra_cuts: Vec::new(),
        name: format!("Tabica {} cm", num(p.gap)),
    })
}

/// Width, depth and height of the room box, cm; parts use the outline's own
/// coordinates, so the group spans from 0.
fn bounds_size(pts: &[[f64; 2]], ceiling: f64) -> [f64; 3] {
    let (_, hi) = pts
        .iter()
        .fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| {
            (
                [lo[0].min(p[0]), lo[1].min(p[1])],
                [hi[0].max(p[0]), hi[1].max(p[1])],
            )
        });
    [hi[0], hi[1], ceiling]
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)]

    use super::*;

    fn room() -> Vec<[f64; 2]> {
        vec![[0.0, 0.0], [400.0, 0.0], [400.0, 300.0], [0.0, 300.0]]
    }

    #[test]
    fn coves_follow_the_room_with_mitred_strips_and_led() {
        let out = cove(&CoveParams {
            pts: room(),
            ..CoveParams::default()
        })
        .unwrap();
        let plates: Vec<&Part> = out
            .parts
            .iter()
            .filter(|p| p.name.starts_with("Placa"))
            .collect();
        assert_eq!(plates.len(), 4);
        // The long wall's plate: 400 cm along the wall, 40 cm wide, at 260 − 15.
        let long = plates.iter().find(|p| p.cut.unwrap()[0] == 400.0).unwrap();
        assert!((long.cut.unwrap()[1] - 40.0).abs() < 1e-9);
        assert!((long.at[2] - 245.0).abs() < 1e-9);
        // Open cove: the lip stops 8 cm below the ceiling.
        let lip = out
            .parts
            .iter()
            .find(|p| p.name.starts_with("Espelho"))
            .unwrap();
        assert!((lip.at[2] + lip.size[2] - 252.0).abs() < 1e-9, "{lip:?}");
        // LED along the inner perimeter: (320 + 220) × 2 = 1080 cm.
        assert!(
            out.hardware.iter().any(|h| h == "10,8 m de fita de LED"),
            "{:?}",
            out.hardware
        );
        // Same result with the outline wound the other way.
        let mut reversed = room();
        reversed.reverse();
        let other = cove(&CoveParams {
            pts: reversed,
            ..CoveParams::default()
        })
        .unwrap();
        assert!(other.hardware.contains(&"10,8 m de fita de LED".to_owned()));

        assert!(
            cove(&CoveParams {
                pts: room(),
                width: 200.0,
                ..CoveParams::default()
            })
            .unwrap_err()
            .contains("reduza width")
        );
        // A slot wider than the drop shows the strip: built, and said.
        let showing = cove(&CoveParams {
            pts: room(),
            slot: 20.0,
            ..CoveParams::default()
        })
        .unwrap();
        assert!(
            showing.notes.iter().any(|n| n.contains("slot até 12")),
            "{:?}",
            showing.notes
        );
        let inverted = cove(&CoveParams {
            pts: room(),
            kind: CoveType::Inverted,
            ..CoveParams::default()
        })
        .unwrap();
        assert!(inverted.parts.iter().any(|p| p.name == "Forro rebaixado"));
        let closed = cove(&CoveParams {
            pts: room(),
            kind: CoveType::Closed,
            ..CoveParams::default()
        })
        .unwrap();
        assert!(closed.hardware.iter().all(|h| !h.contains("LED")));
    }

    #[test]
    fn shadow_gaps_run_around_the_walls() {
        let out = shadow_gap(&ShadowGapParams {
            pts: room(),
            led: true,
            ..ShadowGapParams::default()
        })
        .unwrap();
        assert_eq!(
            out.parts
                .iter()
                .filter(|p| p.name.starts_with("Tabica"))
                .count(),
            4
        );
        assert!(out.hardware.contains(&"14 m de perfil tabica".to_owned()));
        let wide = shadow_gap(&ShadowGapParams {
            pts: room(),
            gap: 10.0,
            ..ShadowGapParams::default()
        })
        .unwrap();
        assert!(
            wide.notes.iter().any(|n| n.contains("fora do usual")),
            "{:?}",
            wide.notes
        );
        assert!(
            shadow_gap(&ShadowGapParams::default())
                .unwrap_err()
                .contains("contorno")
        );
    }
}
