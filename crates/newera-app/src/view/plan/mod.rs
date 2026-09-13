//! Interactive 2D floor plan editor.
//!
//! The plan renders the same [`newera_draw::Scene`] used for PNG/SVG output.
//! Drags edit a scratch copy of the home for live preview and commit a single
//! undoable command on release.

mod camera;
mod hit;
mod magnet;
mod paint;

use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui::{self, Color32, CursorIcon, Key, PointerButton, Pos2, Rect, Stroke, Vec2};
use newera_core::{
    Command, CoreResult, Dimension, Document, Element, ElementId, Furniture, FurnitureId, Home,
    LengthUnit, Point2, Room, SharedDocument, Wall, WallId, detect_room, ops, polygon_area,
};
use newera_draw::{Align, Palette, Scene, SceneOptions, plan_scene};

pub(crate) use camera::Camera;
use paint::{Textures, color, paint_grid, paint_rulers, paint_scene, paint_text};

pub(crate) type Selection = BTreeSet<ElementId>;

/// Editing mode, like the plan toolbar of desktop home design tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Tool {
    #[default]
    Select,
    Pan,
    Walls,
    Rooms,
    Dimensions,
    Labels,
    /// Mark two points of known distance on the background image.
    Calibrate,
    /// Place one piece of this catalog item.
    Place(&'static str),
}

/// Requests the plan makes to the rest of the app.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PlanEvent {
    Modify(Vec<ElementId>),
    NewLabel(Point2),
    Calibrate {
        a: Point2,
        b: Point2,
    },
    /// A piece was placed; the placement tool is done.
    Placed(ElementId),
    Status(String),
}

pub(crate) struct PlanInput<'a> {
    pub(crate) document: &'a SharedDocument,
    pub(crate) selection: &'a mut Selection,
    pub(crate) tool: Tool,
    pub(crate) unit: LengthUnit,
    pub(crate) palette: &'a Palette,
}

#[derive(Debug, Clone, PartialEq)]
enum Drag {
    Pan,
    Box { start: Point2 },
    Move { origin: Point2, ids: Vec<ElementId> },
    WallPoint { id: WallId, at_start: bool },
    WallArc { id: WallId },
    RoomPoint { id: ElementId, index: usize },
    DimPoint { id: ElementId, at_start: bool },
    DimOffset { id: ElementId },
    Background { origin: Point2, offset: Point2 },
    Rotate { id: FurnitureId },
    Resize { id: FurnitureId },
}

/// What a cached scene was built from: revision, selection and unit.
type SceneKey = (u64, Vec<ElementId>, LengthUnit);

/// Wall outlines of the storey below, drawn as a faint reference.
type Outlines = Arc<Vec<Vec<Point2>>>;

#[derive(Debug, Default)]
pub(crate) struct PlanView {
    camera: Camera,
    pending_fit: bool,
    /// `(revision, level view, outlines of the walls one storey below)`.
    snapshot: Option<(u64, Arc<Home>, Outlines)>,
    scene_cache: Option<(SceneKey, Scene)>,
    textures: Textures,
    drag: Option<Drag>,
    cursor: Option<Point2>,
    wall_chain: Option<Point2>,
    room_points: Vec<Point2>,
    dim_points: Vec<Point2>,
    calibration: Vec<Point2>,
    typed_length: String,
    /// Screen area of the last frame, for tests and zoom commands.
    rect: Option<Rect>,
}

impl PlanView {
    pub(crate) fn new() -> Self {
        Self {
            pending_fit: true,
            ..Self::default()
        }
    }

    pub(crate) fn request_fit(&mut self) {
        self.pending_fit = true;
    }

    /// Cancels any drawing in progress.
    pub(crate) fn cancel(&mut self) {
        self.drag = None;
        self.wall_chain = None;
        self.room_points.clear();
        self.dim_points.clear();
        self.calibration.clear();
        self.typed_length.clear();
    }

    pub(crate) fn is_drawing(&self) -> bool {
        self.wall_chain.is_some()
            || !self.room_points.is_empty()
            || !self.dim_points.is_empty()
            || !self.calibration.is_empty()
    }

    pub(crate) fn cursor(&self) -> Option<Point2> {
        self.cursor
    }

    pub(crate) fn zoom_percent(&self) -> f32 {
        self.camera.zoom * 100.0
    }

    pub(crate) fn zoom_by(&mut self, rect: Rect, factor: f32) {
        self.camera.zoom_at(rect, rect.center(), factor);
    }

    /// Typing digits while drawing walls enters an exact length.
    pub(crate) fn accepts_length_input(&self) -> bool {
        self.wall_chain.is_some()
    }

    /// The storey being edited, as its own home, plus the outline of the walls
    /// right below it (drawn faintly as a reference, like tracing paper).
    fn home(&mut self, document: &SharedDocument) -> (Arc<Home>, u64, Option<PathBuf>, Outlines) {
        let doc = document.read();
        let revision = doc.revision();
        let path = doc.path().map(std::path::Path::to_path_buf);
        if let Some((rev, home, below)) = &self.snapshot
            && *rev == revision
        {
            return (home.clone(), revision, path, below.clone());
        }
        let full = doc.home();
        let current = full.current_level();
        let view = Arc::new(full.level_view(current));
        let below = full
            .sorted_levels()
            .into_iter()
            .take_while(|l| Some(l.id) != current)
            .last()
            .map(|l| full.level_view(Some(l.id)).wall_outlines())
            .unwrap_or_default();
        let below = Arc::new(below);
        self.snapshot = Some((revision, view.clone(), below.clone()));
        (view, revision, path, below)
    }

    #[allow(clippy::needless_pass_by_value)] // the input bundles borrows for one frame
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui, input: PlanInput<'_>) -> Vec<PlanEvent> {
        let mut events = Vec::new();
        let rect = ui.available_rect_before_wrap();
        self.rect = Some(rect);
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        let (home, revision, project, below) = self.home(input.document);
        let magnetism = !ui.input(|i| i.modifiers.shift);
        let zoom = self.camera.zoom;
        let tolerance = self.camera.cm(6.0);

