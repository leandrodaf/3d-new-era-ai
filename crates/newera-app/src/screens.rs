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
