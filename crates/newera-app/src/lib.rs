//! Desktop editor for 3D New Era AI.
//!
//! Layout follows Sweet Home 3D: catalog and home tree on the left, the 2D
//! floor plan on top and the native 3D view below it.

mod app;
mod view;

use newera_core::SharedDocument;

/// Options for [`run`].
#[derive(Debug, Clone, Default)]
pub struct AppOptions {
    /// MCP endpoint shown in the status bar, if the server is running.
    pub mcp_url: Option<String>,
}

/// Opens the editor window and blocks until it is closed.
pub fn run(document: SharedDocument, options: AppOptions) -> eframe::Result {
    let native = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("3D New Era AI")
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "3D New Era AI",
        native,
        Box::new(move |_cc| Ok(Box::new(app::NewEraApp::new(document, options.mcp_url)))),
    )
}
