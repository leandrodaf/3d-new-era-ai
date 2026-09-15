//! How writes answer, and the pieces every write needs first.
//!
//! Version targeting, background pixel coordinates, `ok rev=N`, dry runs and
//! errors: the handful of things that belong to no single domain because
//! nearly every domain writes.

use newera_core::{Document, Home, Point2};
use rmcp::ErrorData;

use crate::compact;

/// `ok rev=N [v=I] [ids=…]`; the active variant is named once there are
/// several, so an agent notices when it writes to another tab than it meant.
pub(super) fn ok(doc: &Document, ids: &[String]) -> String {
    use std::fmt::Write as _;
    let mut reply = format!("ok rev={}", doc.revision());
    if doc.variant_count() > 1 {
        let _ = write!(reply, " v={}", doc.active_variant());
    }
    if !ids.is_empty() {
        let _ = write!(reply, " ids={}", ids.join(","));
    }
    reply
}

/// Background image placement, to read coordinates given in its pixels.
pub(super) struct BackgroundScale {
    pub(super) image: newera_core::BackgroundImage,
    pub(super) scale: f64,
}

impl BackgroundScale {
    pub(super) fn point(&self, px: Point2) -> Point2 {
        self.image.plan_point(px)
    }
}

pub(super) fn background_scale(doc: &Document) -> Result<BackgroundScale, ErrorData> {
    let home = doc.home();
    let bg = home
        .current_level()
        .and_then(|id| home.level(id))
        .and_then(|l| l.background.as_ref())
        .or(home.background.as_ref())
        .ok_or_else(|| invalid("`px` needs a background image (set_background)"))?;
    Ok(BackgroundScale {
        image: bg.clone(),
        scale: bg.cm_per_px,
    })
}

/// Makes version `v` the active one before a write, so the write lands where
/// the agent means even if someone switched tabs meanwhile.
pub(super) fn on_variant(doc: &mut Document, v: Option<usize>) -> Result<(), ErrorData> {
    match v {
        Some(v) if v != doc.active_variant() => doc.switch_variant(v).map_err(core),
        _ => Ok(()),
    }
}

