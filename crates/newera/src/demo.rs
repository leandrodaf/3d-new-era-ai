use newera_core::{Home, Point2, Room, Wall};

/// A small two-room house used by `--demo`.
pub(crate) fn sample_home() -> Home {
    let p = Point2::new;
    let mut home = Home::new("Casa demo");

    let outline = [p(0.0, 0.0), p(800.0, 0.0), p(800.0, 600.0), p(0.0, 600.0)];
    for (a, b) in outline.iter().zip(outline.iter().cycle().skip(1)) {
        let id = home.new_wall_id();
        home.walls.push(Wall::new(id, *a, *b));
    }
    let id = home.new_wall_id();
    home.walls.push(Wall {
        thickness: 10.0,
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
    home
}
