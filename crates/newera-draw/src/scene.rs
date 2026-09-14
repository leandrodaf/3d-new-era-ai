//! Backend-agnostic floor plan scene.
//!
//! [`plan_scene`] turns a [`Home`] into styled primitives in plan
//! coordinates (cm). Screen, PNG and SVG backends only map coordinates and
//! paint, so every output looks the same.

use std::collections::HashSet;

use newera_catalog::{SymbolShape, plan_symbol};
use newera_core::{
    Compass, Dimension, ElementId, Furniture, Home, Label, LengthUnit, Point2, Room, cut_outline,
    polygon_centroid, triangulate,
};

/// Straight (non-premultiplied) RGBA color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color(pub [u8; 4]);

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self([r, g, b, 255])
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self([r, g, b, a])
    }

    #[must_use]
    pub fn with_alpha(self, alpha: f64) -> Self {
        let [r, g, b, a] = self.0;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let a = (f64::from(a) * alpha.clamp(0.0, 1.0)).round() as u8;
        Self([r, g, b, a])
    }
}

/// Size that is either fixed on screen or scales with the drawing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Size {
    /// Screen/output pixels, independent of zoom.
    Px(f32),
    /// Plan centimeters, so it grows and shrinks with zoom.
    Cm(f64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Center,
    /// Horizontally centered, baseline just above the anchor.
    Above,
    /// The last line's baseline sits on the anchor; lines start at it.
    BaselineLeft,
    /// The last line's baseline sits on the anchor, centered on it.
    BaselineCenter,
    /// The last line's baseline sits on the anchor; lines end at it.
    BaselineRight,
}

impl Align {
    pub fn from_text_align(align: newera_core::TextAlign) -> Self {
        match align {
            newera_core::TextAlign::Left => Self::BaselineLeft,
            newera_core::TextAlign::Center => Self::BaselineCenter,
            newera_core::TextAlign::Right => Self::BaselineRight,
        }
    }

    /// Horizontal placement: 0 left, 0.5 center, 1 right.
    pub fn horizontal(self) -> f32 {
        match self {
            Self::BaselineLeft => 0.0,
            Self::BaselineRight => 1.0,
            Self::Center | Self::Above | Self::BaselineCenter => 0.5,
        }
    }

    pub fn is_baseline(self) -> bool {
        matches!(
            self,
            Self::BaselineLeft | Self::BaselineCenter | Self::BaselineRight
        )
    }
}

/// Extra text decoration.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TextLook {
    pub bold: bool,
    pub italic: bool,
    /// Halo around the letters.
    pub outline: Option<Color>,
}

/// Font sizes in styles are em sizes; primitives take the glyph height
/// (ascent to descent), which is this much larger for the plan font.
pub const EM_TO_HEIGHT: f64 = 1.12;

#[derive(Debug, Clone, PartialEq)]
pub enum Primitive {
    /// Filled polygon; `triangles` index `points` and are precomputed so
    /// backends never re-triangulate per frame.
    Fill {
        points: Vec<Point2>,
        triangles: Vec<[usize; 3]>,
        color: Color,
    },
    Line {
        points: Vec<Point2>,
        closed: bool,
        color: Color,
        width: Size,
    },
    Text {
        text: String,
        position: Point2,
        size: Size,
        color: Color,
        align: Align,
        /// Clockwise degrees.
        angle: f64,
        look: TextLook,
    },
    /// Image covering `min`..`max` (plan cm), turned `angle` degrees
    /// clockwise around the box center.
    Image {
        path: String,
        min: Point2,
        max: Point2,
        opacity: f64,
        angle: f64,
    },
}

/// A primitive tagged with the element it depicts, for hit-testing.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub owner: Option<ElementId>,
    pub primitive: Primitive,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Scene {
    pub items: Vec<Item>,
}

impl Scene {
    fn push(&mut self, owner: Option<ElementId>, primitive: Primitive) {
        self.items.push(Item { owner, primitive });
    }

    fn fill(&mut self, owner: Option<ElementId>, points: &[Point2], color: Color) {
        let triangles = triangulate(points);
        if !triangles.is_empty() {
            self.push(
                owner,
                Primitive::Fill {
                    points: points.to_vec(),
                    triangles,
                    color,
                },
            );
        }
    }

    /// Bounds of all geometry (`min`, `max`), ignoring text extents.
    pub fn bounds(&self) -> Option<(Point2, Point2)> {
        let points = self.items.iter().flat_map(|item| match &item.primitive {
            Primitive::Fill { points, .. } | Primitive::Line { points, .. } => points.clone(),
            Primitive::Text {
                text,
                position,
                size: Size::Cm(size),
                align,
                ..
            } => {
                // Rough extent so fitting includes long labels.
                let lines = text.lines().count().max(1);
                #[allow(clippy::cast_precision_loss)]
                let (w, h) = (
                    text.lines().map(|l| l.chars().count()).max().unwrap_or(0) as f64 * size * 0.5,
                    lines as f64 * size * 1.2,
                );
                let left = position.x - w * f64::from(align.horizontal());
                vec![
                    Point2::new(left, position.y - h),
                    Point2::new(left + w, position.y + size * 0.3),
                ]
            }
            Primitive::Text { position, .. } => vec![*position],
            Primitive::Image {
                min, max, angle, ..
            } => {
                let center = Point2::new(min.x.midpoint(max.x), min.y.midpoint(max.y));
                let (sin, cos) = angle.to_radians().sin_cos();
                [
                    (min.x, min.y),
                    (max.x, min.y),
                    (max.x, max.y),
                    (min.x, max.y),
                ]
                .iter()
                .map(|&(x, y)| {
                    let (dx, dy) = (x - center.x, y - center.y);
                    Point2::new(
                        center.x + dx * cos - dy * sin,
                        center.y + dx * sin + dy * cos,
                    )
                })
                .collect()
            }
        });
        points.fold(None, |acc, p| {
            let (min, max) = acc.unwrap_or((p, p));
            Some((
                Point2::new(min.x.min(p.x), min.y.min(p.y)),
                Point2::new(max.x.max(p.x), max.y.max(p.y)),
            ))
        })
    }
}

/// Colors of the plan. One place to restyle every output.
#[derive(Debug, Clone, PartialEq)]
pub struct Palette {
    pub paper: Color,
    pub grid_minor: Color,
    pub grid_major: Color,
    /// Masonry walls (and walls without a type).
    pub wall: Color,
    /// Fill under the wall hatch.
    pub wall_background: Color,
    pub wall_drywall: Color,
    pub wall_concrete: Color,
    pub wall_glass: Color,
    pub wall_wood: Color,
    pub room_fill: Color,
    pub room_line: Color,
    pub room_text: Color,
    pub dimension: Color,
    pub label: Color,
    pub compass: Color,
    pub selection: Color,
    pub furniture_fill: Color,
    pub furniture_detail: Color,
    pub furniture_line: Color,
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            paper: Color::rgb(252, 252, 250),
            grid_minor: Color::rgb(234, 236, 238),
            grid_major: Color::rgb(210, 214, 219),
            wall: Color::rgb(78, 80, 88),
            wall_background: Color::rgb(250, 250, 248),
            wall_drywall: Color::rgb(150, 155, 164),
            wall_concrete: Color::rgb(48, 50, 56),
            wall_glass: Color::rgb(150, 196, 222),
            wall_wood: Color::rgb(150, 112, 76),
            room_fill: Color::rgb(238, 228, 212),
            room_line: Color::rgb(170, 150, 120),
            room_text: Color::rgb(90, 75, 55),
            dimension: Color::rgb(60, 90, 140),
            label: Color::rgb(40, 40, 48),
            compass: Color::rgb(70, 70, 80),
            selection: Color::rgb(40, 120, 230),
            furniture_fill: Color::rgb(255, 255, 253),
            furniture_detail: Color::rgb(228, 230, 233),
            furniture_line: Color::rgb(60, 62, 70),
        }
    }
}

