//! Slatted panels: the count of slats and the exact gap are solved so the
//! panel starts and ends with a whole slat, never a sliver.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Output, Part, cm, num};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    #[default]
    Vertical,
    Horizontal,
}

/// A slatted wall or furniture panel. Sizes in cm, slats in mm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct SlatsParams {
    /// Panel width, cm (default 120).
    pub w: f64,
    /// Panel height, cm (default 240).
    pub h: f64,
    /// Slat face width, mm (default 40).
    pub slat: f64,
    /// Slat thickness, mm (default 20).
    pub thickness: f64,
    /// Wanted gap between slats, mm (default 15); adjusted to fit exactly.
    pub gap: f64,
    pub orientation: Orientation,
    /// MDF backing board behind the slats (default true).
    pub backing: bool,
    /// Slat finish (default `wood`).
    pub finish: Option<String>,
}

impl Default for SlatsParams {
    fn default() -> Self {
        Self {
            w: 120.0,
            h: 240.0,
            slat: 40.0,
            thickness: 20.0,
            gap: 15.0,
            orientation: Orientation::Vertical,
            backing: true,
            finish: None,
        }
    }
}

/// How many slats and the exact gap (mm) to cover `length` mm edge to edge.
pub(crate) fn layout(length: f64, slat: f64, gap: f64) -> Result<(u32, f64), String> {
    if slat <= 0.0 || gap < 0.0 {
        return Err(
            "A ripa precisa de largura positiva e o espaçamento não pode ser negativo.".into(),
        );
    }
    if length < slat {
        return Err(format!(
            "O painel de {} cm é mais estreito que uma ripa de {} mm.",
            num(length / 10.0),
            num(slat)
        ));
    }
    let ideal = ((length + gap) / (slat + gap)).round().max(1.0);
    let candidates = [ideal - 1.0, ideal, ideal + 1.0];
    let (n, actual) = candidates
        .into_iter()
        .filter(|n| *n >= 1.0)
        .map(|n| {
            let actual = if n > 1.0 {
                (length - n * slat) / (n - 1.0)
            } else {
                0.0
            };
            (n, actual)
        })
        .filter(|(_, actual)| *actual >= 0.0)
        .min_by(|a, b| (a.1 - gap).abs().total_cmp(&(b.1 - gap).abs()))
        .ok_or("Não há distribuição de ripas possível nesta medida.")?;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Ok((n as u32, actual))
}

pub(crate) fn generate(p: &SlatsParams) -> Result<Output, String> {
    if p.w <= 0.0 || p.h <= 0.0 {
        return Err("O painel precisa de largura e altura positivas.".into());
    }
    let vertical = p.orientation == Orientation::Vertical;
    let length_mm = if vertical { p.w } else { p.h } * 10.0;
    let (n, gap) = layout(length_mm, p.slat, p.gap)?;
    let (slat, gap_cm) = (cm(p.slat), cm(gap));
    let depth = cm(p.thickness);
    let backing = if p.backing { 1.5 } else { 0.0 };
    let finish = p
        .finish
        .as_deref()
        .unwrap_or("wood")
        .parse::<newera_core::Material>()
        .ok();
    let mut parts = Vec::new();
    if p.backing {
        parts.push(Part::board(
            "Painel de fundo",
            [0.0, 0.0, 0.0],
            [p.w, backing, p.h],
            "MDF 15",
            [40, 40, 42],
        ));
    }
    for k in 0..n {
        let offset = f64::from(k) * (slat + gap_cm);
        let (at, size) = if vertical {
            ([offset, backing, 0.0], [slat, depth, p.h])
        } else {
            ([0.0, backing, offset], [p.w, depth, slat])
        };
        let mut part = Part::board(
            &format!("Ripa {}", k + 1),
            at,
            size,
            &format!("Ripa {}×{} mm", num(p.slat), num(p.thickness)),
            [168, 124, 84],
        );
        part.finish.clone_from(&finish);
        parts.push(part);
    }
    let mut notes = Vec::new();
    if (gap - p.gap).abs() > 0.05 {
        notes.push(format!(
            "Espaçamento ajustado de {} para {} mm para fechar {} cm com {n} ripas inteiras.",
            num(p.gap),
            num(gap),
            num(length_mm / 10.0)
        ));
    }
    let span = if vertical { p.h } else { p.w };
    Ok(Output {
        parts,
        size: [p.w, backing + depth, p.h],
        hardware: vec![format!(
            "{} m lineares de ripa {}×{} mm",
            num(f64::from(n) * span / 100.0),
            num(p.slat),
            num(p.thickness)
        )],
        notes,
        extra_cuts: Vec::new(),
        name: format!("Painel ripado {} × {} cm", num(p.w), num(p.h)),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)]

    use super::*;

    #[test]
    fn slats_cover_the_panel_exactly() {
        // 1200 mm with 40 mm slats and ~15 mm gaps: 22 slats, gap (1200 − 880)/21.
        let (n, gap) = layout(1200.0, 40.0, 15.0).unwrap();
        assert_eq!(n, 22);
        assert!((gap - 320.0 / 21.0).abs() < 1e-9);
        let out = generate(&SlatsParams::default()).unwrap();
        let slats: Vec<&Part> = out
            .parts
            .iter()
            .filter(|p| p.name.starts_with("Ripa"))
            .collect();
        assert_eq!(slats.len(), 22);
        // First slat at the left edge, last one flush with the right edge.
        let last = slats.last().unwrap();
        assert!(slats[0].at[0].abs() < 1e-9);
        assert!((last.at[0] + last.size[0] - 120.0).abs() < 1e-9, "{last:?}");
        assert!(
            out.notes.iter().any(|n| n.contains("15,2 mm")),
            "{:?}",
            out.notes
        );
        // Horizontal slats stack up the height.
        let out = generate(&SlatsParams {
            orientation: Orientation::Horizontal,
            h: 100.0,
            ..SlatsParams::default()
        })
        .unwrap();
        let top = out
            .parts
            .iter()
            .rfind(|p| p.name.starts_with("Ripa"))
            .unwrap();
        assert!((top.at[2] + top.size[2] - 100.0).abs() < 1e-9);
        assert!(
            generate(&SlatsParams {
                w: 3.0,
                ..SlatsParams::default()
            })
            .is_err()
        );
    }
}