        if self.pending_fit && rect.width() > 10.0 {
            self.fit(&home, rect);
            self.pending_fit = false;
        }

        // --- Navigation -------------------------------------------------
        if let Some(pointer) = response.hover_pos() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.camera.zoom_at(rect, pointer, (scroll * 0.002).exp());
            }
            if ui.input(|i| i.key_pressed(Key::F) && i.modifiers.is_none())
                && !self.accepts_length_input()
            {
                self.fit(&home, rect);
            }
        }
        let panning = response.dragged_by(PointerButton::Middle)
            || (response.dragged_by(PointerButton::Secondary) && !self.is_drawing())
            || (input.tool == Tool::Pan && response.dragged_by(PointerButton::Primary));
        if panning {
            self.camera.pan(response.drag_delta());
        }

        // `hover_pos` is empty on the frame a button is released, which would
        // drop the end of drags and clicks; use the latest pointer position.
        let raw = ui
            .input(|i| i.pointer.latest_pos())
            .filter(|p| rect.contains(*p) || self.drag.is_some())
            .map(|p| self.camera.to_world(rect, p));
        self.cursor = raw;
        let press = ui
            .input(|i| i.pointer.press_origin())
            .map(|p| self.camera.to_world(rect, p));

        let mut preview: Option<Home> = None;
        let mut overlays: Vec<Overlay> = Vec::new();
        let commit = |events: &mut Vec<PlanEvent>,
                      f: &mut dyn FnMut(&mut Document) -> CoreResult<()>| {
            let mut doc = input.document.write();
            if let Err(err) = f(&mut doc) {
                events.push(PlanEvent::Status(format!("⚠ {err}")));
            }
        };

        // --- Tools --------------------------------------------------------
        match input.tool {
            Tool::Pan => {}
            Tool::Select => {
                if response.drag_started_by(PointerButton::Primary)
                    && let Some(origin) = press
                {
                    self.drag = Some(Self::begin_drag(
                        &home,
                        origin,
                        tolerance,
                        input.selection,
                        ui,
                    ));
                }
                if let (Some(drag), Some(current)) = (self.drag.clone(), raw) {
                    let snapped = self.drag_target(&home, &drag, current, magnetism);
                    if let Drag::Box { start } = drag {
                        overlays.push(Overlay::Box(start, current));
                    } else if drag != Drag::Pan {
                        let mut scratch = Document::new((*home).clone());
                        if apply_drag(&mut scratch, &drag, snapped).is_ok() {
                            if let Drag::WallPoint { id, .. } | Drag::WallArc { id } = drag
                                && let Some(w) = scratch.home().wall(id)
                            {
                                overlays.push(Overlay::WallLength(w.clone()));
                            }
                            preview = Some(scratch.home().clone());
                        }
                    }
                    if response.drag_stopped() {
                        match drag {
                            Drag::Box { start } => {
                                let (min, max) = corners(start, current);
                                let found = hit::in_rect(&home, min, max);
                                if !ui.input(|i| i.modifiers.command) {
                                    input.selection.clear();
                                }
                                input.selection.extend(found);
                            }
                            Drag::Pan => {}
                            other => {
                                commit(&mut events, &mut |doc| apply_drag(doc, &other, snapped));
                            }
                        }
                        self.drag = None;
                    }
                } else if response.drag_stopped() {
                    self.drag = None;
                }

                if response.clicked_by(PointerButton::Primary)
                    && let Some(p) = raw
                {
                    let outlines = home.wall_outlines();
                    let hit = hit::pick(&home, &outlines, p, tolerance);
                    let additive = ui.input(|i| i.modifiers.command);
                    match hit {
                        Some(id) if additive => {
                            if !input.selection.remove(&id) {
                                input.selection.insert(id);
                            }
                        }
                        Some(id) => {
                            if !input.selection.contains(&id) {
                                input.selection.clear();
                                input.selection.insert(id);
                            }
                        }
                        None if !additive => input.selection.clear(),
                        None => {}
                    }
                    if multi_click(&response) && hit.is_some() {
                        events.push(PlanEvent::Modify(input.selection.iter().copied().collect()));
                    }
                }
            }
            Tool::Walls => self.walls_tool(
                ui,
                &response,
                &home,
                raw,
                magnetism,
                &mut overlays,
                &mut events,
                input.document,
            ),
            Tool::Rooms => {
                if let Some(p) = raw {
                    let snapped = if magnetism {
                        magnet::snap_point(&home, p, zoom, &[])
                    } else {
                        p
                    };
                    if multi_click(&response) {
                        if self.room_points.len() >= 3 {
                            let points = std::mem::take(&mut self.room_points);
                            commit(&mut events, &mut |doc| {
                                let room = Room::new(doc.new_room_id(), "", points.clone());
                                doc.execute(Command::insert(room))
                            });
                        } else {
                            self.room_points.clear();
                            match detect_room(&home.walls, p) {
                                Some(points) => commit(&mut events, &mut |doc| {
                                    let room = Room::new(doc.new_room_id(), "", points.clone());
                                    doc.execute(Command::insert(room))
                                }),
                                None => events.push(PlanEvent::Status(
                                    "Nenhum espaço fechado por paredes aqui".into(),
                                )),
                            }
                        }
                    } else if response.clicked_by(PointerButton::Primary)
                        && self
                            .room_points
                            .last()
                            .is_none_or(|last| last.distance(snapped) > 0.5)
                    {
                        self.room_points.push(snapped);
                    }
                    if !self.room_points.is_empty() {
                        let mut pts = self.room_points.clone();
                        pts.push(snapped);
                        overlays.push(Overlay::Polygon(pts));
                    }
                    overlays.push(Overlay::Cross(snapped));
                }
            }
            Tool::Dimensions => {
                if let Some(p) = raw {
                    let snapped = match self.dim_points.first() {
                        Some(a) if self.dim_points.len() == 1 && magnetism => {
                            magnet::snap_segment(&home, *a, p, zoom, &[])
                        }
                        _ if magnetism => magnet::snap_point(&home, p, zoom, &[]),
                        _ => p,
                    };
                    if multi_click(&response) && self.dim_points.len() <= 1 {
                        self.dim_points.clear();
                        let outlines = home.wall_outlines();
                        if let Some(ElementId::Wall(id)) = hit::pick(&home, &outlines, p, tolerance)
                        {
                            commit(&mut events, &mut |doc| {
                                let dim = ops::wall_dimension(doc, id, 40.0)?;
                                doc.execute(Command::insert(dim))
                            });
                        }
                    } else if response.clicked_by(PointerButton::Primary) {
                        if self.dim_points.len() < 2 {
                            self.dim_points.push(snapped);
                        } else {
                            let (a, b) = (self.dim_points[0], self.dim_points[1]);
                            let offset = signed_offset(a, b, p);
                            self.dim_points.clear();
                            commit(&mut events, &mut |doc| {
                                let dim = Dimension {
                                    id: doc.new_dimension_id(),
                                    start: a,
                                    end: b,
                                    offset,
                                    level: None,
                                };
                                doc.execute(Command::insert(dim))
                            });
                        }
                    }
                    match self.dim_points.as_slice() {
                        [a] => overlays.push(Overlay::Dimension(*a, snapped, 0.0)),
                        [a, b] => {
                            overlays.push(Overlay::Dimension(*a, *b, signed_offset(*a, *b, p)));
                        }
                        _ => overlays.push(Overlay::Cross(snapped)),
                    }
                }
            }
            Tool::Labels => {
                if let Some(p) = raw {
                    overlays.push(Overlay::Cross(p));
                    if response.clicked_by(PointerButton::Primary) {
                        events.push(PlanEvent::NewLabel(p));
                    }
                }
            }
            Tool::Place(catalog) => {
                if let (Some(p), Some(item)) = (raw, newera_catalog::find(catalog)) {
                    let at = if magnetism {
                        magnet::snap_point(&home, p, zoom, &[])
                    } else {
                        p
                    };
                    let mut ghost = item.instantiate(FurnitureId(0), at);
                    if ghost.is_opening()
                        && let Some((wall_id, along)) =
                            ops::nearest_wall(&home, p, ops::OPENING_REACH)
                        && let Some(wall) = home.wall(wall_id)
                    {
                        newera_core::align_to_wall(&mut ghost, wall, along);
                    }
                    if response.clicked_by(PointerButton::Primary) {
                        let mut placed = None;
                        commit(&mut events, &mut |doc| {
                            let mut piece = ghost.clone();
                            piece.id = doc.new_furniture_id();
                            placed = Some(piece.id);
                            doc.execute(Command::insert(piece))
                        });
                        if let Some(id) = placed {
                            input.selection.clear();
                            input.selection.insert(id.into());
                            events.push(PlanEvent::Placed(id.into()));
                        }
                    }
                    overlays.push(Overlay::Ghost(ghost));
                }
            }
            Tool::Calibrate => {
                if let Some(p) = raw {
                    if response.clicked_by(PointerButton::Primary) {
                        self.calibration.push(p);
                        if self.calibration.len() == 2 {
                            let (a, b) = (self.calibration[0], self.calibration[1]);
                            self.calibration.clear();
                            events.push(PlanEvent::Calibrate { a, b });
                        }
                    }
                    if let Some(a) = self.calibration.first() {
                        overlays.push(Overlay::Measure(*a, p));
                    }
                    overlays.push(Overlay::Cross(p));
                }
                // Dragging with the calibrate tool moves the image.
                if response.drag_started_by(PointerButton::Primary)
                    && let (Some(origin), Some(bg)) = (press, home.background.as_ref())
                {
                    self.drag = Some(Drag::Background {
                        origin,
                        offset: bg.offset,
                    });
                    self.calibration.clear();
                }
                if let (Some(drag @ Drag::Background { .. }), Some(current)) =
                    (self.drag.clone(), raw)
                {
                    let mut scratch = Document::new((*home).clone());
                    if apply_drag(&mut scratch, &drag, current).is_ok() {
                        preview = Some(scratch.home().clone());
                    }
                    if response.drag_stopped() {
                        commit(&mut events, &mut |doc| apply_drag(doc, &drag, current));
                        self.drag = None;
                    }
                }
            }
        }

        // --- Paint --------------------------------------------------------
        let selected: Vec<ElementId> = input.selection.iter().copied().collect();
        let options = |home: &Home| {
            let _ = home;
            SceneOptions {
                selected: selected.iter().copied().collect::<HashSet<_>>(),
                unit: input.unit,
                palette: input.palette.clone(),
                show_background: true,
            }
        };
        painter.rect_filled(rect, 0.0, color(input.palette.paper));
        paint_grid(&painter, rect, &self.camera, input.palette);
        for outline in below.iter() {
            let mut scene = Scene::default();
            scene.items.push(newera_draw::Item {
                owner: None,
                primitive: newera_draw::Primitive::Fill {
                    points: outline.clone(),
                    triangles: newera_core::triangulate(outline),
                    color: input.palette.wall.with_alpha(0.16),
                },
            });
            paint_scene(
                &painter,
                rect,
                &self.camera,
                &scene,
                &mut self.textures,
                None,
            );
        }
        let shown: &Home = preview.as_ref().unwrap_or(&home);
        if preview.is_some() {
            let scene = plan_scene(shown, &options(shown));
            paint_scene(
                &painter,
                rect,
                &self.camera,
                &scene,
                &mut self.textures,
                project.as_deref(),
            );
        } else {
            let key = (revision, selected.clone(), input.unit);
            if self.scene_cache.as_ref().is_none_or(|(k, _)| *k != key) {
                self.scene_cache = Some((key, plan_scene(&home, &options(&home))));
            }
            if let Some((_, scene)) = &self.scene_cache {
                paint_scene(
                    &painter,
                    rect,
                    &self.camera,
                    scene,
                    &mut self.textures,
                    project.as_deref(),
                );
            }
        }

        if input.tool == Tool::Select && self.drag.is_none() && selected.len() == 1 {
            for handle in handles(shown, selected[0]) {
                let s = self.camera.to_screen(rect, handle.at);
                painter.rect(
                    Rect::from_center_size(s, Vec2::splat(8.0)),
                    1.0,
                    Color32::WHITE,
                    Stroke::new(1.5, color(input.palette.selection)),
                    egui::StrokeKind::Middle,
                );
            }
        }
        for overlay in &overlays {
            self.paint_overlay(&painter, rect, overlay, input.unit, input.palette);
        }
        if !self.typed_length.is_empty()
            && let Some(p) = raw
        {
            let s = self.camera.to_screen(rect, p) + Vec2::new(16.0, 16.0);
            let text = format!("{} cm ⏎", self.typed_length);
            let galley =
                painter.layout_no_wrap(text, egui::FontId::proportional(14.0), Color32::WHITE);
            let bg = Rect::from_min_size(s, galley.size() + Vec2::splat(8.0));
            painter.rect_filled(bg, 4.0, color(input.palette.selection));
            painter.galley(s + Vec2::splat(4.0), galley, Color32::WHITE);
        }
        paint_rulers(&painter, rect, &self.camera, input.unit, raw);

        // Cursor feedback.
        if response.hovered() {
            let icon = match (input.tool, &self.drag) {
                (_, Some(Drag::Pan)) => CursorIcon::Grabbing,
                (Tool::Pan, _) => CursorIcon::Grab,
                (Tool::Select, Some(Drag::Move { .. } | Drag::Background { .. })) => {
                    CursorIcon::Move
                }
                (Tool::Select, _) => CursorIcon::Default,
                (Tool::Labels, _) => CursorIcon::Text,
                (Tool::Place(_), _) => CursorIcon::Copy,
                _ => CursorIcon::Crosshair,
            };
            ui.ctx().set_cursor_icon(icon);
        }
        events
    }

    fn fit(&mut self, home: &Home, rect: Rect) {
        let bounds = home.bounds().map(|(min, max)| match &home.background {
            Some(bg) if bg.visible => {
                let (bmin, bmax) = bg.bounds();
                (
                    Point2::new(min.x.min(bmin.x), min.y.min(bmin.y)),
                    Point2::new(max.x.max(bmax.x), max.y.max(bmax.y)),
                )
            }
            _ => (min, max),
        });
        let bounds = bounds.or_else(|| {
            home.background
                .as_ref()
                .map(newera_core::BackgroundImage::bounds)
        });
        if let Some((min, max)) = bounds {
            self.camera.fit(rect, min, max);
        }
    }

    fn begin_drag(
        home: &Home,
        origin: Point2,
        tolerance: f64,
        selection: &mut Selection,
        ui: &egui::Ui,
    ) -> Drag {
        if selection.len() == 1
            && let Some(first) = selection.iter().next()
            && let Some(handle) = handles(home, *first)
                .into_iter()
                .find(|h| h.at.distance(origin) <= tolerance * 1.4)
        {
            return handle.drag;
        }
        let outlines = home.wall_outlines();
        match hit::pick(home, &outlines, origin, tolerance) {
            Some(id) => {
                if !selection.contains(&id) {
                    if !ui.input(|i| i.modifiers.command) {
                        selection.clear();
                    }
                    selection.insert(id);
                }
                Drag::Move {
                    origin,
                    ids: selection.iter().copied().collect(),
                }
            }
            None => Drag::Box { start: origin },
        }
    }

    /// Applies magnetism to the drag target point.
    fn drag_target(&self, home: &Home, drag: &Drag, current: Point2, magnetism: bool) -> Point2 {
        if !magnetism {
            return current;
        }
        let zoom = self.camera.zoom;
        match drag {
            Drag::Move { origin, .. } => {
                let step = magnet::round_step(zoom);
                let dx = ((current.x - origin.x) / step).round() * step;
                let dy = ((current.y - origin.y) / step).round() * step;
                Point2::new(origin.x + dx, origin.y + dy)
            }
            Drag::WallPoint { id, at_start } => home.wall(*id).map_or(current, |w| {
                let (moving, fixed) = if *at_start {
                    (w.start, w.end)
                } else {
                    (w.end, w.start)
                };
                magnet::snap_segment(home, fixed, current, zoom, &[moving])
            }),
            Drag::RoomPoint { id, index } => match home.element(*id) {
                Some(Element::Room(r)) => {
                    magnet::snap_point(home, current, zoom, &[r.points[*index]])
                }
                _ => current,
            },
            Drag::DimPoint { .. } => magnet::snap_point(home, current, zoom, &[]),
            Drag::Rotate { id } => home.piece(*id).map_or(current, |f| {
                let (dx, dy) = (current.x - f.position.x, current.y - f.position.y);
                let r = dx.hypot(dy);
                let a = (dy.atan2(dx).to_degrees() / 15.0).round() * 15.0;
                Point2::new(
                    f.position.x + r * a.to_radians().cos(),
                    f.position.y + r * a.to_radians().sin(),
                )
            }),
            _ => current,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn walls_tool(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        home: &Home,
        raw: Option<Point2>,
        magnetism: bool,
        overlays: &mut Vec<Overlay>,
        events: &mut Vec<PlanEvent>,
        document: &SharedDocument,
    ) {
        let zoom = self.camera.zoom;
        // Exact length typed on the keyboard while drawing.
        if self.wall_chain.is_some() {
            ui.input(|i| {
                for event in &i.events {
                    if let egui::Event::Text(text) = event {
                        self.typed_length.extend(
                            text.chars()
                                .filter(|c| c.is_ascii_digit() || *c == '.' || *c == ','),
                        );
                    }
                }
            });
            if ui.input(|i| i.key_pressed(Key::Backspace)) {
                self.typed_length.pop();
            }
        }
        let Some(p) = raw else { return };
        let target = match self.wall_chain {
            Some(anchor) if magnetism => magnet::snap_segment(home, anchor, p, zoom, &[]),
            None if magnetism => magnet::snap_point(home, p, zoom, &[]),
            _ => p,
        };

        let create = |start: Point2, end: Point2, events: &mut Vec<PlanEvent>| {
            let mut doc = document.write();
            let wall = Wall::new(doc.new_wall_id(), start, end);
            if let Err(err) = doc.execute(Command::insert(wall)) {
                events.push(PlanEvent::Status(format!("⚠ {err}")));
            }
        };

        if let Some(anchor) = self.wall_chain
            && ui.input(|i| i.key_pressed(Key::Enter))
            && let Ok(length) = self.typed_length.replace(',', ".").parse::<f64>()
        {
            let end = magnet::along(anchor, target, length);
            create(anchor, end, events);
            self.wall_chain = Some(end);
            self.typed_length.clear();
        }

        if multi_click(response) || response.clicked_by(PointerButton::Secondary) {
            self.wall_chain = None;
            self.typed_length.clear();
        } else if response.clicked_by(PointerButton::Primary) {
            match self.wall_chain {
                Some(anchor) if anchor.distance(target) >= 1.0 => {
                    create(anchor, target, events);
                    self.wall_chain = Some(target);
                }
                Some(_) => {}
                None => self.wall_chain = Some(target),
            }
            self.typed_length.clear();
        }

        match self.wall_chain {
            Some(anchor) if anchor.distance(target) >= 1.0 => {
                let end = match self.typed_length.replace(',', ".").parse::<f64>() {
                    Ok(length) => magnet::along(anchor, target, length),
                    Err(_) => target,
                };
                let wall = Wall::new(WallId(0), anchor, end);
                let mut walls = home.walls.clone();
                walls.push(wall.clone());
                overlays.push(Overlay::WallPreview(walls));
                overlays.push(Overlay::WallLength(wall));
            }
            _ => {}
        }
        overlays.push(Overlay::Cross(target));
    }

    fn paint_overlay(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        overlay: &Overlay,
        unit: LengthUnit,
        palette: &Palette,
    ) {
        let accent = color(palette.selection);
        let to = |p: Point2| self.camera.to_screen(rect, p);
        match overlay {
            Overlay::Ghost(piece) => {
                let mut scene = Scene::default();
                newera_draw::furniture_items(&mut scene, piece, true, palette);
                let mut textures = Textures::default();
                paint_scene(painter, rect, &self.camera, &scene, &mut textures, None);
            }
            Overlay::Cross(p) => {
                let s = to(*p);
                let stroke = Stroke::new(1.0, accent.gamma_multiply(0.7));
                painter.line_segment([s - Vec2::X * 9.0, s + Vec2::X * 9.0], stroke);
                painter.line_segment([s - Vec2::Y * 9.0, s + Vec2::Y * 9.0], stroke);
            }
            Overlay::Box(a, b) => {
                let r = Rect::from_two_pos(to(*a), to(*b));
                painter.rect(
                    r,
                    0.0,
                    accent.gamma_multiply(0.08),
                    Stroke::new(1.0, accent),
                    egui::StrokeKind::Middle,
                );
            }
            Overlay::WallPreview(walls) => {
                if let Some(outline) = newera_core::wall_outlines(walls).last() {
                    let scene = {
                        let mut s = Scene::default();
                        s.items.push(newera_draw::Item {
                            owner: None,
                            primitive: newera_draw::Primitive::Fill {
                                points: outline.clone(),
                                triangles: newera_core::triangulate(outline),
                                color: palette.selection.with_alpha(0.45),
                            },
                        });
                        s
                    };
                    let mut textures = Textures::default();
                    paint_scene(painter, rect, &self.camera, &scene, &mut textures, None);
                }
            }
            Overlay::WallLength(wall) => {
                let (a, b) = (to(wall.start), to(wall.end));
                let mid = to(wall.point_at(0.5));
                let mut angle = f64::from((b - a).angle()).to_degrees();
                if angle > 90.0 {
                    angle -= 180.0;
                } else if angle <= -90.0 {
                    angle += 180.0;
                }
                let degrees = (-(wall.end.y - wall.start.y))
                    .atan2(wall.end.x - wall.start.x)
                    .to_degrees()
                    .rem_euclid(360.0);
                let text = format!("{}  ·  {degrees:.0}°", unit.format_length(wall.length()));
                #[allow(clippy::cast_possible_truncation)]
                let normal = egui::emath::Rot2::from_angle(f64::to_radians(angle) as f32)
                    * Vec2::new(0.0, -14.0);
                paint_text(
                    painter,
                    mid + normal,
                    &text,
                    12.0,
                    accent,
                    Align::Center,
                    angle,
                );
            }
            Overlay::Polygon(points) => {
                let mut screen: Vec<Pos2> = points.iter().map(|p| to(*p)).collect();
                if let Some(&first) = screen.first() {
                    screen.push(first);
                }
                painter.add(egui::Shape::line(screen, Stroke::new(1.5, accent)));
                if points.len() >= 3 {
                    let area = unit.format_area(polygon_area(points));
                    let center = newera_core::polygon_centroid(points).unwrap_or(points[0]);
                    paint_text(painter, to(center), &area, 13.0, accent, Align::Center, 0.0);
                }
            }
            Overlay::Dimension(a, b, offset) => {
                if a.distance(*b) > 0.1 {
                    let mut scene = Scene::default();
                    let dim = Dimension {
                        id: newera_core::DimensionId(0),
                        start: *a,
                        end: *b,
                        offset: *offset,
                        level: None,
                    };
                    newera_draw::dimension_items(&mut scene, &dim, unit, palette.selection);
                    let mut textures = Textures::default();
                    paint_scene(painter, rect, &self.camera, &scene, &mut textures, None);
                }
            }
            Overlay::Measure(a, b) => {
                painter.line_segment(
                    [to(*a), to(*b)],
                    Stroke::new(2.0, Color32::from_rgb(230, 90, 40)),
                );
                painter.circle_filled(to(*a), 4.0, Color32::from_rgb(230, 90, 40));
            }
        }
    }
}

#[derive(Debug, Clone)]
enum Overlay {
    Ghost(Furniture),
    Cross(Point2),
    Box(Point2, Point2),
    WallPreview(Vec<Wall>),
    WallLength(Wall),
    Polygon(Vec<Point2>),
    Dimension(Point2, Point2, f64),
    Measure(Point2, Point2),
}

struct Handle {
    at: Point2,
    drag: Drag,
}

/// Edit handles of a single selected element.
fn handles(home: &Home, id: ElementId) -> Vec<Handle> {
    match home.element(id) {
        Some(Element::Furniture(f)) => vec![
            Handle {
                at: f.to_plan((0.0, -f.depth / 2.0 - 25.0)),
                drag: Drag::Rotate { id: f.id },
            },
            Handle {
                at: f.to_plan((f.width / 2.0, f.depth / 2.0)),
                drag: Drag::Resize { id: f.id },
            },
        ],
        Some(Element::Wall(w)) => vec![
            Handle {
                at: w.start,
                drag: Drag::WallPoint {
                    id: w.id,
                    at_start: true,
                },
            },
            Handle {
                at: w.end,
                drag: Drag::WallPoint {
                    id: w.id,
                    at_start: false,
                },
            },
            Handle {
                at: w.point_at(0.5),
                drag: Drag::WallArc { id: w.id },
            },
        ],
        Some(Element::Room(r)) => r
            .points
            .iter()
            .enumerate()
            .map(|(index, p)| Handle {
                at: *p,
                drag: Drag::RoomPoint { id, index },
            })
            .collect(),
        Some(Element::Dimension(d)) => {
            let len = d.length().max(1e-9);
            let n = ((d.end.y - d.start.y) / len, -(d.end.x - d.start.x) / len);
            let mid = Point2::new(
                d.start.x.midpoint(d.end.x) + n.0 * d.offset,
                d.start.y.midpoint(d.end.y) + n.1 * d.offset,
            );
            vec![
                Handle {
                    at: d.start,
                    drag: Drag::DimPoint { id, at_start: true },
                },
                Handle {
                    at: d.end,
                    drag: Drag::DimPoint {
                        id,
                        at_start: false,
                    },
                },
                Handle {
                    at: mid,
                    drag: Drag::DimOffset { id },
                },
            ]
        }
        _ => Vec::new(),
    }
}

/// A double click; a quick third click (egui reports it as a triple click)
/// must count too, or "click the last corner, then double click" misfires.
fn multi_click(response: &egui::Response) -> bool {
    response.double_clicked() || response.triple_clicked()
}

/// Signed distance of `p` from line `a→b`, positive on the left (plan axes).
fn signed_offset(a: Point2, b: Point2, p: Point2) -> f64 {
    let len = a.distance(b).max(1e-9);
    let n = ((b.y - a.y) / len, -(b.x - a.x) / len);
    (p.x - a.x) * n.0 + (p.y - a.y) * n.1
}

fn corners(a: Point2, b: Point2) -> (Point2, Point2) {
    (
        Point2::new(a.x.min(b.x), a.y.min(b.y)),
        Point2::new(a.x.max(b.x), a.y.max(b.y)),
    )
}

/// Performs a drag on `doc`: used on a scratch copy for preview and on the
/// real document on release.
fn apply_drag(doc: &mut Document, drag: &Drag, target: Point2) -> CoreResult<()> {
    match drag {
        Drag::Pan | Drag::Box { .. } => Ok(()),
        Drag::Move { origin, ids } => {
            let (dx, dy) = (target.x - origin.x, target.y - origin.y);
            if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
                return Ok(());
            }
            ops::translate(doc, ids, dx, dy, true)
        }
        Drag::WallPoint { id, at_start } => ops::move_wall_point(doc, *id, *at_start, target),
        Drag::WallArc { id } => {
            let wall = doc
                .home()
                .wall(*id)
                .cloned()
                .ok_or(newera_core::CoreError::NotFound((*id).into()))?;
            let chord = wall.start.distance(wall.end).max(1e-9);
            let sagitta = signed_offset(wall.start, wall.end, target);
            let extent = (4.0 * (2.0 * sagitta / chord).atan()).to_degrees();
            let arc = (extent.abs() >= 3.0).then(|| (extent / 5.0).round() * 5.0);
            doc.execute(Command::update(Wall {
                arc_extent: arc,
                ..wall
            }))
        }
        Drag::RoomPoint { id, index } => match doc.home().element(*id) {
            Some(Element::Room(mut room)) => {
                room.points[*index] = target;
                doc.execute(Command::update(room))
            }
            _ => Ok(()),
        },
        Drag::DimPoint { id, at_start } => match doc.home().element(*id) {
            Some(Element::Dimension(mut dim)) => {
                if *at_start {
                    dim.start = target;
                } else {
                    dim.end = target;
                }
                doc.execute(Command::update(dim))
            }
            _ => Ok(()),
        },
        Drag::DimOffset { id } => match doc.home().element(*id) {
            Some(Element::Dimension(mut dim)) => {
                dim.offset = signed_offset(dim.start, dim.end, target);
                doc.execute(Command::update(dim))
            }
            _ => Ok(()),
        },
        Drag::Rotate { id } => match doc.home().piece(*id).cloned() {
            Some(mut f) => {
                let (dx, dy) = (target.x - f.position.x, target.y - f.position.y);
                if dx.hypot(dy) < 1.0 {
                    return Ok(());
                }
                // The handle sits behind the piece (local -y), 90° off its angle.
                f.angle = (dy.atan2(dx).to_degrees() + 90.0).rem_euclid(360.0);
                doc.execute(Command::update(f))
            }
            None => Ok(()),
        },
        Drag::Resize { id } => match doc.home().piece(*id).cloned() {
            Some(mut f) => {
                let (lx, ly) = f.to_local(target);
                f.width = (2.0 * lx.abs()).round().max(1.0);
                if !f.is_opening() {
                    f.depth = (2.0 * ly.abs()).round().max(1.0);
                }
                doc.execute(Command::update(f))
            }
            None => Ok(()),
        },
        Drag::Background { origin, offset } => match doc.home().background.clone() {
            Some(mut bg) => {
                bg.offset = Point2::new(
                    offset.x + target.x - origin.x,
                    offset.y + target.y - origin.y,
                );
                doc.execute(Command::SetBackground {
                    background: Some(bg),
                })
            }
            None => Ok(()),
        },
    }
}