/// Supplies a top-view image file for a piece, when one should replace its
/// plan symbol.
pub type PieceImageFn = dyn Fn(&Furniture) -> Option<String> + Send + Sync;

#[derive(Clone)]
pub struct PieceImages(pub std::sync::Arc<PieceImageFn>);

impl std::fmt::Debug for PieceImages {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PieceImages")
    }
}

#[derive(Debug, Clone, Default)]
pub struct SceneOptions {
    pub selected: HashSet<ElementId>,
    pub unit: LengthUnit,
    pub palette: Palette,
    pub show_background: bool,
    /// Top views drawn instead of symbols, for the pieces it returns.
    pub piece_images: Option<PieceImages>,
}

/// Builds the plan scene. Draw order: background, rooms, furniture, walls
/// (with door and window holes), openings, dimensions, labels, compass.
pub fn plan_scene(home: &Home, options: &SceneOptions) -> Scene {
    // While a technical project is edited, everything else steps back.
    let focus = home.active_discipline;
    let dimmed = |d: Option<newera_core::Discipline>| focus.is_some() && d != focus;
    let shown = |d: Option<newera_core::Discipline>| {
        d.is_none_or(|d| !home.hidden_disciplines.contains(&d))
    };
    let faded_options = SceneOptions {
        palette: options.palette.faded(),
        ..options.clone()
    };
    let fade = |c: Color, d: Option<newera_core::Discipline>| {
        if dimmed(d) {
            blend(c, options.palette.paper, FADE)
        } else {
            c
        }
    };
    let arch_options = if focus.is_some() {
        &faded_options
    } else {
        options
    };
    let options_for = |d: Option<newera_core::Discipline>| {
        let base = if dimmed(d) {
            &faded_options.palette
        } else {
            &options.palette
        };
        let mut palette = base.clone();
        if let Some(d) = d {
            palette.furniture_line = fade(discipline_color(d), d.into());
            // Solid parts of technical symbols take the discipline color.
            palette.furniture_detail = palette.furniture_line;
        }
        palette
    };
    let palette = &arch_options.palette;
    let mut scene = Scene::default();
    let pick = |id: ElementId, normal: Color| {
        if options.selected.contains(&id) {
            palette.selection
        } else {
            normal
        }
    };

    if options.show_background
        && let Some(bg) = home
            .current_level()
            .and_then(|id| home.level(id))
            .and_then(|l| l.background.as_ref())
            .or(home.background.as_ref())
            .filter(|bg| bg.visible)
    {
        let (min, max) = bg.bounds();
        scene.push(
            None,
            Primitive::Image {
                path: bg.path.clone(),
                min,
                max,
                opacity: bg.opacity,
                angle: bg.angle,
            },
        );
    }

    for room in &home.rooms {
        room_items(
            &mut scene,
            room,
            arch_options,
            pick(room.id.into(), palette.room_line),
        );
    }

    // Furniture sits on the floor, under the walls; tall pieces over low ones.
    // Groups draw their pieces, owned (and selected) as the group.
    let mut pieces: Vec<(Furniture, bool)> = home
        .furniture
        .iter()
        .flat_map(|top| {
            let selected = options.selected.contains(&top.id.into());
            top.visible_leaves().into_iter().map(move |leaf| {
                let mut piece = leaf.clone();
                piece.id = top.id;
                (piece, selected)
            })
        })
        .filter(|(f, _)| !f.is_opening() && shown(f.discipline))
        .collect();
    pieces.sort_by(|(a, _), (b, _)| (a.elevation + a.height).total_cmp(&(b.elevation + b.height)));
    for (piece, selected) in &pieces {
        // Above the plan cut (roofs, rafters, mezzanines): dashed outline of
        // what it covers, leaving the floor below readable.
        if piece.discipline.is_none()
            && (piece.pitch != 0.0
                || piece.roll != 0.0
                || piece.height_range().0 >= PLAN_CUT_HEIGHT)
        {
            let color = if *selected {
                options.palette.selection
            } else {
                blend(options.palette.furniture_line, options.palette.paper, 0.35)
            };
            let mut outline = piece.projected_footprint().to_vec();
            outline.push(outline[0]);
            dashed(
                &mut scene,
                Some(piece.id.into()),
                &outline,
                color,
                12.0,
                8.0,
            );
            continue;
        }
        if let Some(path) = options
            .piece_images
            .as_ref()
            .and_then(|images| (images.0)(piece))
        {
            let (hw, hd) = (piece.width / 2.0, piece.depth / 2.0);
            let owner = Some(piece.id.into());
            scene.push(
                owner,
                Primitive::Image {
                    path,
                    min: Point2::new(piece.position.x - hw, piece.position.y - hd),
                    max: Point2::new(piece.position.x + hw, piece.position.y + hd),
                    opacity: if dimmed(piece.discipline) { 0.35 } else { 1.0 },
                    angle: piece.angle,
                },
            );
            if *selected {
                scene.push(
                    owner,
                    Primitive::Line {
                        points: piece.footprint().to_vec(),
                        closed: true,
                        color: options.palette.selection,
                        width: Size::Px(1.5),
                    },
                );
            }
            continue;
        }
        furniture_items(&mut scene, piece, *selected, &options_for(piece.discipline));
    }

    let cuts = home.wall_cuts();
    for ((wall, outline), wall_cuts) in home.walls.iter().zip(home.wall_outlines()).zip(&cuts) {
        let owner = Some(wall.id.into());
        let color = pick(wall.id.into(), wall_fill(palette, wall));
        for part in cut_outline(&outline, wall, wall_cuts) {
            // Architectural convention: light fill, diagonal hatch, outline.
            let background = if color == palette.selection {
                blend(palette.paper, palette.selection, 0.25)
            } else {
                palette.wall_background
            };
            scene.fill(owner, &part, background);
            for (a, b) in hatch(&part, WALL_HATCH_SPACING) {
                scene.push(
                    owner,
                    Primitive::Line {
                        points: vec![a, b],
                        closed: false,
                        color,
                        width: Size::Px(0.8),
                    },
                );
            }
            scene.push(
                owner,
                Primitive::Line {
                    points: part,
                    closed: true,
                    color,
                    width: Size::Px(1.2),
                },
            );
        }
    }

    for opening in home
        .furniture
        .iter()
        .filter(|f| f.visible && f.is_opening())
    {
        furniture_items(
            &mut scene,
            opening,
            options.selected.contains(&opening.id.into()),
            palette,
        );
    }

    for polyline in home.polylines.iter().filter(|p| shown(p.discipline)) {
        let own = {
            let [r, g, b] = polyline.color;
            fade(Color::rgb(r, g, b), polyline.discipline)
        };
        polyline_items(&mut scene, polyline, pick(polyline.id.into(), own));
    }

    // Furniture the room names should keep clear of (not rugs or ceiling lights).
    let obstacles: Vec<[Point2; 4]> = home
        .furniture
        .iter()
        .flat_map(Furniture::visible_leaves)
        .filter(|f| {
            !f.is_opening() && f.height > 2.0 && f.elevation < 150.0 && f.discipline.is_none()
        })
        .map(Furniture::footprint)
        .collect();
    for room in &home.rooms {
        // Rooms drawn inside this one (a pantry in a kitchen) keep their own label room.
        let mut around = obstacles.clone();
        let centroid = polygon_centroid(&room.points);
        for other in home
            .rooms
            .iter()
            .filter(|o| o.id != room.id && o.points.len() >= 3)
        {
            let (lo, hi) =
                other
                    .points
                    .iter()
                    .fold((other.points[0], other.points[0]), |(lo, hi), p| {
                        (
                            Point2::new(lo.x.min(p.x), lo.y.min(p.y)),
                            Point2::new(hi.x.max(p.x), hi.y.max(p.y)),
                        )
                    });
            let contains_me =
                centroid.is_some_and(|c| c.x > lo.x && c.x < hi.x && c.y > lo.y && c.y < hi.y);
            if !contains_me {
                around.push([lo, Point2::new(hi.x, lo.y), hi, Point2::new(lo.x, hi.y)]);
            }
        }
        room_texts(&mut scene, room, arch_options, &around);
    }

    for dimension in home.dimensions.iter().filter(|d| shown(d.discipline)) {
        let own = fade(
            dimension
                .color
                .map_or(options.palette.dimension, |[r, g, b]| Color::rgb(r, g, b)),
            dimension.discipline,
        );
        let color = pick(dimension.id.into(), own);
        dimension_items(&mut scene, dimension, options.unit, color);
    }

    for label in home.labels.iter().filter(|l| shown(l.discipline)) {
        let own = fade(
            label
                .color
                .map_or(options.palette.label, |[r, g, b]| Color::rgb(r, g, b)),
            label.discipline,
        );
        let color = pick(label.id.into(), own);
        label_items(&mut scene, label, color);
    }

    if home.annotations.auto_dimensions {
        let color = fade(options.palette.dimension, None);
        for dimension in newera_core::auto_dimensions(home) {
            dimension_items(&mut scene, &dimension, options.unit, color);
        }
    }
    if home.compass.visible {
        compass_items(&mut scene, &home.compass, palette.compass);
    }
    if home.annotations.references {
        reference_items(&mut scene, home, options);
    }
    if home.annotations.legend {
        legend_items(&mut scene, home, options);
    }
    scene
}

