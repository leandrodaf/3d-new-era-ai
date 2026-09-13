//! Parameter types and the edit logic behind the MCP tools, kept free of
//! protocol types so it can be unit-tested directly.

use newera_core::{
    BackgroundImage, Command, CoreError, Dimension, Document, Element, ElementId, Label, Point2,
    Room, Wall, detect_room, ops,
};
use schemars::JsonSchema;
use serde::Deserialize;

pub(crate) type EditResult<T> = Result<T, String>;

#[allow(clippy::needless_pass_by_value)] // used as `map_err(core)`
fn core(err: CoreError) -> String {
    err.to_string()
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
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct RoomSpec {
    pub name: String,
    /// Floor polygon. Omit and give `at` to detect the room enclosed by walls.
    pub pts: Option<Vec<Point2>>,
    /// A point inside a space enclosed by walls (auto-detects the polygon).
    pub at: Option<Point2>,
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
        let mut pts = path.pts;
        if path.closed && pts.len() > 2 {
            pts.push(pts[0]);
        }
        for pair in pts.windows(2) {
            let wall = Wall {
                thickness: path.t.unwrap_or(Wall::DEFAULT_THICKNESS),
                height: path.h.unwrap_or(Wall::DEFAULT_HEIGHT),
                arc_extent: path.arc.filter(|a| *a != 0.0),
                ..Wall::new(doc.new_wall_id(), pair[0], pair[1])
            };
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
        let room = Room::new(doc.new_room_id(), spec.name, points);
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
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct UpdateSpec {
    pub id: String,
    /// Wall start / dimension start.
    pub a: Option<Point2>,
    /// Wall end / dimension end.
    pub b: Option<Point2>,
    /// Wall thickness cm.
    pub t: Option<f64>,
    /// Wall height cm.
    pub h: Option<f64>,
    /// Wall arc degrees (0 = straight).
    pub arc: Option<f64>,
    /// Room name.
    pub name: Option<String>,
    /// Room polygon.
    pub pts: Option<Vec<Point2>>,
    /// Room floor visible.
    pub floor: Option<bool>,
    /// Room ceiling visible.
    pub ceiling: Option<bool>,
    /// Label text.
    pub text: Option<String>,
    /// Label position.
    pub at: Option<Point2>,
    /// Label size cm.
    pub size: Option<f64>,
    /// Label angle degrees.
    pub angle: Option<f64>,
    /// Dimension offset cm.
    pub off: Option<f64>,
}

pub(crate) fn update(doc: &mut Document, items: Vec<UpdateSpec>) -> EditResult<()> {
    let mut commands = Vec::with_capacity(items.len());
    for spec in items {
        let id: ElementId = spec.id.parse().map_err(|e| format!("{e}"))?;
        let element = doc
            .home()
            .element(id)
            .ok_or_else(|| format!("{id} not found"))?;
        let reject = |field: &str, present: bool| -> EditResult<()> {
            if present {
                Err(format!("`{field}` does not apply to {id}"))
            } else {
                Ok(())
            }
        };
        let updated = match element {
            Element::Wall(mut w) => {
                reject(
                    "name/pts/floor/ceiling/text/at/size/angle/off",
                    spec.name.is_some()
                        || spec.pts.is_some()
                        || spec.floor.is_some()
                        || spec.ceiling.is_some()
                        || spec.text.is_some()
                        || spec.at.is_some()
                        || spec.size.is_some()
                        || spec.angle.is_some()
                        || spec.off.is_some(),
                )?;
                w.start = spec.a.unwrap_or(w.start);
                w.end = spec.b.unwrap_or(w.end);
                w.thickness = spec.t.unwrap_or(w.thickness);
                w.height = spec.h.unwrap_or(w.height);
                if let Some(arc) = spec.arc {
                    w.arc_extent = (arc != 0.0).then_some(arc);
                }
                Element::Wall(w)
            }
            Element::Room(mut r) => {
                reject(
                    "a/b/t/h/arc/text/at/size/angle/off",
                    spec.a.is_some()
                        || spec.b.is_some()
                        || spec.t.is_some()
                        || spec.h.is_some()
                        || spec.arc.is_some()
                        || spec.text.is_some()
                        || spec.at.is_some()
                        || spec.size.is_some()
                        || spec.angle.is_some()
                        || spec.off.is_some(),
                )?;
                r.name = spec.name.unwrap_or(r.name);
                r.points = spec.pts.unwrap_or(r.points);
                r.floor_visible = spec.floor.unwrap_or(r.floor_visible);
                r.ceiling_visible = spec.ceiling.unwrap_or(r.ceiling_visible);
                Element::Room(r)
            }
            Element::Dimension(mut d) => {
                reject(
                    "t/h/arc/name/pts/floor/ceiling/text/at/size/angle",
                    spec.t.is_some()
                        || spec.h.is_some()
                        || spec.arc.is_some()
                        || spec.name.is_some()
                        || spec.pts.is_some()
                        || spec.floor.is_some()
                        || spec.ceiling.is_some()
                        || spec.text.is_some()
                        || spec.at.is_some()
                        || spec.size.is_some()
                        || spec.angle.is_some(),
                )?;
                d.start = spec.a.unwrap_or(d.start);
                d.end = spec.b.unwrap_or(d.end);
                d.offset = spec.off.unwrap_or(d.offset);
                Element::Dimension(d)
            }
            Element::Label(mut l) => {
                reject(
                    "a/b/t/h/arc/name/pts/floor/ceiling/off",
                    spec.a.is_some()
                        || spec.b.is_some()
                        || spec.t.is_some()
                        || spec.h.is_some()
                        || spec.arc.is_some()
                        || spec.name.is_some()
                        || spec.pts.is_some()
                        || spec.floor.is_some()
                        || spec.ceiling.is_some()
                        || spec.off.is_some(),
                )?;
                l.text = spec.text.unwrap_or(l.text);
                l.position = spec.at.unwrap_or(l.position);
                l.size = spec.size.unwrap_or(l.size);
                l.angle = spec.angle.unwrap_or(l.angle);
                Element::Label(l)
            }
        };
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
                }],
                ..CreateParams::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("no space enclosed"));
        assert!(doc.home().walls.is_empty());
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
