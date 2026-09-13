//! 2D floor plan editor.

use eframe::egui::{self, Color32, Pos2, Stroke, Vec2};
use newera_core::{Compass, Home, Point2, Wall, WallId};

/// Editing tool active in the plan, like Sweet Home 3D's toolbar modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Tool {
    #[default]
    Select,
    CreateWalls,
}

#[derive(Debug)]
pub(crate) struct PlanView {
    /// World point (cm) shown at the center of the viewport.
    center: Point2,
    /// Screen points per centimeter.
    zoom: f32,
    /// First point of the wall being drawn.
    wall_start: Option<Point2>,
    /// Frame the home on the next frame (set on startup and on demand).
    pending_fit: bool,
}

/// What the plan wants the app to do after a frame.
#[derive(Debug, Default)]
pub(crate) struct PlanOutput {
    /// A wall the user finished drawing, as `(start, end)`.
    pub(crate) new_wall: Option<(Point2, Point2)>,
    /// `Some` when the user clicked in select mode; the inner value is the
    /// wall under the cursor, or `None` to clear the selection.
    #[allow(clippy::option_option)]
    pub(crate) select: Option<Option<WallId>>,
}

const GRID_CM: f64 = 50.0;
const SNAP_CM: f64 = 5.0;
const SNAP_PX: f32 = 10.0;

const BACKGROUND: Color32 = Color32::from_rgb(252, 252, 250);
const GRID_MINOR: Color32 = Color32::from_rgb(234, 236, 238);
const GRID_MAJOR: Color32 = Color32::from_rgb(210, 214, 219);
const WALL_FILL: Color32 = Color32::from_rgb(90, 90, 96);
const WALL_SELECTED: Color32 = Color32::from_rgb(40, 120, 230);
const ROOM_FILL: Color32 = Color32::from_rgb(238, 228, 212);
const PREVIEW: Color32 = Color32::from_rgb(40, 120, 230);

impl Default for PlanView {
    fn default() -> Self {
        Self {
            center: Point2::new(400.0, 300.0),
            zoom: 0.8,
            wall_start: None,
            pending_fit: true,
        }
    }
}

impl PlanView {
    pub(crate) fn cancel(&mut self) {
        self.wall_start = None;
    }

    /// Requests that the next frame zooms to fit the whole home.
    pub(crate) fn request_fit(&mut self) {
        self.pending_fit = true;
    }

    #[allow(clippy::cast_possible_truncation)]
    fn fit(&mut self, home: &Home, rect: egui::Rect) {
        let Some((min, max)) = home.bounds() else {
            return;
        };
        let margin = 0.85;
        let (w, h) = ((max.x - min.x).max(100.0), (max.y - min.y).max(100.0));
        let zoom = f64::from(rect.width()) / w * margin;
        let zoom = zoom.min(f64::from(rect.height()) / h * margin);
        self.zoom = (zoom as f32).clamp(0.05, 20.0);
        self.center = Point2::new(min.x.midpoint(max.x), min.y.midpoint(max.y));
    }

    pub(crate) fn ui(
        &mut self,
        ui: &mut egui::Ui,
        home: &Home,
        tool: Tool,
        selected: Option<WallId>,
    ) -> PlanOutput {
        let rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        let mut out = PlanOutput::default();

        if self.pending_fit && rect.width() > 1.0 {
            self.fit(home, rect);
            self.pending_fit = false;
        }
        if response.hovered() && ui.input(|i| i.key_pressed(egui::Key::F)) {
            self.fit(home, rect);
        }

        if tool != Tool::CreateWalls {
            self.wall_start = None;
        }

        self.navigate(ui, &response, rect);

        painter.rect_filled(rect, 0.0, BACKGROUND);
        self.draw_grid(&painter, rect);
        for room in &home.rooms {
            self.draw_room(&painter, room);
        }
        if home.compass.visible {
            self.draw_compass(&painter, rect, &home.compass);
        }
        for wall in &home.walls {
            let color = if Some(wall.id) == selected {
                WALL_SELECTED
            } else {
                WALL_FILL
            };
            self.draw_wall(&painter, wall, color);
        }

        let hover = response
            .hover_pos()
            .map(|p| self.snap(home, self.to_world(rect, p)));

        match tool {
            Tool::Select => {
                if response.clicked_by(egui::PointerButton::Primary) {
                    out.select = Some(hover.and_then(|p| self.pick_wall(home, p)));
                }
            }
            Tool::CreateWalls => {
                if let Some(p) = hover {
                    self.draw_crosshair(&painter, rect, p);
                }
                if response.clicked_by(egui::PointerButton::Primary)
                    && let Some(p) = hover
                {
                    match self.wall_start {
                        Some(start) if start.distance(p) >= 1.0 => {
                            out.new_wall = Some((start, p));
                            self.wall_start = Some(p); // keep chaining walls
                        }
                        Some(_) => {}
                        None => self.wall_start = Some(p),
                    }
                }
                if response.double_clicked() || response.clicked_by(egui::PointerButton::Secondary)
                {
                    self.wall_start = None;
                }
                if let (Some(start), Some(end)) = (self.wall_start, hover) {
                    self.draw_preview(&painter, rect, start, end);
                }
            }
        }

        self.draw_scale(&painter, rect);
        out
    }

