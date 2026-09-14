//! Walls traced from a scanned plan: thick dark bands running across or down
//! the image become wall axes with their thickness. Good for orthogonal plans
//! with filled walls (raise the threshold for gray fills); thin lines (text,
//! furniture, dimensions) are left out by the thickness range.

// Image coordinates are far below 2^52.
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]

use image::{GrayImage, Luma, RgbImage};

/// A wall found in the image, in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Traced {
    pub a: [f64; 2],
    pub b: [f64; 2],
    /// Band thickness, px.
    pub thickness: f64,
}

/// Tuning, in pixels.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TraceOptions {
    /// Luminance below this is ink.
    pub threshold: u8,
    pub min_length: f64,
    pub min_thickness: f64,
    pub max_thickness: f64,
    /// Collinear pieces this close (door and window gaps) become one wall.
    pub max_gap: f64,
}

/// Ink of a colored plan: dark and gray, not saturated. Humanized plans paint
/// lawns, plants, furniture and cars in color; walls are black or gray.
pub(crate) fn ink_mask(image: &RgbImage, threshold: u8) -> GrayImage {
    GrayImage::from_fn(image.width(), image.height(), |x, y| {
        let [r, g, b] = image.get_pixel(x, y).0;
        let (max, min) = (r.max(g).max(b), r.min(g).min(b));
        let luma = (u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114) / 1000;
        // Saturation as in HSV: spread over the brightest channel.
        let saturated = max > 0 && f32::from(max - min) / f32::from(max) > 0.28 && max - min > 30;
        Luma([if luma < u32::from(threshold) && !saturated {
            0
        } else {
            255
        }])
    })
}

#[derive(Debug, Clone, Copy)]
struct Band {
    /// Span along the wall on the line where it started.
    start: f64,
    end: f64,
    first: usize,
    last: usize,
}

/// Ink runs of at least `min` pixels along one line.
fn runs(line: impl Iterator<Item = bool>, min: f64) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    let mut open: Option<usize> = None;
    let mut n = 0;
    for (i, ink) in line.enumerate() {
        n = i + 1;
        match (ink, open) {
            (true, None) => open = Some(i),
            (false, Some(s)) => {
                if (i - s) as f64 >= min {
                    out.push((s as f64, i as f64));
                }
                open = None;
            }
            _ => {}
        }
    }
    if let Some(s) = open
        && (n - s) as f64 >= min
    {
        out.push((s as f64, n as f64));
    }
    out
}

/// Bands of runs that repeat on consecutive lines (`lines` long, each `len`).
fn bands(
    ink: &dyn Fn(usize, usize) -> bool,
    lines: usize,
    len: usize,
    options: &TraceOptions,
) -> Vec<Band> {
    let mut done = Vec::new();
    let mut active: Vec<Band> = Vec::new();
    for line in 0..lines {
        let found = runs((0..len).map(|i| ink(line, i)), options.min_length);
        let mut next = Vec::with_capacity(active.len());
        let mut used = vec![false; found.len()];
        for band in active.drain(..) {
            // Continues where a run covers most of the band's span.
            let hit = found.iter().enumerate().find(|(k, (s, e))| {
                !used[*k] && {
                    let overlap = e.min(band.end) - s.max(band.start);
                    overlap > 0.8 * (band.end - band.start).min(e - s)
                }
            });
            match hit {
                Some((k, _)) => {
                    used[k] = true;
                    next.push(Band { last: line, ..band });
                }
                None => done.push(band),
            }
        }
        for (k, (s, e)) in found.into_iter().enumerate() {
            if !used[k] {
                next.push(Band {
                    start: s,
                    end: e,
                    first: line,
                    last: line,
                });
            }
        }
        active = next;
    }
    done.extend(active);
    done.retain(|b| {
        let t = (b.last - b.first + 1) as f64;
        t >= options.min_thickness && t <= options.max_thickness
    });
    done
}

/// Union-find root of `i`.
fn root(group: &mut [usize], i: usize) -> usize {
    let mut i = i;
    while group[i] != i {
        group[i] = group[group[i]];
        i = group[i];
    }
    i
}

