//! SVG export of a plan [`Scene`], in real-world units.
//!
//! The SVG user unit is one centimeter, so the file opens at true scale in
//! vector tools. Pixel-sized strokes and texts are converted with
//! `px_per_cm`, which stands for the print/view scale they were designed for.

use std::fmt::Write as _;

use newera_core::Point2;

use crate::scene::{Align, Color, Palette, Primitive, Scene, Size};

#[derive(Debug, Clone)]
pub struct SvgOptions {
    /// How many screen pixels a centimeter represents for pixel-sized styles.
    pub px_per_cm: f64,
    pub margin_cm: f64,
    pub palette: Palette,
}

impl Default for SvgOptions {
    fn default() -> Self {
        Self {
            px_per_cm: 0.8,
            margin_cm: 60.0,
            palette: Palette::default(),
        }
    }
}

pub fn to_svg(scene: &Scene, options: &SvgOptions) -> String {
    let (min, max) = scene
        .bounds()
        .unwrap_or((Point2::new(0.0, 0.0), Point2::new(100.0, 100.0)));
    let m = options.margin_cm;
    let (x, y) = (min.x - m, min.y - m);
    let (w, h) = (max.x - min.x + 2.0 * m, max.y - min.y + 2.0 * m);
    let size = |s: Size| match s {
        Size::Px(px) => f64::from(px) / options.px_per_cm,
        Size::Cm(cm) => cm,
    };

    let mut out = String::new();
    let _ = writeln!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="{} {} {} {}" width="{}cm" height="{}cm">"#,
        f(x),
        f(y),
        f(w),
        f(h),
        f(w),
        f(h)
    );
    let _ = writeln!(
        out,
        r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>"#,
        f(x),
        f(y),
        f(w),
        f(h),
        hex(options.palette.paper)
    );
    for item in &scene.items {
        match &item.primitive {
            Primitive::Fill { points, color, .. } => {
                let _ = writeln!(
                    out,
                    r#"<polygon points="{}" {}/>"#,
                    points_attr(points),
                    fill_attr(*color)
                );
            }
            Primitive::Line {
                points,
                closed,
                color,
                width,
            } => {
                let tag = if *closed { "polygon" } else { "polyline" };
                let _ = writeln!(
                    out,
                    r#"<{tag} points="{}" fill="none" stroke="{}"{} stroke-width="{}" stroke-linejoin="round"/>"#,
                    points_attr(points),
                    hex(*color),
                    opacity_attr("stroke-opacity", *color),
                    f(size(*width))
                );
            }
            Primitive::Text {
                text,
                position,
                size: text_size,
                color,
                align,
                angle,
                look,
            } => {
                let font_size = size(*text_size);
                let baseline = match align {
                    Align::Center => "central",
                    Align::Above => "text-after-edge",
                    Align::BaselineLeft | Align::BaselineCenter | Align::BaselineRight => {
                        "alphabetic"
                    }
                };
                let anchor = match align {
                    Align::BaselineLeft => "start",
                    Align::BaselineRight => "end",
                    _ => "middle",
                };
                let lines: Vec<&str> = text.lines().collect();
                #[allow(clippy::cast_precision_loss)]
                let first_dy = if align.is_baseline() {
                    -(lines.len() as f64 - 1.0) * 1.2
                } else {
                    -(lines.len() as f64 - 1.0) / 2.0 * 1.2
                };
                let mut decoration = String::new();
                if look.bold {
                    decoration.push_str(r#" font-weight="bold""#);
                }
                if look.italic {
                    decoration.push_str(r#" font-style="italic""#);
                }
                if let Some(halo) = look.outline {
                    let _ = write!(
                        decoration,
                        r#" stroke="rgb({},{},{})" stroke-width="{}" paint-order="stroke""#,
                        halo.0[0],
                        halo.0[1],
                        halo.0[2],
                        f(font_size * 0.16)
                    );
                }
                let _ = write!(
                    out,
                    r#"<text x="{}" y="{}" font-family="Ubuntu, Helvetica, Arial, sans-serif" font-size="{}" text-anchor="{anchor}" dominant-baseline="{baseline}"{decoration} {} transform="rotate({} {} {})">"#,
                    f(position.x),
                    f(position.y),
                    f(font_size),
                    fill_attr(*color),
                    f(*angle),
                    f(position.x),
                    f(position.y)
                );
                for (i, line) in lines.iter().enumerate() {
                    let dy = if i == 0 { first_dy } else { 1.2 };
                    let _ = write!(
                        out,
                        r#"<tspan x="{}" dy="{}em">{}</tspan>"#,
                        f(position.x),
                        f(dy),
                        escape(line)
                    );
                }
                out.push_str("</text>\n");
            }
            Primitive::Image {
                path,
                min,
                max,
                opacity,
                angle,
            } => {
                let _ = writeln!(
                    out,
                    r#"<image href="{}" x="{}" y="{}" width="{}" height="{}" opacity="{}" preserveAspectRatio="none" transform="rotate({} {} {})"/>"#,
                    escape(path),
                    f(min.x),
                    f(min.y),
                    f(max.x - min.x),
                    f(max.y - min.y),
                    f(*opacity),
                    f(*angle),
                    f(min.x.midpoint(max.x)),
                    f(min.y.midpoint(max.y))
                );
            }
        }
    }
    out.push_str("</svg>\n");
    out
}

fn f(v: f64) -> String {
    let text = format!("{:.2}", (v * 100.0).round() / 100.0);
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

fn hex(color: Color) -> String {
    let [r, g, b, _] = color.0;
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn opacity_attr(name: &str, color: Color) -> String {
    let a = color.0[3];
    if a == 255 {
        String::new()
    } else {
        format!(r#" {name}="{}""#, f(f64::from(a) / 255.0))
    }
}

fn fill_attr(color: Color) -> String {
    format!(
        r#"fill="{}"{}"#,
        hex(color),
        opacity_attr("fill-opacity", color)
    )
}

fn points_attr(points: &[Point2]) -> String {
    points
        .iter()
        .map(|p| format!("{},{}", f(p.x), f(p.y)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use newera_core::{Command, Document, Label, Wall};

    use super::*;
    use crate::scene::{SceneOptions, plan_scene};

    #[test]
    fn svg_is_in_centimeters_and_escapes_text() {
        let mut doc = Document::default();
        let wall = Wall::new(
            doc.new_wall_id(),
            Point2::new(0.0, 0.0),
            Point2::new(400.0, 0.0),
        );
        let label = Label {
            id: doc.new_label_id(),
            text: "Sala & <Cozinha>".into(),
            position: Point2::new(200.0, 100.0),
            size: 30.0,
            angle: 0.0,
            level: None,
            ..Default::default()
        };
        doc.execute(Command::insert(wall)).unwrap();
        doc.execute(Command::insert(label)).unwrap();
        let svg = to_svg(
            &plan_scene(doc.home(), &SceneOptions::default()),
            &SvgOptions::default(),
        );
        assert!(svg.starts_with("<svg"));
        assert!(
            svg.contains(r#"<polygon points="0,-7.5 "#) || svg.contains("0,7.5"),
            "{svg}"
        );
        assert!(svg.contains("Sala &amp; &lt;Cozinha&gt;"));
        assert!(svg.trim_end().ends_with("</svg>"));
    }
}
