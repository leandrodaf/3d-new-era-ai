//! "Criar foto": realistic renders of the current 3D point of view, made in
//! a background thread so the editor stays responsive.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui::{self, RichText};
use egui_phosphor::regular as icon;

use crate::app::NewEraApp;

/// Settings and progress of the photo window.
pub(crate) struct PhotoWindow {
    pub(crate) quality: newera_render::PhotoQuality,
    /// Local solar hour.
    pub(crate) hour: f64,
    pub(crate) size: [u32; 2],
    job: Option<(Instant, Arc<Mutex<Option<image::RgbaImage>>>)>,
    result: Option<(image::RgbaImage, egui::TextureHandle, Duration)>,
}

impl std::fmt::Debug for PhotoWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PhotoWindow")
            .field("hour", &self.hour)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

impl Default for PhotoWindow {
    fn default() -> Self {
        Self {
            quality: newera_render::PhotoQuality::Draft,
            hour: 10.0,
            size: [800, 600],
            job: None,
            result: None,
        }
    }
}

impl PhotoWindow {
    fn start(&mut self, app: &NewEraApp) {
        let (home, assets) = {
            let doc = app.document.read();
            (doc.home().clone(), doc.asset_dir())
        };
        let view = app.scene.current_view();
        let [w, h] = self.size;
        #[allow(clippy::cast_precision_loss)]
        let view = match &app.scene.visitor {
            Some(visitor) => newera_render::View::from_camera(&visitor.camera, w as f32 / h as f32),
            None => view,
        };
        let base = app
            .scene
            .visitor
            .as_ref()
            .map_or(home.cameras.top.time, |v| v.camera.time);
        let base = if base == 0 { 1_789_214_400_000 } else { base };
        let time =
            newera_render::at_local_hour(base, self.hour, home.compass.longitude.unwrap_or(-46.63));
        let quality = self.quality;
        let slot = Arc::new(Mutex::new(None));
        let out = Arc::clone(&slot);
        std::thread::spawn(move || {
            let image =
                newera_render::photo_home(&home, &view, time, w, h, assets.as_deref(), quality);
            if let Ok(mut guard) = out.lock() {
                *guard = Some(image);
            }
        });
        self.job = Some((Instant::now(), slot));
        self.result = None;
    }
}

/// Shows the photo window while it is open.
pub(crate) fn show(app: &mut NewEraApp, ctx: &egui::Context) {
    let Some(mut window) = app.photo.take() else {
        return;
    };
    let mut open = true;
    let mut start = false;
    let mut save = false;
    egui::Window::new(format!(
        "{} {}",
        icon::CAMERA,
        crate::i18n::tr("Criar foto")
    ))
    .open(&mut open)
    .default_width(560.0)
    .show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label(crate::i18n::tr("Qualidade"));
            for (quality, name) in [
                (
                    newera_render::PhotoQuality::Draft,
                    crate::i18n::tr("Rascunho"),
                ),
                (newera_render::PhotoQuality::Good, crate::i18n::tr("Boa")),
                (newera_render::PhotoQuality::Best, crate::i18n::tr("Máxima")),
            ] {
                ui.selectable_value(&mut window.quality, quality, name);
            }
        });
        ui.horizontal(|ui| {
            ui.label(crate::i18n::tr("Hora do dia"));
            ui.add(
                egui::Slider::new(&mut window.hour, 0.0..=24.0)
                    .step_by(0.25)
                    .suffix(" h"),
            );
        });
        ui.horizontal(|ui| {
            ui.label(crate::i18n::tr("Tamanho"));
            for size in [[640, 480], [800, 600], [1280, 960], [1920, 1080]] {
                ui.selectable_value(&mut window.size, size, format!("{}×{}", size[0], size[1]));
            }
        });
        ui.weak(crate::i18n::tr(
            "Usa o ponto de vista atual da vista 3D (aérea ou visitante).",
        ));
        ui.separator();
        let running = window.job.is_some();
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !running,
                    egui::Button::new(format!(
                        "{} {}",
                        icon::APERTURE,
                        crate::i18n::tr("Renderizar")
                    )),
                )
                .clicked()
            {
                start = true;
            }
            if ui
                .add_enabled(
                    window.result.is_some(),
                    egui::Button::new(format!(
                        "{} {}",
                        icon::FLOPPY_DISK,
                        crate::i18n::tr("Salvar PNG…")
                    )),
                )
                .clicked()
            {
                save = true;
            }
            if let Some((started, _)) = &window.job {
                ui.spinner();
                ui.label(format!(
                    "Renderizando… {:.0} s",
                    started.elapsed().as_secs_f32()
                ));
                ctx.request_repaint_after(Duration::from_millis(200));
            }
            if let Some((_, _, took)) = &window.result {
                ui.label(RichText::new(format!("Pronta em {:.1} s", took.as_secs_f32())).weak());
            }
        });
        if let Some((_, texture, _)) = &window.result {
            let width = ui.available_width();
            let size = texture.size_vec2();
            ui.image((
                texture.id(),
                egui::vec2(width, width * size.y / size.x.max(1.0)),
            ));
        }
    });

    // Collect a finished render.
    let finished = window.job.as_ref().and_then(|(started, slot)| {
        slot.lock()
            .ok()
            .and_then(|mut g| g.take())
            .map(|img| (*started, img))
    });
    if let Some((started, image)) = finished {
        let size = [image.width() as usize, image.height() as usize];
        let color = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
        let texture = ctx.load_texture("newera-photo", color, egui::TextureOptions::LINEAR);
        window.result = Some((image, texture, started.elapsed()));
        window.job = None;
    }
    if start {
        window.start(app);
    }
    if save
        && let Some((image, _, _)) = &window.result
        && let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG", &["png"])
            .set_file_name("foto.png")
            .save_file()
    {
        match image.save(&path) {
            Ok(()) => app.set_status(format!("Foto salva em {}", path.display())),
            Err(err) => app.set_status(format!("⚠ Não foi possível salvar: {err}")),
        }
    }
    if open {
        app.photo = Some(window);
    }
}

#[cfg(test)]
mod tests {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;
    use newera_core::{Command, Document, Point2, SharedDocument, Wall};

    use super::*;

    #[test]
    fn renders_a_photo_in_the_background_and_shows_it() {
        let mut doc = Document::default();
        let wall = Wall::new(
            doc.new_wall_id(),
            Point2::new(0.0, 0.0),
            Point2::new(300.0, 0.0),
        );
        doc.execute(Command::insert(wall)).unwrap();
        let document = SharedDocument::new(doc);
        let mut h = Harness::builder()
            .with_size(egui::vec2(1280.0, 800.0))
            .with_step_dt(1.0 / 60.0)
            .build_eframe(move |cc| NewEraApp::new(cc, document, None));
        h.run_steps(3);
        h.state_mut().photo = Some(PhotoWindow {
            size: [64, 48],
            ..PhotoWindow::default()
        });
        h.run_steps(3);
        h.get_by_label_contains("Renderizar").click();
        h.run_steps(2);
        assert!(
            h.state().photo.as_ref().unwrap().job.is_some(),
            "job started"
        );
        for _ in 0..600 {
            h.run_steps(1);
            if h.state().photo.as_ref().unwrap().result.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let window = h.state().photo.as_ref().unwrap();
        let (image, _, _) = window.result.as_ref().expect("photo finished");
        assert_eq!(image.dimensions(), (64, 48));
    }
}
