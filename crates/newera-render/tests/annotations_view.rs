//! Labels and dimensions shown in 3D: `target/screenshots/soft-annotations.png`.

use newera_core::{Dimension, DimensionId, Home, Label, LabelId, Point2, Wall};
use newera_render::{View, render_home};

#[test]
#[ignore = "visual review"]
fn render_labels_and_dimensions_in_3d() {
    let mut home = Home::default();
    for (a, b) in [((0.0, 0.0), (500.0, 0.0)), ((500.0, 0.0), (500.0, 400.0))] {
        let id = home.new_wall_id();
        let mut wall = Wall::new(id, Point2::new(a.0, a.1), Point2::new(b.0, b.1));
        wall.height = 250.0;
        home.walls.push(wall);
    }
    home.labels.push(Label {
        id: LabelId(900),
        text: "Cozinha gourmet".into(),
        position: Point2::new(250.0, 20.0),
        size: 36.0,
        pitch: Some(90.0),
        elevation: 160.0,
        color: Some([160, 30, 30]),
        ..Label::default()
    });
    home.labels.push(Label {
        id: LabelId(901),
        text: "Área 12 m²\nPiso vinílico".into(),
        position: Point2::new(250.0, 250.0),
        size: 30.0,
        pitch: Some(0.0),
        ..Label::default()
    });
    home.dimensions.push(Dimension {
        id: DimensionId(902),
        start: Point2::new(0.0, 0.0),
        end: Point2::new(500.0, 0.0),
        offset: 40.0,
        elevation: [250.0, 250.0],
        pitch: 90.0,
        visible_in_3d: true,
        ..Dimension::default()
    });
    let image = render_home(&home, &View::aerial(&home, 20.0, 25.0), 900, 600, None);
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/screenshots");
    std::fs::create_dir_all(&out).unwrap();
    image.save(out.join("soft-annotations.png")).unwrap();
}
