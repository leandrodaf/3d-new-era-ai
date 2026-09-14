//! Walls traced from a scanned plan: thick dark bands running across or down
//! the image become wall axes with their thickness. Good for orthogonal plans
//! with filled walls (raise the threshold for gray fills); thin lines (text,
//! furniture, dimensions) are left out by the thickness range.

// Image coordinates are far below 2^52.
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]

use image::GrayImage;

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
    walls.retain(|t| (t.b[0] - t.a[0]).hypot(t.b[1] - t.a[1]) >= options.min_length);
    walls
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