/// Architectural symbol of a piece, placed and rotated on the plan.
pub fn furniture_items(scene: &mut Scene, piece: &Furniture, selected: bool, palette: &Palette) {
    let owner = Some(piece.id.into());
    let to_plan = |pts: &[(f64, f64)]| pts.iter().map(|p| piece.to_plan(*p)).collect::<Vec<_>>();
    let line_color = if selected {
        palette.selection
    } else {
        palette.furniture_line
    };
    for shape in plan_symbol(piece) {
        match shape {
            SymbolShape::Fill { points, detail } => {
                let base = if detail {
                    palette.furniture_detail
                } else {
                    palette.furniture_fill
                };
                let color = if selected {
                    blend(base, palette.selection, 0.18)
                } else {
                    base
                };
                scene.fill(owner, &to_plan(&points), color);
            }
            SymbolShape::Line {
                points,
                closed,
                strong,
            } => scene.push(
                owner,
                Primitive::Line {
                    points: to_plan(&points),
                    closed,
                    color: line_color,
                    width: Size::Px(if strong { 1.4 } else { 0.8 }),
                },
            ),
        }
    }
}

/// Pieces whose underside is at least this high are above the plan's
/// horizontal cut and drawn as dashed outlines, cm (above door heads).
pub(crate) const PLAN_CUT_HEIGHT: f64 = 220.0;

/// A dashed polyline with `on`/`off` dash lengths in cm.
fn dashed(
    scene: &mut Scene,
    owner: Option<ElementId>,
    points: &[Point2],
    color: Color,
    on: f64,
    off: f64,
) {
    let mut drawing = true;
    let mut left = on;
    for pair in points.windows(2) {
        let (mut a, b) = (pair[0], pair[1]);
        let mut remaining = a.distance(b);
        while remaining > 1e-9 {
            let step = left.min(remaining);
            let k = step / remaining;
            let next = Point2::new(a.x + (b.x - a.x) * k, a.y + (b.y - a.y) * k);
            if drawing {
                scene.push(
                    owner,
                    Primitive::Line {
                        points: vec![a, next],
                        closed: false,
                        color,
                        width: Size::Px(1.0),
                    },
                );
            }
            remaining -= step;
            left -= step;
            a = next;
            if left <= 1e-9 {
                drawing = !drawing;
                left = if drawing { on } else { off };
            }
        }
    }
}

/// Distance between wall hatch lines, cm.
const WALL_HATCH_SPACING: f64 = 7.0;

/// Segments of 45° hatch lines inside a polygon.
fn hatch(polygon: &[Point2], spacing: f64) -> Vec<(Point2, Point2)> {
    if polygon.len() < 3 {
        return Vec::new();
    }
    // Lines x + y = c, stepped on a global grid so neighbors line up.
    let sums: Vec<f64> = polygon.iter().map(|p| p.x + p.y).collect();
    let (lo, hi) = sums
        .iter()
        .fold((f64::MAX, f64::MIN), |(lo, hi), s| (lo.min(*s), hi.max(*s)));
    let mut out = Vec::new();
    let mut c = (lo / spacing).ceil() * spacing;
    while c < hi {
        let mut xs: Vec<f64> = Vec::new();
        for i in 0..polygon.len() {
            let (p, q) = (polygon[i], polygon[(i + 1) % polygon.len()]);
            let (sp, sq) = (p.x + p.y, q.x + q.y);
            // Half-open test so a vertex exactly on the line counts once.
            if (sp < c) != (sq < c) {
                let t = (c - sp) / (sq - sp);
                xs.push(p.x + (q.x - p.x) * t);
            }
        }
        xs.sort_by(f64::total_cmp);
        for [a, b] in xs.as_chunks::<2>().0 {
            out.push((Point2::new(*a, c - a), Point2::new(*b, c - b)));
        }
        c += spacing;
    }
    out
}

/// How far dimmed elements blend toward the paper.
const FADE: f64 = 0.65;

/// Line color of a technical project's symbols.
pub fn discipline_color(discipline: newera_core::Discipline) -> Color {
    match discipline {
        newera_core::Discipline::Electrical => Color::rgb(214, 96, 20),
        newera_core::Discipline::Plumbing => Color::rgb(30, 110, 200),
    }
}

