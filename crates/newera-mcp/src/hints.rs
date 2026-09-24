//! What each tool is called for people, and what it does to the world.
//!
//! An AI client shows the title where a person approves a call, and reads the
//! hints to decide how carefully to ask: a read runs without a prompt, a write
//! asks, a destructive write asks harder. The directories that list this
//! server (the Claude and `ChatGPT` ones) check that every tool says so, and that it
//! says so truthfully.
//!
//! One table, applied where the router is assembled, so every transport — the
//! window, `serve`, stdio, the relay in front of a browser tab, the hosted
//! service — hands out the same answer. A tool missing from it fails
//! `every_tool_has_hints`.
//!
//! The hints describe the worst any action of a tool can do: `cameras` lists
//! views, but it also deletes one, so it is a destructive write. A tool that
//! only adds (a wall, a piece, a copy) is a write that is not destructive.

use rmcp::model::{Tool, ToolAnnotations};

/// What a tool does to the world outside the answer it gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Effect {
    /// Reads the project, changes nothing.
    Read,
    /// Adds to the project and never changes or removes what is there.
    Add,
    /// Can change or remove what is there: undoable in the editor, but a
    /// change all the same.
    Change,
}

/// `(tool, title, effect, reaches beyond the project)`.
///
/// "Beyond the project" is the open-world hint: a tool whose effect is not
/// confined to the plan and the files the user names — one that sends
/// something to the developers or starts another program.
const HINTS: &[(&str, &str, Effect, bool)] = &[
    // Reads.
    ("get_home", "Read the home", Effect::Read, false),
    (
        "materials",
        "List wall types and finishes",
        Effect::Read,
        false,
    ),
    (
        "catalog",
        "Search the furniture catalog",
        Effect::Read,
        false,
    ),
    ("measure", "Measure the plan", Effect::Read, false),
    ("sessions", "List who is editing", Effect::Read, false),
    ("render_plan", "Render the floor plan", Effect::Read, false),
    ("render_3d", "Render a 3D view", Effect::Read, false),
    ("render_photo", "Render a photo", Effect::Read, false),
    // Writes that only add.
    ("create", "Draw walls, rooms and more", Effect::Add, false),
    (
        "place",
        "Place furniture, doors and windows",
        Effect::Add,
        false,
    ),
    (
        "trace_background",
        "Trace walls from a scan",
        Effect::Add,
        false,
    ),
    (
        "lighting",
        "Check and fill the lighting",
        Effect::Add,
        false,
    ),
    ("variants", "Plan versions", Effect::Add, false),
    (
        "disciplines",
        "Show electrical and plumbing",
        Effect::Add,
        false,
    ),
    // Writes that change or remove.
    ("update", "Change elements", Effect::Change, false),
    ("move", "Move elements", Effect::Change, false),
    ("delete", "Delete elements", Effect::Change, false),
    ("arrange", "Copy, align and group", Effect::Change, false),
    ("split_wall", "Split a wall", Effect::Change, false),
    ("merge_walls", "Merge walls", Effect::Change, false),
    ("joinery", "Build joinery", Effect::Change, false),
    (
        "cabinet_run",
        "Fill a wall with cabinets",
        Effect::Change,
        false,
    ),
    ("embed", "Embed an appliance or sink", Effect::Change, false),
    ("fit_roof", "Fit walls to the roof", Effect::Change, false),
    ("levels", "Storeys", Effect::Change, false),
    ("cameras", "Points of view", Effect::Change, false),
    ("video", "Video camera path", Effect::Change, false),
    (
        "annotations",
        "Dimensions, tags and notes",
        Effect::Change,
        false,
    ),
    ("check_layout", "Check the layout", Effect::Change, false),
    ("ergonomics", "Review ergonomics", Effect::Change, false),
    ("electrical", "Electrical project", Effect::Change, false),
    ("plumbing", "Plumbing project", Effect::Change, false),
    ("set_home", "Project settings", Effect::Change, false),
    (
        "set_background",
        "Set a scanned plan",
        Effect::Change,
        false,
    ),
    ("checkpoint", "Checkpoints", Effect::Change, false),
    ("undo", "Undo", Effect::Change, false),
    ("redo", "Redo", Effect::Change, false),
    ("new_home", "New project", Effect::Change, false),
    ("open_home", "Open a project", Effect::Change, false),
    ("save_home", "Save the project", Effect::Change, false),
    ("export_plan", "Export the plan", Effect::Change, false),
    ("cut_list", "Cut list", Effect::Change, false),
    // Beyond the project.
    ("plugins", "Run a plugin", Effect::Change, true),
    ("feedback", "Report to the developers", Effect::Add, true),
];

/// Gives a tool its title and hints. A tool not in the table is left as it
/// is, and the test below says which one.
pub(crate) fn apply(tool: &mut Tool) {
    let Some(&(_, title, effect, open)) = HINTS.iter().find(|(name, ..)| *name == tool.name) else {
        return;
    };
    let mut hints = ToolAnnotations::default();
    hints.title = Some(title.to_owned());
    hints.read_only_hint = Some(effect == Effect::Read);
    if effect != Effect::Read {
        hints.destructive_hint = Some(effect == Effect::Change);
    }
    hints.open_world_hint = Some(open);
    tool.title = Some(title.to_owned());
    tool.annotations = Some(hints);
}

#[cfg(test)]
mod tests {
    use super::HINTS;

    /// Every tool says what it does. A new tool without a row here would
    /// reach the directories unlabelled, and be refused there.
    #[test]
    fn every_tool_has_hints() {
        for tool in crate::tools() {
            let hints = tool
                .annotations
                .as_ref()
                .unwrap_or_else(|| panic!("{} has no hints: add it to HINTS", tool.name));
            assert!(tool.title.is_some(), "{} has no title", tool.name);
            assert!(hints.read_only_hint.is_some(), "{}", tool.name);
            assert!(hints.open_world_hint.is_some(), "{}", tool.name);
            if hints.read_only_hint == Some(false) {
                assert!(hints.destructive_hint.is_some(), "{}", tool.name);
            }
        }
    }

    /// And the table names only tools that exist: a row left behind by a
    /// renamed tool would label nothing, and hide that the new name has none.
    #[test]
    fn every_row_names_a_tool() {
        let names: Vec<String> = crate::tools().iter().map(|t| t.name.to_string()).collect();
        for (name, ..) in HINTS {
            assert!(names.iter().any(|n| n == name), "no tool named {name}");
        }
        let mut seen: Vec<&str> = HINTS.iter().map(|(name, ..)| *name).collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), HINTS.len(), "a tool has two rows");
    }

    /// Titles are for people, and the directories cap tool names at 64
    /// characters; keep titles well under that.
    #[test]
    fn titles_are_short() {
        for (name, title, ..) in HINTS {
            assert!(title.len() <= 40, "{name}: title too long");
        }
    }
}
