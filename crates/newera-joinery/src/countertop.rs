//! Countertops and workbenches with cutouts for sinks and cooktops, cable
//! grommets, and legs or brackets.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Output, Part, num};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Support {
    /// Sits on cabinets or walls.
    #[default]
    None,
    Legs,
    /// Wall brackets (mãos-francesas).
    Brackets,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CutoutKind {
    Sink,
    Cooktop,
    /// Round cable pass-through, `w` is the diameter.
    Grommet,
}

/// A hole in the top. `x` is the center along the length from the left end.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Cutout {
    pub kind: CutoutKind,
    pub x: f64,
    /// Width along the length, cm (defaults: sink 50, cooktop 56, grommet 6).
    pub w: Option<f64>,
    /// Depth, cm (defaults: sink 40, cooktop 48, grommet 6).
    pub d: Option<f64>,
}

/// A countertop. Sizes in cm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct CountertopParams {
    /// Length, cm (default 180).
    pub length: f64,
    /// Depth, cm (default 60).
    pub depth: f64,
    /// Top surface height from the floor, cm (default 90).
    pub height: f64,
    /// Slab thickness, cm (default 3).
    pub thickness: f64,
    /// Finish, e.g. `marble #222222 60` (default `stone`).
    pub material: Option<String>,
    pub support: Support,
    pub cutouts: Vec<Cutout>,
}

impl Default for CountertopParams {
    fn default() -> Self {
        Self {
            length: 180.0,
            depth: 60.0,
            height: 90.0,
            thickness: 3.0,
            material: None,
            support: Support::None,
            cutouts: Vec::new(),
        }
    }
}

/// Stone left around a cutout, cm.
const MARGIN: f64 = 5.0;

