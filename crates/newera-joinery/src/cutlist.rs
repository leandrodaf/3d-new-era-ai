//! Cut lists for the workshop: equal boards counted together, sizes in
//! millimeters, edge banding, and a DXF with the boards laid out on sheets
//! for a CNC or a panel saw.

use std::fmt::Write as _;

use crate::{Output, Part};

/// One line of a cut list.
#[derive(Debug, Clone, PartialEq)]
pub struct CutRow {
    pub name: String,
    pub board: String,
    pub qty: u32,
    /// Length, width and thickness, mm (length is the longest side).
    pub size: [f64; 3],
    /// Edges to band `[long, short]`.
    pub edge: [u8; 2],
}

fn mm(cm: f64) -> f64 {
    (cm * 10.0 * 10.0).round() / 10.0
}

fn row(part: &Part) -> Option<CutRow> {
    let board = part.board.clone()?;
    let mut dims = match part.cut {
        Some([length, width]) => [length, width, part.size[2]],
        None => part.size,
    };
    dims.sort_by(|a, b| b.total_cmp(a));
    Some(CutRow {
        name: part.name.clone(),
        board,
        qty: 1,
        size: dims.map(mm),
        edge: part.edge,
    })
}

/// Rows for every board of a build, equal boards merged (first name kept,
/// e.g. `Lateral esquerda` × 2).
pub fn cut_list(output: &Output) -> Vec<CutRow> {
    let mut rows: Vec<CutRow> = Vec::new();
    for part in output.parts.iter().chain(&output.extra_cuts) {
        let Some(new) = row(part) else { continue };
        match rows.iter_mut().find(|r| {
            r.board == new.board
                && r.edge == new.edge
                && r.size
                    .iter()
                    .zip(new.size)
                    .all(|(a, b)| (a - b).abs() < 0.05)
        }) {
            Some(existing) => existing.qty += 1,
            None => rows.push(new),
        }
    }
    rows.sort_by(|a, b| a.board.cmp(&b.board).then(b.size[0].total_cmp(&a.size[0])));
    rows
}

/// The cut list as CSV (semicolon separated, decimal comma), for spreadsheets.
pub fn cut_list_csv(rows: &[CutRow]) -> String {
    let mut out = String::from(
        "peca;material;qtd;comprimento_mm;largura_mm;espessura_mm;fita_longa;fita_curta\n",
    );
    let n = |v: f64| format!("{v}").replace('.', ",");
    for r in rows {
        let _ = writeln!(
            out,
            "{};{};{};{};{};{};{};{}",
            r.name.replace(';', ","),
            r.board,
            r.qty,
            n(r.size[0]),
            n(r.size[1]),
            n(r.size[2]),
            r.edge[0],
            r.edge[1]
        );
    }
    out
}

/// Sheet size for a board material, mm.
fn sheet(board: &str) -> [f64; 2] {
    if board.starts_with("Gesso") {
        [2400.0, 1200.0]
    } else if board.starts_with("Ripa") {
        [3000.0, 300.0]
    } else {
        [2750.0, 1830.0]
    }
}

/// A board placed on a sheet, mm from the drawing origin.
struct Placed {
    layer: String,
    label: String,
    rect: [f64; 4],
    sheet: bool,
}

/// Boards laid out on sheets (shelf packing with a 4 mm saw kerf): sheet
/// outlines, one rectangle per board, and how many sheets each material uses.
fn pack(rows: &[CutRow]) -> (Vec<Placed>, Vec<(String, u32)>) {
    const KERF: f64 = 4.0;
    let mut placed = Vec::new();
    let mut sheets_used: Vec<(String, u32)> = Vec::new();
    let mut origin_y = 0.0;
    let mut boards: Vec<&str> = rows.iter().map(|r| r.board.as_str()).collect();
    boards.dedup();
    for board in boards {
        let layer = board.replace([' ', ','], "_");
        let [sw, sh] = sheet(board);
        let outline = |x: f64| Placed {
            layer: layer.clone(),
            label: String::new(),
            rect: [x, origin_y, sw, sh],
            sheet: true,
        };
        let mut pieces: Vec<(String, f64, f64)> = rows
            .iter()
            .filter(|r| r.board == board)
            .flat_map(|r| (0..r.qty).map(move |_| (r.name.clone(), r.size[0], r.size[1])))
            .collect();
        pieces.sort_by(|a, b| b.2.total_cmp(&a.2).then(b.1.total_cmp(&a.1)));
        let mut sheet_index = 0u32;
        let (mut x, mut y, mut shelf) = (0.0, 0.0, 0.0);
        let mut sheet_x = 0.0;
        placed.push(outline(sheet_x));
        for (name, mut w, mut h) in pieces {
            // Turn pieces that only fit sideways.
            if w > sw && h <= sw && w <= sh {
                std::mem::swap(&mut w, &mut h);
            }
            if x + w > sw {
                x = 0.0;
                y += shelf + KERF;
                shelf = 0.0;
            }
            if y + h > sh {
                sheet_index += 1;
                sheet_x += sw + 200.0;
                x = 0.0;
                y = 0.0;
                shelf = 0.0;
                placed.push(outline(sheet_x));
            }
            placed.push(Placed {
                layer: layer.clone(),
                label: format!("{name} {w}x{h}"),
                rect: [sheet_x + x, origin_y + y, w.min(sw), h.min(sh)],
                sheet: false,
            });
            x += w + KERF;
            shelf = f64::max(shelf, h);
        }
        sheets_used.push((board.to_owned(), sheet_index + 1));
        origin_y += sh + 400.0;
    }
    (placed, sheets_used)
}