    fn navigate(&mut self, ui: &egui::Ui, response: &egui::Response, rect: egui::Rect) {
        if response.dragged_by(egui::PointerButton::Middle)
            || (response.dragged_by(egui::PointerButton::Secondary) && self.wall_start.is_none())
        {
            let delta = response.drag_delta() / self.zoom;
            self.center.x -= f64::from(delta.x);
            self.center.y -= f64::from(delta.y);
        }
        if let Some(pointer) = response.hover_pos() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                // Zoom around the cursor: the world point under it stays put.
                let before = self.to_world(rect, pointer);
                self.zoom = (self.zoom * (scroll * 0.002).exp()).clamp(0.05, 20.0);
                let after = self.to_world(rect, pointer);
                self.center.x += before.x - after.x;
                self.center.y += before.y - after.y;
            }
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn to_screen(&self, rect: egui::Rect, p: Point2) -> Pos2 {
        rect.center()
            + Vec2::new(
                ((p.x - self.center.x) as f32) * self.zoom,
                ((p.y - self.center.y) as f32) * self.zoom,
            )
    }

    fn to_world(&self, rect: egui::Rect, p: Pos2) -> Point2 {
        let d = (p - rect.center()) / self.zoom;
        Point2::new(
            self.center.x + f64::from(d.x),
            self.center.y + f64::from(d.y),
        )
    }

    /// Snaps to nearby wall endpoints first, then to the grid.
    fn snap(&self, home: &Home, p: Point2) -> Point2 {
        let radius = f64::from(SNAP_PX / self.zoom);
        let endpoint = home
            .walls
            .iter()
            .flat_map(|w| [w.start, w.end])
            .chain(self.wall_start)
            .filter(|q| q.distance(p) <= radius)
            .min_by(|a, b| a.distance(p).total_cmp(&b.distance(p)));
        endpoint.unwrap_or_else(|| {
            Point2::new(
                (p.x / SNAP_CM).round() * SNAP_CM,
                (p.y / SNAP_CM).round() * SNAP_CM,
            )
        })
    }

    fn pick_wall(&self, home: &Home, p: Point2) -> Option<WallId> {
        let tolerance = f64::from(6.0 / self.zoom);
        home.walls
            .iter()
            .map(|w| {
                (
                    w.id,
                    p.distance_to_segment(w.start, w.end) - w.thickness / 2.0,
                )
            })
            .filter(|(_, d)| *d <= tolerance)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    }

    #[allow(clippy::cast_possible_truncation)]
    fn draw_grid(&self, painter: &egui::Painter, rect: egui::Rect) {
        let min = self.to_world(rect, rect.left_top());
        let max = self.to_world(rect, rect.right_bottom());
        let step = if f64::from(self.zoom) * GRID_CM < 8.0 {
            GRID_CM * 10.0
        } else {
            GRID_CM
        };

        // Iterate over integer grid indices so float drift never skips a line;
        // every other line is a major one.
        #[allow(clippy::cast_possible_truncation)]
        let range = |lo: f64, hi: f64| (lo / step).floor() as i64..=(hi / step).ceil() as i64;
        let stroke = |i: i64| Stroke::new(1.0, if i % 2 == 0 { GRID_MAJOR } else { GRID_MINOR });

        for i in range(min.x, max.x) {
            #[allow(clippy::cast_precision_loss)]
            let sx = self.to_screen(rect, Point2::new(i as f64 * step, 0.0)).x;
            painter.vline(sx, rect.y_range(), stroke(i));
        }
        for i in range(min.y, max.y) {
            #[allow(clippy::cast_precision_loss)]
            let sy = self.to_screen(rect, Point2::new(0.0, i as f64 * step)).y;
            painter.hline(rect.x_range(), sy, stroke(i));
        }
    }