#[allow(clippy::too_many_lines)]
pub(crate) fn generate(p: &CountertopParams) -> Result<Output, String> {
    let (len, dep, t) = (p.length, p.depth, p.thickness);
    if len < 20.0 || dep < 20.0 || !(1.0..=15.0).contains(&t) {
        return Err("A bancada precisa de ao menos 20 × 20 cm e espessura entre 1 e 15 cm.".into());
    }
    if p.height < t {
        return Err(format!(
            "A altura {} cm é menor que a espessura do tampo.",
            num(p.height)
        ));
    }
    let finish: newera_core::Material = p
        .material
        .as_deref()
        .unwrap_or("stone")
        .parse()
        .map_err(|e| format!("Material inválido: {e}"))?;
    let z = p.height - t;
    // Holes as rectangles (x0, x1, y0, y1), y from the back.
    let mut holes: Vec<(f64, f64, f64, f64, CutoutKind)> = Vec::new();
    for c in &p.cutouts {
        let (w, d) = match c.kind {
            CutoutKind::Sink => (c.w.unwrap_or(50.0), c.d.unwrap_or(40.0)),
            CutoutKind::Cooktop => (c.w.unwrap_or(56.0), c.d.unwrap_or(48.0)),
            CutoutKind::Grommet => (c.w.unwrap_or(6.0), c.d.unwrap_or(6.0)),
        };
        let label = match c.kind {
            CutoutKind::Sink => "a cuba",
            CutoutKind::Cooktop => "o cooktop",
            CutoutKind::Grommet => "o passa-cabos",
        };
        if dep < d + 2.0 * MARGIN {
            return Err(format!(
                "A bancada tem {} cm de profundidade, mas {label} de {} cm precisa de {} cm de pedra em volta: use depth = {}.",
                num(dep),
                num(d),
                num(MARGIN),
                num(d + 2.0 * MARGIN)
            ));
        }
        let (x0, x1) = (c.x - w / 2.0, c.x + w / 2.0);
        if x0 < MARGIN || x1 > len - MARGIN {
            return Err(format!(
                "O recorte d{} em x = {} cm sai da bancada ou fica a menos de {} cm da ponta; use x entre {} e {}.",
                &label[..1],
                num(c.x),
                num(MARGIN),
                num(MARGIN + w / 2.0),
                num(len - MARGIN - w / 2.0)
            ));
        }
        // Centered in the depth, a little toward the front for sinks.
        let y0 = (dep - d) / 2.0;
        if let Some(other) = holes
            .iter()
            .find(|h| x0 < h.1 + MARGIN && h.0 < x1 + MARGIN)
        {
            return Err(format!(
                "Os recortes em x = {} e x = {} cm ficam a menos de {} cm um do outro; afaste-os.",
                num(c.x),
                num(f64::midpoint(other.0, other.1)),
                num(MARGIN)
            ));
        }
        holes.push((x0, x1, y0, y0 + d, c.kind));
    }
    holes.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut parts = Vec::new();
    let slab = |name: String, x0: f64, x1: f64, y0: f64, y1: f64, parts: &mut Vec<Part>| {
        if x1 - x0 > 0.01 && y1 - y0 > 0.01 {
            let mut part = Part::board(
                &name,
                [x0, y0, z],
                [x1 - x0, y1 - y0, t],
                &format!("Tampo {} cm", num(t)),
                [90, 90, 95],
            );
            part.finish = Some(finish.clone());
            parts.push(part);
        }
    };
    let mut cursor = 0.0;
    for (i, &(x0, x1, y0, y1, _)) in holes.iter().enumerate() {
        slab(
            format!("Tampo {}", parts.len() + 1),
            cursor,
            x0,
            0.0,
            dep,
            &mut parts,
        );
        slab(
            format!("Tampo atrás do recorte {}", i + 1),
            x0,
            x1,
            0.0,
            y0,
            &mut parts,
        );
        slab(
            format!("Tampo à frente do recorte {}", i + 1),
            x0,
            x1,
            y1,
            dep,
            &mut parts,
        );
        cursor = x1;
    }
    slab(
        format!("Tampo {}", parts.len() + 1),
        cursor,
        len,
        0.0,
        dep,
        &mut parts,
    );
    // Pieces of one slab, not separate stones: the cut list counts the whole top.
    for part in &mut parts {
        part.board = None;
    }

    let mut hardware = Vec::new();
    for (x0, x1, y0, y1, kind) in &holes {
        let what = match kind {
            CutoutKind::Sink => "cuba",
            CutoutKind::Cooktop => "cooktop",
            CutoutKind::Grommet => "passa-cabos",
        };
        hardware.push(format!(
            "recorte para {what} {} × {} cm a {} cm da ponta esquerda",
            num(x1 - x0),
            num(y1 - y0),
            num(*x0)
        ));
    }
    match p.support {
        Support::Legs => {
            let n = (len / 120.0).ceil().max(1.0) as u32 + 1;
            for k in 0..n {
                let x = 5.0 + (len - 15.0) * f64::from(k) / f64::from(n - 1);
                for y in [5.0, dep - 10.0] {
                    parts.push(Part::solid("Pé", [x, y, 0.0], [5.0, 5.0, z], [60, 60, 64]));
                }
            }
            hardware.push(format!("{} pés metálicos de {} cm", n * 2, num(z)));
        }
        Support::Brackets => {
            let n = (len / 80.0).ceil().max(1.0) as u32 + 1;
            for k in 0..n {
                let x = 5.0 + (len - 13.0) * f64::from(k) / f64::from(n - 1);
                parts.push(Part::solid(
                    "Mão-francesa",
                    [x, 0.0, z - (dep * 0.6)],
                    [3.0, dep * 0.7, dep * 0.6],
                    [60, 60, 64],
                ));
            }
            hardware.push(format!("{n} mãos-francesas de {} cm", num(dep * 0.7)));
        }
        Support::None => {}
    }
    Ok(Output {
        parts,
        size: [len, dep, p.height],
        hardware,
        notes: Vec::new(),
        extra_cuts: vec![
            Part::board(
                "Tampo com recortes",
                [0.0, 0.0, z],
                [len, dep, t],
                &format!("Pedra {} cm", num(t)),
                [90, 90, 95],
            )
            .banded(1, 2),
        ],
        name: format!("Bancada {} × {} cm", num(len), num(dep)),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)]

    use super::*;

    #[test]
    fn cutouts_leave_the_slab_around_them_and_explain_limits() {
        let out = generate(&CountertopParams {
            length: 200.0,
            cutouts: vec![
                Cutout {
                    kind: CutoutKind::Sink,
                    x: 50.0,
                    w: None,
                    d: None,
                },
                Cutout {
                    kind: CutoutKind::Cooktop,
                    x: 140.0,
                    w: None,
                    d: None,
                },
            ],
            support: Support::Legs,
            ..CountertopParams::default()
        })
        .unwrap();
        // Top pieces cover length × depth minus both holes.
        let area: f64 = out
            .parts
            .iter()
            .filter(|p| p.name.starts_with("Tampo"))
            .map(|p| p.size[0] * p.size[1])
            .sum();
        assert!(
            (area - (200.0 * 60.0 - 50.0 * 40.0 - 56.0 * 48.0)).abs() < 1e-6,
            "{area}"
        );
        assert!(
            out.hardware
                .iter()
                .any(|h| h.starts_with("recorte para cooktop 56 × 48 cm")),
            "{:?}",
            out.hardware
        );
        assert!(out.parts.iter().any(|p| p.name == "Pé"));
        let shallow = generate(&CountertopParams {
            depth: 50.0,
            cutouts: vec![Cutout {
                kind: CutoutKind::Cooktop,
                x: 90.0,
                w: None,
                d: None,
            }],
            ..CountertopParams::default()
        })
        .unwrap_err();
        assert!(shallow.contains("use depth = 58"), "{shallow}");
        let edge = generate(&CountertopParams {
            cutouts: vec![Cutout {
                kind: CutoutKind::Sink,
                x: 10.0,
                w: None,
                d: None,
            }],
            ..CountertopParams::default()
        })
        .unwrap_err();
        assert!(edge.contains("use x entre 30 e 150"), "{edge}");
    }
}