/// The sheet layout as a DXF (LINE and TEXT entities, one layer per
/// material) for a CNC or a panel saw, and the sheets each material uses.
pub fn cut_list_dxf(rows: &[CutRow]) -> (String, Vec<(String, u32)>) {
    let (placed, sheets) = pack(rows);
    let mut entities = String::new();
    for p in &placed {
        let [x, y, w, h] = p.rect;
        let corners = [[x, y], [x + w, y], [x + w, y + h], [x, y + h]];
        for i in 0..4 {
            let (a, b) = (corners[i], corners[(i + 1) % 4]);
            let _ = write!(
                entities,
                "0\nLINE\n8\n{}\n10\n{}\n20\n{}\n11\n{}\n21\n{}\n",
                p.layer, a[0], a[1], b[0], b[1]
            );
        }
        if !p.sheet {
            let _ = write!(
                entities,
                "0\nTEXT\n8\n{}\n10\n{}\n20\n{}\n40\n25\n1\n{}\n",
                p.layer,
                x + 10.0,
                y + 10.0,
                p.label
            );
        }
    }
    (
        format!("0\nSECTION\n2\nENTITIES\n{entities}0\nENDSEC\n0\nEOF\n"),
        sheets,
    )
}

/// The same sheet layout as an SVG (1 unit = 1 mm) to view or print.
pub fn cut_list_svg(rows: &[CutRow]) -> (String, Vec<(String, u32)>) {
    let (placed, sheets) = pack(rows);
    let (w, h) = placed.iter().fold((0.0_f64, 0.0_f64), |(w, h), p| {
        (w.max(p.rect[0] + p.rect[2]), h.max(p.rect[1] + p.rect[3]))
    });
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"-20 -20 {} {}\" font-family=\"sans-serif\">\n",
        w + 40.0,
        h + 40.0
    );
    for p in &placed {
        let [x, y, rw, rh] = p.rect;
        if p.sheet {
            let _ = writeln!(
                out,
                "<rect x=\"{x}\" y=\"{y}\" width=\"{rw}\" height=\"{rh}\" fill=\"#f4efe6\" stroke=\"#555\" stroke-width=\"4\"><title>{}</title></rect>",
                p.layer
            );
        } else {
            let label = p.label.replace('&', "&amp;").replace('<', "&lt;");
            let _ = writeln!(
                out,
                "<rect x=\"{x}\" y=\"{y}\" width=\"{rw}\" height=\"{rh}\" fill=\"#d9c3a0\" stroke=\"#333\" stroke-width=\"2\"/><text x=\"{}\" y=\"{}\" font-size=\"{}\">{label}</text>",
                x + 10.0,
                y + 35.0,
                (rh * 0.4).clamp(8.0, 30.0)
            );
        }
    }
    out.push_str("</svg>\n");
    (out, sheets)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)]

    use super::*;
    use crate::{Build, CabinetParams, generate};

    #[test]
    fn equal_boards_are_counted_and_laid_out_on_sheets() {
        let out = generate(&Build::Cabinet(CabinetParams {
            w: 80.0,
            h: 200.0,
            d: 55.0,
            ..CabinetParams::default()
        }))
        .unwrap();
        let rows = cut_list(&out);
        // Both sides are the same board: 1900 × 532 × 18 mm, banded on one long edge.
        let sides = rows.iter().find(|r| r.name == "Lateral esquerda").unwrap();
        assert_eq!(
            (sides.qty, sides.size, sides.edge),
            (2, [1900.0, 532.0, 18.0], [1, 0])
        );
        assert!(rows.iter().any(|r| r.board == "MDF 6"), "back panel listed");
        let csv = cut_list_csv(&rows);
        assert!(csv.starts_with("peca;material;qtd"));
        assert!(
            csv.contains("Lateral esquerda;MDF 18;2;1900;532;18;1;0"),
            "{csv}"
        );
        let (dxf, sheets) = cut_list_dxf(&rows);
        assert!(dxf.starts_with("0\nSECTION") && dxf.ends_with("EOF\n"));
        assert!(dxf.contains("Lateral esquerda 1900x532"));
        // Four boards near 1,9 m long don't share one 2750 × 1830 mm sheet.
        assert_eq!(sheets.iter().find(|(b, _)| b == "MDF 18").unwrap().1, 2);
        let (svg, same) = cut_list_svg(&rows);
        assert!(svg.starts_with("<svg") && svg.contains("Lateral esquerda 1900x532"));
        assert_eq!(same, sheets);
    }
}
