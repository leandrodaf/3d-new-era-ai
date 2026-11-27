//! Docked Camera Control side panel: live visitor positioning, storey
//! eye-level presets, lens sliders, compass nudges, and saved viewpoints.

use eframe::egui::{self, RichText, Vec2};
use newera_core::{Camera, Command, Level};

use crate::app::NewEraApp;
use crate::i18n::tr;
use crate::icon;
use crate::view::scene::Visitor;

/// Default standing eye height above a floor, cm.
const STANDING_EYE_CM: f64 = 165.0;
const SEATED_EYE_CM: f64 = 115.0;
const HIGH_EYE_CM: f64 = 220.0;
const STEP_WALK_CM: f64 = 35.0;
const STEP_Z_CM: f64 = 25.0;

/// UI state for the docked Camera Control side panel.
#[derive(Debug)]
pub(crate) struct CameraControlPanel {
    pub(crate) open: bool,
    pub(crate) eye_offset: f64,
    pub(crate) save_name: String,
    pub(crate) selected_stored: Option<usize>,
}

impl Default for CameraControlPanel {
    fn default() -> Self {
        Self {
            open: false,
            eye_offset: STANDING_EYE_CM,
            save_name: String::new(),
            selected_stored: None,
        }
    }
}

/// Moves `cam` along its plan heading (`forward_cm`, `right_cm`) and vertically (`up_cm`).
pub(crate) fn walk_step(cam: &mut Camera, forward_cm: f64, right_cm: f64, up_cm: f64) {
    let (fx, fy) = cam.direction();
    let (rx, ry) = (-fy, fx);
    cam.x += fx * forward_cm + rx * right_cm;
    cam.y += fy * forward_cm + ry * right_cm;
    cam.z = (cam.z + up_cm).max(0.0);
}

/// Sets `cam.z` to `storey_elev_cm + eye_offset_cm` (clamped at or above ground).
pub(crate) fn apply_storey_eye_level(cam: &mut Camera, storey_elev_cm: f64, eye_offset_cm: f64) {
    cam.z = (storey_elev_cm + eye_offset_cm).max(0.0);
}

/// Maps `yaw` (degrees) to a human-readable compass direction in Newera's plan frame.
pub(crate) fn compass_label(yaw: f64) -> &'static str {
    let norm = (yaw + 180.0).rem_euclid(360.0) - 180.0;
    if (-22.5..=22.5).contains(&norm) {
        "S (+y)"
    } else if norm > 22.5 && norm <= 67.5 {
        "SW"
    } else if norm > 67.5 && norm <= 112.5 {
        "W (-x)"
    } else if norm > 112.5 && norm <= 157.5 {
        "NW"
    } else if (-67.5..-22.5).contains(&norm) {
        "SE"
    } else if (-112.5..-67.5).contains(&norm) {
        "E (+x)"
    } else if (-157.5..-112.5).contains(&norm) {
        "NE"
    } else {
        "N (-y)"
    }
}

fn ensure_visitor<'a>(app: &'a mut NewEraApp, fallback: &Camera) -> &'a mut Camera {
    &mut app
        .scene
        .visitor
        .get_or_insert_with(|| Visitor {
            camera: fallback.clone(),
        })
        .camera
}

/// Draws the docked right-hand Camera Control panel when open.
pub(crate) fn show(app: &mut NewEraApp, ui: &mut egui::Ui) {
    if !app.camera_control.open {
        return;
    }

    let t = crate::theme::of(ui.visuals());
    egui::Panel::right("camera_control")
        .resizable(true)
        .default_size(276.0)
        .min_size(240.0)
        .max_size(360.0)
        .frame(
            egui::Frame::new()
                .fill(t.surface)
                .inner_margin(egui::Margin::symmetric(10, 8)),
        )
        .show(ui, |ui| {
            let close = header(ui);
            if close {
                app.camera_control.open = false;
                app.set_camera_panel_open(false);
                return;
            }
            ui.separator();

            egui::ScrollArea::vertical()
                .id_salt("camera_control_scroll")
                .show(ui, |ui| {
                    panel_body(app, ui);
                });
        });
}

fn header(ui: &mut egui::Ui) -> bool {
    let t = crate::theme::of(ui.visuals());
    let mut close = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new(icon::CAMERA).size(15.0).color(t.accent));
        ui.heading(tr("Controle de câmera"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(egui::Button::new(RichText::new(icon::X).size(12.0)).frame(false))
                .on_hover_text(tr("Fechar"))
                .clicked()
            {
                close = true;
            }
        });
    });
    close
}