#[cfg(test)]
mod tests {
    //! Interaction tests driven through `egui_kittest`: real pointer and key
    //! events go through the plan exactly as a user's would.

    use eframe::egui::{Event, Modifiers};
    use egui_kittest::{Harness, HarnessBuilder};

    use super::*;

    pub(super) struct State {
        pub(super) plan: PlanView,
        pub(super) document: SharedDocument,
        pub(super) selection: Selection,
        pub(super) tool: Tool,
        pub(super) events: Vec<PlanEvent>,
    }

    pub(super) fn harness(home: Home, tool: Tool) -> Harness<'static, State> {
        let state = State {
            plan: PlanView::new(),
            document: SharedDocument::new(Document::new(home)),
            selection: Selection::new(),
            tool,
            events: Vec::new(),
        };
        let palette = Palette::default();
        let mut h = HarnessBuilder::default()
            .with_size(Vec2::new(900.0, 700.0))
            .with_step_dt(1.0 / 60.0)
            .build_ui_state(
                move |ui, state: &mut State| {
                    let events = state.plan.ui(
                        ui,
                        PlanInput {
                            document: &state.document,
                            selection: &mut state.selection,
                            tool: state.tool,
                            unit: LengthUnit::Centimeter,
                            palette: &palette,
                        },
                    );
                    state.events.extend(events);
                },
                state,
            );
        // A fixed view keeps test coordinates on screen regardless of auto-fit.
        h.state_mut().plan.pending_fit = false;
        h.state_mut().plan.camera = Camera {
            center: Point2::new(200.0, 150.0),
            zoom: 0.8,
        };
        h.run_steps(3);
        h
    }

    pub(super) fn screen(h: &Harness<'_, State>, p: (f64, f64)) -> Pos2 {
        let plan = &h.state().plan;
        plan.camera
            .to_screen(plan.rect.unwrap(), Point2::new(p.0, p.1))
    }

    pub(super) fn button(h: &mut Harness<'_, State>, pos: Pos2, pressed: bool) {
        h.event(Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
        h.step();
    }

    /// A click, then enough idle frames that the next click is not a double click.
    pub(super) fn click(h: &mut Harness<'_, State>, p: (f64, f64)) {
        let pos = screen(h, p);
        h.hover_at(pos);
        h.step();
        button(h, pos, true);
        button(h, pos, false);
        h.run_steps(30);
    }

    pub(super) fn double_click(h: &mut Harness<'_, State>, p: (f64, f64)) {
        let pos = screen(h, p);
        h.hover_at(pos);
        h.step();
        for _ in 0..2 {
            button(h, pos, true);
            button(h, pos, false);
        }
        h.run_steps(30);
    }

    pub(super) fn drag(h: &mut Harness<'_, State>, from: (f64, f64), to: (f64, f64)) {
        let (a, b) = (screen(h, from), screen(h, to));
        h.hover_at(a);
        h.step();
        button(h, a, true);
        for i in 1..=10_u8 {
            h.hover_at(a + (b - a) * (f32::from(i) / 10.0));
            h.step();
        }
        button(h, b, false);
        h.run_steps(5);
    }

    pub(super) fn home(h: &Harness<'_, State>) -> Home {
        h.state().document.read().home().clone()
    }

    pub(super) fn close(a: Point2, b: (f64, f64)) -> bool {
        a.distance(Point2::new(b.0, b.1)) < 0.6
    }

    pub(super) fn square(size: f64) -> Home {
        let mut doc = Document::default();
        let pts = [(0.0, 0.0), (size, 0.0), (size, size), (0.0, size)];
        for i in 0..4 {
            let (a, b) = (pts[i], pts[(i + 1) % 4]);
            let wall = Wall::new(
                doc.new_wall_id(),
                Point2::new(a.0, a.1),
                Point2::new(b.0, b.1),
            );
            doc.execute(Command::insert(wall)).unwrap();
        }
        doc.home().clone()
    }

    #[test]
    fn clicking_draws_chained_walls() {
        let mut h = harness(Home::default(), Tool::Walls);
        click(&mut h, (0.0, 0.0));
        click(&mut h, (400.0, 0.0));
        click(&mut h, (400.0, 300.0));
        double_click(&mut h, (400.0, 300.0));
        let walls = home(&h).walls;
        assert_eq!(walls.len(), 2, "{walls:?}");
        assert!(close(walls[0].start, (0.0, 0.0)) && close(walls[0].end, (400.0, 0.0)));
        assert!(close(walls[1].start, (400.0, 0.0)) && close(walls[1].end, (400.0, 300.0)));
        assert!(!h.state().plan.is_drawing(), "double click ends the chain");
    }

    #[test]
    fn typing_a_length_creates_an_exact_wall() {
        let mut h = harness(Home::default(), Tool::Walls);
        click(&mut h, (0.0, 0.0));
        h.hover_at(screen(&h, (300.0, 0.0)));
        h.step();
        for c in ["2", "5", "7"] {
            h.event(Event::Text(c.into()));
            h.step();
        }
        h.key_press(Key::Enter);
        h.run_steps(3);
        let walls = home(&h).walls;
        assert_eq!(walls.len(), 1);
        assert!(close(walls[0].end, (257.0, 0.0)), "{:?}", walls[0].end);
    }

    #[test]
    fn double_click_inside_walls_detects_the_room() {
        let mut h = harness(square(400.0), Tool::Rooms);
        double_click(&mut h, (200.0, 200.0));
        let rooms = home(&h).rooms;
        assert_eq!(rooms.len(), 1);
        assert!(
            (rooms[0].area() - 385.0 * 385.0).abs() < 1.0,
            "{}",
            rooms[0].area()
        );
    }

    #[test]
    fn dimension_tool_places_offset_dimension() {
        let mut h = harness(square(400.0), Tool::Dimensions);
        click(&mut h, (0.0, 0.0));
        click(&mut h, (400.0, 0.0));
        click(&mut h, (200.0, -60.0));
        let dims = home(&h).dimensions;
        assert_eq!(dims.len(), 1);
        assert!((dims[0].length() - 400.0).abs() < 0.6);
        assert!((dims[0].offset - 60.0).abs() < 2.0, "{}", dims[0].offset);
    }

    #[test]
    fn dragging_a_wall_moves_it_and_its_joined_neighbors() {
        let mut h = harness(square(400.0), Tool::Select);
        drag(&mut h, (200.0, 0.0), (200.0, -100.0));
        let home = home(&h);
        let top = &home.walls[0];
        assert!(
            close(top.start, (0.0, -100.0)) && close(top.end, (400.0, -100.0)),
            "{top:?}"
        );
        assert!(
            close(home.walls[1].start, (400.0, -100.0)),
            "neighbor followed"
        );
        assert!(close(home.walls[3].end, (0.0, -100.0)), "neighbor followed");
        assert_eq!(h.state().document.read().revision(), 1, "one undoable step");
    }

    #[test]
    fn corner_handle_moves_both_walls() {
        let mut h = harness(square(400.0), Tool::Select);
        click(&mut h, (200.0, 0.0));
        assert_eq!(h.state().selection.len(), 1);
        // Shift disables magnetism, so the corner lands exactly where dropped.
        h.event(Event::ModifiersChanged(Modifiers::SHIFT));
        drag(&mut h, (400.0, 0.0), (500.0, -50.0));
        h.event(Event::ModifiersChanged(Modifiers::NONE));
        let home = home(&h);
        assert!(
            close(home.walls[0].end, (500.0, -50.0)),
            "{:?}",
            home.walls[0].end
        );
        assert!(close(home.walls[1].start, (500.0, -50.0)));
    }

    #[test]
    fn box_selection_picks_contained_elements() {
        let mut h = harness(square(400.0), Tool::Select);
        drag(&mut h, (-50.0, -50.0), (450.0, 50.0));
        let selection: Vec<_> = h.state().selection.iter().copied().collect();
        assert_eq!(selection, vec![ElementId::Wall(WallId(1))]);
    }

    #[test]
    fn label_tool_asks_for_text_where_clicked() {
        let mut h = harness(Home::default(), Tool::Labels);
        click(&mut h, (120.0, 80.0));
        match h.state().events.as_slice() {
            [PlanEvent::NewLabel(at)] => assert!(at.distance(Point2::new(120.0, 80.0)) < 1.0),
            other => panic!("unexpected events {other:?}"),
        }
    }
}