impl Palette {
    /// The same palette, washed out toward the paper.
    #[must_use]
    pub fn faded(&self) -> Self {
        let f = |c: Color| blend(c, self.paper, FADE);
        Self {
            paper: self.paper,
            grid_minor: self.grid_minor,
            grid_major: self.grid_major,
            wall: f(self.wall),
            wall_background: self.wall_background,
            wall_drywall: f(self.wall_drywall),
            wall_concrete: f(self.wall_concrete),
            wall_glass: f(self.wall_glass),
            wall_wood: f(self.wall_wood),
            room_fill: f(self.room_fill),
            room_line: f(self.room_line),
            room_text: f(self.room_text),
            dimension: f(self.dimension),
            label: f(self.label),
            compass: f(self.compass),
            selection: self.selection,
            furniture_fill: self.furniture_fill,
            furniture_detail: f(self.furniture_detail),
            furniture_line: f(self.furniture_line),
        }
    }
}

/// Plan fill of a wall, by construction family.
fn wall_fill(palette: &Palette, wall: &newera_core::Wall) -> Color {
    use newera_core::WallFamily;
    match wall
        .wall_type
        .as_deref()
        .and_then(newera_core::wall_type)
        .map(|t| t.family)
    {
        Some(WallFamily::Drywall) => palette.wall_drywall,
        Some(WallFamily::Concrete) => palette.wall_concrete,
        Some(WallFamily::Glass) => palette.wall_glass,
        Some(WallFamily::Wood) => palette.wall_wood,
        Some(WallFamily::Masonry) | None => palette.wall,
    }
}

fn blend(a: Color, b: Color, t: f64) -> Color {
    let mix = |x: u8, y: u8| {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let v = (f64::from(x) + (f64::from(y) - f64::from(x)) * t).round() as u8;
        v
    };
    Color([
        mix(a.0[0], b.0[0]),
        mix(a.0[1], b.0[1]),
        mix(a.0[2], b.0[2]),
        a.0[3],
    ])
}

fn room_items(scene: &mut Scene, room: &Room, options: &SceneOptions, line: Color) {
    let owner = Some(room.id.into());
    if room.floor_visible {
        // Finished floors show their color, softened so the plan stays readable.
        let fill = room
            .floor_material
            .as_ref()
            .map_or(options.palette.room_fill, |m| {
                let [r, g, b] = m.base_color([0, 0, 0]);
                blend(options.palette.paper, Color::rgb(r, g, b), 0.45)
            });
        scene.fill(owner, &room.points, fill);
    }
    scene.push(
        owner,
        Primitive::Line {
            points: room.points.clone(),
            closed: true,
            color: line,
            width: Size::Px(if line == options.palette.selection {
                2.0
            } else {
                1.0
            }),
        },
    );
}

/// Name and area of a room, drawn above furniture and walls.
/// Free space a room name needs around its anchor, cm.
const LABEL_CLEARANCE: f64 = 60.0;

/// Where to write a room's name: its centroid when that is clear, otherwise
/// the point inside the room farthest from furniture (tables, beds and
/// sofas usually sit in the middle).
fn label_spot(room: &Room, obstacles: &[[Point2; 4]]) -> Option<Point2> {
    let centroid = polygon_centroid(&room.points)?;
    let inside = |p: Point2| {
        let pts = &room.points;
        let mut hit = false;
        for i in 0..pts.len() {
            let (a, b) = (pts[i], pts[(i + pts.len() - 1) % pts.len()]);
            if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
                hit = !hit;
            }
        }
        hit
    };
    // Distance from a point to a (convex) footprint, 0 inside.
    let clearance = |p: Point2, box_: &[Point2; 4]| {
        let mut outside = false;
        let mut best = f64::MAX;
        for i in 0..4 {
            let (a, b) = (box_[i], box_[(i + 1) % 4]);
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let len2 = (dx * dx + dy * dy).max(1e-9);
            if (p.x - a.x) * dy - (p.y - a.y) * dx > 0.0 {
                outside = true;
            }
            let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
            best = best.min(p.distance(Point2::new(a.x + dx * t, a.y + dy * t)));
        }
        if outside { best } else { 0.0 }
    };
    let nearby: Vec<&[Point2; 4]> = obstacles
        .iter()
        .filter(|o| {
            o.iter().any(|p| inside(*p))
                || inside(Point2::new(
                    f64::midpoint(o[0].x, o[2].x),
                    f64::midpoint(o[0].y, o[2].y),
                ))
        })
        .collect();
    // Room edges count too: a name hugging a wall or a doorway reads as
    // belonging to the room next door.
    let edges = |p: Point2| {
        let pts = &room.points;
        (0..pts.len())
            .map(|i| p.distance_to_segment(pts[i], pts[(i + 1) % pts.len()]))
            .fold(f64::MAX, f64::min)
    };
    let score = |p: Point2| {
        nearby
            .iter()
            .map(|o| clearance(p, o))
            .fold(edges(p), f64::min)
    };
    let (lo, hi) = room
        .points
        .iter()
        .fold((room.points[0], room.points[0]), |(lo, hi), p| {
            (
                Point2::new(lo.x.min(p.x), lo.y.min(p.y)),
                Point2::new(hi.x.max(p.x), hi.y.max(p.y)),
            )
        });
    // The most open spot, near the middle when several are open enough.
    let rate = |p: Point2| score(p).min(LABEL_CLEARANCE * 2.0) - p.distance(centroid) * 0.05;
    let mut best = if inside(centroid) {
        (rate(centroid), centroid)
    } else {
        (f64::MIN, centroid)
    };
    if best.0 >= LABEL_CLEARANCE * 2.0 - 1e-9 {
        return Some(centroid);
    }
    for i in 1..24 {
        for j in 1..24 {
            let p = Point2::new(
                lo.x + (hi.x - lo.x) * f64::from(i) / 24.0,
                lo.y + (hi.y - lo.y) * f64::from(j) / 24.0,
            );
            if !inside(p) {
                continue;
            }
            let s = rate(p);
            if s > best.0 {
                best = (s, p);
            }
        }
    }
    Some(best.1)
}

fn room_texts(scene: &mut Scene, room: &Room, options: &SceneOptions, obstacles: &[[Point2; 4]]) {
    let owner = Some(room.id.into());
    if room.points.is_empty() {
        return;
    }
    let styled = room.name_style.is_some()
        || room.area_style.is_some()
        || room.name_offset != [0.0, 0.0]
        || room.area_offset != [0.0, 0.0];
    if styled {
        // Name and area each at their own offset from the bounds center.
        let (min, max) =
            room.points
                .iter()
                .fold((room.points[0], room.points[0]), |(lo, hi), p| {
                    (
                        Point2::new(lo.x.min(p.x), lo.y.min(p.y)),
                        Point2::new(hi.x.max(p.x), hi.y.max(p.y)),
                    )
                });
        let center = Point2::new(min.x.midpoint(max.x), min.y.midpoint(max.y));
        let mut text = |content: String,
                        style: Option<&newera_core::TextStyle>,
                        offset: [f64; 2],
                        angle: f64,
                        default: f64| {
            if content.is_empty() {
                return;
            }
            let style = style
                .cloned()
                .unwrap_or_else(|| newera_core::TextStyle::new(default));
            scene.push(
                owner,
                Primitive::Text {
                    text: content,
                    position: Point2::new(center.x + offset[0], center.y + offset[1]),
                    size: Size::Cm(style.size * EM_TO_HEIGHT),
                    color: options.palette.room_text,
                    align: Align::from_text_align(style.align),
                    angle,
                    look: TextLook {
                        bold: style.bold,
                        italic: style.italic,
                        outline: None,
                    },
                },
            );
        };
        text(
            room.name.clone(),
            room.name_style.as_ref(),
            room.name_offset,
            room.name_angle,
            24.0,
        );
        if room.area_visible {
            text(
                options.unit.format_area(room.area()),
                room.area_style.as_ref(),
                room.area_offset,
                room.area_angle,
                24.0,
            );
        }
        return;
    }
    let Some(center) = label_spot(room, obstacles) else {
        return;
    };
    let mut text = room.name.clone();
    if room.area_visible {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&options.unit.format_area(room.area()));
    }
    if !text.is_empty() {
        scene.push(
            owner,
            Primitive::Text {
                text,
                position: center,
                size: Size::Px(13.0),
                color: options.palette.room_text,
                align: Align::Center,
                angle: 0.0,
                look: TextLook::default(),
            },
        );
    }
}

