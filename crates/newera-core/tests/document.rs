use newera_core::{Command, CoreError, Document, Home, Point2, Room, RoomId, Wall, WallId};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn wall(x1: f64, y1: f64, x2: f64, y2: f64) -> Wall {
    let id = WallId(NEXT_ID.fetch_add(1, Ordering::Relaxed));
    Wall::new(id, Point2::new(x1, y1), Point2::new(x2, y2))
}

#[test]
fn undo_and_redo_restore_exact_state() {
    let mut doc = Document::default();
    let a = wall(0.0, 0.0, 400.0, 0.0);
    let b = wall(400.0, 0.0, 400.0, 300.0);

    doc.execute(Command::add_wall(a.clone())).unwrap();
    doc.execute(Command::add_wall(b.clone())).unwrap();
    doc.execute(Command::RemoveWall { id: a.id }).unwrap();
    assert_eq!(doc.home().walls, vec![b.clone()]);

    doc.undo().unwrap();
    assert_eq!(
        doc.home().walls,
        vec![a.clone(), b.clone()],
        "order restored"
    );

    doc.redo().unwrap();
    assert_eq!(doc.home().walls, vec![b]);
}

#[test]
fn new_command_clears_redo() {
    let mut doc = Document::default();
    doc.execute(Command::add_wall(wall(0.0, 0.0, 100.0, 0.0)))
        .unwrap();
    doc.undo().unwrap();
    assert!(doc.can_redo());

    doc.execute(Command::RenameHome {
        name: "Casa da praia".into(),
    })
    .unwrap();
    assert!(!doc.can_redo());
    assert_eq!(doc.redo(), Err(CoreError::NothingToRedo));
}

#[test]
fn failed_batch_leaves_home_untouched() {
    let mut doc = Document::default();
    let before = doc.home().clone();
    let revision = doc.revision();

    let result = doc.execute(Command::Batch {
        commands: vec![
            Command::add_wall(wall(0.0, 0.0, 100.0, 0.0)),
            Command::add_wall(wall(0.0, 0.0, 0.0, 0.0)), // zero length: invalid
        ],
    });

    assert!(matches!(result, Err(CoreError::InvalidGeometry(_))));
    assert_eq!(doc.home(), &before);
    assert_eq!(doc.revision(), revision);
    assert!(!doc.can_undo());
}

#[test]
fn batch_undoes_as_a_single_step() {
    let mut doc = Document::default();
    let room = Room {
        id: RoomId(9_000),
        name: "Sala".into(),
        points: vec![
            Point2::new(0.0, 0.0),
            Point2::new(400.0, 0.0),
            Point2::new(400.0, 300.0),
        ],
    };
    doc.execute(Command::Batch {
        commands: vec![
            Command::add_wall(wall(0.0, 0.0, 400.0, 0.0)),
            Command::add_room(room),
        ],
    })
    .unwrap();

    doc.undo().unwrap();
    assert_eq!(doc.home(), &Home::default());
}

#[test]
fn commands_round_trip_through_json() {
    let command = Command::add_wall(wall(0.0, 0.0, 250.0, 0.0));
    let json = serde_json::to_string(&command).unwrap();
    assert!(json.contains(r#""type":"add_wall""#));
    assert!(
        json.contains(r#""start":[0.0,0.0]"#),
        "points are compact: {json}"
    );
    let back: Command = serde_json::from_str(&json).unwrap();
    assert_eq!(back, command);
}
