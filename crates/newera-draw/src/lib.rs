//! Floor plan drawing shared by every output of 3D New Era AI: the editor
//! canvas, PNG renders for AI agents, and SVG export.

mod pdf;
mod raster;
mod scene;
mod svg;

pub use pdf::{PdfOptions, to_pdf};
pub use raster::{RenderError, RenderOptions, Viewport, render_pixmap, render_png};
pub use scene::{
    Align, Color, EM_TO_HEIGHT, Item, Palette, PieceImageFn, PieceImages, Primitive, Scene,
    SceneOptions, Size, TextLook, compass_items, dimension_items, discipline_color,
    furniture_items, plan_scene,
};
pub use svg::{SvgOptions, to_svg};
