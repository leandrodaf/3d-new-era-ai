//! A plan with electrical and plumbing symbols and the legend on:
//! `target/screenshots/legend.png`.

use newera_core::{Discipline, Home, Point2, Wall};
use newera_draw::{RenderOptions, SceneOptions, plan_scene, render_png};

#[test]
fn legend_lists_symbols_with_counts() {
    let mut home = Home::default();
    let pts = [(0.0, 0.0), (500.0, 0.0), (500.0, 400.0), (0.0, 400.0)];
    for i in 0..4 {
        let (a, b) = (pts[i], pts[(i + 1) % 4]);
        let id = home.new_wall_id();
        home.walls
            .push(Wall::new(id, Point2::new(a.0, a.1), Point2::new(b.0, b.1)));
    }
    for (cat, x, y) in [
        ("outlet-low", 100.0, 20.0),
        ("outlet-low", 300.0, 20.0),
        ("switch", 30.0, 200.0),
        ("light-ceiling", 250.0, 200.0),
        ("cold-water", 450.0, 60.0),
        ("sewer", 450.0, 120.0),
    ] {
        let id = home.new_furniture_id();
        let piece = newera_catalog::find(cat)
            .unwrap()
            .instantiate(id, Point2::new(x, y));
        home.furniture.push(piece);
    }
    home.annotations.legend = true;
    let scene = plan_scene(&home, &SceneOptions::default());
    let texts: Vec<String> = scene
        .items
        .iter()
        .filter_map(|i| match &i.primitive {
            newera_draw::Primitive::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert!(texts.iter().any(|t| t == "LEGENDA"), "{texts:?}");
    assert!(
        texts
            .iter()
            .any(|t| t == &Discipline::Electrical.name().to_uppercase())
    );
    assert!(
        texts
            .iter()
            .any(|t| t.starts_with("Tomada baixa") && t.ends_with("×2")),
        "{texts:?}"
    );
    let png = render_png(
        &scene,
        &RenderOptions {
            width: 900,
            height: 1100,
            grid: false,
            ..RenderOptions::default()
        },
        &|_| None,
    )
    .unwrap();
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/screenshots");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("legend.png"), png).unwrap();
}