/// Legend below the drawing: every electrical and plumbing symbol in use,
/// drawn as on the plan, with its name and how many there are.
fn legend_items(scene: &mut Scene, home: &Home, options: &SceneOptions) {
    use std::collections::BTreeMap;
    let mut used: BTreeMap<(newera_core::Discipline, String), (Furniture, usize)> = BTreeMap::new();
    for top in &home.furniture {
        for piece in top.flatten() {
            let Some(d) = piece.discipline.or(top.discipline) else {
                continue;
            };
            if home.hidden_disciplines.contains(&d) {
                continue;
            }
            used.entry((d, piece.catalog.clone()))
                .or_insert_with(|| (piece.clone(), 0))
                .1 += 1;
        }
    }
    if used.is_empty() {
        return;
    }
    let Some((min, max)) = scene.bounds() else {
        return;
    };
    let ink = Color::rgb(40, 44, 52);
    let text = |scene: &mut Scene, content: String, at: Point2, size: f64, bold: bool| {
        scene.push(
            None,
            Primitive::Text {
                text: content,
                position: at,
                size: Size::Cm(size * EM_TO_HEIGHT),
                color: ink,
                align: Align::BaselineLeft,
                angle: 0.0,
                look: TextLook {
                    bold,
                    italic: false,
                    outline: None,
                },
            },
        );
    };
    let x = min.x;
    let mut y = max.y + 90.0;
    text(scene, "LEGENDA".to_owned(), Point2::new(x, y), 22.0, true);
    y += 40.0;
    let mut current = None;
    for ((discipline, _), (piece, count)) in &used {
        if current != Some(*discipline) {
            current = Some(*discipline);
            text(
                scene,
                discipline.name().to_uppercase(),
                Point2::new(x, y),
                16.0,
                true,
            );
            y += 36.0;
        }
        let mut sample = piece.clone();
        sample.position = Point2::new(x + 20.0, y - 6.0);
        sample.angle = 0.0;
        let mut palette = options.palette.clone();
        palette.furniture_line = discipline_color(*discipline);
        palette.furniture_detail = palette.furniture_line;
        furniture_items(scene, &sample, false, &palette);
        text(
            scene,
            format!("{}  ×{count}", piece.name),
            Point2::new(x + 50.0, y),
            13.0,
            false,
        );
        y += 38.0;
    }
}

/// Numbered tags on furniture and, beside the plan, a schedule of every
/// room with the pieces inside and their sizes (plus brand, model and link
/// when details are on).
fn reference_items(scene: &mut Scene, home: &Home, options: &SceneOptions) {
    let groups = newera_core::room_references(home);
    if groups.is_empty() {
        return;
    }
    let ink = Color::rgb(40, 44, 52);
    let accent = Color::rgb(30, 90, 170);
    let paper = options.palette.paper;
    let text = |scene: &mut Scene,
                content: String,
                at: Point2,
                size: f64,
                color: Color,
                bold: bool,
                align: Align| {
        scene.push(
            None,
            Primitive::Text {
                text: content,
                position: at,
                size: Size::Cm(size * EM_TO_HEIGHT),
                color,
                align,
                angle: 0.0,
                look: TextLook {
                    bold,
                    italic: false,
                    outline: None,
                },
            },
        );
    };
    // Tags, in a corner of the piece so they don't cover the room name and
    // area written at the middle of the room.
    for item in groups.iter().flat_map(|g| &g.items) {
        let r = 11.0;
        let at = home
            .piece(item.piece)
            .filter(|p| p.width > 4.0 * r && p.depth > 4.0 * r)
            .map_or(item.position, |p| {
                p.to_plan((-p.width / 2.0 + 1.6 * r, -p.depth / 2.0 + 1.6 * r))
            });
        let circle: Vec<Point2> = (0..20)
            .map(|i| {
                let a = f64::from(i) / 20.0 * std::f64::consts::TAU;
                Point2::new(at.x + r * a.cos(), at.y + r * a.sin())
            })
            .collect();
        scene.fill(Some(item.piece.into()), &circle, paper);
        scene.push(
            Some(item.piece.into()),
            Primitive::Line {
                points: circle,
                closed: true,
                color: accent,
                width: Size::Px(1.2),
            },
        );
        text(
            scene,
            item.tag.to_string(),
            at,
            11.0,
            accent,
            true,
            Align::Center,
        );
    }
    // Schedule to the right of everything already drawn (plan and any
    // legends the user wrote).
    let Some((min, max)) = scene.bounds().or_else(|| home.bounds()) else {
        return;
    };
    let x = max.x + 160.0;
    let mut y = min.y + 20.0;
    text(
        scene,
        "REFERÊNCIAS".to_owned(),
        Point2::new(x, y),
        22.0,
        ink,
        true,
        Align::BaselineLeft,
    );
    y += 20.0;
    text(
        scene,
        "Medidas: largura × profundidade × altura".to_owned(),
        Point2::new(x, y),
        11.0,
        options.palette.room_text,
        false,
        Align::BaselineLeft,
    );
    y += 34.0;
    for group in &groups {
        let title = match group.area {
            Some(area) => format!(
                "{} — {}",
                group.name.to_uppercase(),
                options.unit.format_area(area)
            ),
            None => group.name.to_uppercase(),
        };
        text(
            scene,
            title,
            Point2::new(x, y),
            16.0,
            ink,
            true,
            Align::BaselineLeft,
        );
        y += 24.0;
        for item in &group.items {
            let size = options.unit.format_size(item.size);
            text(
                scene,
                format!("{:>2}  {} — {size}", item.tag, item.name),
                Point2::new(x + 10.0, y),
                13.0,
                ink,
                false,
                Align::BaselineLeft,
            );
            y += 20.0;
            if home.annotations.reference_details {
                let mut detail = Vec::new();
                if let Some(brand) = &item.brand {
                    detail.push(format!("Marca: {brand}"));
                }
                if let Some(model) = &item.model {
                    detail.push(format!("Modelo: {model}"));
                }
                if !detail.is_empty() {
                    text(
                        scene,
                        detail.join(" · "),
                        Point2::new(x + 40.0, y),
                        11.0,
                        options.palette.room_text,
                        false,
                        Align::BaselineLeft,
                    );
                    y += 17.0;
                }
                if let Some(url) = &item.url {
                    text(
                        scene,
                        url.clone(),
                        Point2::new(x + 40.0, y),
                        11.0,
                        accent,
                        false,
                        Align::BaselineLeft,
                    );
                    y += 17.0;
                }
            }
        }
        y += 16.0;
    }
}