/// Horizontal and vertical walls, ends snapped onto the axis of the walls
/// they run into.
#[allow(clippy::cast_precision_loss)]
pub(crate) fn trace(image: &GrayImage, options: &TraceOptions) -> Vec<Traced> {
    let (w, h) = (image.width() as usize, image.height() as usize);
    let dark = |x: usize, y: usize| image.get_pixel(x as u32, y as u32).0[0] < options.threshold;
    let across = bands(&|row, col| dark(col, row), h, w, options);
    let down = bands(&|col, row| dark(col, row), w, h, options);
    let axis = |b: &Band| (b.first + b.last + 1) as f64 / 2.0;
    let thick = |b: &Band| (b.last - b.first + 1) as f64;
    let mut walls: Vec<Traced> = Vec::new();
    for b in &across {
        let y = axis(b);
        let (mut x0, mut x1) = (b.start, b.end);
        for v in &down {
            let (vx, reach) = (axis(v), thick(v) / 2.0 + 2.0);
            let covers = v.start - 2.0 <= y && y <= v.end + 2.0;
            if covers && (x0 - vx).abs() <= reach {
                x0 = vx;
            }
            if covers && (x1 - vx).abs() <= reach {
                x1 = vx;
            }
        }
        walls.push(Traced {
            a: [x0, y],
            b: [x1, y],
            thickness: thick(b),
        });
    }
    for v in &down {
        let x = axis(v);
        let (mut y0, mut y1) = (v.start, v.end);
        for b in &across {
            let (by, reach) = (axis(b), thick(b) / 2.0 + 2.0);
            let covers = b.start - 2.0 <= x && x <= b.end + 2.0;
            if covers && (y0 - by).abs() <= reach {
                y0 = by;
            }
            if covers && (y1 - by).abs() <= reach {
                y1 = by;
            }
        }
        walls.push(Traced {
            a: [x, y0],
            b: [x, y1],
            thickness: thick(v),
        });
    }
    // Pieces of one wall interrupted by doors and windows join up.
    let mut merged: Vec<Traced> = Vec::new();
    walls.sort_by(|p, q| {
        let key = |t: &Traced| {
            let horizontal = (t.a[1] - t.b[1]).abs() < (t.a[0] - t.b[0]).abs();
            (
                horizontal,
                if horizontal { t.a[1] } else { t.a[0] },
                t.a[0].min(t.b[0]) + t.a[1].min(t.b[1]),
            )
        };
        let (a, b) = (key(p), key(q));
        a.0.cmp(&b.0)
            .then(a.1.total_cmp(&b.1))
            .then(a.2.total_cmp(&b.2))
    });
    for wall in walls {
        let horizontal = (wall.a[1] - wall.b[1]).abs() < (wall.a[0] - wall.b[0]).abs();
        let span = |t: &Traced| {
            if horizontal {
                (
                    t.a[0].min(t.b[0]),
                    t.a[0].max(t.b[0]),
                    f64::midpoint(t.a[1], t.b[1]),
                )
            } else {
                (
                    t.a[1].min(t.b[1]),
                    t.a[1].max(t.b[1]),
                    f64::midpoint(t.a[0], t.b[0]),
                )
            }
        };
        let (s0, s1, axis) = span(&wall);
        let joined = merged.iter_mut().rev().find(|m| {
            let m_horizontal = (m.a[1] - m.b[1]).abs() < (m.a[0] - m.b[0]).abs();
            let (m0, m1, m_axis) = span(m);
            m_horizontal == horizontal
                && (m_axis - axis).abs() <= m.thickness.max(wall.thickness) / 2.0
                && (m.thickness - wall.thickness).abs() <= 0.35 * m.thickness.max(wall.thickness)
                && s0 <= m1 + options.max_gap
                && s1 >= m0 - options.max_gap
        });
        match joined {
            Some(m) => {
                let (m0, m1, m_axis) = span(m);
                let (lo, hi) = (m0.min(s0), m1.max(s1));
                let axis = f64::midpoint(m_axis, axis);
                if horizontal {
                    m.a = [lo, axis];
                    m.b = [hi, axis];
                } else {
                    m.a = [axis, lo];
                    m.b = [axis, hi];
                }
                m.thickness = m.thickness.max(wall.thickness);
            }
            None => merged.push(wall),
        }
    }
    merged.retain(|t| (t.b[0] - t.a[0]).hypot(t.b[1] - t.a[1]) >= options.min_length);
    // Walls meet other walls. Short dark pieces touching nothing (a car's
    // windows, a table, a shadow) are drawings, not walls.
    let length = |t: &Traced| (t.b[0] - t.a[0]).hypot(t.b[1] - t.a[1]);
    let touches = |t: &Traced, others: &[Traced]| {
        let near = |p: [f64; 2], o: &Traced| {
            // Distance from p to the other wall's axis segment.
            let (dx, dy) = (o.b[0] - o.a[0], o.b[1] - o.a[1]);
            let len2 = (dx * dx + dy * dy).max(1e-9);
            let k = (((p[0] - o.a[0]) * dx + (p[1] - o.a[1]) * dy) / len2).clamp(0.0, 1.0);
            (p[0] - (o.a[0] + dx * k)).hypot(p[1] - (o.a[1] + dy * k))
                <= o.thickness.max(t.thickness) / 2.0 + 4.0
        };
        others
            .iter()
            .filter(|o| *o != t)
            .any(|o| near(t.a, o) || near(t.b, o) || near(o.a, t) || near(o.b, t))
    };
    for _ in 0..2 {
        let snapshot = merged.clone();
        merged.retain(|t| length(t) >= 3.0 * options.min_length || touches(t, &snapshot));
    }
    // A house is one connected set of walls: small clusters apart from it
    // (shadows of plants, a set of chairs in the garden) go too.
    let n = merged.len();
    let mut group: Vec<usize> = (0..n).collect();
    for i in 0..n {
        for j in i + 1..n {
            if touches(&merged[i], std::slice::from_ref(&merged[j])) {
                let (a, b) = (root(&mut group, i), root(&mut group, j));
                group[a] = b;
            }
        }
    }
    let mut totals = std::collections::HashMap::new();
    for (i, wall) in merged.iter().enumerate() {
        *totals.entry(root(&mut group, i)).or_insert(0.0) += length(wall);
    }
    let biggest = totals.values().copied().fold(0.0, f64::max);
    let keep: Vec<bool> = (0..n)
        .map(|i| totals[&root(&mut group, i)] >= 0.2 * biggest)
        .collect();
    merged
        .into_iter()
        .zip(keep)
        .filter_map(|(t, k)| k.then_some(t))
        .collect()
}