#[cfg(test)]
mod furniture_tests {
    use newera_core::align_to_wall;

    use super::tests::{click, close, drag, harness, home, square};
    use super::*;

    #[test]
    fn placing_from_the_catalog_creates_and_selects_the_piece() {
        let mut h = harness(Home::default(), Tool::Place("sofa-3"));
        click(&mut h, (200.0, 150.0));
        let home = home(&h);
        assert_eq!(home.furniture.len(), 1);
        let sofa = &home.furniture[0];
        assert!(close(sofa.position, (200.0, 150.0)), "{:?}", sofa.position);
        assert_eq!((sofa.width, sofa.depth), (210.0, 90.0));
        assert!(h.state().selection.contains(&sofa.id.into()));
        assert!(matches!(
            h.state().events.as_slice(),
            [PlanEvent::Placed(_)]
        ));
    }

    #[test]
    fn doors_snap_into_the_nearest_wall_when_placed_and_moved() {
        let mut h = harness(square(400.0), Tool::Place("door"));
        click(&mut h, (150.0, 25.0));
        let door = home(&h).furniture[0].clone();
        assert!(
            door.position.y.abs() < 0.6 && (door.position.x - 150.0).abs() < 0.6,
            "{:?}",
            door.position
        );
        assert_eq!(home(&h).wall_cuts()[0].len(), 1, "the door cuts the wall");

        h.state_mut().tool = Tool::Select;
        drag(&mut h, (150.0, 0.0), (260.0, 30.0));
        let moved = home(&h).furniture[0].clone();
        assert!(
            moved.position.y.abs() < 0.6,
            "stays in the wall: {:?}",
            moved.position
        );
        assert!(
            (moved.position.x - 260.0).abs() < 1.0,
            "{:?}",
            moved.position
        );
    }