/// Runs an edit against a copy of the plan and reports what it would do.
///
/// Trying a size used to mean applying it, reviewing, and undoing — a round
/// trip that showed in the user's window and burned a revision each time.
/// The copy has no history and is thrown away, so nothing of that happens.
pub(super) fn preview(
    doc: &Document,
    apply: impl FnOnce(&mut Document) -> Result<(), ErrorData>,
) -> Result<String, ErrorData> {
    let before = doc.home().clone();
    let mut scratch = Document::new(before.clone());
    apply(&mut scratch)?;
    let after = scratch.home().clone();
    let mut out = compact::diff(&before, &after);
    let object = out.as_object_mut().expect("object");
    object.insert("dry".to_owned(), serde_json::json!(true));

    // Free floor around every piece the change touched: the number the
    // change was made for, without a second call.
    let touched: Vec<newera_core::FurnitureId> = out["changed"]
        .as_array()
        .into_iter()
        .flatten()
        .chain(out["added"].as_array().into_iter().flatten())
        .filter_map(|c| {
            let raw = if c.is_string() {
                c.as_str()?
            } else {
                c["id"].as_str()?
            };
            raw.parse().ok()
        })
        .collect();
    let mut clearances = serde_json::Map::new();
    let view = after.level_view(after.current_level());
    // Gathered once for the storey, not once per piece per side.
    let solids = newera_core::obstacles(&view, &|_| false);
    for id in touched.iter().take(12) {
        let Some(piece) = view.find_piece(*id) else {
            continue;
        };
        let sides: serde_json::Map<String, serde_json::Value> = newera_core::Dir::PLAN
            .iter()
            .map(|dir| {
                let c = newera_core::measure::clearance_against(
                    &solids,
                    piece,
                    *dir,
                    newera_core::measure::MAX_REACH,
                );
                (
                    dir.name().to_owned(),
                    serde_json::json!([
                        compact::num(c.cm),
                        c.against.map(|s| s.id().to_string()),
                        c.name
                    ]),
                )
            })
            .collect();
        clearances.insert(id.to_string(), serde_json::Value::Object(sides));
    }
    if !clearances.is_empty() {
        out.as_object_mut().expect("object").insert(
            "clearances".to_owned(),
            serde_json::Value::Object(clearances),
        );
    }

    // Which findings it would settle, and which it would create.
    let defects = |home: &newera_core::Home| -> std::collections::BTreeSet<String> {
        newera_core::check_layout(&home.level_view(home.current_level()))
            .into_iter()
            .filter(newera_core::Issue::is_defect)
            .map(|i| {
                i.ids()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("+")
            })
            .collect()
    };
    let (was, now) = (defects(&before), defects(&after));
    for (key, list) in [
        ("issues_resolved", was.difference(&now).collect::<Vec<_>>()),
        ("issues_new", now.difference(&was).collect()),
    ] {
        if !list.is_empty() {
            out.as_object_mut()
                .expect("object")
                .insert(key.to_owned(), serde_json::json!(list));
        }
    }

    let profile = newera_ergonomics::Profile::default();
    let (was, now) = (
        newera_ergonomics::review(&before, &profile),
        newera_ergonomics::review(&after, &profile),
    );
    // Findings are matched without their numbers, so one that merely got
    // better reads as improved rather than as one gone and one new.
    let key = |f: &newera_ergonomics::Finding| {
        let text: String = f.message.chars().filter(|c| !c.is_ascii_digit()).collect();
        format!("{} {text}", f.place)
    };
    let old_keys: std::collections::BTreeSet<String> = was.findings.iter().map(&key).collect();
    let new_keys: std::collections::BTreeSet<String> = now.findings.iter().map(&key).collect();
    let object = out.as_object_mut().expect("object");
    if was.score != now.score {
        object.insert(
            "score".to_owned(),
            serde_json::json!([was.score, now.score]),
        );
    }
    for (label, findings, other) in [
        ("resolved", &was.findings, &new_keys),
        ("new_findings", &now.findings, &old_keys),
    ] {
        let list: Vec<serde_json::Value> = findings
            .iter()
            .filter(|f| !other.contains(&key(f)))
            .map(|f| serde_json::json!([f.severity, f.place, f.message]))
            .collect();
        if !list.is_empty() {
            object.insert(label.to_owned(), serde_json::json!(list));
        }
    }
    Ok(serde_json::Value::Object(object.clone()).to_string())
}

/// How many changed elements a write names before it just counts them.
pub(super) const DIFF_LIMIT: usize = 20;

/// `ok rev=N` with what the write actually did appended.
///
/// A write that says only `ok` forces a full read to learn its effect —
/// which is how a plan ends up edited blind. A batch touching hundreds of
/// pieces is counted instead of listed: past a point the list is the read
/// it was meant to save.
pub(super) fn applied(doc: &Document, before: &Home) -> String {
    let mut diff = compact::diff(before, doc.home());
    let object = diff.as_object_mut().expect("object");
    if object.is_empty() {
        return ok(doc, &[]);
    }
    for key in ["changed", "added", "gone"] {
        let Some(list) = object.get_mut(key).and_then(|v| v.as_array_mut()) else {
            continue;
        };
        if list.len() > DIFF_LIMIT {
            let n = list.len();
            *object.get_mut(key).expect("present") = serde_json::json!(n);
        }
    }
    format!("{} {diff}", ok(doc, &[]))
}

pub(super) fn invalid(message: impl Into<String>) -> ErrorData {
    ErrorData::invalid_params(message.into(), None)
}

#[allow(clippy::needless_pass_by_value)] // used as `map_err(core)`
pub(super) fn core(err: newera_core::CoreError) -> ErrorData {
    invalid(err.to_string())
}
