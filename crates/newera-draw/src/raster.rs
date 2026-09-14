//! CPU rasterization of a plan [`Scene`] to PNG — used by the MCP
//! `render_plan` tool so agents can see their work, and by PNG export.

use std::collections::HashMap;

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use newera_core::Point2;
use tiny_skia::{
    FillRule, FilterQuality, Paint, PathBuilder, Pixmap, PixmapPaint, Stroke, Transform,
};

use crate::scene::{Align, Color, Palette, Primitive, Scene, Size, TextLook};

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("invalid image size {0}x{1}")]
    InvalidSize(u32, u32),
    #[error("png encoding failed: {0}")]
    Encode(String),
}

/// Plan-to-pixel mapping that fits a region into an image.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    /// Pixels per centimeter.
    pub scale: f64,
    /// Pixel position of plan (0, 0).
    pub origin: (f64, f64),
}

impl Viewport {
    /// Fits `min..max` into `width`×`height` pixels with a margin, keeping aspect.
    pub fn fit(min: Point2, max: Point2, width: u32, height: u32, margin: f64) -> Self {
        let w = (max.x - min.x).max(1.0);
        let h = (max.y - min.y).max(1.0);
        let avail_w = (f64::from(width) - 2.0 * margin).max(1.0);
        let avail_h = (f64::from(height) - 2.0 * margin).max(1.0);
        let scale = (avail_w / w).min(avail_h / h);
        let origin = (
            f64::from(width) / 2.0 - (min.x + w / 2.0) * scale,
            f64::from(height) / 2.0 - (min.y + h / 2.0) * scale,
        );
        Self { scale, origin }
    }

    pub fn to_px(self, p: Point2) -> (f32, f32) {
        #[allow(clippy::cast_possible_truncation)]
        (
            (self.origin.0 + p.x * self.scale) as f32,
            (self.origin.1 + p.y * self.scale) as f32,
        )
    }