fn panel_body(app: &mut NewEraApp, ui: &mut egui::Ui) {
    let (levels, cameras) = {
        let doc = app.document.read();
        let home = doc.home();
        (home.levels.clone(), home.cameras.clone())
    };

    mode_section(app, ui, &cameras.observer);
    ui.add_space(6.0);
    ui.separator();

    storey_section(app, ui, &levels, &cameras.observer);
    ui.add_space(6.0);
    ui.separator();

    pose_section(app, ui, &cameras.observer);
    ui.add_space(6.0);
    ui.separator();

    saved_section(app, ui, &cameras);
}

fn mode_section(app: &mut NewEraApp, ui: &mut egui::Ui, observer: &Camera) {
    let visitor_on = app.scene.visitor.is_some();
    ui.horizontal(|ui| {
        if ui
            .selectable_label(
                visitor_on,
                format!("{} {}", icon::PERSON_SIMPLE_WALK, tr("Visitante")),
            )
            .clicked()
        {
            let _ = ensure_visitor(app, observer);
        }
        if ui
            .selectable_label(
                !visitor_on,
                format!("{} {}", icon::GLOBE, tr("Visão aérea")),
            )
            .clicked()
        {
            app.scene.visitor = None;
        }
    });
}

fn storey_section(app: &mut NewEraApp, ui: &mut egui::Ui, levels: &[Level], observer: &Camera) {
    let t = crate::theme::of(ui.visuals());
    ui.label(
        RichText::new(tr("Pavimento e altura do olho"))
            .strong()
            .color(t.ink),
    );
    ui.add_space(3.0);

    let current_z = app
        .scene
        .visitor
        .as_ref()
        .map_or(observer.z, |v| v.camera.z);

    if levels.is_empty() {
        ui.weak(tr("Térreo único (0.00 m)"));
    } else {
        ui.horizontal_wrapped(|ui| {
            for (idx, level) in levels.iter().enumerate() {
                let next_elev = levels.get(idx + 1).map_or(f64::INFINITY, |l| l.elevation);
                let active = current_z >= level.elevation - 5.0 && current_z < next_elev - 5.0;
                let label = format!("{} (+{:.2}m)", level.name, level.elevation / 100.0);
                if ui.selectable_label(active, label).clicked() {
                    let offset = app.camera_control.eye_offset;
                    let cam = ensure_visitor(app, observer);
                    apply_storey_eye_level(cam, level.elevation, offset);
                }
            }
        });
    }

    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        for (label, offset) in [
            ("Seated 115", SEATED_EYE_CM),
            ("Standing 165", STANDING_EYE_CM),
            ("High 220", HIGH_EYE_CM),
        ] {
            let active = (app.camera_control.eye_offset - offset).abs() < 1.0;
            if ui.selectable_label(active, format!("{label} cm")).clicked() {
                app.camera_control.eye_offset = offset;
                let base = current_storey_elev(levels, current_z);
                let cam = ensure_visitor(app, observer);
                apply_storey_eye_level(cam, base, offset);
            }
        }
    });

    ui.add_space(2.0);
    let mut offset = app.camera_control.eye_offset;
    let resp = ui.add(
        egui::Slider::new(&mut offset, 20.0..=330.0)
            .step_by(5.0)
            .text("Eye offset")
            .suffix(" cm"),
    );
    if resp.changed() {
        app.camera_control.eye_offset = offset;
        let base = current_storey_elev(levels, current_z);
        let cam = ensure_visitor(app, observer);
        apply_storey_eye_level(cam, base, offset);
    }
}

fn current_storey_elev(levels: &[Level], z: f64) -> f64 {
    let mut chosen = 0.0;
    for lv in levels {
        if z >= lv.elevation - 5.0 {
            chosen = lv.elevation;
        }
    }
    chosen
}

