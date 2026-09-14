//! Browser entry point of the editor: `make web-editor` builds it into
//! `web/editor/`, where `index.html` calls [`start`].

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

/// Starts the editor in the canvas with id `canvas_id`, opening `bytes`
/// (a `.newera` project named `name`) when given.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn start(
    canvas_id: String,
    name: Option<String>,
    bytes: Option<Vec<u8>>,
) -> Result<(), JsValue> {
    use wasm_bindgen::JsCast;
    console_error_panic_hook::set_once();
    let canvas = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(&canvas_id))
        .ok_or_else(|| JsValue::from_str("canvas not found"))?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;
    let project = bytes.map(|b| (name.unwrap_or_else(|| "projeto.newera".into()), b));
    newera_app::start_web(canvas, project).await
}