    pub fn size_px(self, size: Size) -> f32 {
        #[allow(clippy::cast_possible_truncation)]
        match size {
            Size::Px(px) => px,
            Size::Cm(cm) => (cm * self.scale) as f32,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RenderOptions {
    pub width: u32,
    pub height: u32,
    pub margin_px: f64,
    pub grid: bool,
    pub palette: Palette,
    /// Region to show; defaults to the scene bounds.
    pub region: Option<(Point2, Point2)>,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            width: 768,
            height: 512,
            margin_px: 24.0,
            grid: true,
            palette: Palette::default(),
            region: None,
        }
    }
}

/// Renders the scene and encodes it as PNG. `load_image` resolves background
/// image paths to decoded RGBA images (return `None` to skip).
pub fn render_png(
    scene: &Scene,
    options: &RenderOptions,
    load_image: &dyn Fn(&str) -> Option<image::RgbaImage>,
) -> Result<Vec<u8>, RenderError> {
    let pixmap = render_pixmap(scene, options, load_image)?;
    pixmap
        .encode_png()
        .map_err(|e| RenderError::Encode(e.to_string()))
}

/// Renders the scene to a raw RGBA pixmap.
///
/// # Panics
///
/// Never in practice: the bundled font is always valid.
pub fn render_pixmap(
    scene: &Scene,
    options: &RenderOptions,
    load_image: &dyn Fn(&str) -> Option<image::RgbaImage>,
) -> Result<Pixmap, RenderError> {
    let (width, height) = (options.width, options.height);
    let mut pixmap = Pixmap::new(width, height).ok_or(RenderError::InvalidSize(width, height))?;
    pixmap.fill(skia_color(options.palette.paper));

    let (min, max) = options
        .region
        .or_else(|| scene.bounds())
        .unwrap_or((Point2::new(0.0, 0.0), Point2::new(500.0, 500.0)));
    let view = Viewport::fit(min, max, width, height, options.margin_px);
    if options.grid {
        draw_grid(&mut pixmap, view, &options.palette);
    }

    let font =
        FontRef::try_from_slice(epaint_default_fonts::UBUNTU_LIGHT).expect("bundled font is valid");
    let mut images: HashMap<String, Option<Pixmap>> = HashMap::new();

    for item in &scene.items {
        match &item.primitive {
            Primitive::Fill { points, color, .. } => {
                if let Some(path) = polygon_path(points, view, true) {
                    pixmap.fill_path(
                        &path,
                        &paint(*color),
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }
            }
            Primitive::Line {
                points,
                closed,
                color,
                width,
            } => {
                if let Some(path) = polygon_path(points, view, *closed) {
                    let stroke = Stroke {
                        width: view.size_px(*width).max(0.5),
                        ..Stroke::default()
                    };
                    pixmap.stroke_path(&path, &paint(*color), &stroke, Transform::identity(), None);
                }
            }
            Primitive::Text {
                text,
                position,
                size,
                color,
                align,
                angle,
                look,
            } => {
                draw_text(
                    &mut pixmap,
                    &font,
                    text,
                    view.to_px(*position),
                    view.size_px(*size),
                    *color,
                    *align,
                    *angle,
                    *look,
                );
            }
            Primitive::Image {
                path,
                min,
                max,
                opacity,
            } => {
                let image = images
                    .entry(path.clone())
                    .or_insert_with(|| load_image(path).and_then(rgba_to_pixmap));
                if let Some(image) = image {
                    let (x0, y0) = view.to_px(*min);
                    let (x1, y1) = view.to_px(*max);
                    #[allow(clippy::cast_precision_loss)]
                    let (sx, sy) = (
                        (x1 - x0) / image.width() as f32,
                        (y1 - y0) / image.height() as f32,
                    );
                    #[allow(clippy::cast_possible_truncation)]
                    let paint = PixmapPaint {
                        opacity: *opacity as f32,
                        quality: FilterQuality::Bilinear,
                        ..PixmapPaint::default()
                    };
                    pixmap.draw_pixmap(
                        0,
                        0,
                        image.as_ref(),
                        &paint,
                        Transform::from_row(sx, 0.0, 0.0, sy, x0, y0),
                        None,
                    );
                }
            }
        }
    }
    Ok(pixmap)
}

fn skia_color(color: Color) -> tiny_skia::Color {
    let [r, g, b, a] = color.0;
    tiny_skia::Color::from_rgba8(r, g, b, a)
}

fn paint(color: Color) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color(skia_color(color));
    paint.anti_alias = true;
    paint
}

fn polygon_path(points: &[Point2], view: Viewport, closed: bool) -> Option<tiny_skia::Path> {
    let mut builder = PathBuilder::new();
    let mut iter = points.iter().map(|p| view.to_px(*p));
    let (x, y) = iter.next()?;
    builder.move_to(x, y);
    for (x, y) in iter {
        builder.line_to(x, y);
    }
    if closed {
        builder.close();
    }
    builder.finish()
}

fn draw_grid(pixmap: &mut Pixmap, view: Viewport, palette: &Palette) {
    let step = if view.scale * 50.0 < 8.0 { 500.0 } else { 50.0 };
    let (w, h) = (f64::from(pixmap.width()), f64::from(pixmap.height()));
    let to_plan = |px: f64, origin: f64| (px - origin) / view.scale;
    let mut lines = |horizontal: bool| {
        let (extent, origin) = if horizontal {
            (h, view.origin.1)
        } else {
            (w, view.origin.0)
        };
        #[allow(clippy::cast_possible_truncation)]
        let first = (to_plan(0.0, origin) / step).floor() as i64;
        #[allow(clippy::cast_possible_truncation)]
        let last = (to_plan(extent, origin) / step).ceil() as i64;
        for i in first..=last {
            #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
            let at = (origin + i as f64 * step * view.scale) as f32;
            let mut path = PathBuilder::new();
            #[allow(clippy::cast_possible_truncation)]
            if horizontal {
                path.move_to(0.0, at);
                path.line_to(w as f32, at);
            } else {
                path.move_to(at, 0.0);
                path.line_to(at, h as f32);
            }
            let color = if i % 2 == 0 {
                palette.grid_major
            } else {
                palette.grid_minor
            };
            if let Some(path) = path.finish() {
                pixmap.stroke_path(
                    &path,
                    &paint(color),
                    &Stroke {
                        width: 1.0,
                        ..Stroke::default()
                    },
                    Transform::identity(),
                    None,
                );
            }
        }
    };
    lines(false);
    lines(true);
}

fn rgba_to_pixmap(image: image::RgbaImage) -> Option<Pixmap> {
    let (w, h) = image.dimensions();
    let mut data = image.into_raw();
    // tiny-skia stores premultiplied alpha.
    for px in data.as_chunks_mut::<4>().0 {
        let a = u16::from(px[3]);
        for c in &mut px[..3] {
            *c = u8::try_from(u16::from(*c) * a / 255).unwrap_or(u8::MAX);
        }
    }
    Pixmap::from_vec(data, tiny_skia::IntSize::from_wh(w, h)?)
}

/// Measures and rasterizes (possibly multi-line) text into its own pixmap,
/// then composites it rotated around the anchor.
#[allow(clippy::too_many_arguments)]
fn draw_text(
    target: &mut Pixmap,
    font: &FontRef<'_>,
    text: &str,
    anchor: (f32, f32),
    size_px: f32,
    color: Color,
    align: Align,
    angle: f64,
    look: TextLook,
) {
    if size_px < 2.0 || text.is_empty() {
        return;
    }
    let scale = PxScale::from(size_px);
    let scaled = font.as_scaled(scale);
    let line_height = scaled.height() + scaled.line_gap();
    let lines: Vec<&str> = text.lines().collect();
    let bold_shift = if look.bold {
        (size_px * 0.045).max(0.6)
    } else {
        0.0
    };
    let halo = if look.outline.is_some() {
        (size_px * 0.08).max(1.0)
    } else {
        0.0
    };
    let pad = 2.0 + halo.ceil() + bold_shift.ceil();
    let widths: Vec<f32> = lines
        .iter()
        .map(|line| {
            line.chars()
                .map(|c| scaled.h_advance(font.glyph_id(c)))
                .sum::<f32>()
                + bold_shift
        })
        .collect();
    let text_w = widths.iter().copied().fold(0.0, f32::max).ceil() + 2.0 * pad;
    #[allow(clippy::cast_precision_loss)]
    let text_h = (line_height * lines.len() as f32).ceil() + 2.0 * pad;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let Some(mut layer) = Pixmap::new(text_w as u32, text_h as u32) else {
        return;
    };
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let layer_w = text_w as u32;
    let horizontal = align.horizontal();
    let skew = if look.italic { 0.2 } else { 0.0 };

    // Coverage of every glyph, drawn with offsets (halo, fake bold).
    let stamp = |data: &mut [u8], rgba: [u8; 4], offsets: &[(f32, f32)]| {
        let [r, g, b, a] = rgba;
        for (row, (line, line_w)) in lines.iter().zip(&widths).enumerate() {
            #[allow(clippy::cast_precision_loss)]
            let baseline = pad + scaled.ascent() + line_height * row as f32;
            let start = pad + (text_w - 2.0 * pad - line_w) * horizontal;
            for &(ox, oy) in offsets {
                let mut x = start;
                for c in line.chars() {
                    let glyph_id = font.glyph_id(c);
                    let glyph = glyph_id
                        .with_scale_and_position(scale, ab_glyph::point(x + ox, baseline + oy));
                    x += scaled.h_advance(glyph_id);
                    let Some(outline) = font.outline_glyph(glyph) else {
                        continue;
                    };
                    let bounds = outline.px_bounds();
                    outline.draw(|gx, gy, coverage| {
                        #[allow(clippy::cast_possible_truncation)]
                        let py = bounds.min.y as i64 + i64::from(gy);
                        #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
                        let lean = (skew * (baseline + oy - py as f32)) as i64;
                        #[allow(clippy::cast_possible_truncation)]
                        let px = bounds.min.x as i64 + i64::from(gx) + lean;
                        #[allow(clippy::cast_precision_loss)]
                        if px < 0 || py < 0 || px >= i64::from(layer_w) || py as f32 >= text_h {
                            return;
                        }
                        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                        let idx = ((py as u32 * layer_w + px as u32) * 4) as usize;
                        let alpha = coverage.clamp(0.0, 1.0) * f32::from(a) / 255.0;
                        // Source-over on premultiplied pixels.
                        for (k, v) in [r, g, b, 255].into_iter().enumerate() {
                            let src = f32::from(v) * alpha;
                            let dst = f32::from(data[idx + k]);
                            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                            let out = (src + dst * (1.0 - alpha)).round().clamp(0.0, 255.0) as u8;
                            data[idx + k] = out;
                        }
                    });
                }
            }
        }
    };
    let fill_offsets: Vec<(f32, f32)> = if look.bold {
        vec![(0.0, 0.0), (bold_shift * 0.5, 0.0), (bold_shift, 0.0)]
    } else {
        vec![(0.0, 0.0)]
    };
    if let Some(outline) = look.outline {
        let ring: Vec<(f32, f32)> = (0..12)
            .map(|k| {
                #[allow(clippy::cast_precision_loss)]
                let t = k as f32 / 12.0 * std::f32::consts::TAU;
                (halo * t.cos() + bold_shift * 0.5, halo * t.sin())
            })
            .collect();
        stamp(layer.data_mut(), outline.0, &ring);
    }
    stamp(layer.data_mut(), color.0, &fill_offsets);

    // Place the layer relative to the anchor, then rotate around it.
    #[allow(clippy::cast_precision_loss)]
    let dy = match align {
        Align::Center => -text_h / 2.0,
        Align::Above => -text_h + pad - 1.0,
        Align::BaselineLeft | Align::BaselineCenter | Align::BaselineRight => {
            -(pad + scaled.ascent() + line_height * (lines.len() as f32 - 1.0))
        }
    };
    let dx = -pad - (text_w - 2.0 * pad) * horizontal;
    #[allow(clippy::cast_possible_truncation)]
    let transform = Transform::from_translate(anchor.0, anchor.1)
        .pre_rotate(angle as f32)
        .pre_translate(dx, dy);
    target.draw_pixmap(
        0,
        0,
        layer.as_ref(),
        &PixmapPaint {
            quality: FilterQuality::Bilinear,
            ..PixmapPaint::default()
        },
        transform,
        None,
    );
}

#[cfg(test)]
mod tests {
    use newera_core::{Command, Document, Room, Wall};