fn pose_section(app: &mut NewEraApp, ui: &mut egui::Ui, observer: &Camera) {
    let t = crate::theme::of(ui.visuals());
    ui.label(
        RichText::new(tr("Posição, elevação e lente"))
            .strong()
            .color(t.ink),
    );
    ui.add_space(3.0);

    let mut draft = app
        .scene
        .visitor
        .as_ref()
        .map_or_else(|| observer.clone(), |v| v.camera.clone());
    let mut edited = false;

    edited |= ui
        .add(
            egui::Slider::new(&mut draft.x, -500.0..=1500.0)
                .step_by(5.0)
                .text("X")
                .suffix(" cm"),
        )
        .changed();
    edited |= ui
        .add(
            egui::Slider::new(&mut draft.y, -500.0..=1500.0)
                .step_by(5.0)
                .text("Y")
                .suffix(" cm"),
        )
        .changed();
    edited |= ui
        .add(
            egui::Slider::new(&mut draft.z, 0.0..=1200.0)
                .step_by(5.0)
                .text("Z")
                .suffix(" cm"),
        )
        .changed();
    let yaw_label = format!("Yaw ({})", compass_label(draft.yaw));
    edited |= ui
        .add(
            egui::Slider::new(&mut draft.yaw, -180.0..=180.0)
                .step_by(1.0)
                .text(yaw_label)
                .suffix("°"),
        )
        .changed();
    edited |= ui
        .add(
            egui::Slider::new(&mut draft.pitch, -85.0..=85.0)
                .step_by(1.0)
                .text("Pitch")
                .suffix("°"),
        )
        .changed();
    edited |= ui
        .add(
            egui::Slider::new(&mut draft.fov, 25.0..=120.0)
                .step_by(1.0)
                .text("FOV")
                .suffix("°"),
        )
        .changed();

    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        for (label, yaw) in [("N", 180.0), ("E", -90.0), ("S", 0.0), ("W", 90.0)] {
            if ui.button(label).clicked() {
                draft.yaw = yaw;
                edited = true;
            }
        }
        if ui.button("Level 0°").clicked() {
            draft.pitch = 0.0;
            edited = true;
        }
    });

    ui.add_space(2.0);
    ui.horizontal_wrapped(|ui| {
        if ui.button("Fwd").clicked() {
            walk_step(&mut draft, STEP_WALK_CM, 0.0, 0.0);
            edited = true;
        }
        if ui.button("Back").clicked() {
            walk_step(&mut draft, -STEP_WALK_CM, 0.0, 0.0);
            edited = true;
        }
        if ui.button("Left").clicked() {
            walk_step(&mut draft, 0.0, -STEP_WALK_CM, 0.0);
            edited = true;
        }
        if ui.button("Right").clicked() {
            walk_step(&mut draft, 0.0, STEP_WALK_CM, 0.0);
            edited = true;
        }
        if ui.button("Z +25").clicked() {
            walk_step(&mut draft, 0.0, 0.0, STEP_Z_CM);
            edited = true;
        }
        if ui.button("Z -25").clicked() {
            walk_step(&mut draft, 0.0, 0.0, -STEP_Z_CM);
            edited = true;
        }
    });

    if edited {
        app.scene.visitor = Some(Visitor { camera: draft });
    }
}

fn saved_section(app: &mut NewEraApp, ui: &mut egui::Ui, cameras: &newera_core::Cameras) {
    let t = crate::theme::of(ui.visuals());
    ui.label(
        RichText::new(tr("Pontos de vista salvos"))
            .strong()
            .color(t.ink),
    );
    ui.add_space(3.0);

    let mut jump_to: Option<(usize, Camera)> = None;
    let mut overwrite_idx: Option<usize> = None;
    let mut delete_idx: Option<usize> = None;

    if cameras.stored.is_empty() {
        ui.weak(tr("Nenhum ponto de vista salvo"));
    } else {
        for (i, cam) in cameras.stored.iter().enumerate() {
            let name = cam
                .name
                .clone()
                .unwrap_or_else(|| crate::i18n::fill("Ponto de vista {}", &[&(i + 1)]));
            let selected = app.camera_control.selected_stored == Some(i);
            ui.horizontal(|ui| {
                let room = (ui.available_width() - 62.0).max(60.0);
                let btn = egui::Button::selectable(selected, &name).min_size(Vec2::new(room, 0.0));
                if ui.add_sized(Vec2::new(room, 22.0), btn).clicked() {
                    jump_to = Some((i, cam.clone()));
                }
                let update_resp = ui
                    .small_button(icon::FLOPPY_DISK)
                    .on_hover_text("Update saved viewpoint from current camera");
                update_resp.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::Button, true, format!("Update {name}"))
                });
                if update_resp.clicked() {
                    overwrite_idx = Some(i);
                }
                let delete_resp = ui
                    .small_button(icon::TRASH)
                    .on_hover_text("Delete saved viewpoint");
                delete_resp.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::Button, true, format!("Delete {name}"))
                });
                if delete_resp.clicked() {
                    delete_idx = Some(i);
                }
            });
        }
    }

    ui.add_space(4.0);
    let mut save_new = false;
    ui.horizontal(|ui| {
        let field_w = (ui.available_width() - 60.0).max(80.0);
        ui.add_sized(
            Vec2::new(field_w, 22.0),
            egui::TextEdit::singleline(&mut app.camera_control.save_name)
                .hint_text("Viewpoint name…"),
        );
        if ui.button("Save").clicked() {
            save_new = true;
        }
    });

    if let Some((i, cam)) = jump_to {
        app.camera_control.selected_stored = Some(i);
        app.scene.visitor = Some(Visitor { camera: cam });
    }

    if let Some(i) = overwrite_idx
        && i < cameras.stored.len()
    {
        let mut cam = active_camera(app, &cameras.observer);
        cam.name.clone_from(&cameras.stored[i].name);
        let mut next = cameras.clone();
        next.observer = cam.clone();
        next.observer_active = true;
        next.stored[i] = cam;
        app.camera_control.selected_stored = Some(i);
        app.run(|doc| doc.execute(Command::SetCameras { cameras: next }));
    } else if let Some(i) = delete_idx
        && i < cameras.stored.len()
    {
        let mut next = cameras.clone();
        next.stored.remove(i);
        if app.camera_control.selected_stored == Some(i) {
            app.camera_control.selected_stored = None;
        }
        app.run(|doc| doc.execute(Command::SetCameras { cameras: next }));
    } else if save_new {
        let mut cam = active_camera(app, &cameras.observer);
        let mut next = cameras.clone();
        let trimmed = app.camera_control.save_name.trim().to_owned();
        cam.name = Some(if trimmed.is_empty() {
            crate::i18n::fill("Ponto de vista {}", &[&(next.stored.len() + 1)])
        } else {
            trimmed
        });
        next.observer = cam.clone();
        next.observer_active = true;
        next.stored.push(cam);
        let idx = next.stored.len() - 1;
        app.camera_control.selected_stored = Some(idx);
        app.camera_control.save_name.clear();
        app.run(|doc| doc.execute(Command::SetCameras { cameras: next }));
    }
}

