//! Renders each level of the Sweet Home 3D project given by
//! `NEWERA_SH3D_SAMPLE` to `target/screenshots/plan-level*.png` at 1 px/cm,
//! for side-by-side comparison with a reference render.

use newera_core::{Document, Point2};
use newera_draw::{RenderOptions, SceneOptions, plan_scene, render_png};

#[test]
#[ignore = "needs NEWERA_SH3D_SAMPLE"]
fn render_sh3d_levels() {
    let path = std::env::var("NEWERA_SH3D_SAMPLE").expect("NEWERA_SH3D_SAMPLE");
    let mut doc = Document::default();
    newera_sh3d::open_file(&mut doc, std::path::Path::new(&path)).unwrap();
    let assets = doc.asset_dir();
    let home = doc.home().clone();
    let (min, max) = home.bounds().unwrap();
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/screenshots");
    std::fs::create_dir_all(&out).unwrap();
    for (i, level) in home.sorted_levels().iter().enumerate() {
        let mut view = home.level_view(Some(level.id));
        view.selected_level = Some(level.id);
        let scene = plan_scene(
            &view,
            &SceneOptions {
                show_background: true,
                ..SceneOptions::default()
            },
        );
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let options = RenderOptions {
            width: (max.x - min.x + 80.0) as u32,
            height: (max.y - min.y + 80.0) as u32,
            margin_px: 40.0,
            grid: false,
            region: Some((Point2::new(min.x, min.y), Point2::new(max.x, max.y))),
            ..RenderOptions::default()
        };
        let load = |p: &str| {
            image::open(newera_core::resolve_asset(assets.as_deref(), p))
                .ok()
                .map(|i| i.to_rgba8())
        };
        let png = render_png(&scene, &options, &load).unwrap();
        std::fs::write(out.join(format!("plan-level{i}.png")), png).unwrap();
    }
}
