//! Parameter types and the edit logic behind the MCP tools, kept free of
//! protocol types so it can be unit-tested directly.

use newera_core::{
    BackgroundImage, Command, CoreError, Dimension, Document, Element, ElementId, Label, Material,
    Point2, Room, Wall, ops,
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
    /// Height cm at each point, for sloping walls and gables (overrides `h`).
    pub hs: Option<Vec<f64>>,
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
    /// Measure a wall: `side` picks the line, `chain` splits at openings.
    pub wall: Option<String>,
    /// `out` (outer face, default for chains), `in` (inner face) or `axis` (default).
    pub side: Option<String>,
    /// With `wall`: one dimension per wall piece and opening along the face.
    #[serde(default)]
    pub chain: bool,
    /// Clear width and depth of a room, e.g. `r5` (two dimensions).
    pub room: Option<String>,
    /// Offset cm from the measured line (default 40 for walls; a/b: left of a→b is positive).
    pub off: Option<f64>,
    /// Also draw it in 3D, at `elev` cm, tilted `pitch`° around its line (90 = offset upwards).
    #[serde(default)]
    pub in3d: bool,
    pub elev: Option<f64>,
    pub pitch: Option<f64>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct LabelSpec {
    pub text: String,
    pub at: Point2,
    /// Text height cm (default 24).
    pub size: Option<f64>,
    /// Clockwise degrees.
    pub angle: Option<f64>,
    /// Show in 3D tilted this much: 0 lying on the floor, 90 standing.
    pub pitch: Option<f64>,
    /// Height cm in 3D.
    pub elev: Option<f64>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct PolylineSpec {
    pub pts: Vec<Point2>,
    #[serde(default)]
    pub closed: bool,
    /// Line width cm (default 1).
    pub t: Option<f64>,
    /// `[r,g,b]` (default black).
    pub color: Option<[u8; 3]>,
    /// `solid`, `dot`, `dash`, `dash_dot`, `dash_dot_dot`.
    pub dash: Option<newera_core::DashStyle>,
    /// Smooth curve through the points.
    #[serde(default)]
    pub curved: bool,
    /// `[start, end]` arrows: `none`, `delta`, `open`, `disc`.
    pub arrows: Option<[newera_core::ArrowStyle; 2]>,
    /// Room divider: splits an open space into rooms without a wall (dashed).
    #[serde(default)]
    pub divider: bool,
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
    /// Free lines: annotations, arrows, electrical or plumbing runs.
    #[serde(default)]
    pub polylines: Vec<PolylineSpec>,
    /// Pitched roofs over a rectangle, built as one group of sloping panels.
    #[serde(default)]
    pub roofs: Vec<RoofSpec>,
    /// Solids from polygons: plan outlines raised by `h` (slabs, mezzanines,
    /// decks with any shape) or cross-sections swept from `a` to `b` (gables, ramps).
    #[serde(default)]
    pub solids: Vec<SolidSpec>,
    /// Every point is given in pixels of the background image (converted with its
    /// scale and offset); lengths (t, h, off) stay in cm.
    #[serde(default)]
    pub px: bool,
}

impl CreateParams {
    /// Converts every point with `f` (pixels of the background → cm).
    pub(crate) fn map_points(&mut self, f: &dyn Fn(Point2) -> Point2) {
        let all = |pts: &mut Vec<Point2>| pts.iter_mut().for_each(|p| *p = f(*p));
        for w in &mut self.walls {
            all(&mut w.pts);
        }
        for r in &mut self.rooms {
            if let Some(pts) = &mut r.pts {
                all(pts);
            }
            r.at = r.at.map(f);
        }
        for d in &mut self.dims {
            d.a = d.a.map(f);
            d.b = d.b.map(f);
        }
        for l in &mut self.labels {
            l.at = f(l.at);
        }
        for p in &mut self.polylines {
            all(&mut p.pts);
        }
        for solid in &mut self.solids {
            if let Some(pts) = &mut solid.pts {
                all(pts);
            }
            solid.a = solid.a.map(f);
            solid.b = solid.b.map(f);
        }
        for r in &mut self.roofs {
            all(&mut r.pts);
        }
    }
}

#[derive(Debug, Default, Clone, Copy, Deserialize, JsonSchema)]
pub(crate) struct SkylightSpec {
    pub at: Point2,
    pub w: Option<f64>,
    pub d: Option<f64>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct SolidSpec {
    /// Plan outline `[[x,y],…]` cm.
    pub pts: Option<Vec<Point2>>,
    /// Instead of `pts`: cross-section `[[u,z],…]` cm swept from `a` to `b`;
    /// u runs across the path (for a→b going down the plan, +u is +x),
    /// z is the height above the storey floor.
    pub profile: Option<Vec<Point2>>,
    pub a: Option<Point2>,
    pub b: Option<Point2>,
    /// Outline thickness cm (default 15).
    pub h: Option<f64>,
    /// Outline bottom above the floor cm (default 0).
    pub elev: Option<f64>,
    /// Finish (`wood`, `concrete`, `img:…`).
    pub mat: Option<String>,
    pub color: Option<[u8; 3]>,
    pub opacity: Option<f64>,
    pub name: Option<String>,
}

/// A piece built from a polygon.
fn solid(doc: &mut Document, spec: &SolidSpec) -> EditResult<newera_core::Furniture> {
    use newera_core::SolidShape;
    let bounds = |pts: &[Point2]| {
        pts.iter().fold(
            (
                Point2::new(f64::MAX, f64::MAX),
                Point2::new(f64::MIN, f64::MIN),
            ),
            |(lo, hi), p| {
                (
                    Point2::new(lo.x.min(p.x), lo.y.min(p.y)),
                    Point2::new(hi.x.max(p.x), hi.y.max(p.y)),
                )
            },
        )
    };
    let mut piece = newera_core::Furniture {
        id: doc.new_furniture_id(),
        catalog: "solid".into(),
        name: spec.name.clone().unwrap_or_else(|| "Sólido".into()),
        color: spec.color,
        ..newera_core::Furniture::default()
    };
    match (&spec.pts, &spec.profile) {
        (Some(pts), None) => {
            if pts.len() < 3 || newera_core::polygon_area(pts) < 1.0 {
                return Err("a solid outline needs at least 3 points enclosing an area".into());
            }
            let (lo, hi) = bounds(pts);
            let center = Point2::new(f64::midpoint(lo.x, hi.x), f64::midpoint(lo.y, hi.y));
            piece.position = center;
            piece.width = (hi.x - lo.x).max(0.1);
            piece.depth = (hi.y - lo.y).max(0.1);
            piece.height = spec.h.unwrap_or(15.0).max(0.1);
            piece.elevation = spec.elev.unwrap_or(0.0);
            piece.shape = Some(SolidShape::Outline(
                pts.iter()
                    .map(|p| [p.x - center.x, p.y - center.y])
                    .collect(),
            ));
        }
        (None, Some(profile)) => {
            let (Some(a), Some(b)) = (spec.a, spec.b) else {
                return Err("a profile solid needs `a` and `b` (the path it runs along)".into());
            };
            if profile.len() < 3 || newera_core::polygon_area(profile) < 1.0 {
                return Err("a profile needs at least 3 points enclosing an area".into());
            }
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let length = dx.hypot(dy);
            if length < 0.5 {
                return Err("`a` and `b` must be apart".into());
            }
            let (lo, hi) = bounds(profile);
            let mid_u = f64::midpoint(lo.x, hi.x);
            // The piece's local x axis (its width) for this heading.
            let right = (dy / length, -dx / length);
            piece.angle = (-dx).atan2(dy).to_degrees();
            piece.position = Point2::new(
                f64::midpoint(a.x, b.x) + right.0 * mid_u,
                f64::midpoint(a.y, b.y) + right.1 * mid_u,
            );
            piece.width = (hi.x - lo.x).max(0.1);
            piece.depth = length;
            piece.height = (hi.y - lo.y).max(0.1);
            piece.elevation = lo.y;
            piece.shape = Some(SolidShape::Profile(
                profile.iter().map(|p| [p.x - mid_u, p.y - lo.y]).collect(),
            ));
        }
        _ => return Err("a solid needs either `pts` or `profile`".into()),
    }
    if let Some(raw) = &spec.mat {
        piece.texture = material(raw)?;
    }
    piece.opacity = spec.opacity.filter(|o| *o < 1.0).map(|o| o.clamp(0.0, 1.0));
    Ok(piece)
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct RoofSpec {
    /// Rectangle corners in order; the ridge runs along pts[0]→pts[1].
    pub pts: Vec<Point2>,
    /// `gable` (two slopes, default; steep = A-frame) or `shed` (one slope rising
    /// from the pts[0]→pts[1] side).
    pub kind: Option<String>,
    /// Slope in degrees (default 30), unless `ridge_h` is given.
    pub pitch: Option<f64>,
    /// Height of the ridge (gable) or high side (shed) above the storey floor, cm.
    pub ridge_h: Option<f64>,
    /// Eave height above the storey floor, cm (default 250).
    pub h: Option<f64>,
    /// Eave and gable overhang, cm (default 30).
    pub overhang: Option<f64>,
    /// Panel thickness cm (default 12).
    pub t: Option<f64>,
    pub color: Option<[u8; 3]>,
    /// Also close the gable ends with sloping walls.
    #[serde(default)]
    pub gables: bool,
    /// Glazed openings cut through the roof: center `at:[x,y]` on the plan,
    /// `w` along the ridge and `d` across it (plan size), cm.
    #[serde(default)]
    pub skylights: Vec<SkylightSpec>,
    pub name: Option<String>,
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
        let mut heights = path.hs.clone();
        if let Some(hs) = &heights
            && hs.len() != pts.len()
        {
            return Err(format!("`hs` needs one height per point ({})", pts.len()));
        }
        if path.closed && pts.len() > 2 {
            pts.push(pts[0]);
            if let Some(hs) = &mut heights {
                hs.push(hs[0]);
            }
        }
        for (i, pair) in pts.windows(2).enumerate() {
            let (height, height_at_end) = match &heights {
                Some(hs) => (
                    hs[i],
                    ((hs[i + 1] - hs[i]).abs() > 1e-6).then_some(hs[i + 1]),
                ),
                None => (path.h.unwrap_or(Wall::DEFAULT_HEIGHT), None),
            };
            let mut wall = Wall {
                height,
                height_at_end,
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

    // Dividers already on this storey plus the ones created now.
    let mut dividers: Vec<newera_core::Polyline> = {
        let home = doc.home();
        home.level_view(home.current_level())
            .polylines
            .into_iter()
            .filter(|p| p.room_divider)
            .collect()
    };
    for spec in params.polylines.iter().filter(|p| p.divider) {
        let mut line = newera_core::Polyline::new(newera_core::PolylineId(0), spec.pts.clone());
        line.closed = spec.closed;
        dividers.push(line);
    }
    for spec in params.rooms {
        let (points, auto) = match (spec.pts, spec.at) {
            (Some(pts), _) => (pts, false),
            (None, Some(at)) => {
                let home = doc.home();
                let mut walls = home.level_view(home.current_level()).walls;
                walls.extend(new_walls.iter().cloned());
                let dividers: Vec<&newera_core::Polyline> = dividers.iter().collect();
                let points = newera_core::detect_room_with_dividers(&walls, &dividers, at)
                    .ok_or_else(|| {
                        format!("no space enclosed by walls around [{}, {}]", at.x, at.y)
                    })?;
                (points, true)
            }
            (None, None) => return Err(format!("room `{}` needs `pts` or `at`", spec.name)),
        };
        let mut room = Room::new(doc.new_room_id(), spec.name, points);
        room.auto = auto;
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
        let side = match spec.side.as_deref() {
            None | Some("axis") => ops::WallSide::Axis,
            Some("out") => ops::WallSide::Outer,
            Some("in") => ops::WallSide::Inner,
            Some(other) => return Err(format!("unknown side `{other}` (out, in, axis)")),
        };
        let dims: Vec<Dimension> = match (spec.wall, spec.room, spec.a, spec.b) {
            (Some(wall), _, _, _) => {
                let id = wall.parse().map_err(|e| format!("{e}"))?;
                let gap = spec.off.map_or(40.0, f64::abs);
                if spec.chain {
                    ops::wall_chain_dimensions(doc, id, side, gap).map_err(core)?
                } else {
                    vec![ops::wall_side_dimension(doc, id, side, gap).map_err(core)?]
                }
            }
            (None, Some(room), _, _) => {
                let id = room.parse().map_err(|e| format!("{e}"))?;
                ops::room_dimensions(doc, id).map_err(core)?
            }
            (None, None, Some(a), Some(b)) => vec![Dimension {
                id: doc.new_dimension_id(),
                start: a,
                end: b,
                offset: spec.off.unwrap_or(0.0),
                level: None,
                ..Default::default()
            }],
            _ => return Err("dimension needs `a` and `b`, `wall` or `room`".into()),
        };
        for mut dim in dims {
            if spec.in3d || spec.elev.is_some() || spec.pitch.is_some() {
                dim.visible_in_3d = true;
                dim.elevation = [spec.elev.unwrap_or(0.0); 2];
                dim.pitch = spec.pitch.unwrap_or(0.0);
            }
            ids.push(dim.id.to_string());
            commands.push(Command::insert(dim));
        }
    }

    for spec in params.labels {
        let label = Label {
            id: doc.new_label_id(),
            text: spec.text,
            position: spec.at,
            size: spec.size.unwrap_or(Label::DEFAULT_SIZE),
            angle: spec.angle.unwrap_or(0.0),
            level: None,
            pitch: spec.pitch.or(spec.elev.map(|_| 90.0)),
            elevation: spec.elev.unwrap_or(0.0),
            ..Default::default()
        };
        ids.push(label.id.to_string());
        commands.push(Command::insert(label));
    }

    for spec in &params.solids {
        let piece = solid(doc, spec)?;
        ids.push(piece.id.to_string());
        commands.push(Command::insert(piece));
    }

    for spec in params.roofs {
        let (roof, gables) = roof(doc, &spec)?;
        for wall in gables {
            ids.push(wall.id.to_string());
            commands.push(Command::insert(wall));
        }
        ids.push(roof.id.to_string());
        commands.push(Command::insert(roof));
    }

    for spec in params.polylines {
        let mut line = newera_core::Polyline::new(doc.new_polyline_id(), spec.pts);
        line.closed = spec.closed;
        line.thickness = spec.t.unwrap_or(1.0);
        line.color = spec.color.unwrap_or([0, 0, 0]);
        line.dash = spec.dash.unwrap_or_default();
        if spec.curved {
            line.join = newera_core::LineJoin::Curved;
        }
        if let Some([start, end]) = spec.arrows {
            line.start_arrow = start;
            line.end_arrow = end;
        }
        if spec.divider {
            line.room_divider = true;
            line.dash = spec.dash.unwrap_or(newera_core::DashStyle::Dash);
            line.color = spec.color.unwrap_or([120, 120, 120]);
        }
        ids.push(line.id.to_string());
        commands.push(Command::insert(line));
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
    /// Elevation cm (furniture, level, 3D label/dimension).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elev: Option<f64>,
    /// Room follows its walls (re-detected when they change); `pts` turns it off.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto: Option<bool>,
    /// Polyline is a room divider.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub divider: Option<bool>,
    /// Wall height at its end cm (sloping wall).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h_end: Option<f64>,
    /// Furniture tilt around its depth axis, degrees.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roll: Option<f64>,
    /// Furniture color `[r,g,b]`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 3]>,
    /// Furniture finish (`wood`, `img:…`, `none` clears).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mat: Option<String>,
    /// Furniture opacity 0..1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f64>,
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
    /// Furniture brand (references).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brand: Option<String>,
    /// Furniture commercial model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_name: Option<String>,
    /// Furniture product link.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Label bold text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    /// Label/dimension shown in 3D.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in3d: Option<bool>,
    /// 3D tilt degrees: label 0 flat, 90 standing; dimension around its line;
    /// furniture around its width axis.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pitch: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    /// Label alignment: `left`, `center`, `right`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<newera_core::TextAlign>,
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
            Element::Polyline(_) => &["pts", "t", "color", "level", "divider"],
            Element::Wall(_) => &[
                "a", "b", "t", "h", "h_end", "arc", "level", "type", "left", "right", "sides",
            ],
            Element::Room(_) => &[
                "name",
                "pts",
                "floor",
                "ceiling",
                "level",
                "floor_mat",
                "ceil_mat",
                "auto",
            ],
            Element::Dimension(_) => &["a", "b", "off", "level", "in3d", "elev", "pitch"],
            Element::Label(_) => &[
                "text", "at", "size", "angle", "level", "bold", "italic", "align", "color", "in3d",
                "elev", "pitch",
            ],
            Element::Level(_) => &["name", "elev", "h", "slab"],
            Element::Furniture(_) => &[
                "at",
                "angle",
                "w",
                "d",
                "h",
                "elev",
                "pitch",
                "roll",
                "name",
                "color",
                "mat",
                "opacity",
                "mirror",
                "visible",
                "hinge_right",
                "level",
                "brand",
                "model_name",
                "url",
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
            Element::Polyline(mut p) => {
                p.points = spec.pts.unwrap_or(p.points);
                p.thickness = spec.t.unwrap_or(p.thickness);
                if let Some([r, g, b]) = spec.color {
                    p.color = [r, g, b];
                }
                if let Some(divider) = spec.divider {
                    p.room_divider = divider;
                }
                Element::Polyline(p)
            }
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
                if let Some(end) = spec.h_end {
                    w.height_at_end = ((end - w.height).abs() > 1e-6).then_some(end);
                }
                if let Some(arc) = spec.arc {
                    w.arc_extent = (arc != 0.0).then_some(arc);
                }
                Element::Wall(w)
            }
            Element::Room(mut r) => {
                r.name = spec.name.unwrap_or(r.name);
                if spec.pts.is_some() {
                    r.auto = false;
                }
                if spec.auto == Some(true) {
                    let home = doc.home();
                    let view = home.level_view(r.level);
                    let dividers: Vec<&newera_core::Polyline> =
                        view.polylines.iter().filter(|l| l.room_divider).collect();
                    let inside = newera_core::interior_point(&r.points)
                        .ok_or("room outline has too few points")?;
                    r.points =
                        newera_core::detect_room_with_dividers(&view.walls, &dividers, inside)
                            .ok_or("no space enclosed by walls around this room")?;
                }
                r.auto = spec.auto.unwrap_or(r.auto);
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
                d.visible_in_3d = spec
                    .in3d
                    .unwrap_or(d.visible_in_3d || spec.elev.is_some() || spec.pitch.is_some());
                if let Some(elev) = spec.elev {
                    d.elevation = [elev; 2];
                }
                d.pitch = spec.pitch.unwrap_or(d.pitch);
                Element::Dimension(d)
            }
            Element::Label(mut l) => {
                l.text = spec.text.unwrap_or(l.text);
                l.position = spec.at.unwrap_or(l.position);
                l.size = spec.size.unwrap_or(l.size);
                l.angle = spec.angle.unwrap_or(l.angle);
                l.bold = spec.bold.unwrap_or(l.bold);
                l.italic = spec.italic.unwrap_or(l.italic);
                l.align = spec.align.unwrap_or(l.align);
                l.color = spec.color.or(l.color);
                l.elevation = spec.elev.unwrap_or(l.elevation);
                l.pitch = match (spec.in3d, spec.pitch) {
                    (Some(false), _) => None,
                    (_, Some(pitch)) => Some(pitch),
                    (Some(true), None) => l.pitch.or(Some(90.0)),
                    (None, None) => l.pitch.or(spec.elev.map(|_| 90.0)),
                };
                Element::Label(l)
            }
            Element::Furniture(mut f) => {
                f.position = spec.at.unwrap_or(f.position);
                f.angle = spec.angle.unwrap_or(f.angle);
                f.width = spec.w.unwrap_or(f.width);
                f.depth = spec.d.unwrap_or(f.depth);
                f.height = spec.h.unwrap_or(f.height);
                f.elevation = spec.elev.unwrap_or(f.elevation);
                f.pitch = spec.pitch.unwrap_or(f.pitch);
                f.roll = spec.roll.unwrap_or(f.roll);
                f.name = spec.name.unwrap_or(f.name);
                f.color = spec.color.or(f.color);
                if let Some(raw) = &spec.mat {
                    f.texture = material(raw)?;
                }
                if let Some(o) = spec.opacity {
                    f.opacity = (o < 1.0).then_some(o.clamp(0.0, 1.0));
                }
                f.mirrored = spec.mirror.unwrap_or(f.mirrored);
                f.visible = spec.visible.unwrap_or(f.visible);
                let text = |v: Option<String>, old: Option<String>| match v {
                    Some(v) if v.is_empty() => None,
                    Some(v) => Some(v),
                    None => old,
                };
                f.info.brand = text(spec.brand, f.info.brand.take());
                f.info.model_name = text(spec.model_name, f.info.model_name.take());
                f.info.url = text(spec.url, f.info.url.take());
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

#[derive(Debug, Default, Clone, Deserialize, serde::Serialize, JsonSchema)]
pub(crate) struct PlaceSpec {
    /// Catalog id, e.g. `bed-double` (see the `catalog` tool).
    #[serde(default)]
    pub cat: String,
    /// Instead of `cat`: path of an .obj/.gltf/.glb model to import.
    pub model: Option<String>,
    /// Center on the plan. Optional when `wall` is given; a door or window
    /// placed `at` a point near a wall snaps into it.
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
    /// Tilt around the width axis / the depth axis, degrees (rafters, ramps, panels).
    pub pitch: Option<f64>,
    pub roll: Option<f64>,
    pub name: Option<String>,
    pub color: Option<[u8; 3]>,
    /// Finish over the whole piece: `wood`, `marble #222 60`, `img:photo.jpg 100x80`.
    pub mat: Option<String>,
    /// 0..1; e.g. 0.3 for glass.
    pub opacity: Option<f64>,
    pub mirror: Option<bool>,
    pub hinge_right: Option<bool>,
    /// Doors: a point `[x,y]` on the side the leaf swings into (e.g. inside the bathroom).
    pub into: Option<Point2>,
    /// With `cat:"beam"`: end points `[x,y,z]` cm (z above the storey floor);
    /// `w`×`h` is the section (default 10×20).
    pub a: Option<[f64; 3]>,
    pub b: Option<[f64; 3]>,
}

/// A box from `a` to `b` (plan cm, height cm above the storey floor) with a
/// `w` × `h` section: beams, rafters, posts, braces and sloping panels.
fn beam(
    doc: &mut Document,
    a: [f64; 3],
    b: [f64; 3],
    w: f64,
    h: f64,
) -> EditResult<newera_core::Furniture> {
    let (dx, dy, dz) = (b[0] - a[0], b[1] - a[1], b[2] - a[2]);
    let horizontal = dx.hypot(dy);
    let length = horizontal.hypot(dz);
    if length < 0.5 {
        return Err("a beam needs two distinct points".into());
    }
    let item = newera_catalog::find("box").ok_or("catalog has no box")?;
    let center = [
        f64::midpoint(a[0], b[0]),
        f64::midpoint(a[1], b[1]),
        f64::midpoint(a[2], b[2]),
    ];
    let mut piece = item.instantiate(doc.new_furniture_id(), Point2::new(center[0], center[1]));
    piece.name = "Viga".into();
    piece.width = w;
    piece.depth = length;
    piece.height = h;
    // The depth axis points from a to b on the plan; pitch lifts its far end.
    piece.angle = if horizontal > 1e-6 {
        (-dx).atan2(dy).to_degrees()
    } else {
        0.0
    };
    piece.pitch = -(dz / length).asin().to_degrees();
    if horizontal <= 1e-6 {
        piece.pitch = if dz > 0.0 { -90.0 } else { 90.0 };
    }
    piece.elevation = center[2] - h / 2.0;
    Ok(piece)
}

/// A pitched roof over a rectangle: a group of sloping panels, plus gable
/// walls when asked.
fn roof(doc: &mut Document, spec: &RoofSpec) -> EditResult<(newera_core::Furniture, Vec<Wall>)> {
    let [p0, p1, p2, ..] = spec.pts[..] else {
        return Err("a roof needs the 4 corners of a rectangle in `pts`".into());
    };
    let along = (p1.x - p0.x, p1.y - p0.y);
    let length = along.0.hypot(along.1);
    if length < 1.0 {
        return Err("roof corners pts[0] and pts[1] must differ".into());
    }
    let u = (along.0 / length, along.1 / length);
    // Across the ridge, toward pts[2].
    let mut n = (-u.1, u.0);
    let span = (p2.x - p0.x) * n.0 + (p2.y - p0.y) * n.1;
    if span < 0.0 {
        n = (-n.0, -n.1);
    }
    let span = span.abs();
    if span < 1.0 {
        return Err("roof rectangle has no width".into());
    }
    let shed = match spec.kind.as_deref().unwrap_or("gable") {
        "gable" | "a-frame" => false,
        "shed" => true,
        other => return Err(format!("unknown roof kind `{other}` (gable, shed)")),
    };
    let eave = spec.h.unwrap_or(Wall::DEFAULT_HEIGHT);
    let run = if shed { span } else { span / 2.0 };
    let rise = match (spec.ridge_h, spec.pitch) {
        (Some(ridge), _) => ridge - eave,
        (None, pitch) => run * pitch.unwrap_or(30.0).clamp(1.0, 85.0).to_radians().tan(),
    };
    if rise <= 0.0 {
        return Err("the ridge must be higher than the eaves".into());
    }
    let slope = rise / run;
    let overhang = spec.overhang.unwrap_or(30.0).max(0.0);
    let t = spec.t.unwrap_or(12.0).max(1.0);
    let at = |s: f64, k: f64| Point2::new(p0.x + u.0 * s + n.0 * k, p0.y + u.1 * s + n.1 * k);
    let mid = length / 2.0;
    // Each panel runs from its eave (overhang included) up to the ridge line,
    // with its centerline half a thickness above the rafters.
    let slopes: Vec<(f64, f64)> = if shed {
        vec![(0.0, span)]
    } else {
        vec![(0.0, span / 2.0), (span, span / 2.0)]
    };
    let mut panels = Vec::new();
    let lift = t / 2.0 * (1.0 + slope * slope).sqrt();
    let color = spec.color.unwrap_or([150, 75, 55]);
    // Skylights in plan coordinates along the ridge (s) and across it (k).
    let holes: Vec<(f64, f64, f64, f64)> = spec
        .skylights
        .iter()
        .map(|h| {
            let s = (h.at.x - p0.x) * u.0 + (h.at.y - p0.y) * u.1;
            let k = (h.at.x - p0.x) * n.0 + (h.at.y - p0.y) * n.1;
            let (w, d) = (h.w.unwrap_or(80.0) / 2.0, h.d.unwrap_or(100.0) / 2.0);
            (s - w, s + w, k - d, k + d)
        })
        .collect();
    for (from, to) in slopes {
        let dir = (to - from).signum();
        // e: horizontal distance up the slope from the eave line.
        let e_of = |k: f64| (k - from) * dir;
        let k_of = |e: f64| from + dir * e;
        let mut make = |doc: &mut Document,
                        s0: f64,
                        s1: f64,
                        e0: f64,
                        e1: f64,
                        glass: bool|
         -> EditResult<()> {
            if s1 - s0 < 0.5 || e1 - e0 < 0.5 {
                return Ok(());
            }
            let sm = f64::midpoint(s0, s1);
            let (a, b) = (at(sm, k_of(e0)), at(sm, k_of(e1)));
            let thickness = if glass { (t * 0.3).max(1.0) } else { t };
            let mut panel = beam(
                doc,
                [a.x, a.y, eave + e0 * slope + lift],
                [b.x, b.y, eave + e1 * slope + lift],
                s1 - s0,
                thickness,
            )?;
            if glass {
                panel.name = "Claraboia".into();
                panel.color = Some([168, 206, 226]);
                panel.opacity = Some(0.35);
            } else {
                panel.name = "Água do telhado".into();
                panel.color = Some(color);
            }
            panels.push(panel);
            Ok(())
        };
        let run = (to - from).abs();
        let (start, end) = (-overhang, length + overhang);
        let mut mine: Vec<(f64, f64, f64, f64)> = holes
            .iter()
            .map(|&(s0, s1, k0, k1)| {
                let (e0, e1) = (e_of(k0).min(e_of(k1)), e_of(k0).max(e_of(k1)));
                (s0.max(start), s1.min(end), e0.max(0.0), e1.min(run))
            })
            .filter(|&(s0, s1, e0, e1)| s1 > s0 && e1 > e0)
            .collect();
        mine.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut cursor = start;
        for &(s0, s1, e0, e1) in &mine {
            if s0 < cursor {
                return Err("skylights on the same slope overlap along the ridge".into());
            }
            make(doc, cursor, s0, -overhang, run, false)?;
            make(doc, s0, s1, -overhang, e0, false)?;
            make(doc, s0, s1, e0, e1, true)?;
            make(doc, s0, s1, e1, run, false)?;
            cursor = s1;
        }
        make(doc, cursor, end, -overhang, run, false)?;
    }
    let mut gables = Vec::new();
    if spec.gables {
        for s in [0.0, length] {
            let heights = if shed {
                vec![eave, eave + rise]
            } else {
                vec![eave, eave + rise, eave]
            };
            let pts: Vec<Point2> = if shed {
                vec![at(s, 0.0), at(s, span)]
            } else {
                vec![at(s, 0.0), at(s, span / 2.0), at(s, span)]
            };
            for (i, pair) in pts.windows(2).enumerate() {
                let mut wall = Wall::new(doc.new_wall_id(), pair[0], pair[1]);
                wall.height = heights[i].max(1.0);
                wall.height_at_end = Some(heights[i + 1].max(1.0));
                gables.push(wall);
            }
        }
    }
    let item = newera_catalog::find("box").ok_or("catalog has no box")?;
    let center = at(mid, span / 2.0);
    let mut group = item.instantiate(doc.new_furniture_id(), center);
    group.catalog = "group".into();
    group.name = spec.name.clone().unwrap_or_else(|| "Telhado".into());
    group.angle = (-n.0).atan2(n.1).to_degrees();
    group.width = length + 2.0 * overhang;
    group.depth = span + 2.0 * overhang;
    group.elevation = eave - overhang * slope;
    group.height = rise + overhang * slope + 2.0 * t;
    group.children = panels;
    Ok((group, gables))
}

/// The wall of the current storey closest to `at` within snapping distance,
/// with the distance along it.
fn nearest_wall(doc: &Document, at: Point2) -> Option<(newera_core::Wall, f64)> {
    let home = doc.home();
    let level = home.current_level();
    home.walls
        .iter()
        .filter(|w| home.on_level(w.level, level))
        .filter_map(|w| {
            let (a, b) = (w.start, w.end);
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let len2 = (dx * dx + dy * dy).max(1e-9);
            let t = (((at.x - a.x) * dx + (at.y - a.y) * dy) / len2).clamp(0.0, 1.0);
            let p = Point2::new(a.x + dx * t, a.y + dy * t);
            let distance = p.distance(at);
            (distance <= (w.thickness / 2.0 + 30.0)).then(|| (distance, w, t * len2.sqrt()))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, w, along)| (w.clone(), along))
}

/// Turns a door so its leaf swings to the side of `into`, keeping the hinge
/// at the same end of the opening.
fn swing_into(piece: &mut newera_core::Furniture, into: Point2) {
    let Some(swing) = newera_core::door_swing(piece) else {
        return;
    };
    #[allow(clippy::cast_precision_loss)]
    let n = swing.len() as f64;
    let (cx, cy) = swing
        .iter()
        .fold((0.0, 0.0), |(x, y), p| (x + p.x / n, y + p.y / n));
    let a = piece.angle.to_radians();
    let normal = (-a.sin(), a.cos());
    let side =
        |x: f64, y: f64| (x - piece.position.x) * normal.0 + (y - piece.position.y) * normal.1;
    if side(cx, cy) * side(into.x, into.y) < 0.0 {
        piece.angle += 180.0;
        if let Some(opening) = piece.opening.as_mut() {
            opening.hinge_right = !opening.hinge_right;
        }
    }
}

/// Fills what each item leaves out from `defaults` (e.g. the same `cat`,
/// section and color for a row of rafters).
pub(crate) fn with_defaults(
    items: Vec<PlaceSpec>,
    defaults: Option<&PlaceSpec>,
) -> EditResult<Vec<PlaceSpec>> {
    let Some(defaults) = defaults else {
        return Ok(items);
    };
    let base = serde_json::to_value(defaults).map_err(|e| e.to_string())?;
    items
        .into_iter()
        .map(|item| {
            let mut value = serde_json::to_value(&item).map_err(|e| e.to_string())?;
            if let (Some(target), Some(base)) = (value.as_object_mut(), base.as_object()) {
                for (key, default) in base {
                    let missing = match target.get(key) {
                        None | Some(serde_json::Value::Null) => true,
                        Some(serde_json::Value::String(s)) => s.is_empty(),
                        _ => false,
                    };
                    if missing && !default.is_null() {
                        target.insert(key.clone(), default.clone());
                    }
                }
            }
            serde_json::from_value(value).map_err(|e| e.to_string())
        })
        .collect()
}

/// Places catalog pieces in one undoable step; returns their ids.
pub(crate) fn place(doc: &mut Document, items: Vec<PlaceSpec>) -> EditResult<Vec<String>> {
    if items.is_empty() {
        return Err("nothing to place".into());
    }
    let mut commands = Vec::with_capacity(items.len());
    let mut ids = Vec::with_capacity(items.len());
    for spec in items {
        if spec.cat == "beam" && (spec.a.is_some() || spec.b.is_some()) {
            let (Some(a), Some(b)) = (spec.a, spec.b) else {
                return Err("a beam needs `a` and `b` as [x,y,z]".into());
            };
            let mut piece = beam(doc, a, b, spec.w.unwrap_or(10.0), spec.h.unwrap_or(20.0))?;
            piece.name = spec.name.clone().unwrap_or(piece.name);
            piece.color = spec.color.or(Some([176, 132, 92]));
            ids.push(piece.id.to_string());
            commands.push(Command::insert(piece));
            continue;
        }
        let mut piece = if let Some(model) = &spec.model {
            let path = doc.resolve_asset(model);
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
                ..Default::default()
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
        piece.pitch = spec.pitch.unwrap_or(piece.pitch);
        piece.roll = spec.roll.unwrap_or(piece.roll);
        piece.name = spec.name.unwrap_or(piece.name);
        piece.color = spec.color.or(piece.color);
        if let Some(raw) = &spec.mat {
            piece.texture = material(raw)?;
        }
        if let Some(o) = spec.opacity {
            piece.opacity = (o < 1.0).then_some(o.clamp(0.0, 1.0));
        }
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
            (None, Some(at)) => {
                if piece.is_opening()
                    && let Some((wall, along)) = nearest_wall(doc, at)
                {
                    newera_core::align_to_wall(&mut piece, &wall, along);
                    if let Some(depth) = spec.d {
                        piece.depth = depth;
                    }
                }
            }
            (None, None) => return Err(format!("`{}` needs `at` or `wall`", spec.cat)),
        }
        if let Some(into) = spec.into {
            swing_into(&mut piece, into);
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
    fn doors_snap_to_the_nearest_wall_and_swing_into_a_chosen_side() {
        let mut doc = Document::default();
        create(
            &mut doc,
            CreateParams {
                walls: vec![WallPath {
                    pts: vec![Point2::new(0.0, 0.0), Point2::new(500.0, 0.0)],
                    ..WallPath::default()
                }],
                ..CreateParams::default()
            },
        )
        .unwrap();
        for into_y in [100.0, -100.0] {
            let ids = place(
                &mut doc,
                vec![PlaceSpec {
                    cat: "door".into(),
                    at: Some(Point2::new(203.0, 12.0)),
                    into: Some(Point2::new(200.0, into_y)),
                    ..PlaceSpec::default()
                }],
            )
            .unwrap();
            let door = doc
                .home()
                .furniture
                .iter()
                .find(|f| f.id.to_string() == ids[0])
                .unwrap()
                .clone();
            // Snapped onto the wall axis, as thick as the wall.
            assert!(door.position.y.abs() < 1e-6 && (door.position.x - 203.0).abs() < 1e-6);
            assert!((door.depth - doc.home().walls[0].thickness).abs() < 1e-9);
            let swing = newera_core::door_swing(&door).unwrap();
            #[allow(clippy::cast_precision_loss)]
            let cy = swing.iter().map(|p| p.y).sum::<f64>() / swing.len() as f64;
            assert!(cy * into_y > 0.0, "swings toward {into_y}: {cy}");
        }
        // Far from every wall a door stays where it was put.
        let ids = place(
            &mut doc,
            vec![PlaceSpec {
                cat: "door".into(),
                at: Some(Point2::new(200.0, 300.0)),
                ..PlaceSpec::default()
            }],
        )
        .unwrap();
        let door = doc
            .home()
            .furniture
            .iter()
            .find(|f| f.id.to_string() == ids[0])
            .unwrap();
        assert!((door.position.y - 300.0).abs() < 1e-6);
    }

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