/// A free polyline: smooth when curved, dashed by its style, with arrows.
fn polyline_items(scene: &mut Scene, polyline: &newera_core::Polyline, color: Color) {
    use newera_core::{ArrowStyle, DashStyle, LineJoin};
    let owner = Some(polyline.id.into());
    let mut points = if polyline.join == LineJoin::Curved {
        smooth(&polyline.points, polyline.closed)
    } else {
        polyline.points.clone()
    };
    if polyline.closed && polyline.join != LineJoin::Curved && points.len() > 2 {
        points.push(points[0]);
    }
    let t = polyline.thickness;
    let pattern: Vec<f64> = match polyline.dash {
        DashStyle::Custom => polyline.dash_pattern.clone(),
        other => other.pattern().to_vec(),
    };
    let width = Size::Cm(t);
    if pattern.iter().all(|d| *d <= 0.0) {
        scene.push(
            owner,
            Primitive::Line {
                points: points.clone(),
                closed: false,
                color,
                width,
            },
        );
    } else {
        // Walk the path, emitting the "on" parts of the dash pattern.
        let lengths: Vec<f64> = pattern.iter().map(|d| (d * t).max(0.1)).collect();
        let mut index = 0;
        let mut left = lengths[0] - (polyline.dash_offset * t).rem_euclid(lengths[0]);
        let mut current: Vec<Point2> = vec![points[0]];
        for pair in points.windows(2) {
            let (mut a, b) = (pair[0], pair[1]);
            let mut remaining = a.distance(b);
            while remaining > 1e-9 {
                let step = left.min(remaining);
                let k = step / remaining;
                let next = Point2::new(a.x + (b.x - a.x) * k, a.y + (b.y - a.y) * k);
                if index % 2 == 0 {
                    current.push(next);
                }
                remaining -= step;
                left -= step;
                a = next;
                if left <= 1e-9 {
                    if index % 2 == 0 && current.len() > 1 {
                        scene.push(
                            owner,
                            Primitive::Line {
                                points: std::mem::take(&mut current),
                                closed: false,
                                color,
                                width,
                            },
                        );
                    }
                    current.clear();
                    index = (index + 1) % lengths.len();
                    left = lengths[index];
                    current.push(a);
                }
            }
        }
        if index % 2 == 0 && current.len() > 1 {
            scene.push(
                owner,
                Primitive::Line {
                    points: current,
                    closed: false,
                    color,
                    width,
                },
            );
        }
    }
    let n = points.len();
    if n >= 2 && !polyline.closed {
        for (style, tip, from) in [
            (polyline.start_arrow, points[0], points[1]),
            (polyline.end_arrow, points[n - 1], points[n - 2]),
        ] {
            let len = tip.distance(from).max(1e-9);
            let (dx, dy) = ((tip.x - from.x) / len, (tip.y - from.y) / len);
            let size = (t * 5.0).max(6.0);
            let back = Point2::new(tip.x - dx * size, tip.y - dy * size);
            let side =
                |s: f64| Point2::new(back.x - dy * size * 0.5 * s, back.y + dx * size * 0.5 * s);
            match style {
                ArrowStyle::None => {}
                ArrowStyle::Delta => scene.fill(owner, &[tip, side(1.0), side(-1.0)], color),
                ArrowStyle::Open => scene.push(
                    owner,
                    Primitive::Line {
                        points: vec![side(1.0), tip, side(-1.0)],
                        closed: false,
                        color,
                        width,
                    },
                ),
                ArrowStyle::Disc => {
                    let r = size * 0.35;
                    let circle: Vec<Point2> = (0..16)
                        .map(|i| {
                            let a = f64::from(i) / 16.0 * std::f64::consts::TAU;
                            Point2::new(tip.x + r * a.cos(), tip.y + r * a.sin())
                        })
                        .collect();
                    scene.fill(owner, &circle, color);
                }
            }
        }
    }
}

/// Catmull-Rom curve through the points.
fn smooth(points: &[Point2], closed: bool) -> Vec<Point2> {
    let n = points.len();
    if n < 3 {
        return points.to_vec();
    }
    let at = |i: isize| {
        let len = isize::try_from(n).unwrap_or(isize::MAX);
        let k = if closed {
            i.rem_euclid(len)
        } else {
            i.clamp(0, len - 1)
        };
        points[usize::try_from(k).unwrap_or(0)]
    };
    let segments = if closed { n } else { n - 1 };
    let mut out = Vec::with_capacity(segments * 8 + 1);
    for s in 0..segments {
        let i = isize::try_from(s).unwrap_or(0);
        let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
        for step in 0..8 {
            let t = f64::from(step) / 8.0;
            let (t2, t3) = (t * t, t * t * t);
            let blend = |a: f64, b: f64, c: f64, d: f64| {
                0.5 * (2.0 * b
                    + (c - a) * t
                    + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
                    + (3.0 * b - a - 3.0 * c + d) * t3)
            };
            out.push(Point2::new(
                blend(p0.x, p1.x, p2.x, p3.x),
                blend(p0.y, p1.y, p2.y, p3.y),
            ));
        }
    }
    out.push(if closed { points[0] } else { points[n - 1] });
    out
}

/// Architectural dimension: extension lines, a dimension line with 45° ticks
/// and the measured length along it, kept readable (never upside down).
pub fn dimension_items(scene: &mut Scene, dimension: &Dimension, unit: LengthUnit, color: Color) {
    let owner = Some(dimension.id.into());
    let (a, b) = (dimension.start, dimension.end);
    let length = a.distance(b);
    if length < 1e-6 {
        return;
    }
    let dir = ((b.x - a.x) / length, (b.y - a.y) / length);
    // Left normal in plan axes (y down) is (dy, -dx).
    let normal = (dir.1, -dir.0);
    let offset = dimension.offset;
    let at = |p: Point2, d: f64| Point2::new(p.x + normal.0 * d, p.y + normal.1 * d);
    let (a2, b2) = (at(a, offset), at(b, offset));
    let overshoot = 8.0 * offset.signum();
    let hair = Size::Px(1.0);

    for (p, p2) in [(a, a2), (b, b2)] {
        if offset.abs() > 1e-6 {
            scene.push(
                owner,
                Primitive::Line {
                    points: vec![at(p, offset.signum() * 4.0), at(p2, overshoot)],
                    closed: false,
                    color,
                    width: hair,
                },
            );
        }
    }
    scene.push(
        owner,
        Primitive::Line {
            points: vec![a2, b2],
            closed: false,
            color,
            width: hair,
        },
    );
    let tick = 6.0;
    let diag = (
        (dir.0 + normal.0) * std::f64::consts::FRAC_1_SQRT_2 * tick,
        (dir.1 + normal.1) * std::f64::consts::FRAC_1_SQRT_2 * tick,
    );
    for p in [a2, b2] {
        scene.push(
            owner,
            Primitive::Line {
                points: vec![
                    Point2::new(p.x - diag.0, p.y - diag.1),
                    Point2::new(p.x + diag.0, p.y + diag.1),
                ],
                closed: false,
                color,
                width: Size::Px(1.6),
            },
        );
    }

    let mut angle = dir.1.atan2(dir.0).to_degrees();
    if angle > 90.0 {
        angle -= 180.0;
    } else if angle <= -90.0 {
        angle += 180.0;
    }
    scene.push(
        owner,
        Primitive::Text {
            text: unit.format_dimension(length),
            position: Point2::new(a2.x.midpoint(b2.x), a2.y.midpoint(b2.y)),
            size: dimension
                .style
                .as_ref()
                .map_or(Size::Px(12.0), |s| Size::Cm(s.size * EM_TO_HEIGHT)),
            color,
            align: Align::Above,
            angle,
            look: TextLook {
                bold: dimension.style.as_ref().is_some_and(|s| s.bold),
                italic: dimension.style.as_ref().is_some_and(|s| s.italic),
                outline: None,
            },
        },
    );
}

