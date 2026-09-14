//! Headless screenshots of the real UI for visual review:
//! `cargo test -p newera-app screens -- --ignored` writes `target/screenshots/*.png`.

use egui_kittest::Harness;
use newera_core::{Command, Document, Point2, SharedDocument, Wall};

use crate::app::NewEraApp;
use crate::dialogs::Dialog;

fn house(doc: &mut Document, width: f64) {
    let pts = [(0.0, 0.0), (width, 0.0), (width, 500.0), (0.0, 500.0)];
    let commands = (0..4)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % 4]);
            Command::insert(Wall::new(
                doc.new_wall_id(),
                Point2::new(a.0, a.1),
                Point2::new(b.0, b.1),
            ))
        })
        .collect();
    doc.execute(Command::Batch { commands }).unwrap();
    let room =
        newera_core::detect_room(&doc.home().walls, Point2::new(width / 2.0, 250.0)).unwrap();
    let room = newera_core::Room::new(doc.new_room_id(), "Sala", room);
    doc.execute(Command::insert(room)).unwrap();
}

pub(crate) fn render(name: &str, document: SharedDocument, setup: impl FnOnce(&mut NewEraApp)) {
    let mut harness = Harness::builder()
        .with_size(eframe::egui::vec2(1400.0, 860.0))
        .with_step_dt(1.0 / 60.0)
        .wgpu()
        .build_eframe(move |cc| NewEraApp::new(cc, document, None));
    harness.run_steps(5);
    setup(harness.state_mut());
    harness.run_steps(12);
    let image = harness.render().expect("render");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/screenshots");
    std::fs::create_dir_all(&dir).unwrap();
    image.save(dir.join(format!("{name}.png"))).unwrap();
}

#[test]
#[ignore = "visual review; needs a GPU"]
fn variants() {
    let mut doc = Document::default();
    house(&mut doc, 600.0);
    doc.add_variant(Some("Sala ampliada".into()), false);
    house(&mut doc, 800.0);
    doc.add_variant(Some("Opção compacta".into()), false);
    house(&mut doc, 450.0);
    render("variants-tabs", SharedDocument::new(doc), |_| {});

    let mut doc = Document::default();
    house(&mut doc, 600.0);
    doc.add_variant(Some("Sala ampliada".into()), false);
    house(&mut doc, 800.0);
    render(
        "variants-compare",
        SharedDocument::new(doc),
        NewEraApp::open_compare,
    );
    let _ = Dialog::Help;
}

#[test]
#[ignore = "visual review; needs a GPU"]
fn levels() {
    let mut doc = Document::default();
    house(&mut doc, 700.0);
    newera_core::ops::add_level(&mut doc, Some("Superior".into()), None).unwrap();
    house(&mut doc, 450.0);
    let stairs = newera_catalog::find("stairs")
        .expect("stairs in catalog")
        .instantiate(doc.new_furniture_id(), Point2::new(300.0, 250.0));
    let ground = doc.home().base_level();
    let mut stairs = stairs;
    stairs.level = ground;
    doc.execute(Command::insert(stairs)).unwrap();
    render("levels", SharedDocument::new(doc), |_| {});
}

#[test]
#[ignore = "visual review; needs a GPU"]
fn materials() {
    use newera_core::{Element, Material};
    let mut doc = Document::default();
    house(&mut doc, 700.0);
    let mut home = doc.home().clone();
    let finishes = ["brick", "#9fb8c8", "subway 20x10", "stone"];
    for (wall, finish) in home.walls.iter_mut().zip(finishes) {
        wall.right_side = Some(finish.parse().unwrap());
        wall.left_side = Some(finish.parse().unwrap());
        wall.apply_type(newera_core::wall_type("tijolo-14").unwrap());
    }
    home.rooms[0].floor_material = Some("wood".parse::<Material>().unwrap());
    let commands = home
        .walls
        .iter()
        .cloned()
        .map(Element::Wall)
        .chain(home.rooms.iter().cloned().map(Element::Room))
        .map(|element| Command::Update { element })
        .collect();
    doc.execute(Command::Batch { commands }).unwrap();
    let shared = SharedDocument::new(doc);
    render("materials", shared.clone(), |_| {});
    render("materials-wall-dialog", shared.clone(), |app| {
        app.open_modify(&[newera_core::WallId(1).into()]);
    });
    render("materials-room-dialog", shared, |app| {
        app.open_modify(&[newera_core::RoomId(5).into()]);
    });
}

/// The real Sweet Home 3D project given by `NEWERA_SH3D_SAMPLE`.
#[test]
#[ignore = "visual review; needs a GPU and NEWERA_SH3D_SAMPLE"]
fn sh3d_import() {
    let path = std::env::var("NEWERA_SH3D_SAMPLE").expect("NEWERA_SH3D_SAMPLE");
    let mut doc = Document::default();
    let opened = newera_sh3d::open_file(&mut doc, std::path::Path::new(&path)).unwrap();
    assert!(opened.warnings.is_empty(), "{:?}", opened.warnings);
    let shared = SharedDocument::new(doc);
    render("sh3d-import", shared.clone(), |_| {});
    let levels: Vec<_> = shared
        .read()
        .home()
        .sorted_levels()
        .iter()
        .map(|l| l.id)
        .collect();
    for (i, level) in levels.into_iter().enumerate() {
        shared.write().select_level(Some(level));
        render(&format!("sh3d-import-level{i}"), shared.clone(), |_| {});
    }
}
