//! "Criar vídeo": edits the camera path of the home (keyframes taken from the
//! visitor or an aerial orbit) and renders it to a Motion-JPEG AVI in a
//! background thread.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui::{self, RichText};
use egui_phosphor::regular as icon;
use newera_core::Command;

use crate::app::NewEraApp;
use crate::i18n::tr;

type Outcome = Result<newera_render::video::VideoInfo, String>;

/// A running video render.
struct Job {
    started: Instant,
    done: Arc<AtomicUsize>,
    total: Arc<AtomicUsize>,
    outcome: Arc<Mutex<Option<Outcome>>>,
    file: PathBuf,
}

/// Settings and progress of the video window.
pub(crate) struct VideoWindow {
    pub(crate) size: [u32; 2],
    job: Option<Job>,
    pub(crate) last: Option<(PathBuf, Outcome)>,
}

impl std::fmt::Debug for VideoWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoWindow")
            .field("size", &self.size)
            .field("running", &self.job.is_some())
            .finish_non_exhaustive()
    }
}

impl Default for VideoWindow {
    fn default() -> Self {
        Self {
            size: [640, 360],
            job: None,
            last: None,
        }
    }
}

impl VideoWindow {
    pub(crate) fn start(&mut self, app: &NewEraApp, file: PathBuf) {
        let (home, assets) = {
            let doc = app.document.read();
            (doc.home().clone(), doc.asset_dir())
        };
        let done = Arc::new(AtomicUsize::new(0));
        let total = Arc::new(AtomicUsize::new(0));
        let outcome = Arc::new(Mutex::new(None));
        let (d, t, o, f) = (
            Arc::clone(&done),
            Arc::clone(&total),
            Arc::clone(&outcome),
            file.clone(),
        );
        let size = (self.size[0], self.size[1]);
        std::thread::spawn(move || {
            let video = &home.environment.video;
            let result = newera_render::video::render_video(
                &home,
                &home.environment.camera_path,
                video.frame_rate,
                video.speed,
                size,
                assets.as_deref(),
                &f,
                |i, n| {
                    t.store(n, Ordering::Relaxed);
                    d.store(i, Ordering::Relaxed);
                },
            );
            if let Ok(mut guard) = o.lock() {
                *guard = Some(result);
            }
        });
        self.job = Some(Job {
            started: Instant::now(),
            done,
            total,
            outcome,
            file,
        });
        self.last = None;
    }

    pub(crate) fn running(&self) -> bool {
        self.job.is_some()
    }
}

fn set_environment(app: &NewEraApp, edit: impl FnOnce(&mut newera_core::Environment)) {
    let mut doc = app.document.write();
    let mut environment = doc.home().environment.clone();
    edit(&mut environment);
    if environment != doc.home().environment {
        let _ = doc.execute(Command::SetEnvironment { environment });
    }
}

