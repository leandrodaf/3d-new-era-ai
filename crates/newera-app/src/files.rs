//! Files on every platform: native dialogs and the disk on the desktop; the
//! browser's file picker and downloads on the web, where picked files arrive
//! a few frames later through [`take_picked`].

/// What a picked file is for.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PickKind {
    Project,
    /// A scanned plan to trace over.
    Background,
}

/// A file chosen in the browser.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[derive(Debug)]
pub(crate) struct Picked {
    pub(crate) kind: PickKind,
    pub(crate) name: String,
    pub(crate) bytes: Vec<u8>,
}

/// Asks where to save `name`. The bytes come later, from work that can take
/// its time: the dialog belongs to the window's thread, the writing does not.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn pick_save(filter: &str, ext: &str, name: &str) -> Option<std::path::PathBuf> {
    rfd::FileDialog::new()
        .add_filter(filter, &[ext])
        .set_file_name(name)
        .save_file()
}

/// Asks where to save `name` (desktop) or downloads it (web), then writes the
/// bytes made by `make`. Returns where it went, `None` when cancelled.
///
/// # Errors
/// When `make` fails or the file can't be written.
pub(crate) fn save_bytes(
    filter: &str,
    ext: &str,
    name: &str,
    make: impl FnOnce() -> Result<Vec<u8>, String>,
) -> Result<Option<String>, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(path) = rfd::FileDialog::new()
            .add_filter(filter, &[ext])
            .set_file_name(name)
            .save_file()
        else {
            return Ok(None);
        };
        let bytes = make()?;
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
        Ok(Some(path.display().to_string()))
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (filter, ext);
        web::download(name, &make()?)?;
        Ok(Some(name.to_owned()))
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) use web::{pick, take_picked};

#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::RefCell;

    use wasm_bindgen::JsCast;

    use super::{PickKind, Picked};

    thread_local! {
        static PICKED: RefCell<Vec<Picked>> = const { RefCell::new(Vec::new()) };
    }

    /// Opens the browser's file picker; the file shows up in [`take_picked`].
    pub(crate) fn pick(kind: PickKind, ctx: &eframe::egui::Context) {
        let dialog = match kind {
            PickKind::Project => rfd::AsyncFileDialog::new().add_filter(
                "3D New Era AI / Sweet Home 3D",
                &[newera_core::PROJECT_EXTENSION, "sh3d"],
            ),
            PickKind::Background => rfd::AsyncFileDialog::new().add_filter(
                "PNG, JPEG, WebP, BMP",
                &["png", "jpg", "jpeg", "webp", "bmp"],
            ),
        };
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(file) = dialog.pick_file().await {
                let picked = Picked {
                    kind,
                    name: file.file_name(),
                    bytes: file.read().await,
                };
                PICKED.with(|p| p.borrow_mut().push(picked));
                ctx.request_repaint();
            }
        });
    }

    /// Files picked since the last call.
    pub(crate) fn take_picked() -> Vec<Picked> {
        PICKED.with(|p| std::mem::take(&mut *p.borrow_mut()))
    }

    /// Hands `bytes` to the browser as a download named `name`.
    pub(crate) fn download(name: &str, bytes: &[u8]) -> Result<(), String> {
        let fail = |e: wasm_bindgen::JsValue| format!("{e:?}");
        let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes));
        let blob = web_sys::Blob::new_with_u8_array_sequence(&parts).map_err(fail)?;
        let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(fail)?;
        let document = web_sys::window()
            .and_then(|w| w.document())
            .ok_or("no document")?;
        let anchor: web_sys::HtmlAnchorElement = document
            .create_element("a")
            .map_err(fail)?
            .dyn_into()
            .map_err(|_| "not an anchor")?;
        anchor.set_href(&url);
        anchor.set_download(name);
        anchor.click();
        web_sys::Url::revoke_object_url(&url).map_err(fail)
    }
}
