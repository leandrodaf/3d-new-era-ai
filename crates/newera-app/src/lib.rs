//! Desktop editor for 3D New Era AI.
//!
//! Layout follows classic home design tools: catalog and home contents on the
//! left, the 2D floor plan on top and the native 3D view below it.

mod app;
mod dialogs;
mod panels;
mod photo;
#[cfg(test)]
mod screens;
mod tabs;
mod theme;
mod view;

use std::path::PathBuf;

use newera_core::SharedDocument;

/// Options for [`run`].
#[derive(Debug, Clone, Default)]
pub struct AppOptions {
    /// MCP endpoint shown in the status bar, if the server is running.
    pub mcp_url: Option<String>,
    /// Project to open at startup.
    pub open: Option<PathBuf>,
}

/// Opens the editor window and blocks until it is closed.
pub fn run(document: SharedDocument, options: AppOptions) -> eframe::Result {
    let native = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("3D New Era AI")
            .with_inner_size([1440.0, 920.0])
            .with_min_inner_size([960.0, 620.0]),
        ..Default::default()
    };
    eframe::run_native(
        "3D New Era AI",
        native,
        Box::new(move |cc| {
            let mut app = app::NewEraApp::new(cc, document, options.mcp_url);
            if let Some(path) = options.open {
                app.open_path(&path);
            }
            Ok(Box::new(app))
        }),
    )
}
