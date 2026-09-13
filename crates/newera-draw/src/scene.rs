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
}

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
    },
    /// Background image covering `min`..`max` (plan cm).
    Image {
        path: String,
        min: Point2,
        max: Point2,
        opacity: f64,
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
            Primitive::Text { position, .. } => vec![*position],
            Primitive::Image { min, max, .. } => vec![*min, *max],
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
    pub wall: Color,
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

#[derive(Debug, Clone, Default)]
pub struct SceneOptions {
    pub selected: HashSet<ElementId>,
    pub unit: LengthUnit,
    pub palette: Palette,
    pub show_background: bool,
}

/// Builds the plan scene. Draw order: background, rooms, furniture, walls
/// (with door and window holes), openings, dimensions, labels, compass.
pub fn plan_scene(home: &Home, options: &SceneOptions) -> Scene {
    let palette = &options.palette;
    let mut scene = Scene::default();
    let pick = |id: ElementId, normal: Color| {
        if options.selected.contains(&id) {
            palette.selection
        } else {
            normal
        }
    };

    if options.show_background
        && let Some(bg) = home.background.as_ref().filter(|bg| bg.visible)
    {
        let (min, max) = bg.bounds();
        scene.push(
            None,
            Primitive::Image {
                path: bg.path.clone(),
                min,
                max,
                opacity: bg.opacity,
            },
        );
    }

    for room in &home.rooms {
        room_items(
            &mut scene,
            room,
            options,
            pick(room.id.into(), palette.room_line),
        );
    }

    // Furniture sits on the floor, under the walls; tall pieces over low ones.
    let mut pieces: Vec<&Furniture> = home
        .furniture
        .iter()
        .filter(|f| f.visible && !f.is_opening())
        .collect();
    pieces.sort_by(|a, b| (a.elevation + a.height).total_cmp(&(b.elevation + b.height)));
    for piece in pieces {
        furniture_items(
            &mut scene,
            piece,
            options.selected.contains(&piece.id.into()),
            palette,
        );
    }

    let cuts = home.wall_cuts();
    for ((wall, outline), wall_cuts) in home.walls.iter().zip(home.wall_outlines()).zip(&cuts) {
        let color = pick(wall.id.into(), palette.wall);
        for part in cut_outline(&outline, wall, wall_cuts) {
            scene.fill(Some(wall.id.into()), &part, color);
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

    for dimension in &home.dimensions {
        let color = pick(dimension.id.into(), palette.dimension);
        dimension_items(&mut scene, dimension, options.unit, color);
    }

    for label in &home.labels {
        let color = pick(label.id.into(), palette.label);
        label_items(&mut scene, label, color);
    }

    if home.compass.visible {
        compass_items(&mut scene, &home.compass, palette.compass);
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
        scene.fill(owner, &room.points, options.palette.room_fill);
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
    let Some(center) = polygon_centroid(&room.points) else {
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
            },
        );
    }
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
            text: unit.format_length(length),
            position: Point2::new(a2.x.midpoint(b2.x), a2.y.midpoint(b2.y)),
            size: Size::Px(12.0),
            color,
            align: Align::Above,
            angle,
        },
    );
}

fn label_items(scene: &mut Scene, label: &Label, color: Color) {
    scene.push(
        Some(label.id.into()),
        Primitive::Text {
            text: label.text.clone(),
            position: label.position,
            size: Size::Cm(label.size),
            color,
            align: Align::Center,
            angle: label.angle,
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
        assert!(texts.contains(&"400 cm"), "{texts:?}");
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
