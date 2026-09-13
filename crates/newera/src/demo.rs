use newera_core::{Furniture, Home, Point2, Room, Wall, align_to_wall};

/// A small furnished two-room house used by `--demo`.
pub(crate) fn sample_home() -> Home {
    let p = Point2::new;
    let mut home = Home::new("Casa demo");

    let outline = [p(0.0, 0.0), p(800.0, 0.0), p(800.0, 600.0), p(0.0, 600.0)];
    for (a, b) in outline.iter().zip(outline.iter().cycle().skip(1)) {
        let id = home.new_wall_id();
        home.walls.push(Wall {
            thickness: 20.0,
            ..Wall::new(id, *a, *b)
        });
    }
    let id = home.new_wall_id();
    home.walls.push(Wall {
        thickness: 12.0,
        ..Wall::new(id, p(450.0, 0.0), p(450.0, 600.0))
    });

    for (name, x0, x1) in [("Sala", 0.0, 450.0), ("Quarto", 450.0, 800.0)] {
        let id = home.new_room_id();
        home.rooms.push(Room::new(
            id,
            name,
            vec![p(x0, 0.0), p(x1, 0.0), p(x1, 600.0), p(x0, 600.0)],
        ));
    }

    let put =
        |home: &mut Home, catalog: &str, at: (f64, f64), angle: f64, wall: Option<(usize, f64)>| {
            let item = newera_catalog::find(catalog).expect("demo uses catalog ids");
            let mut piece: Furniture = item.instantiate(home.new_furniture_id(), p(at.0, at.1));
            piece.angle = angle;
            if let Some((index, along)) = wall {
                align_to_wall(&mut piece, &home.walls[index], along);
            }
            home.furniture.push(piece);
        };
    put(&mut home, "door", (0.0, 0.0), 0.0, Some((2, 700.0)));
    put(&mut home, "door", (0.0, 0.0), 0.0, Some((4, 450.0)));
    put(&mut home, "window", (0.0, 0.0), 0.0, Some((0, 220.0)));
    put(&mut home, "window", (0.0, 0.0), 0.0, Some((0, 630.0)));
    put(&mut home, "sofa-3", (252.0, 470.0), 180.0, None);
    put(&mut home, "coffee-table", (252.0, 360.0), 0.0, None);
    put(&mut home, "tv-stand", (252.0, 35.0), 0.0, None);
    put(&mut home, "armchair", (60.0, 330.0), 90.0, None);
    put(&mut home, "plant", (400.0, 540.0), 0.0, None);
    put(&mut home, "rug", (252.0, 400.0), 0.0, None);
    put(&mut home, "bed-double", (625.0, 124.0), 0.0, None);
    put(&mut home, "nightstand", (515.0, 30.0), 0.0, None);
    put(&mut home, "nightstand", (735.0, 30.0), 0.0, None);
    put(&mut home, "wardrobe", (625.0, 555.0), 180.0, None);
    home
}