    use super::*;
    use crate::scene::{SceneOptions, plan_scene};

    fn sample() -> Document {
        let mut doc = Document::default();
        let pts = [(0.0, 0.0), (400.0, 0.0), (400.0, 300.0), (0.0, 300.0)];
        for i in 0..4 {
            let (a, b) = (pts[i], pts[(i + 1) % 4]);
            let wall = Wall::new(
                doc.new_wall_id(),
                Point2::new(a.0, a.1),
                Point2::new(b.0, b.1),
            );
            doc.execute(Command::insert(wall)).unwrap();
        }
        let room = Room::new(
            doc.new_room_id(),
            "Sala",
            pts.iter().map(|&(x, y)| Point2::new(x, y)).collect(),
        );
        doc.execute(Command::insert(room)).unwrap();
        doc
    }

    #[test]
    fn renders_a_valid_png_with_walls_in_the_middle() {
        let doc = sample();
        let scene = plan_scene(doc.home(), &SceneOptions::default());
        let options = RenderOptions {
            width: 300,
            height: 200,
            ..RenderOptions::default()
        };
        let pixmap = render_pixmap(&scene, &options, &|_| None).unwrap();
        let png = render_png(&scene, &options, &|_| None).unwrap();
        assert_eq!(&png[1..4], b"PNG");

        // The top wall crosses the horizontal center line near the top margin:
        // somewhere in that column there must be a dark wall pixel.
        let wall = options.palette.wall.0;
        let column = 150;
        let dark = (0..200).any(|y| {
            let px = pixmap.pixel(column, y).unwrap().demultiply();
            px.red().abs_diff(wall[0]) < 12 && px.green().abs_diff(wall[1]) < 12
        });
        assert!(dark, "no wall pixel found in column {column}");
    }

    #[test]
    fn viewport_fit_centers_region() {
        let v = Viewport::fit(
            Point2::new(0.0, 0.0),
            Point2::new(100.0, 50.0),
            220,
            120,
            10.0,
        );
        assert!((v.scale - 2.0).abs() < 1e-9);
        assert_eq!(v.to_px(Point2::new(50.0, 25.0)), (110.0, 60.0));
    }
}