/// Shows the video window while it is open.
#[allow(clippy::too_many_lines)]
pub(crate) fn show(app: &mut NewEraApp, ctx: &egui::Context) {
    let Some(mut window) = app.video.take() else {
        return;
    };
    let mut open = true;
    let mut render_to = None;
    let environment = app.document.read().home().environment.clone();
    let (mut fps, mut speed) = (environment.video.frame_rate, environment.video.speed);
    let keys = environment.camera_path.len();
    let seconds: f64 = newera_render::video::segment_durations(&environment.camera_path, speed)
        .iter()
        .sum();
    egui::Window::new(format!("{} {}", icon::FILM_STRIP, tr("Criar vídeo")))
        .open(&mut open)
        .default_width(420.0)
        .show(ctx, |ui| {
            ui.label(format!(
                "{}: {keys} · {seconds:.1} s",
                tr("Pontos do caminho")
            ));
            ui.horizontal_wrapped(|ui| {
                let visitor = app.scene.visitor.as_ref().map(|v| v.camera.clone());
                if ui
                    .add_enabled(
                        visitor.is_some(),
                        egui::Button::new(format!(
                            "{} {}",
                            icon::PLUS,
                            tr("Adicionar ponto atual")
                        )),
                    )
                    .on_disabled_hover_text(tr("Ative o visitante na vista 3D"))
                    .clicked()
                    && let Some(camera) = visitor
                {
                    set_environment(app, |e| e.camera_path.push(camera));
                }
                if ui
                    .button(format!("{} {}", icon::ARROWS_CLOCKWISE, tr("Órbita aérea")))
                    .clicked()
                {
                    let orbit =
                        newera_render::video::orbit_path(app.document.read().home(), 800.0, 8);
                    set_environment(app, |e| e.camera_path = orbit);
                }
                if ui
                    .add_enabled(
                        keys > 0,
                        egui::Button::new(format!("{} {}", icon::TRASH, tr("Limpar"))),
                    )
                    .clicked()
                {
                    set_environment(app, |e| e.camera_path.clear());
                }
            });
            ui.horizontal(|ui| {
                ui.label(tr("Quadros por segundo"));
                ui.add(egui::Slider::new(&mut fps, 5..=60));
            });
            ui.horizontal(|ui| {
                ui.label(tr("Velocidade"));
                ui.add(
                    egui::Slider::new(&mut speed, 0.2..=20.0)
                        .logarithmic(true)
                        .suffix(" m/s"),
                );
            });
            ui.horizontal(|ui| {
                ui.label(tr("Tamanho"));
                for size in [[320, 240], [640, 360], [1280, 720], [1920, 1080]] {
                    ui.selectable_value(&mut window.size, size, format!("{}×{}", size[0], size[1]));
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        keys >= 2 && !window.running(),
                        egui::Button::new(format!("{} {}", icon::FILM_SLATE, tr("Gerar vídeo…"))),
                    )
                    .clicked()
                {
                    render_to = rfd::FileDialog::new()
                        .add_filter("AVI", &["avi"])
                        .set_file_name("video.avi")
                        .save_file();
                }
                if let Some(job) = &window.job {
                    let total = job.total.load(Ordering::Relaxed).max(1);
                    let done = job.done.load(Ordering::Relaxed);
                    #[allow(clippy::cast_precision_loss)]
                    ui.add(
                        egui::ProgressBar::new(done as f32 / total as f32)
                            .text(format!("{done}/{total}"))
                            .desired_width(160.0),
                    );
                    ctx.request_repaint_after(Duration::from_millis(200));
                }
            });
            match &window.last {
                Some((file, Ok(info))) => {
                    ui.label(
                        RichText::new(format!(
                            "{} {} · {} {} · {:.1} s",
                            tr("Vídeo salvo em"),
                            file.display(),
                            info.frames,
                            tr("quadros"),
                            info.seconds
                        ))
                        .weak(),
                    );
                }
                Some((_, Err(err))) => {
                    ui.colored_label(ui.visuals().warn_fg_color, format!("⚠ {err}"));
                }
                None => {}
            }
        });

    if fps != environment.video.frame_rate || (speed - environment.video.speed).abs() > f64::EPSILON
    {
        set_environment(app, |e| {
            e.video.frame_rate = fps;
            e.video.speed = speed;
        });
    }
    let finished = window.job.as_ref().and_then(|job| {
        job.outcome
            .lock()
            .ok()
            .and_then(|mut g| g.take())
            .map(|outcome| (job.file.clone(), job.started, outcome))
    });
    if let Some((file, started, outcome)) = finished {
        if outcome.is_ok() {
            app.set_status(format!(
                "{} {} ({:.0} s)",
                tr("Vídeo salvo em"),
                file.display(),
                started.elapsed().as_secs_f32()
            ));
        }
        window.last = Some((file, outcome));
        window.job = None;
    }
    if let Some(file) = render_to {
        window.start(app, file);
    }
    if open {
        app.video = Some(window);
    }
}

#[cfg(test)]
mod tests {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;
    use newera_core::{Document, Point2, SharedDocument, Wall};

    use super::*;

    #[test]
    fn orbit_button_fills_the_path_and_the_video_renders() {
        let mut doc = Document::default();
        let wall = Wall::new(
            doc.new_wall_id(),
            Point2::new(0.0, 0.0),
            Point2::new(300.0, 0.0),
        );
        doc.execute(Command::insert(wall)).unwrap();
        let document = SharedDocument::new(doc);
        let shared = document.clone();
        let mut h = Harness::builder()
            .with_size(egui::vec2(1280.0, 800.0))
            .with_step_dt(1.0 / 60.0)
            .build_eframe(move |cc| NewEraApp::new(cc, document, None));
        h.run_steps(3);
        h.state_mut().video = Some(VideoWindow {
            size: [64, 48],
            ..VideoWindow::default()
        });
        h.run_steps(3);
        h.get_by_label_contains("Órbita aérea").click();
        h.run_steps(2);
        assert_eq!(shared.read().home().environment.camera_path.len(), 9);
        // Fast tour so the test renders few frames.
        set_environment(h.state(), |e| {
            e.video.speed = 50.0;
            e.video.frame_rate = 2;
        });
        let dir = std::env::temp_dir().join(format!("newera-app-video-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("tour.avi");
        let mut window = h.state_mut().video.take().unwrap();
        window.start(h.state(), file.clone());
        h.state_mut().video = Some(window);
        for _ in 0..1500 {
            h.run_steps(1);
            if h.state().video.as_ref().unwrap().last.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let (_, outcome) = h
            .state()
            .video
            .as_ref()
            .unwrap()
            .last
            .as_ref()
            .expect("video finished");
        assert!(outcome.as_ref().unwrap().frames >= 2);
        assert!(std::fs::read(&file).unwrap().starts_with(b"RIFF"));
        std::fs::remove_dir_all(dir).ok();
    }
}
