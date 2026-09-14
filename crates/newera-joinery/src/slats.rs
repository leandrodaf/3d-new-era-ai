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
    /// Top following a roof: points `[x from the left, height]` cm; empty is flat at `h`.
    pub top: Vec<[f64; 2]>,
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
            top: Vec::new(),
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
    // Height available at x: the roof line when there is one, never above h.
    let top_at = |x: f64| -> f64 {
        if p.top.len() < 2 {
            return p.h;
        }
        let pts = &p.top;
        let x = x.clamp(pts[0][0], pts[pts.len() - 1][0]);
        let seg = pts
            .windows(2)
            .find(|s| x >= s[0][0] && x <= s[1][0])
            .unwrap_or(&pts[..2]);
        let t = (x - seg[0][0]) / (seg[1][0] - seg[0][0]).max(1e-9);
        (seg[0][1] + (seg[1][1] - seg[0][1]) * t).min(p.h)
    };
    let lowest_over = |x0: f64, x1: f64| {
        let mut low = top_at(x0).min(top_at(x1));
        for c in &p.top {
            if c[0] > x0 && c[0] < x1 {
                low = low.min(c[1].min(p.h));
            }
        }
        low
    };
    let slat_board = format!("Ripa {}×{} mm", num(p.slat), num(p.thickness));
    let mut parts = Vec::new();
    if p.backing {
        let mut back = Part::board(
            "Painel de fundo",
            [0.0, 0.0, 0.0],
            [p.w, backing, p.h],
            "MDF 15",
            [40, 40, 42],
        );
        if p.top.len() >= 2 {
            let mut ring = vec![[0.0, 0.0], [p.w, 0.0], [p.w, top_at(p.w)]];
            ring.extend(
                p.top
                    .iter()
                    .rev()
                    .filter(|c| c[0] > 0.0 && c[0] < p.w)
                    .map(|c| [c[0], c[1].min(p.h)]),
            );
            ring.push([0.0, top_at(0.0)]);
            back.profile = Some(ring);
        }
        parts.push(back);
    }
    for k in 0..n {
        let offset = f64::from(k) * (slat + gap_cm);
        if vertical {
            let height = lowest_over(offset, offset + slat);
            if height < 5.0 {
                continue;
            }
            let mut part = Part::board(
                &format!("Ripa {}", k + 1),
                [offset, backing, 0.0],
                [slat, depth, height],
                &slat_board,
                [168, 124, 84],
            );
            part.finish.clone_from(&finish);
            parts.push(part);
        } else {
            // A horizontal slat runs where the roof is above its top edge.
            let needed = offset + slat;
            let mut x = 0.0;
            let mut piece = 0;
            while x < p.w {
                while x < p.w && top_at(x) < needed {
                    x += 1.0;
                }
                let start = x;
                while x < p.w && top_at(x) >= needed {
                    x += 1.0;
                }
                let end = x.min(p.w);
                if end - start >= 5.0 {
                    piece += 1;
                    let mut part = Part::board(
                        &format!(
                            "Ripa {}{}",
                            k + 1,
                            if piece > 1 {
                                format!(".{piece}")
                            } else {
                                String::new()
                            }
                        ),
                        [start, backing, offset],
                        [end - start, depth, slat],
                        &slat_board,
                        [168, 124, 84],
                    );
                    part.finish.clone_from(&finish);
                    parts.push(part);
                }
            }
        }
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
        // Under a roof rising to the right, slats get taller toward it and the
        // backing is cut along the slope.
        let sloped = generate(&SlatsParams {
            w: 120.0,
            h: 240.0,
            top: vec![[0.0, 150.0], [120.0, 230.0]],
            ..SlatsParams::default()
        })
        .unwrap();
        let slats: Vec<&Part> = sloped
            .parts
            .iter()
            .filter(|p| p.name.starts_with("Ripa"))
            .collect();
        assert!(slats[0].size[2] < slats[slats.len() - 1].size[2]);
        assert!(
            (slats[0].size[2] - 150.0).abs() < 1e-9,
            "the low edge of the first slat"
        );
        let back = sloped
            .parts
            .iter()
            .find(|p| p.name == "Painel de fundo")
            .unwrap();
        assert!(back.profile.as_ref().is_some_and(|r| r.len() >= 4));
        assert!(
            generate(&SlatsParams {
                w: 3.0,
                ..SlatsParams::default()
            })
            .is_err()
        );
    }
}
