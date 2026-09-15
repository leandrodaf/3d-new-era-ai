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

pub(crate) fn room(r: &newera_core::Room) -> Value {
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
    if let Some(m) = &r.floor_material {
        v["floor_mat"] = json!(m.to_string());
    }
    if let Some(m) = &r.ceiling_material {
        v["ceil_mat"] = json!(m.to_string());
    }
    v
}

pub(crate) fn dimension(d: &newera_core::Dimension) -> Value {
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
}

pub(crate) fn label(l: &newera_core::Label) -> Value {
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
    if l.bold {
        v["bold"] = json!(true);
    }
    if let Some(d) = l.discipline {
        v["layer"] = json!(d);
    }
    if l.italic {
        v["italic"] = json!(true);
    }
    if l.align != newera_core::TextAlign::Center {
        v["align"] = json!(l.align);
    }
    if let Some(color) = l.color {
        v["color"] = json!(color);
    }
    v
}

pub(crate) fn polyline(p: &newera_core::Polyline) -> Value {
    let mut v = obj([
        ("id", json!(p.id.to_string())),
        ("pts", points(&p.points)),
        ("t", num(p.thickness)),
        ("color", json!(p.color)),
    ]);
    if p.closed {
        v["closed"] = json!(true);
    }
    if let Some(d) = p.discipline {
        v["layer"] = json!(d);
    }
    if p.dash != newera_core::DashStyle::Solid {
        v["dash"] = json!(p.dash);
    }
    if p.join == newera_core::LineJoin::Curved {
        v["curved"] = json!(true);
    }
    if p.start_arrow != newera_core::ArrowStyle::None
        || p.end_arrow != newera_core::ArrowStyle::None
    {
        v["arrows"] = json!([p.start_arrow, p.end_arrow]);
    }
    v
}

/// The array key each kind of element is listed under.
pub(crate) const KINDS: [&str; 6] = ["walls", "rooms", "dims", "labels", "furniture", "polylines"];

pub(crate) fn kind_of(id: newera_core::ElementId) -> &'static str {
    use newera_core::ElementId;
    match id {
        ElementId::Wall(_) => "walls",
        ElementId::Room(_) => "rooms",
        ElementId::Dimension(_) => "dims",
        ElementId::Label(_) => "labels",
        ElementId::Furniture(_) => "furniture",
        ElementId::Polyline(_) => "polylines",
        ElementId::Level(_) => "levels",
    }
}

/// One element, in the same shape `home` lists it in. Reaches pieces inside
/// groups, which the top-level listing only counts.
pub(crate) fn element(home: &Home, id: newera_core::ElementId) -> Option<Value> {
    use newera_core::ElementId;
    match id {
        ElementId::Wall(w) => home.wall(w).map(wall),
        ElementId::Room(r) => home.room(r).map(room),
        ElementId::Dimension(d) => home.dimension(d).map(dimension),
        ElementId::Label(l) => home.label(l).map(label),
        ElementId::Polyline(p) => home.polyline(p).map(polyline),
        ElementId::Furniture(f) => {
            let cuts = home.wall_cuts();
            home.find_piece(f).map(|p| piece(home, &cuts, p))
        }
        ElementId::Level(_) => None,
    }
}