    fn draw_room(&self, painter: &egui::Painter, room: &newera_core::Room) {
        let rect = painter.clip_rect();
        let points: Vec<Pos2> = room
            .points
            .iter()
            .map(|p| self.to_screen(rect, *p))
            .collect();
        painter.add(egui::Shape::convex_polygon(
            points.clone(),
            ROOM_FILL,
            Stroke::new(1.0, Color32::from_rgb(170, 150, 120)),
        ));
        let centroid = egui::Rect::from_points(&points).center();
        let area_m2 = room.area() / 10_000.0;
        painter.text(
            centroid,
            egui::Align2::CENTER_CENTER,
            format!("{}\n{area_m2:.2} m²", room.name),
            egui::FontId::proportional(13.0),
            Color32::from_rgb(90, 75, 55),
        );
    }

    #[allow(clippy::cast_possible_truncation)]
    fn draw_wall(&self, painter: &egui::Painter, wall: &Wall, color: Color32) {
        let rect = painter.clip_rect();
        let (a, b) = (
            self.to_screen(rect, wall.start),
            self.to_screen(rect, wall.end),
        );
        let dir = (b - a).normalized();
        let half = (wall.thickness as f32 * self.zoom / 2.0).max(1.0);
        let side = Vec2::new(-dir.y, dir.x) * half;
        painter.add(egui::Shape::convex_polygon(
            vec![a + side, b + side, b - side, a - side],
            color,
            Stroke::NONE,
        ));
    }

    fn draw_crosshair(&self, painter: &egui::Painter, rect: egui::Rect, p: Point2) {
        let s = self.to_screen(rect, p);
        let stroke = Stroke::new(1.0, PREVIEW.gamma_multiply(0.6));
        painter.line_segment([s - Vec2::X * 8.0, s + Vec2::X * 8.0], stroke);
        painter.line_segment([s - Vec2::Y * 8.0, s + Vec2::Y * 8.0], stroke);
    }

    fn draw_preview(&self, painter: &egui::Painter, rect: egui::Rect, start: Point2, end: Point2) {
        let preview = Wall::new(WallId(0), start, end);
        self.draw_wall(painter, &preview, PREVIEW.gamma_multiply(0.5));
        let mid = self.to_screen(
            rect,
            Point2::new(start.x.midpoint(end.x), start.y.midpoint(end.y)),
        );
        painter.text(
            mid + Vec2::new(0.0, -14.0),
            egui::Align2::CENTER_BOTTOM,
            format!("{:.0} cm", start.distance(end)),
            egui::FontId::proportional(12.0),
            PREVIEW,
        );
    }

    /// Compass rose: a circle with a needle pointing north and an "N" label.
    #[allow(clippy::cast_possible_truncation)]
    fn draw_compass(&self, painter: &egui::Painter, rect: egui::Rect, compass: &Compass) {
        const INK: Color32 = Color32::from_rgb(70, 70, 80);
        let center = self.to_screen(rect, compass.center);
        let radius = (compass.diameter as f32 * self.zoom / 2.0).max(8.0);
        painter.circle_stroke(center, radius, Stroke::new(1.5, INK));

        let angle = (compass.north_degrees as f32).to_radians();
        let north = Vec2::new(angle.sin(), -angle.cos());
        let side = Vec2::new(-north.y, north.x) * radius * 0.25;
        painter.add(egui::Shape::convex_polygon(
            vec![center + north * radius, center + side, center - side],
            INK,
            Stroke::NONE,
        ));
        painter.add(egui::Shape::convex_polygon(
            vec![center - north * radius, center - side, center + side],
            Color32::TRANSPARENT,
            Stroke::new(1.0, INK),
        ));
        painter.text(
            center + north * (radius + 10.0),
            egui::Align2::CENTER_CENTER,
            "N",
            egui::FontId::proportional(13.0),
            INK,
        );
    }

    #[allow(clippy::cast_possible_truncation)]
    fn draw_scale(&self, painter: &egui::Painter, rect: egui::Rect) {
        let meters = [0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0]
            .into_iter()
            .find(|m| (m * 100.0) as f32 * self.zoom >= 60.0)
            .unwrap_or(100.0);
        let width = (meters * 100.0) as f32 * self.zoom;
        let origin = rect.left_bottom() + Vec2::new(12.0, -12.0);
        let stroke = Stroke::new(2.0, Color32::from_gray(80));
        painter.line_segment([origin, origin + Vec2::X * width], stroke);
        painter.text(
            origin + Vec2::new(width / 2.0, -4.0),
            egui::Align2::CENTER_BOTTOM,
            format!("{meters} m"),
            egui::FontId::proportional(11.0),
            Color32::from_gray(80),
        );
    }
}