fn active_camera(app: &NewEraApp, fallback: &Camera) -> Camera {
    app.scene
        .visitor
        .as_ref()
        .map_or_else(|| fallback.clone(), |v| v.camera.clone())
}

#[cfg(test)]
mod tests {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;
    use newera_core::{Camera, Document, SharedDocument};

    use super::*;

    #[test]
    fn walk_step_moves_along_heading_and_clamps_elevation() {
        let mut cam = Camera {
            x: 100.0,
            y: 100.0,
            z: 10.0,
            yaw: 0.0, // Facing +y (South)
            ..Camera::default()
        };
        walk_step(&mut cam, 50.0, 20.0, -30.0);
        assert!((cam.x - 80.0).abs() < 1e-6);
        assert!((cam.y - 150.0).abs() < 1e-6);
        assert!(cam.z.abs() < 1e-6);
    }

    #[test]
    fn storey_eye_level_combines_elevation_and_offset() {
        let mut cam = Camera::default();
        apply_storey_eye_level(&mut cam, 350.0, 165.0);
        assert!((cam.z - 515.0).abs() < 1e-6);
    }

    #[test]
    fn compass_label_maps_cardinal_directions() {
        assert_eq!(compass_label(0.0), "S (+y)");
        assert_eq!(compass_label(90.0), "W (-x)");
        assert_eq!(compass_label(-90.0), "E (+x)");
        assert_eq!(compass_label(180.0), "N (-y)");
        assert_eq!(compass_label(-180.0), "N (-y)");
    }

    #[test]
    fn panel_toggles_visitor_and_saves_viewpoint() {
        let document = SharedDocument::new(Document::default());
        let mut h = Harness::builder()
            .with_size(egui::vec2(1280.0, 800.0))
            .with_step_dt(1.0 / 60.0)
            .build_eframe(move |cc| NewEraApp::new(cc, document, None));
        h.run_steps(4);

        assert!(!h.state().camera_control.open);
        h.state_mut().camera_control.open = true;
        h.run_steps(3);

        h.get_by_label("Controle de câmera");
        h.get_by_label("Standing 165 cm").click();
        h.run_steps(3);

        let visitor_z = h
            .state()
            .scene
            .visitor
            .as_ref()
            .expect("visitor active")
            .camera
            .z;
        assert!((visitor_z - 165.0).abs() < 1e-6);

        h.state_mut().camera_control.save_name = "Test Floor Eye".to_owned();
        h.run_steps(2);
        h.get_by_label("Save").click();
        h.run_steps(3);

        let stored = h.state().document.read().home().cameras.stored.clone();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].name.as_deref(), Some("Test Floor Eye"));

        // Move camera and overwrite saved viewpoint in place
        h.get_by_label("N").click();
        h.run_steps(2);
        h.get_by_label("Update Test Floor Eye").click();
        h.run_steps(3);
        let stored = h.state().document.read().home().cameras.stored.clone();
        assert_eq!(stored.len(), 1);
        assert!((stored[0].yaw - 180.0).abs() < 1e-6);

        // Delete saved viewpoint
        h.get_by_label("Delete Test Floor Eye").click();
        h.run_steps(3);
        assert!(h.state().document.read().home().cameras.stored.is_empty());
    }
}