/// Full compact state.
pub(crate) fn home(home: &Home, revision: u64) -> Value {
    let mut out = obj([("rev", json!(revision)), ("name", json!(home.name))]);

    let cuts = home.wall_cuts();
    for (key, list) in [
        ("walls", home.walls.iter().map(wall).collect::<Vec<_>>()),
        ("rooms", home.rooms.iter().map(room).collect()),
        ("dims", home.dimensions.iter().map(dimension).collect()),
        ("labels", home.labels.iter().map(label).collect()),
        (
            "furniture",
            home.furniture
                .iter()
                .map(|f| piece(home, &cuts, f))
                .collect(),
        ),
        ("polylines", home.polylines.iter().map(polyline).collect()),
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

/// Which tool built a composite piece, so an id from a report can be taken
/// back to the tool that owns it instead of guessing and being told no.
pub(crate) fn made_by(f: &newera_core::Furniture) -> &'static str {
    let has = |key: &str| f.properties.contains_key(key);
    if has(newera_joinery::PARAMS_KEY) {
        "joinery"
    } else if has(newera_joinery::RUN_KEY) {
        "cabinet_run"
    } else if f.catalog.starts_with("roof") {
        "fit_roof"
    } else if f.model.is_some() {
        "import"
    } else {
        "group"
    }
}

/// A piece, omitting whatever matches its catalog defaults.
pub(crate) fn piece(
    home: &Home,
    cuts: &[Vec<newera_core::WallCut>],
    f: &newera_core::Furniture,
) -> Value {
    let mut v = obj([
        ("id", json!(f.id.to_string())),
        ("cat", json!(f.catalog)),
        ("at", point(f.position)),
    ]);
    let item = newera_catalog::find(&f.catalog);
    let default_size = item.map(|i| i.size);
    if f.angle.rem_euclid(360.0).abs() > 0.05 {
        v["angle"] = num(f.angle.rem_euclid(360.0));
    }
    // `wdh` is in the piece's own frame: with a quarter turn, width and
    // depth swap in the plan. The resolved box is always given so nobody
    // has to redo that rotation by hand — and `faces`, the side the piece
    // opens toward, is not readable from `angle` alone.
    let (min, max) = newera_core::plan_bounds(f);
    v["bounds"] = json!([point(min), point(max)]);
    v["faces"] = json!(newera_core::facing(f));
    if default_size != Some([f.width, f.depth, f.height]) {
        v["wdh"] = json!([num(f.width), num(f.depth), num(f.height)]);
    }
    if (item.map_or(0.0, |i| i.elevation) - f.elevation).abs() > 0.05 {
        v["elev"] = num(f.elevation);
    }
    if item.map(|i| i.name) != Some(f.name.as_str()) {
        v["name"] = json!(f.name);
    }
    if let Some(model) = &f.model {
        v["model"] = json!(model);
    }
    if let Some(color) = f.color {
        v["color"] = json!(color);
    }
    if f.mirrored {
        v["mirror"] = json!(true);
    }
    if f.is_group() {
        v["parts"] = json!(f.flatten().len() - 1);
        v["made_by"] = json!(made_by(f));
    }
    if let Some(d) = f.discipline {
        v["layer"] = json!(d);
    }
    if let Some(light) = &f.light {
        let mut l = json!({"lm": light.flux().round()});
        if let Some(k) = light.kelvin {
            l["k"] = num(k);
        }
        if let Some(beam) = light.beam {
            l["beam"] = num(beam);
        }
        v["light"] = l;
    }
    if !f.visible {
        v["visible"] = json!(false);
    }
    if f.is_opening()
        && let Some(i) = cuts
            .iter()
            .position(|c| c.iter().any(|cut| cut.furniture == f.id))
    {
        v["wall"] = json!(home.walls[i].id.to_string());
    }
    v
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
    if let Some(kind) = &w.wall_type {
        v["type"] = json!(kind);
    }
    match (&w.left_side, &w.right_side) {
        (Some(l), Some(r)) if l == r => v["sides"] = json!(l.to_string()),
        (left, right) => {
            if let Some(l) = left {
                v["left"] = json!(l.to_string());
            }
            if let Some(r) = right {
                v["right"] = json!(r.to_string());
            }
        }
    }
    v
}

/// Wall types and material patterns, for the `materials` tool.
pub(crate) fn materials() -> Value {
    let walls: Vec<Value> = newera_core::WALL_TYPES
        .iter()
        .map(|t| json!([t.id, t.name, num(t.thickness)]))
        .collect();
    let patterns: Vec<Value> = newera_core::Pattern::ALL
        .iter()
        .map(|p| {
            let [r, g, b] = p.default_color();
            let [w, h] = p.default_tile();
            json!([
                p.key(),
                p.label(),
                format!("#{r:02x}{g:02x}{b:02x}"),
                format!("{w}x{h}")
            ])
        })
        .collect();
    obj([("wall_types", json!(walls)), ("patterns", json!(patterns))])
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
                ("furniture", json!(home.furniture.len())),
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

/// Catalog rows `[id, name, w, d, h]`, optionally filtered.
pub(crate) fn catalog(query: Option<&str>, category: Option<&str>, limit: usize) -> Value {
    let items: Vec<_> = newera_catalog::search(query.unwrap_or(""))
        .into_iter()
        .filter(|i| category.is_none_or(|c| i.category.id() == c))
        .collect();
    let rows: Vec<Value> = items
        .iter()
        .take(limit)
        .map(|i| json!([i.id, i.name, num(i.size[0]), num(i.size[1]), num(i.size[2])]))
        .collect();
    let mut out = json!({ "items": rows });
    if items.len() > limit {
        out["more"] = json!(items.len() - limit);
    }
    if query.is_none() && category.is_none() {
        out["categories"] = json!(
            newera_catalog::Category::ALL
                .iter()
                .map(|c| c.id())
                .collect::<Vec<_>>()
        );
    }
    out
}

/// One element in an issue report: id, name, box and height range.
///
/// A bare pair of ids says nothing — a cooktop set into its worktop and a
/// sink eating into the dishwasher next to it read the same. With the name
/// and the geometry alongside, the call that found the problem is also the
/// call that explains it.
pub(crate) fn issue_ref(home: &Home, id: newera_core::ElementId) -> Value {
    use newera_core::ElementId;
    let mut v = obj([("id", json!(id.to_string()))]);
    if let ElementId::Furniture(f) = id
        && let Some(piece) = home.find_piece(f)
    {
        v["name"] = json!(piece.name);
        let (lo, hi) = piece.height_range();
        v["z"] = json!([num(lo), num(hi)]);
        if let Some(level) = home.piece_level(f) {
            v["level"] = json!(level.to_string());
        }
    }
    if let ElementId::Wall(w) = id
        && let Some(wall) = home.wall(w)
    {
        v["name"] = json!(
            wall.wall_type
                .clone()
                .unwrap_or_else(|| "parede".to_owned())
        );
    }
    if let Some((min, max)) = newera_core::element_bounds(home, id) {
        v["bounds"] = json!([point(min), point(max)]);
    }
    v
}

/// Layout issues grouped by kind; empty object when everything is fine.
///
/// Overlaps come classified and measured, worst first, so a report of
/// twenty pairs reads as "one real clash, four built-in pieces, fifteen
/// artefacts of two storeys drawn on top of each other" instead of twenty
/// equally alarming pairs.
pub(crate) fn issues(home: &Home, scope: newera_core::Storeys) -> Value {
    use newera_core::{Issue, Overlap};
    let mut out = serde_json::Map::new();
    let mut push = |key: &str, value: Value| {
        out.entry(key.to_owned())
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
            .expect("array")
            .push(value);
    };
    let mut found = newera_core::check_layout_in(home, scope);
    // Real clashes first: everything after them is explained, not broken.
    found.sort_by_key(|i| match i {
        Issue::Overlap {
            kind: Overlap::Collision,
            ..
        } => 0,
        Issue::Overlap {
            kind: Overlap::Nesting,
            ..
        } => 2,
        Issue::Overlap {
            kind: Overlap::CrossLevel,
            ..
        } => 3,
        _ => 1,
    });
    let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for issue in found {
        match issue {
            Issue::Overlap { a, b, kind, extent } => {
                *counts.entry(kind.name()).or_default() += 1;
                push(
                    "overlap",
                    obj([
                        ("a", issue_ref(home, a.into())),
                        ("b", issue_ref(home, b.into())),
                        ("kind", json!(kind.name())),
                        ("extent", json!(extent.map(num))),
                    ]),
                );
            }
            Issue::Blocked { piece, against, cm } => push(
                "blocked",
                obj([
                    ("piece", issue_ref(home, piece.into())),
                    ("against", issue_ref(home, against)),
                    ("cm", num(cm)),
                ]),
            ),
            Issue::InWall(f, w) => push(
                "in_wall",
                json!([issue_ref(home, f.into()), issue_ref(home, w.into())]),
            ),
            Issue::BlocksDoor { door, by } => push(
                "blocks_door",
                json!([issue_ref(home, door.into()), issue_ref(home, by.into())]),
            ),
            Issue::OutsideRooms(f) => push("outside_rooms", issue_ref(home, f.into())),
            Issue::LooseOpening(f) => push("loose_opening", issue_ref(home, f.into())),
            Issue::Turned {
                piece,
                built,
                placed,
            } => push(
                "turned",
                obj([
                    ("piece", issue_ref(home, piece.into())),
                    ("built", json!(built)),
                    ("placed", json!(placed)),
                ]),
            ),
        }
    }
    if !counts.is_empty() {
        out.insert("overlap_kinds".to_owned(), json!(counts));
    }
    for warning in warnings(home) {
        out.entry("warnings".to_owned())
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
            .expect("array")
            .push(json!(warning));
    }
    Value::Object(out)
}

/// Things about the project itself that make every other reading suspect.
///
/// Today that is one thing, and it cost a whole session to diagnose: two
/// storeys that are not reference layers sharing a floor-to-ceiling range,
/// which makes every check report pieces clashing with their own copies.
pub(crate) fn warnings(home: &Home) -> Vec<String> {
    home.stacked_levels()
        .into_iter()
        .map(|(a, b)| {
            let name = |id| home.level(id).map_or_else(String::new, |l| l.name.clone());
            format!(
                "{a} \"{}\" and {b} \"{}\" share an elevation; if one is a tracing or an \
                 older version mark it with levels(reference=true) so checks skip it",
                name(a),
                name(b),
            )
        })
        .collect()
}

/// What one write did, element by element.
///
/// Writes used to answer `ok rev=N` and nothing else, so the only way to
/// learn the effect of a change was to read the whole home back. Comparing
/// the compact view before and after says it in a line: which elements
/// appeared, which went, and exactly which fields moved.
pub(crate) fn diff(before: &Home, after: &Home) -> Value {
    let index = |home: &Home| -> std::collections::BTreeMap<String, Value> {
        let cuts = home.wall_cuts();
        let mut map = std::collections::BTreeMap::new();
        for wall in &home.walls {
            map.insert(wall.id.to_string(), self::wall(wall));
        }
        for r in &home.rooms {
            map.insert(r.id.to_string(), room(r));
        }
        for d in &home.dimensions {
            map.insert(d.id.to_string(), dimension(d));
        }
        for l in &home.labels {
            map.insert(l.id.to_string(), label(l));
        }
        for pl in &home.polylines {
            map.insert(pl.id.to_string(), polyline(pl));
        }
        for top in &home.furniture {
            for f in top.flatten() {
                map.insert(f.id.to_string(), piece(home, &cuts, f));
            }
        }
        map
    };
    let (old, new) = (index(before), index(after));
    let mut changed = Vec::new();
    let mut added = Vec::new();
    for (id, now) in &new {
        let Some(was) = old.get(id) else {
            added.push(json!(id));
            continue;
        };
        if was == now {
            continue;
        }
        // Only the fields that actually moved, both sides, so the reply can
        // be read without holding the previous state in mind.
        let mut from = serde_json::Map::new();
        let mut to = serde_json::Map::new();
        let keys: std::collections::BTreeSet<&String> = was
            .as_object()
            .into_iter()
            .chain(now.as_object())
            .flat_map(serde_json::Map::keys)
            .collect();
        for key in keys {
            let (a, b) = (was.get(key), now.get(key));
            if a != b {
                from.insert(key.clone(), a.cloned().unwrap_or(Value::Null));
                to.insert(key.clone(), b.cloned().unwrap_or(Value::Null));
            }
        }
        changed.push(json!({"id": id, "from": from, "to": to}));
    }
    let gone: Vec<Value> = old
        .keys()
        .filter(|id| !new.contains_key(*id))
        .map(|id| json!(id))
        .collect();
    let mut out = serde_json::Map::new();
    for (key, list) in [("changed", changed), ("added", added), ("gone", gone)] {
        if !list.is_empty() {
            out.insert(key.to_owned(), Value::Array(list));
        }
    }
    Value::Object(out)
}

/// Variant rows `[i, name, active, walls, rooms, m2, furniture, issues]`.
pub(crate) fn variants(doc: &newera_core::Document) -> Value {
    Value::Array(
        doc.variants()
            .enumerate()
            .map(|(i, v)| {
                let home = v.home();
                let area: f64 = home.rooms.iter().map(newera_core::Room::area).sum();
                json!([
                    i,
                    v.name,
                    i == doc.active_variant(),
                    home.walls.len(),
                    home.rooms.len(),
                    // `+ 0.0` turns the empty sum (-0.0) into 0.0.
                    (area / 100.0).round() / 100.0 + 0.0,
                    home.furniture.len(),
                    newera_core::check_layout(home)
                        .iter()
                        .filter(|i| i.is_defect())
                        .count()
                ])
            })
            .collect(),
    )
}

/// Storey rows `[id, name, elevation, height, selected]`, ground first.
pub(crate) fn levels(home: &Home) -> Value {
    let current = home.current_level();
    Value::Array(
        home.sorted_levels()
            .into_iter()
            .map(|l| {
                json!([
                    l.id.to_string(),
                    l.name,
                    num(l.elevation),
                    num(l.height),
                    Some(l.id) == current,
                    l.elevation_index,
                    l.viewable,
                    l.is_reference()
                ])
            })
            .collect(),
    )
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