#[cfg(test)]
mod tests {
    use image::Luma;

    use super::*;

    fn fill(image: &mut GrayImage, x0: u32, y0: u32, x1: u32, y1: u32) {
        for y in y0..y1 {
            for x in x0..x1 {
                image.put_pixel(x, y, Luma([20]));
            }
        }
    }

    #[test]
    fn colored_blobs_are_not_walls_and_openings_do_not_split_them() {
        use image::Rgb;
        let mut image = RgbImage::from_pixel(420, 200, Rgb([245, 245, 245]));
        let mut paint = |x0: u32, y0: u32, x1: u32, y1: u32, c: [u8; 3]| {
            for y in y0..y1 {
                for x in x0..x1 {
                    image.put_pixel(x, y, Rgb(c));
                }
            }
        };
        // A wall with a 60 px window gap, a dark green hedge as thick as a wall,
        // and a short dark block standing alone (a car's windshield).
        paint(10, 20, 150, 32, [30, 30, 30]);
        paint(210, 20, 410, 32, [30, 30, 30]);
        paint(10, 120, 300, 132, [40, 110, 50]);
        paint(150, 160, 210, 172, [25, 25, 25]);
        let mask = ink_mask(&image, 128);
        let walls = trace(
            &mask,
            &TraceOptions {
                threshold: 128,
                min_length: 40.0,
                min_thickness: 5.0,
                max_thickness: 30.0,
                max_gap: 80.0,
            },
        );
        assert_eq!(walls.len(), 1, "{walls:?}");
        assert!(
            (walls[0].a[0] - 10.0).abs() < 0.5 && (walls[0].b[0] - 410.0).abs() < 0.5,
            "{walls:?}"
        );
    }

    #[test]
    fn traces_a_two_room_plan_and_skips_thin_lines() {
        // 400 × 300 px, walls 12 px thick, a partition at x = 200 and a
        // 2 px dimension line that must be ignored.
        let mut image = GrayImage::from_pixel(420, 320, Luma([245]));
        fill(&mut image, 10, 10, 410, 22);
        fill(&mut image, 10, 298, 410, 310);
        fill(&mut image, 10, 10, 22, 310);
        fill(&mut image, 398, 10, 410, 310);
        fill(&mut image, 194, 22, 206, 298);
        fill(&mut image, 30, 150, 380, 152);
        let walls = trace(
            &image,
            &TraceOptions {
                threshold: 128,
                min_length: 40.0,
                min_thickness: 5.0,
                max_thickness: 30.0,
                max_gap: 0.0,
            },
        );
        assert_eq!(walls.len(), 5, "{walls:?}");
        assert!(walls.iter().all(|w| (w.thickness - 12.0).abs() < 0.5));
        // The top wall runs axis to axis: x 16 → 404 at y 16.
        let top = walls
            .iter()
            .find(|w| (w.a[1] - 16.0).abs() < 0.5 && (w.b[1] - 16.0).abs() < 0.5)
            .unwrap();
        assert!(
            (top.a[0] - 16.0).abs() < 0.5 && (top.b[0] - 404.0).abs() < 0.5,
            "{top:?}"
        );
        // The partition meets the outer walls' axes.
        let partition = walls.iter().find(|w| (w.a[0] - 200.0).abs() < 0.5).unwrap();
        assert!(
            (partition.a[1] - 16.0).abs() < 0.5 && (partition.b[1] - 304.0).abs() < 0.5,
            "{partition:?}"
        );
    }
}
