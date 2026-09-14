//! New structure and outdoor catalog pieces: `target/screenshots/soft-catalog-*.png`.

use newera_core::{Home, Material, Pattern, Point2, Room, RoomId};
use newera_render::{View, render_home};

#[test]
#[ignore = "visual review"]
fn render_structure_and_outdoor_pieces() {
    let mut home = Home::default();
    let mut deck = Room::new(
        RoomId(1),
        "Deck",
        vec![
            Point2::new(-50.0, -50.0),
            Point2::new(1250.0, -50.0),
            Point2::new(1250.0, 700.0),
            Point2::new(-50.0, 700.0),
        ],
    );
    deck.floor_material = Some(Material::pattern(Pattern::Deck));
    home.rooms.push(deck);
    let items = [
        ("footing", 0.0, 0.0),
        ("railing", 200.0, 0.0),
        ("glass-railing", 550.0, 0.0),
        ("fence", 900.0, 0.0),
        ("roof-sheet", 0.0, 250.0),
        ("pool", 400.0, 300.0),
        ("lounger", 850.0, 300.0),
        ("grill", 1050.0, 300.0),
        ("sofa-l", 100.0, 550.0),
        ("dining-set-6", 500.0, 580.0),
        ("bench", 800.0, 600.0),
        ("planter", 1000.0, 600.0),
        ("shower-glass", 1150.0, 600.0),
    ];
    for (i, (cat, x, y)) in items.iter().enumerate() {
        let id = newera_core::FurnitureId(100 + i as u64);
        let piece = newera_catalog::find(cat)
            .expect(cat)
            .instantiate(id, Point2::new(*x, *y));
        home.furniture.push(piece);
    }
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/screenshots");
    std::fs::create_dir_all(&out).unwrap();
    render_home(&home, &View::aerial(&home, 60.0, 35.0), 1000, 700, None)
        .save(out.join("soft-catalog-3d.png"))
        .unwrap();
    let scene = newera_draw::plan_scene(&home, &newera_draw::SceneOptions::default());
    let png = newera_draw::render_png(
        &scene,
        &newera_draw::RenderOptions {
            width: 1000,
            height: 700,
            ..Default::default()
        },
        &|_| None,
    )
    .unwrap();
    std::fs::write(out.join("soft-catalog-plan.png"), png).unwrap();
}
