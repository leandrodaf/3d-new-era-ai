//! Parameter types and the edit logic behind the MCP tools, kept free of
//! protocol types so it can be unit-tested directly.

use newera_core::{
    BackgroundImage, Command, CoreError, Dimension, Document, Element, ElementId, Label, Material,
    Point2, Room, Wall, detect_room, ops,
};
use schemars::JsonSchema;
use serde::Deserialize;

pub(crate) type EditResult<T> = Result<T, String>;

#[allow(clippy::needless_pass_by_value)] // used as `map_err(core)`
fn core(err: CoreError) -> String {
    err.to_string()
}

/// Parses a material in short form; `none` means "no finish".
pub(crate) fn material(raw: &str) -> EditResult<Option<Material>> {
    if raw.trim() == "none" {
        return Ok(None);
    }
    raw.parse().map(Some)
}

/// Resolves a wall type id; `none` clears it.
fn wall_type(raw: &str) -> EditResult<Option<&'static newera_core::WallType>> {
    if raw == "none" {
        return Ok(None);
    }
    newera_core::wall_type(raw)
        .map(Some)
        .ok_or_else(|| format!("unknown wall type `{raw}` (see the materials tool)"))
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct WallPath {
    /// Polyline vertices; N points make N-1 joined walls.
    pub pts: Vec<Point2>,
    /// Join the last point back to the first.
    #[serde(default)]
    pub closed: bool,
    /// Thickness cm (default 15).
    pub t: Option<f64>,
    /// Height cm (default 250).
    pub h: Option<f64>,
    /// Arc extent in degrees applied to every segment (positive bulges left).
    pub arc: Option<f64>,
    /// Wall type id, e.g. `drywall-95`; sets the thickness unless `t` is given.
    #[serde(rename = "type")]
    pub kind: Option<String>,
    /// Finish on both sides, e.g. `#f2efe6` or `subway 20x10`.
    pub sides: Option<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct RoomSpec {
    pub name: String,
    /// Floor polygon. Omit and give `at` to detect the room enclosed by walls.
    pub pts: Option<Vec<Point2>>,
    /// A point inside a space enclosed by walls (auto-detects the polygon).
    pub at: Option<Point2>,
    /// Floor finish, e.g. `wood` or `tiles #ffffff 60`.
    pub floor_mat: Option<String>,
    /// Ceiling finish.
    pub ceil_mat: Option<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct DimSpec {
    pub a: Option<Point2>,
    pub b: Option<Point2>,
    /// Measure an existing wall instead of a/b; placed outside the house.
    pub wall: Option<String>,
    /// Offset cm from the measured points (left of a→b is positive).
    pub off: Option<f64>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct LabelSpec {
    pub text: String,
    pub at: Point2,
    /// Text height cm (default 24).
    pub size: Option<f64>,
    /// Clockwise degrees.
    pub angle: Option<f64>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct CreateParams {
    #[serde(default)]
    pub walls: Vec<WallPath>,
    #[serde(default)]
    pub rooms: Vec<RoomSpec>,
    #[serde(default)]
    pub dims: Vec<DimSpec>,
    #[serde(default)]
    pub labels: Vec<LabelSpec>,
}

/// Creates everything in one undoable step and returns the new ids in order.
pub(crate) fn create(doc: &mut Document, params: CreateParams) -> EditResult<Vec<String>> {
    let mut commands = Vec::new();
    let mut ids = Vec::new();
    let mut new_walls = Vec::new();

    for path in params.walls {
        if path.pts.len() < 2 {
            return Err("each wall path needs at least 2 points".into());
        }
        let kind = path.kind.as_deref().map(wall_type).transpose()?.flatten();
        let sides = path.sides.as_deref().map(material).transpose()?.flatten();
        let mut pts = path.pts;
        if path.closed && pts.len() > 2 {
            pts.push(pts[0]);
        }
        for pair in pts.windows(2) {
            let mut wall = Wall {
                height: path.h.unwrap_or(Wall::DEFAULT_HEIGHT),
                arc_extent: path.arc.filter(|a| *a != 0.0),
                left_side: sides.clone(),
                right_side: sides.clone(),
                ..Wall::new(doc.new_wall_id(), pair[0], pair[1])
            };
            if let Some(kind) = kind {
                wall.apply_type(kind);
            }
            if let Some(t) = path.t {
                wall.thickness = t;
            }
            ids.push(wall.id.to_string());
            new_walls.push(wall.clone());
            commands.push(Command::insert(wall));
        }
    }

    for spec in params.rooms {
        let points = match (spec.pts, spec.at) {
            (Some(pts), _) => pts,
            (None, Some(at)) => {
                let mut walls = doc.home().walls.clone();
                walls.extend(new_walls.iter().cloned());
                detect_room(&walls, at).ok_or_else(|| {
                    format!("no space enclosed by walls around [{}, {}]", at.x, at.y)
                })?
            }
            (None, None) => return Err(format!("room `{}` needs `pts` or `at`", spec.name)),
        };
        let mut room = Room::new(doc.new_room_id(), spec.name, points);
        room.floor_material = spec
            .floor_mat
            .as_deref()
            .map(material)
            .transpose()?
            .flatten();
        room.ceiling_material = spec
            .ceil_mat
            .as_deref()
            .map(material)
            .transpose()?
            .flatten();
        ids.push(room.id.to_string());
        commands.push(Command::insert(room));
    }

    for spec in params.dims {
        let dim = match (spec.wall, spec.a, spec.b) {
            (Some(wall), _, _) => {
                let id = wall.parse().map_err(|e| format!("{e}"))?;
                let mut dim = ops::wall_dimension(doc, id, 40.0).map_err(core)?;
                if let Some(off) = spec.off {
                    dim.offset = off;
                }
                dim
            }
            (None, Some(a), Some(b)) => Dimension {
                id: doc.new_dimension_id(),
                start: a,
                end: b,
                offset: spec.off.unwrap_or(0.0),
                level: None,
            },
            _ => return Err("dimension needs `a` and `b`, or `wall`".into()),
        };
        ids.push(dim.id.to_string());
        commands.push(Command::insert(dim));
    }

    for spec in params.labels {
        let label = Label {
            id: doc.new_label_id(),
            text: spec.text,
            position: spec.at,
            size: spec.size.unwrap_or(Label::DEFAULT_SIZE),
            angle: spec.angle.unwrap_or(0.0),
            level: None,
        };
        ids.push(label.id.to_string());
        commands.push(Command::insert(label));
    }

    if commands.is_empty() {
        return Err("nothing to create".into());
    }
    doc.execute(Command::Batch { commands }).map_err(core)?;
    Ok(ids)
}

/// Fields that can be changed on an element. Each applies only to the kinds
/// that have it; anything else is rejected so mistakes are loud.
#[derive(Debug, Default, Deserialize, serde::Serialize, JsonSchema)]
pub(crate) struct UpdateSpec {
    #[serde(skip_serializing)]
    pub id: String,
    /// Start point (wall, dimension).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub a: Option<Point2>,
    /// End point (wall, dimension).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub b: Option<Point2>,
    /// Wall thickness cm.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub t: Option<f64>,
    /// Height cm (wall, furniture).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h: Option<f64>,
    /// Wall arc degrees (0 = straight).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arc: Option<f64>,
    /// Name (room, furniture).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Room polygon.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pts: Option<Vec<Point2>>,
    /// Room floor visible.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub floor: Option<bool>,
    /// Room ceiling visible.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ceiling: Option<bool>,
    /// Label text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Position (label, furniture center).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<Point2>,
    /// Label size cm.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    /// Clockwise degrees (label, furniture).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub angle: Option<f64>,
    /// Dimension offset cm.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub off: Option<f64>,
    /// Furniture width cm.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub w: Option<f64>,
    /// Furniture depth cm.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub d: Option<f64>,
    /// Furniture elevation cm.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elev: Option<f64>,
    /// Furniture color `[r,g,b]`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mirror: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    /// Door hinge on the right.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hinge_right: Option<bool>,
    /// Move the element to this level id (e.g. `lv2`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    /// Level slab thickness cm.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slab: Option<f64>,
    /// Wall type id (sets thickness unless `t` is given); `none` clears.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Wall finish on the left of a→b; `none` clears.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left: Option<String>,
    /// Wall finish on the right of a→b.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right: Option<String>,
    /// Wall finish on both sides.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sides: Option<String>,
    /// Room floor finish.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub floor_mat: Option<String>,
    /// Room ceiling finish.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ceil_mat: Option<String>,
}

impl UpdateSpec {
    fn fields(&self) -> Vec<String> {
        match serde_json::to_value(self) {
            Ok(serde_json::Value::Object(map)) => map.keys().cloned().collect(),
            _ => Vec::new(),
        }
    }
}

pub(crate) fn update(doc: &mut Document, items: Vec<UpdateSpec>) -> EditResult<()> {
    let mut commands = Vec::with_capacity(items.len());
    for spec in items {
        let id: ElementId = spec.id.parse().map_err(|e| format!("{e}"))?;
        let element = doc
            .home()
            .element(id)
            .ok_or_else(|| format!("{id} not found"))?;
        let allowed: &[&str] = match element {
            Element::Wall(_) => &[
                "a", "b", "t", "h", "arc", "level", "type", "left", "right", "sides",
            ],
            Element::Room(_) => &[
                "name",
                "pts",
                "floor",
                "ceiling",
                "level",
                "floor_mat",
                "ceil_mat",
            ],
            Element::Dimension(_) => &["a", "b", "off", "level"],
            Element::Label(_) => &["text", "at", "size", "angle", "level"],
            Element::Level(_) => &["name", "elev", "h", "slab"],
            Element::Furniture(_) => &[
                "at",
                "angle",
                "w",
                "d",
                "h",
                "elev",
                "name",
                "color",
                "mirror",
                "visible",
                "hinge_right",
                "level",
            ],
        };
        if let Some(bad) = spec
            .fields()
            .into_iter()
            .find(|f| !allowed.contains(&f.as_str()))
        {
            return Err(format!(
                "`{bad}` does not apply to {id} (allowed: {})",
                allowed.join(", ")
            ));
        }
        let level = match &spec.level {
            Some(raw) => {
                let level_id: newera_core::LevelId = raw.parse().map_err(|e| format!("{e}"))?;
                if doc.home().level(level_id).is_none() {
                    return Err(format!("{raw} not found"));
                }
                Some(Some(level_id))
            }
            None => None,
        };
        let mut updated = match element {
            Element::Level(mut l) => {
                l.name = spec.name.unwrap_or(l.name);
                l.elevation = spec.elev.unwrap_or(l.elevation);
                l.height = spec.h.unwrap_or(l.height);
                l.floor_thickness = spec.slab.unwrap_or(l.floor_thickness);
                Element::Level(l)
            }
            Element::Wall(mut w) => {
                w.start = spec.a.unwrap_or(w.start);
                w.end = spec.b.unwrap_or(w.end);
                if let Some(raw) = &spec.kind {
                    match wall_type(raw)? {
                        Some(kind) => w.apply_type(kind),
                        None => w.wall_type = None,
                    }
                }
                if let Some(raw) = &spec.sides {
                    w.left_side = material(raw)?;
                    w.right_side = w.left_side.clone();
                }
                if let Some(raw) = &spec.left {
                    w.left_side = material(raw)?;
                }
                if let Some(raw) = &spec.right {
                    w.right_side = material(raw)?;
                }
                w.thickness = spec.t.unwrap_or(w.thickness);
                w.height = spec.h.unwrap_or(w.height);
                if let Some(arc) = spec.arc {
                    w.arc_extent = (arc != 0.0).then_some(arc);
                }
                Element::Wall(w)
            }
            Element::Room(mut r) => {
                r.name = spec.name.unwrap_or(r.name);
                r.points = spec.pts.unwrap_or(r.points);
                r.floor_visible = spec.floor.unwrap_or(r.floor_visible);
                r.ceiling_visible = spec.ceiling.unwrap_or(r.ceiling_visible);
                if let Some(raw) = &spec.floor_mat {
                    r.floor_material = material(raw)?;
                }
                if let Some(raw) = &spec.ceil_mat {
                    r.ceiling_material = material(raw)?;
                }
                Element::Room(r)
            }
            Element::Dimension(mut d) => {
                d.start = spec.a.unwrap_or(d.start);
                d.end = spec.b.unwrap_or(d.end);
                d.offset = spec.off.unwrap_or(d.offset);
                Element::Dimension(d)
            }
            Element::Label(mut l) => {
                l.text = spec.text.unwrap_or(l.text);
                l.position = spec.at.unwrap_or(l.position);
                l.size = spec.size.unwrap_or(l.size);
                l.angle = spec.angle.unwrap_or(l.angle);
                Element::Label(l)
            }
            Element::Furniture(mut f) => {
                f.position = spec.at.unwrap_or(f.position);
                f.angle = spec.angle.unwrap_or(f.angle);
                f.width = spec.w.unwrap_or(f.width);
                f.depth = spec.d.unwrap_or(f.depth);
                f.height = spec.h.unwrap_or(f.height);
                f.elevation = spec.elev.unwrap_or(f.elevation);
                f.name = spec.name.unwrap_or(f.name);
                f.color = spec.color.or(f.color);
                f.mirrored = spec.mirror.unwrap_or(f.mirrored);
                f.visible = spec.visible.unwrap_or(f.visible);
                if let (Some(right), Some(opening)) = (spec.hinge_right, f.opening.as_mut()) {
                    opening.hinge_right = right;
                }
                Element::Furniture(f)
            }
        };
        if let Some(level) = level {
            updated.set_level(level);
        }
        commands.push(Command::Update { element: updated });
    }
    doc.execute(Command::Batch { commands }).map_err(core)
}

pub(crate) fn parse_ids(raw: &[String]) -> EditResult<Vec<ElementId>> {
    raw.iter()
        .map(|s| s.parse::<ElementId>().map_err(|e| format!("{e}")))
        .collect()
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct Calibration {
    /// First image pixel.
    pub a: Point2,
    /// Second image pixel.
    pub b: Point2,
    /// Real distance between them, cm.
    pub cm: f64,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct BackgroundParams {
    /// Image file (png/jpg/webp/bmp). Required when there is no background yet.
    pub path: Option<String>,
    /// Scale directly, cm per image pixel.
    pub cm_per_px: Option<f64>,
    /// Or scale by marking two pixels with a known real distance.
    pub calibrate: Option<Calibration>,
    /// Plan position cm of the image's top-left corner.
    pub offset: Option<Point2>,
    pub opacity: Option<f64>,
    pub visible: Option<bool>,
    /// Remove the background.
    #[serde(default)]
    pub clear: bool,
}

/// Applies background settings. `image_size` reads an image's pixel size.
pub(crate) fn set_background(
    doc: &mut Document,
    params: &BackgroundParams,
    image_size: &dyn Fn(&str) -> EditResult<[u32; 2]>,
) -> EditResult<()> {
    if params.clear {
        return doc
            .execute(Command::SetBackground { background: None })
            .map_err(core);
    }
    let current = doc.home().background.clone();
    let path = match (&params.path, &current) {
        (Some(p), _) => p.clone(),
        (None, Some(bg)) => bg.path.clone(),
        (None, None) => return Err("`path` is required to add a background".into()),
    };
    let size_px = match (&params.path, &current) {
        (None, Some(bg)) => bg.size_px,
        _ => image_size(&path)?,
    };
    let cm_per_px = match (&params.calibrate, params.cm_per_px) {
        (Some(c), _) => BackgroundImage::cm_per_px_from(c.a, c.b, c.cm)
            .ok_or("calibration points must differ and distance be positive")?,
        (None, Some(v)) => v,
        (None, None) => current.as_ref().map_or(1.0, |bg| bg.cm_per_px),
    };
    let background = BackgroundImage {
        path,
        size_px,
        cm_per_px,
        offset: params
            .offset
            .or(current.as_ref().map(|bg| bg.offset))
            .unwrap_or_default(),
        opacity: params
            .opacity
            .or(current.as_ref().map(|bg| bg.opacity))
            .unwrap_or(0.5),
        visible: params
            .visible
            .or(current.as_ref().map(|bg| bg.visible))
            .unwrap_or(true),
    };
    doc.execute(Command::SetBackground {
        background: Some(background),
    })
    .map_err(core)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(doc: &mut Document) -> Vec<String> {
        create(
            doc,
            CreateParams {
                walls: vec![WallPath {
                    pts: [(0.0, 0.0), (400.0, 0.0), (400.0, 300.0), (0.0, 300.0)]
                        .iter()
                        .map(|&(x, y)| Point2::new(x, y))
                        .collect(),
                    closed: true,
                    ..WallPath::default()
                }],
                ..CreateParams::default()
            },
        )
        .unwrap()
    }

    #[test]
    fn create_walls_and_detected_room_in_one_call() {
        let mut doc = Document::default();
        let ids = create(
            &mut doc,
            CreateParams {
                walls: vec![WallPath {
                    pts: vec![
                        Point2::new(0.0, 0.0),
                        Point2::new(400.0, 0.0),
                        Point2::new(400.0, 300.0),
                        Point2::new(0.0, 300.0),
                    ],
                    closed: true,
                    ..WallPath::default()
                }],
                rooms: vec![RoomSpec {
                    name: "Sala".into(),
                    at: Some(Point2::new(200.0, 150.0)),
                    pts: None,
                    ..RoomSpec::default()
                }],
                labels: vec![LabelSpec {
                    text: "Entrada".into(),
                    at: Point2::new(0.0, -50.0),
                    ..LabelSpec::default()
                }],
                ..CreateParams::default()
            },
        )
        .unwrap();
        assert_eq!(ids, ["w1", "w2", "w3", "w4", "r5", "t6"]);
        assert_eq!(doc.revision(), 1, "single undoable step");
        let room = &doc.home().rooms[0];
        assert!((room.area() - 385.0 * 285.0).abs() < 0.5, "{}", room.area());
    }

    #[test]
    fn failed_detection_creates_nothing() {
        let mut doc = Document::default();
        let err = create(
            &mut doc,
            CreateParams {
                walls: vec![WallPath {
                    pts: vec![Point2::new(0.0, 0.0), Point2::new(400.0, 0.0)],
                    ..WallPath::default()
                }],
                rooms: vec![RoomSpec {
                    name: "X".into(),
                    at: Some(Point2::new(10.0, 10.0)),
                    pts: None,
                    ..RoomSpec::default()
                }],
                ..CreateParams::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("no space enclosed"));
        assert!(doc.home().walls.is_empty());
    }

    #[test]
    fn wall_types_and_finishes() {
        let mut doc = Document::default();
        let params: CreateParams = serde_json::from_str(
            r##"{"walls":[{"pts":[[0,0],[400,0],[400,300],[0,300]],"closed":true,"type":"drywall-95","sides":"#f2efe6"}],
                "rooms":[{"name":"Sala","at":[200,150],"floor_mat":"wood"}]}"##,
        )
        .unwrap();
        create(&mut doc, params).unwrap();
        let wall = &doc.home().walls[0];
        assert_eq!(
            (wall.wall_type.as_deref(), wall.thickness),
            (Some("drywall-95"), 9.5)
        );
        assert_eq!(wall.left_side.as_ref().unwrap().to_string(), "#f2efe6");
        assert_eq!(
            doc.home().rooms[0].floor_material,
            Some(Material::pattern(newera_core::Pattern::Wood))
        );

        let spec: UpdateSpec =
            serde_json::from_str(r#"{"id":"w1","type":"tijolo-14","right":"brick","left":"none"}"#)
                .unwrap();
        update(&mut doc, vec![spec]).unwrap();
        let wall = &doc.home().walls[0];
        assert!((wall.thickness - 19.0).abs() < 1e-9);
        assert!(wall.left_side.is_none());
        assert_eq!(wall.right_side.as_ref().unwrap().to_string(), "brick");

        let bad: UpdateSpec = serde_json::from_str(r#"{"id":"w1","type":"papelao"}"#).unwrap();
        assert!(
            update(&mut doc, vec![bad])
                .unwrap_err()
                .contains("unknown wall type")
        );
        let bad: UpdateSpec = serde_json::from_str(r#"{"id":"r5","floor_mat":"lava"}"#).unwrap();
        assert!(update(&mut doc, vec![bad]).is_err());
    }

    #[test]
    fn update_rejects_fields_of_other_kinds() {
        let mut doc = Document::default();
        square(&mut doc);
        update(
            &mut doc,
            vec![UpdateSpec {
                id: "w1".into(),
                t: Some(25.0),
                arc: Some(30.0),
                ..UpdateSpec::default()
            }],
        )
        .unwrap();
        let wall = doc.home().walls[0].clone();
        assert_eq!((wall.thickness, wall.arc_extent), (25.0, Some(30.0)));
        let err = update(
            &mut doc,
            vec![UpdateSpec {
                id: "w1".into(),
                text: Some("x".into()),
                ..UpdateSpec::default()
            }],
        )
        .unwrap_err();
        assert!(err.contains("does not apply"), "{err}");
    }

    #[test]
    fn wall_dimension_via_create() {
        let mut doc = Document::default();
        square(&mut doc);
        let ids = create(
            &mut doc,
            CreateParams {
                dims: vec![DimSpec {
                    wall: Some("w1".into()),
                    ..DimSpec::default()
                }],
                ..CreateParams::default()
            },
        )
        .unwrap();
        assert_eq!(ids, ["d5"]);
        assert!((doc.home().dimensions[0].length() - 400.0).abs() < 1e-9);
    }

    #[test]
    fn background_calibration_keeps_previous_values() {
        let mut doc = Document::default();
        let size = |_: &str| Ok([1000, 800]);
        set_background(
            &mut doc,
            &BackgroundParams {
                path: Some("planta.png".into()),
                calibrate: Some(Calibration {
                    a: Point2::new(0.0, 0.0),
                    b: Point2::new(200.0, 0.0),
                    cm: 500.0,
                }),
                ..BackgroundParams::default()
            },
            &size,
        )
        .unwrap();
        set_background(
            &mut doc,
            &BackgroundParams {
                opacity: Some(0.8),
                ..BackgroundParams::default()
            },
            &size,
        )
        .unwrap();
        let bg = doc.home().background.clone().unwrap();
        assert_eq!(
            (bg.cm_per_px, bg.opacity, bg.size_px),
            (2.5, 0.8, [1000, 800])
        );
    }
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct PlaceSpec {
    /// Catalog id, e.g. `bed-double` (see the `catalog` tool).
    #[serde(default)]
    pub cat: String,
    /// Instead of `cat`: path of an .obj/.gltf/.glb model to import.
    pub model: Option<String>,
    /// Center on the plan. Optional when `wall` is given.
    pub at: Option<Point2>,
    /// Put it in/against this wall (doors and windows cut it).
    pub wall: Option<String>,
    /// Distance along the wall from its start, cm (default: middle).
    pub along: Option<f64>,
    /// Clockwise degrees.
    pub angle: Option<f64>,
    /// Size overrides, cm.
    pub w: Option<f64>,
    pub d: Option<f64>,
    pub h: Option<f64>,
    pub elev: Option<f64>,
    pub name: Option<String>,
    pub color: Option<[u8; 3]>,
    pub mirror: Option<bool>,
    pub hinge_right: Option<bool>,
}

/// Places catalog pieces in one undoable step; returns their ids.
pub(crate) fn place(doc: &mut Document, items: Vec<PlaceSpec>) -> EditResult<Vec<String>> {
    if items.is_empty() {
        return Err("nothing to place".into());
    }
    let mut commands = Vec::with_capacity(items.len());
    let mut ids = Vec::with_capacity(items.len());
    for spec in items {
        let mut piece = if let Some(model) = &spec.model {
            let path = newera_core::resolve_project_path(doc.path(), model);
            let loaded = newera_catalog::load_model(&path)
                .map_err(|e| format!("{}: {e}", path.display()))?;
            let name = path
                .file_stem()
                .map_or_else(|| "Modelo".to_owned(), |n| n.to_string_lossy().into_owned());
            newera_core::Furniture {
                id: doc.new_furniture_id(),
                catalog: "imported".to_owned(),
                name,
                position: spec.at.unwrap_or_default(),
                elevation: 0.0,
                angle: 0.0,
                width: loaded.size[0],
                depth: loaded.size[1],
                height: loaded.size[2],
                mirrored: false,
                color: None,
                opening: None,
                model: Some(model.clone()),
                visible: true,
                level: None,
            }
        } else {
            let item = newera_catalog::find(&spec.cat).ok_or_else(|| {
                format!("unknown catalog id `{}` (use the catalog tool)", spec.cat)
            })?;
            item.instantiate(doc.new_furniture_id(), spec.at.unwrap_or_default())
        };
        piece.width = spec.w.unwrap_or(piece.width);
        piece.depth = spec.d.unwrap_or(piece.depth);
        piece.height = spec.h.unwrap_or(piece.height);
        piece.elevation = spec.elev.unwrap_or(piece.elevation);
        piece.name = spec.name.unwrap_or(piece.name);
        piece.color = spec.color;
        piece.mirrored = spec.mirror.unwrap_or(false);
        if let (Some(right), Some(opening)) = (spec.hinge_right, piece.opening.as_mut()) {
            opening.hinge_right = right;
        }
        match (&spec.wall, spec.at) {
            (Some(wall), _) => {
                let wall_id = wall.parse().map_err(|e| format!("{e}"))?;
                let wall = doc
                    .home()
                    .wall(wall_id)
                    .ok_or_else(|| format!("{wall} not found"))?
                    .clone();
                let along = spec
                    .along
                    .unwrap_or_else(|| wall.start.distance(wall.end) / 2.0);
                newera_core::align_to_wall(&mut piece, &wall, along);
                if let Some(depth) = spec.d {
                    piece.depth = depth;
                }
                if !piece.is_opening() {
                    // Back against the wall face, facing the middle of the house.
                    let offset = wall.thickness / 2.0 + piece.depth / 2.0;
                    let a = piece.angle.to_radians();
                    let front = (-a.sin(), a.cos());
                    let center = doc.home().bounds().map_or(piece.position, |(min, max)| {
                        Point2::new(min.x.midpoint(max.x), min.y.midpoint(max.y))
                    });
                    let toward_center = (center.x - piece.position.x) * front.0
                        + (center.y - piece.position.y) * front.1;
                    let side = if toward_center >= 0.0 { 1.0 } else { -1.0 };
                    if side < 0.0 {
                        piece.angle += 180.0;
                    }
                    piece.position = Point2::new(
                        piece.position.x + front.0 * side * offset,
                        piece.position.y + front.1 * side * offset,
                    );
                }
            }
            (None, Some(_)) => {}
            (None, None) => return Err(format!("`{}` needs `at` or `wall`", spec.cat)),
        }
        if let Some(angle) = spec.angle {
            piece.angle = angle;
        }
        ids.push(piece.id.to_string());
        commands.push(Command::insert(piece));
    }
    doc.execute(Command::Batch { commands }).map_err(core)?;
    Ok(ids)
}

#[cfg(test)]
mod place_tests {
    use super::*;

    #[test]
    fn doors_align_to_walls_and_furniture_backs_onto_them() {
        let mut doc = Document::default();
        create(
            &mut doc,
            CreateParams {
                walls: vec![WallPath {
                    pts: vec![
                        Point2::new(0.0, 0.0),
                        Point2::new(500.0, 0.0),
                        Point2::new(500.0, 400.0),
                        Point2::new(0.0, 400.0),
                    ],
                    closed: true,
                    ..WallPath::default()
                }],
                ..CreateParams::default()
            },
        )
        .unwrap();
        let ids = place(
            &mut doc,
            vec![
                PlaceSpec {
                    cat: "door".into(),
                    wall: Some("w1".into()),
                    along: Some(100.0),
                    ..PlaceSpec::default()
                },
                PlaceSpec {
                    cat: "sofa-3".into(),
                    wall: Some("w3".into()),
                    ..PlaceSpec::default()
                },
                PlaceSpec {
                    cat: "rug".into(),
                    at: Some(Point2::new(250.0, 200.0)),
                    w: Some(250.0),
                    ..PlaceSpec::default()
                },
            ],
        )
        .unwrap();
        assert_eq!(ids, ["f5", "f6", "f7"]);
        let home = doc.home();
        let door = &home.furniture[0];
        assert_eq!((door.position, door.depth), (Point2::new(100.0, 0.0), 15.0));
        assert_eq!(home.wall_cuts()[0].len(), 1, "door cuts w1");
        let sofa = &home.furniture[1];
        // w3 runs (500,400)->(0,400); the sofa sits inside the room with its back on the wall.
        assert!(
            (sofa.position.y - (400.0 - 7.5 - 45.0)).abs() < 1e-6,
            "{:?}",
            sofa.position
        );
        assert!(
            newera_core::check_layout(home).is_empty(),
            "{:?}",
            newera_core::check_layout(home)
        );
        assert!((home.furniture[2].width - 250.0).abs() < 1e-9);
        assert!(
            place(
                &mut doc,
                vec![PlaceSpec {
                    cat: "nope".into(),
                    at: Some(Point2::default()),
                    ..PlaceSpec::default()
                }]
            )
            .is_err()
        );
    }
}