fn label_items(scene: &mut Scene, label: &Label, color: Color) {
    scene.push(
        Some(label.id.into()),
        Primitive::Text {
            text: label.text.clone(),
            position: label.position,
            size: Size::Cm(label.size * EM_TO_HEIGHT),
            color,
            align: Align::from_text_align(label.align),
            angle: label.angle,
            look: TextLook {
                bold: label.bold,
                italic: label.italic,
                outline: label.outline.map(|[r, g, b]| Color::rgb(r, g, b)),
            },
        },
    );
}

/// Compass rose: circle, a filled needle pointing north, and an "N".
pub fn compass_items(scene: &mut Scene, compass: &Compass, color: Color) {
    let c = compass.center;
    let r = compass.diameter / 2.0;
    let circle: Vec<Point2> = (0..48)
        .map(|i| {
            let a = f64::from(i) / 48.0 * std::f64::consts::TAU;
            Point2::new(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect();
    scene.push(
        None,
        Primitive::Line {
            points: circle,
            closed: true,
            color,
            width: Size::Px(1.5),
        },
    );
    let angle = compass.north_degrees.to_radians();
    let north = (angle.sin(), -angle.cos());
    let side = (-north.1 * r * 0.25, north.0 * r * 0.25);
    let tip = Point2::new(c.x + north.0 * r, c.y + north.1 * r);
    let tail = Point2::new(c.x - north.0 * r, c.y - north.1 * r);
    let left = Point2::new(c.x + side.0, c.y + side.1);
    let right = Point2::new(c.x - side.0, c.y - side.1);
    scene.fill(None, &[tip, left, right], color);
    scene.push(
        None,
        Primitive::Line {
            points: vec![tail, right, left],
            closed: true,
            color,
            width: Size::Px(1.0),
        },
    );
    scene.push(
        None,
        Primitive::Text {
            text: "N".to_owned(),
            position: Point2::new(c.x + north.0 * r * 1.35, c.y + north.1 * r * 1.35),
            size: Size::Cm(r * 0.45),
            color,
            align: Align::Center,
            angle: 0.0,
            look: TextLook::default(),
        },
    );
}

#[cfg(test)]
mod tests {
    use newera_core::{Command, Document, Wall};

    use super::*;

    #[test]
    fn scene_contains_every_element_in_draw_order() {
        let mut doc = Document::default();
        let wall = Wall::new(
            doc.new_wall_id(),
            Point2::new(0.0, 0.0),
            Point2::new(400.0, 0.0),
        );
        let dim = Dimension {
            id: doc.new_dimension_id(),
            start: Point2::new(0.0, 0.0),
            end: Point2::new(400.0, 0.0),
            offset: 40.0,
            level: None,
            ..Default::default()
        };
        doc.execute(Command::insert(wall.clone())).unwrap();
        doc.execute(Command::insert(dim.clone())).unwrap();

        let scene = plan_scene(doc.home(), &SceneOptions::default());
        let owners: Vec<_> = scene.items.iter().filter_map(|i| i.owner).collect();
        let wall_at = owners.iter().position(|o| *o == wall.id.into()).unwrap();
        let dim_at = owners.iter().position(|o| *o == dim.id.into()).unwrap();
        assert!(wall_at < dim_at, "dimensions draw over walls");
        let texts: Vec<_> = scene
            .items
            .iter()
            .filter_map(|i| match &i.primitive {
                Primitive::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(texts.contains(&"400"), "{texts:?}");
        assert!(texts.contains(&"N"));
    }

    #[test]
    fn dimension_text_is_never_upside_down() {
        for (a, b) in [((0.0, 0.0), (-300.0, 10.0)), ((0.0, 0.0), (0.0, -200.0))] {
            let mut scene = Scene::default();
            let dim = Dimension {
                id: newera_core::DimensionId(1),
                start: Point2::new(a.0, a.1),
                end: Point2::new(b.0, b.1),
                offset: 0.0,
                level: None,
                ..Default::default()
            };
            dimension_items(
                &mut scene,
                &dim,
                LengthUnit::Centimeter,
                Color::rgb(0, 0, 0),
            );
            let angle = scene
                .items
                .iter()
                .find_map(|i| match i.primitive {
                    Primitive::Text { angle, .. } => Some(angle),
                    _ => None,
                })
                .unwrap();
            assert!(angle > -90.0 && angle <= 90.0, "{angle}");
        }
    }
}

#[cfg(test)]
mod furniture_tests {
    use newera_core::{Command, Document, Point2, Wall, align_to_wall};

    use super::*;

    #[test]
    fn doors_open_a_gap_in_the_wall_and_draw_their_swing() {
        let mut doc = Document::default();
        let wall = Wall::new(
            doc.new_wall_id(),
            Point2::new(0.0, 0.0),
            Point2::new(500.0, 0.0),
        );
        let mut door = newera_catalog::find("door")
            .unwrap()
            .instantiate(doc.new_furniture_id(), Point2::new(0.0, 0.0));
        align_to_wall(&mut door, &wall, 250.0);
        let sofa = newera_catalog::find("sofa-3")
            .unwrap()
            .instantiate(doc.new_furniture_id(), Point2::new(250.0, 200.0));
        let (wall_id, door_id, sofa_id) = (wall.id, door.id, sofa.id);
        doc.execute(Command::Batch {
            commands: vec![
                Command::insert(wall),
                Command::insert(door),
                Command::insert(sofa),
            ],
        })
        .unwrap();

        let scene = plan_scene(doc.home(), &SceneOptions::default());
        let wall_fills = scene
            .items
            .iter()
            .filter(|i| {
                i.owner == Some(wall_id.into()) && matches!(i.primitive, Primitive::Fill { .. })
            })
            .count();
        assert_eq!(wall_fills, 2, "the door splits the wall");
        let order: Vec<_> = scene.items.iter().filter_map(|i| i.owner).collect();
        let first = |id: ElementId| order.iter().position(|o| *o == id).unwrap();
        assert!(
            first(sofa_id.into()) < first(wall_id.into()),
            "furniture under walls"
        );
        assert!(
            first(wall_id.into()) < first(door_id.into()),
            "doors over walls"
        );
        let door_lines = scene
            .items
            .iter()
            .filter(|i| i.owner == Some(door_id.into()))
            .count();
        assert!(door_lines >= 3, "leaf, arc and jambs");
    }
}

#[cfg(test)]
mod reference_tests {
    use newera_core::{
        Command, Document, Furniture, PlanAnnotations, Point2, Room, Wall, detect_room, ops,
    };

    use super::*;

    fn texts(doc: &Document) -> Vec<String> {
        plan_scene(doc.home(), &SceneOptions::default())
            .items
            .iter()
            .filter_map(|i| match &i.primitive {
                Primitive::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn reference_tags_and_schedule_follow_every_edit() {
        let mut doc = Document::default();
        // Two rooms side by side: 0..300 and 300..600.
        let pts = [(0.0, 0.0), (600.0, 0.0), (600.0, 300.0), (0.0, 300.0)];
        let mut commands: Vec<Command> = (0..4)
            .map(|i| {
                let (a, b) = (pts[i], pts[(i + 1) % 4]);
                Command::insert(Wall::new(
                    doc.new_wall_id(),
                    Point2::new(a.0, a.1),
                    Point2::new(b.0, b.1),
                ))
            })
            .collect();
        commands.push(Command::insert(Wall::new(
            doc.new_wall_id(),
            Point2::new(300.0, 0.0),
            Point2::new(300.0, 300.0),
        )));
        doc.execute(Command::Batch { commands }).unwrap();
        for (name, x) in [("Sala", 150.0), ("Quarto", 450.0)] {
            let points = detect_room(&doc.home().walls, Point2::new(x, 150.0)).unwrap();
            let mut room = Room::new(doc.new_room_id(), name, points);
            room.auto = true;
            doc.execute(Command::insert(room)).unwrap();
        }
        let sofa = Furniture {
            id: doc.new_furniture_id(),
            catalog: "sofa-3".into(),
            name: "Sofá".into(),
            position: Point2::new(150.0, 150.0),
            ..Furniture::default()
        };
        let sofa_id = sofa.id;
        doc.execute(Command::insert(sofa)).unwrap();
        assert!(
            !texts(&doc).iter().any(|t| t.contains("REFERÊNCIAS")),
            "off by default"
        );

        doc.execute(Command::SetAnnotations {
            annotations: PlanAnnotations {
                references: true,
                ..PlanAnnotations::default()
            },
        })
        .unwrap();
        let t = texts(&doc);
        assert!(t.iter().any(|t| t == "REFERÊNCIAS"));
        assert!(t.iter().any(|t| t.starts_with("SALA — ")), "{t:?}");
        assert!(t.iter().any(|t| t.contains("1  Sofá")), "{t:?}");

        // Moving the sofa into the bedroom moves it in the schedule.
        ops::translate(&mut doc, &[sofa_id.into()], 300.0, 0.0, true).unwrap();
        let t = texts(&doc);
        assert!(t.iter().any(|t| t.starts_with("QUARTO — ")), "{t:?}");
        assert!(!t.iter().any(|t| t.starts_with("SALA — ")), "{t:?}");

        // A new piece in the living room takes number 1 (reading order).
        let lamp = Furniture {
            id: doc.new_furniture_id(),
            catalog: "floor-lamp".into(),
            name: "Luminária".into(),
            position: Point2::new(100.0, 100.0),
            ..Furniture::default()
        };
        doc.execute(Command::insert(lamp)).unwrap();
        let t = texts(&doc);
        assert!(t.iter().any(|t| t.contains("1  Luminária")), "{t:?}");
        assert!(t.iter().any(|t| t.contains("2  Sofá")), "{t:?}");

        // Renaming a room renames its schedule title.
        let mut room = doc.home().rooms[1].clone();
        room.name = "Suíte".into();
        doc.execute(Command::update(room)).unwrap();
        assert!(texts(&doc).iter().any(|t| t.starts_with("SUÍTE — ")));

        // Moving the partition wall resizes the rooms: the area follows.
        let before: Vec<String> = texts(&doc)
            .into_iter()
            .filter(|t| t.contains(" — "))
            .collect();
        let partition = doc.home().walls[4].id;
        ops::translate(&mut doc, &[partition.into()], 50.0, 0.0, true).unwrap();
        let after: Vec<String> = texts(&doc)
            .into_iter()
            .filter(|t| t.contains(" — "))
            .collect();
        assert_ne!(before, after);

        // Turning references off removes tags and schedule.
        doc.execute(Command::SetAnnotations {
            annotations: PlanAnnotations::default(),
        })
        .unwrap();
        assert!(!texts(&doc).iter().any(|t| t == "REFERÊNCIAS"));
    }
}

#[cfg(test)]
mod label_tests {
    use newera_core::{Furniture, FurnitureId, Home, Point2, Room, RoomId};

    use super::*;

    #[test]
    fn room_names_keep_clear_of_furniture_and_rugs_show_the_floor() {
        let mut home = Home::default();
        home.rooms.push(Room::new(
            RoomId(1),
            "Sala",
            vec![
                Point2::new(0.0, 0.0),
                Point2::new(500.0, 0.0),
                Point2::new(500.0, 400.0),
                Point2::new(0.0, 400.0),
            ],
        ));
        home.furniture.push(Furniture {
            id: FurnitureId(2),
            catalog: "dining-table-4".into(),
            position: Point2::new(250.0, 200.0),
            width: 160.0,
            depth: 100.0,
            height: 75.0,
            ..Furniture::default()
        });
        let scene = plan_scene(&home, &SceneOptions::default());
        let at = scene
            .items
            .iter()
            .find_map(|i| match &i.primitive {
                Primitive::Text { text, position, .. } if text.starts_with("Sala") => {
                    Some(*position)
                }
                _ => None,
            })
            .unwrap();
        let table = home.furniture[0].footprint();
        let (lo, hi) = (table[0], table[2]);
        let clear = at.x < lo.x.min(hi.x) - 30.0
            || at.x > lo.x.max(hi.x) + 30.0
            || at.y < lo.y.min(hi.y) - 30.0
            || at.y > lo.y.max(hi.y) + 30.0;
        assert!(clear, "label at {at:?} over the table");

        // Without furniture the name stays centered.
        home.furniture.clear();
        let rug = newera_catalog::find("rug")
            .unwrap()
            .instantiate(FurnitureId(3), Point2::new(250.0, 200.0));
        home.furniture.push(rug);
        let scene = plan_scene(&home, &SceneOptions::default());
        let centered = scene.items.iter().any(|i| {
            matches!(&i.primitive, Primitive::Text { text, position, .. }
                if text.starts_with("Sala") && position.distance(Point2::new(250.0, 200.0)) < 1.0)
        });
        assert!(centered, "rugs don't push the name away");
        let rug_fills = scene
            .items
            .iter()
            .filter(|i| {
                i.owner == Some(FurnitureId(3).into())
                    && matches!(i.primitive, Primitive::Fill { .. })
            })
            .count();
        assert_eq!(rug_fills, 0, "rugs are drawn without fill");
    }
}
