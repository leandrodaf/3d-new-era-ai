//! Token-lean JSON views of a home.
//!
//! Conventions (documented once in the server instructions, never repeated):
//! short ids, `[x,y]` points, numbers rounded to 0.1 cm without trailing
//! `.0`, and fields omitted when they hold their default value.

use newera_core::{Compass, Home, Point2, Wall};
use serde_json::{Map, Value, json};

/// Rounds to 0.1 and prints integers without a fractional part.
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn num(value: f64) -> Value {
    let rounded = (value * 10.0).round() / 10.0;
    if rounded.fract() == 0.0 && rounded.abs() < 9e15 {
        json!(rounded as i64)
    } else {
        json!(rounded)
    }
}

pub(crate) fn point(p: Point2) -> Value {
    json!([num(p.x), num(p.y)])
}

fn points(points: &[Point2]) -> Value {
    Value::Array(points.iter().copied().map(point).collect())
}

fn obj(pairs: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    Value::Object(
        pairs
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect::<Map<_, _>>(),
    )
}

/// Full compact state.
pub(crate) fn home(home: &Home, revision: u64) -> Value {
    let mut out = obj([("rev", json!(revision)), ("name", json!(home.name))]);

    let walls: Vec<Value> = home.walls.iter().map(wall).collect();
    let rooms: Vec<Value> = home
        .rooms
        .iter()
        .map(|r| {
            let mut v = obj([
                ("id", json!(r.id.to_string())),
                ("name", json!(r.name)),
                ("pts", points(&r.points)),
                ("m2", json!((r.area() / 100.0).round() / 100.0)),
            ]);
            if !r.floor_visible {
                v["floor"] = json!(false);
            }
            if !r.ceiling_visible {
                v["ceiling"] = json!(false);
            }
            v
        })
        .collect();
    let dims: Vec<Value> = home
        .dimensions
        .iter()
        .map(|d| {
            let mut v = obj([
                ("id", json!(d.id.to_string())),
                ("a", point(d.start)),
                ("b", point(d.end)),
                ("len", num(d.length())),
            ]);
            if d.offset != 0.0 {
                v["off"] = num(d.offset);
            }
            v
        })
        .collect();
    let labels: Vec<Value> = home
        .labels
        .iter()
        .map(|l| {
            let mut v = obj([
                ("id", json!(l.id.to_string())),
                ("text", json!(l.text)),
                ("at", point(l.position)),
            ]);
            if (l.size - newera_core::Label::DEFAULT_SIZE).abs() > f64::EPSILON {
                v["size"] = num(l.size);
            }
            if l.angle != 0.0 {
                v["angle"] = num(l.angle);
            }
            v
        })
        .collect();

    for (key, list) in [
        ("walls", walls),
        ("rooms", rooms),
        ("dims", dims),
        ("labels", labels),
    ] {
        if !list.is_empty() {
            out[key] = Value::Array(list);
        }
    }
    if home.compass != Compass::default() {
        out["north"] = num(home.compass.north_degrees);
    }
    if let Some(bg) = &home.background {
        let (min, max) = bg.bounds();
        out["background"] = obj([
            ("path", json!(bg.path)),
            ("cm_per_px", json!(bg.cm_per_px)),
            ("from", point(min)),
            ("to", point(max)),
            ("opacity", json!(bg.opacity)),
            ("visible", json!(bg.visible)),
        ]);
    }
    out
}

pub(crate) fn wall(w: &Wall) -> Value {
    let mut v = obj([
        ("id", json!(w.id.to_string())),
        ("a", point(w.start)),
        ("b", point(w.end)),
    ]);
    if (w.thickness - Wall::DEFAULT_THICKNESS).abs() > f64::EPSILON {
        v["t"] = num(w.thickness);
    }
    if (w.height - Wall::DEFAULT_HEIGHT).abs() > f64::EPSILON {
        v["h"] = num(w.height);
    }
    if let Some(arc) = w.arc_extent.filter(|_| w.is_arc()) {
        v["arc"] = num(arc);
    }
    v
}

/// Cheapest overview: counts, bounds and room areas, no geometry.
pub(crate) fn summary(home: &Home, revision: u64) -> Value {
    let mut out = obj([
        ("rev", json!(revision)),
        ("name", json!(home.name)),
        (
            "counts",
            obj([
                ("walls", json!(home.walls.len())),
                ("rooms", json!(home.rooms.len())),
                ("dims", json!(home.dimensions.len())),
                ("labels", json!(home.labels.len())),
            ]),
        ),
    ]);
    if let Some((min, max)) = home.bounds() {
        out["bounds"] = json!([point(min), point(max)]);
    }
    if !home.rooms.is_empty() {
        out["rooms"] = Value::Array(
            home.rooms
                .iter()
                .map(|r| json!([r.id.to_string(), r.name, (r.area() / 100.0).round() / 100.0]))
                .collect(),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use newera_core::WallId;

    use super::*;

    #[test]
    fn compact_view_omits_defaults_and_rounds() {
        let mut h = Home::default();
        h.walls.push(Wall::new(
            WallId(1),
            Point2::new(0.0, 0.0),
            Point2::new(800.04, 0.0),
        ));
        assert_eq!(
            home(&h, 3).to_string(),
            r#"{"name":"Nova casa","rev":3,"walls":[{"a":[0,0],"b":[800,0],"id":"w1"}]}"#
        );
    }
}
