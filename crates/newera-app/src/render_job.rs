//! Resource ownership and progress shared by photo and video jobs.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use web_time::Instant;

pub(crate) type ByteOutcome = Arc<Mutex<Option<Result<Vec<u8>, String>>>>;

/// Conservative workload caps, not an estimate of free system memory.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct BrowserBudget {
    pub pixels: u64,
    pub scene_bytes: usize,
    pub asset_bytes: usize,
}

#[cfg(any(target_arch = "wasm32", test))]
impl BrowserBudget {
    pub(crate) fn for_device(memory_gb: Option<f64>, cores: Option<f64>) -> Self {
        let valid = |v: Option<f64>| v.filter(|v| v.is_finite() && *v > 0.0);
        let (memory_gb, cores) = (valid(memory_gb), valid(cores));
        let (pixels, mib) =
            if memory_gb.is_some_and(|v| v <= 2.0) || cores.is_some_and(|v| v <= 2.0) {
                (640 * 480, 8)
            } else if memory_gb.is_some_and(|v| v >= 8.0) && cores.is_some_and(|v| v >= 8.0) {
                (1280 * 960, 32)
            } else {
                // Missing/rounded capability hints never grant the largest job.
                (960 * 720, 16)
            };
        Self {
            pixels,
            scene_bytes: mib * 1024 * 1024 / 2,
            asset_bytes: mib * 1024 * 1024,
        }
    }

    pub(crate) fn restricted_by(self, other: Self) -> Self {
        Self {
            pixels: self.pixels.min(other.pixels),
            scene_bytes: self.scene_bytes.min(other.scene_bytes),
            asset_bytes: self.asset_bytes.min(other.asset_bytes),
        }
    }
}

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
    fn browser_budget_uses_the_limiting_hint_and_cannot_be_expanded_by_a_request() {
        let small = BrowserBudget::for_device(Some(2.0), Some(32.0));
        assert_eq!(small.pixels, 640 * 480);
        assert_eq!(
            BrowserBudget::for_device(Some(32.0), Some(2.0)).pixels,
            small.pixels
        );
        let unknown = BrowserBudget::for_device(None, None);
        assert_eq!(unknown.pixels, 960 * 720);
        assert_eq!(
            BrowserBudget::for_device(Some(f64::NAN), Some(-1.0)).pixels,
            unknown.pixels
        );
        let large = BrowserBudget::for_device(Some(8.0), Some(8.0));
        assert_eq!(large.pixels, 1280 * 960);
        let bounded = small.restricted_by(large);
        assert_eq!(bounded.pixels, small.pixels);
        assert_eq!(bounded.asset_bytes, 8 * 1024 * 1024);
        assert_eq!(bounded.scene_bytes, 4 * 1024 * 1024);
        assert_eq!(large.restricted_by(small).pixels, small.pixels);
    }
    #[test]
    fn only_one_render_runs_and_cancellation_releases_its_budget() {
        let permit = Permit::acquire().unwrap();
        assert!(Permit::acquire().is_err());
        drop(permit);
        assert!(Permit::acquire().is_ok());
    }
}
