//! Resource ownership and progress shared by photo and video jobs.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use web_time::Instant;

pub(crate) type ByteOutcome = Arc<Mutex<Option<Result<Vec<u8>, String>>>>;

thread_local! {
    static BUSY: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
}

pub(crate) struct Permit(Arc<AtomicBool>);
impl Permit {
    pub(crate) fn acquire() -> Result<Self, String> {
        BUSY.with(|busy| {
            busy.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .map(|_| Self(busy.clone()))
                .map_err(|_| "Uma renderização já está em andamento. Aguarde ou cancele.".into())
        })
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

pub(crate) struct Report {
    state: Mutex<(String, u64, u64, Instant)>,
    pub(crate) cancelled: AtomicBool,
}
impl Default for Report {
    fn default() -> Self {
        Self {
            state: Mutex::new(("Preparando cena".into(), 0, 0, Instant::now())),
            cancelled: AtomicBool::new(false),
        }
    }
}
impl newera_core::progress::Watcher for Report {
    fn step(&self, what: &str, done: u64, total: u64) {
        if let Ok(mut state) = self.state.lock() {
            if state.0 == what {
                state.1 = state.1.max(done);
                state.2 = total;
            } else {
                *state = (what.into(), done, total, Instant::now());
            }
        }
    }
    fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}
impl Report {
    #[allow(clippy::cast_precision_loss)]
    pub(crate) fn ui(&self, ui: &mut egui::Ui) -> bool {
        let (what, done, total, started) = self.state.lock().expect("render progress").clone();
        let elapsed = started.elapsed().as_secs_f64();
        let text = if done > 0 && total > done && elapsed >= 1.0 {
            format!(
                "{done}/{total} · ~{:.0} s restantes",
                elapsed / done as f64 * (total - done) as f64
            )
        } else if total > 0 && done < total {
            format!("{done}/{total} · estimando tempo…")
        } else {
            what.clone()
        };
        ui.add(
            egui::ProgressBar::new(if total == 0 {
                0.0
            } else {
                (done as f32 / total as f32).min(0.99)
            })
            .animate(true)
            .text(text)
            .desired_width(280.0),
        )
        .on_hover_text(what);
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(150));
        ui.button(crate::i18n::tr("Cancelar")).clicked()
    }
}
use eframe::egui;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_one_render_runs_and_cancellation_releases_its_budget() {
        let permit = Permit::acquire().unwrap();
        assert!(Permit::acquire().is_err());
        drop(permit);
        assert!(Permit::acquire().is_ok());
    }
}