    #[test]
    fn handles_rotate_and_resize_a_piece() {
        let mut base = Home::default();
        let id = base.new_furniture_id();
        let mut table = newera_catalog::find("dining-table-4")
            .unwrap()
            .instantiate(id, Point2::new(200.0, 150.0));
        let wall = newera_core::Wall::new(
            newera_core::WallId(99),
            Point2::new(-500.0, -500.0),
            Point2::new(-400.0, -500.0),
        );
        align_to_wall(&mut table, &wall, 0.0);
        table.position = Point2::new(200.0, 150.0);
        table.angle = 0.0;
        base.furniture.push(table.clone());

        let mut h = harness(base, Tool::Select);
        h.state_mut().selection.insert(id.into());
        h.run_steps(2);
        // Rotation handle sits 25 cm behind the back edge; drag it to the right.
        let handle = table.to_plan((0.0, -table.depth / 2.0 - 25.0));
        drag(&mut h, (handle.x, handle.y), (320.0, 150.0));
        let rotated = home(&h).furniture[0].clone();
        assert!(
            (rotated.angle - 90.0).abs() < 0.5,
            "angle {}",
            rotated.angle
        );

        // Resize from the front-right corner.
        let corner = rotated.to_plan((rotated.width / 2.0, rotated.depth / 2.0));
        let target = rotated.to_plan((80.0, 50.0));
        drag(&mut h, (corner.x, corner.y), (target.x, target.y));
        let resized = home(&h).furniture[0].clone();
        assert!(
            (resized.width - 160.0).abs() < 1.5 && (resized.depth - 100.0).abs() < 1.5,
            "{} x {}",
            resized.width,
            resized.depth
        );
    }
}

impl PlanView {
    /// Plan point at the middle of the view.
    pub(crate) fn view_center(&self) -> Point2 {
        self.camera.center
    }
}
