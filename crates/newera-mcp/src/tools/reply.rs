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
/// How much of a dry run to answer with.
///
/// `true` answers with everything it would change; `"summary"` answers with
/// the decision — how many pieces move, which roots, the clearances, the
/// findings and the score — because a group that rebuilds lists ninety-seven
/// parts for a choice that fits in five lines.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub(crate) enum Dry {
    All(bool),
    How(String),
}

impl Dry {
    /// Whether this asks for a dry run at all.
    pub(crate) fn on(value: Option<&Self>) -> bool {
        match value {
            Some(Self::All(on)) => *on,
            Some(Self::How(how)) => !how.trim().is_empty() && how.trim() != "false",
            None => false,
        }
    }

    /// Whether it asks for the short answer.
    pub(crate) fn brief(value: Option<&Self>) -> bool {
        matches!(value, Some(Self::How(how)) if how.trim().eq_ignore_ascii_case("summary"))
    }
}

pub(super) fn preview_with(
    doc: &Document,
    brief: bool,
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
        if piece.is_opening() {
            clearances.insert(
                id.to_string(),
                serde_json::json!({
                    "opening": super::measure::opening_measure(&view, piece)
                }),
            );
            continue;
        }
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

    // Which findings it would settle, and which it would create — each with
    // what it is and how big, as `check_layout` reports it: trading a clash
    // for another clash and trading it for a piece resting in place are
    // different decisions, and a pair of ids alone reads the same for both.
    let defects =
        |home: &newera_core::Home| -> std::collections::BTreeMap<String, serde_json::Value> {
            newera_core::check_layout(&home.level_view(home.current_level()))
                .into_iter()
                .filter(|i| i.is_pending(home))
                .map(|i| {
                    let ids = i
                        .ids()
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("+");
                    let mut row = serde_json::json!({"ids": ids, "kind": i.kind_name()});
                    match &i {
                        newera_core::Issue::Overlap { extent, .. } => {
                            row["extent"] = serde_json::json!(extent.map(compact::num));
                        }
                        newera_core::Issue::BlocksWindow { extent, .. } => {
                            row["extent"] = serde_json::json!(extent.map(compact::num));
                        }
                        newera_core::Issue::Blocked { cm, .. } => {
                            row["cm"] = compact::num(*cm);
                        }
                        newera_core::Issue::AboveCeiling { top, ceiling, .. } => {
                            row["over"] = compact::num(*top - *ceiling);
                        }
                        newera_core::Issue::OutgrewNiche { over, .. } => {
                            row["over"] = serde_json::json!(over.map(compact::num));
                        }
                        _ => {}
                    }
                    (format!("{}:{ids}", i.family()), row)
                })
                .collect()
        };
    let (was, now) = (defects(&before), defects(&after));
    let object = out.as_object_mut().expect("object");
    for (key, from, other) in [("issues_resolved", &was, &now), ("issues_new", &now, &was)] {
        let list: Vec<&serde_json::Value> = from
            .iter()
            .filter(|(k, _)| !other.contains_key(*k))
            .map(|(_, row)| row)
            .collect();
        if !list.is_empty() {
            object.insert(key.to_owned(), serde_json::json!(list));
        }
    }
    // A clash that stays but grows or shrinks is neither new nor resolved,
    // and it is the number a nudge was made for.
    let changed: Vec<serde_json::Value> = now
        .iter()
        .filter_map(|(k, row)| {
            let old = was.get(k)?;
            (old != row).then(|| {
                let mut row = row.clone();
                for field in ["extent", "cm", "over"] {
                    if let Some(v) = old.get(field) {
                        row[format!("{field}_was")] = v.clone();
                    }
                }
                row
            })
        })
        .collect();
    if !changed.is_empty() {
        object.insert("issues_changed".to_owned(), serde_json::json!(changed));
    }

    // Scored for the people the project is reviewed for, so the number is
    // the one the review gives and not the score of two default occupants.
    let profile = newera_ergonomics::Profile::of(&before);
    let (was, now) = (
        newera_ergonomics::review(&before, &profile),
        newera_ergonomics::review(&after, &profile),
    );
    // Stable rule/element identity survives a renamed room or changed measure.
    let key = |f: &newera_ergonomics::Finding| f.key.clone();
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
    let changed: Vec<serde_json::Value> =
        now.findings
            .iter()
            .filter_map(|f| {
                let old = was.findings.iter().find(|old| old.key == f.key)?;
                (old.message != f.message || old.place != f.place || old.severity != f.severity)
                    .then(|| {
                        serde_json::json!({"key":f.key,
                "from":{"sev":old.severity,"place":old.place,"msg":old.message},
                "to":{"sev":f.severity,"place":f.place,"msg":f.message}})
                    })
            })
            .collect();
    if !changed.is_empty() {
        object.insert("findings_changed".into(), serde_json::json!(changed));
    }
    if brief {
        // The parts a group rebuilds are not a decision; the roots are.
        for key in ["changed", "added", "removed"] {
            let Some(list) = object.get(key).and_then(|v| v.as_array()).cloned() else {
                continue;
            };
            let roots: Vec<serde_json::Value> = list
                .iter()
                .filter_map(|c| {
                    let raw = if c.is_string() {
                        c.as_str()?
                    } else {
                        c["id"].as_str()?
                    };
                    let id: newera_core::ElementId = raw.parse().ok()?;
                    match id {
                        newera_core::ElementId::Furniture(piece)
                            if after.part_owner(piece).is_some() =>
                        {
                            None
                        }
                        _ => Some(serde_json::json!(raw)),
                    }
                })
                .collect();
            object.insert(format!("{key}_count"), serde_json::json!(list.len()));
            object.insert(key.to_owned(), serde_json::json!(roots));
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
