//! Arguments nobody declared, refused by name before a tool runs.
//!
//! Serde drops a field a struct does not have unless the struct says
//! otherwise, and one struct forgetting to is enough for a misspelled `dry` to
//! write, or for a write's argument sent to its read to look like a change
//! that was made. So the check is made once, here, against the schema the
//! agent was handed — the same one for every tool, including the next one.

use serde_json::Value;

/// The first argument in `args` that `schema` does not declare, as the
/// sentence that says so and what the tool takes instead.
pub(crate) fn unknown(schema: &Value, args: &Value) -> Option<String> {
    walk(schema, schema, args, "").or_else(|| out_of_scope(schema, args))
}

/// The actions an argument belongs to, when its description opens by
/// saying so: "For `route`: …", "For `cable` and `route`: …", or
/// "rotate/mirror: …". Only names the tool's `action` enum has count, so a
/// description that merely starts with a word and a colon scopes nothing.
fn scope<'a>(description: &'a str, actions: &[&str]) -> Option<Vec<&'a str>> {
    let names: Vec<&str> = if let Some(rest) = description.strip_prefix("For ") {
        let head = rest.split([':', ';']).next()?;
        let head = head.split(" of ").next()?.split(" on ").next()?;
        head.split(['`', ',', ' '])
            .filter(|w| !w.is_empty() && *w != "and" && *w != "or")
            .collect()
    } else {
        let head = description.split_once(':')?.0;
        if head.contains(' ') {
            return None;
        }
        head.split('/').collect()
    };
    (!names.is_empty() && names.iter().all(|n| actions.contains(n))).then_some(names)
}

/// An argument given with an action it does not belong to. Ignored, it
/// would answer as if it had been used: the agent asked for a pivot and got
/// the rotation about the centre, and never learns why.
fn out_of_scope(schema: &Value, args: &Value) -> Option<String> {
    let props = schema.get("properties")?.as_object()?;
    let actions: Vec<&str> = props
        .get("action")?
        .get("enum")?
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let action = args.get("action")?.as_str()?;
    if !actions.contains(&action) {
        return None;
    }
    for key in args.as_object()?.keys() {
        let Some(description) = props
            .get(key)
            .and_then(|p| p.get("description"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        if let Some(belongs) = scope(description, &actions)
            && !belongs.contains(&action)
        {
            return Some(format!(
                "`{key}` is for {}, not `{action}`",
                belongs
                    .iter()
                    .map(|b| format!("`{b}`"))
                    .collect::<Vec<_>>()
                    .join(" and ")
            ));
        }
    }
    None
}

fn resolve<'a>(root: &'a Value, schema: &'a Value) -> &'a Value {
    match schema.get("$ref").and_then(Value::as_str) {
        Some(reference) => reference
            .strip_prefix("#/$defs/")
            .and_then(|name| root.get("$defs")?.get(name))
            .unwrap_or(schema),
        None => schema,
    }
}

/// The properties an object may carry, when the schema names them all: the
/// union over `anyOf`/`oneOf` branches, or `None` when any branch (or the
/// schema itself) accepts keys it does not name.
fn declared<'a>(root: &'a Value, schema: &'a Value) -> Option<Vec<(&'a str, &'a Value)>> {
    let schema = resolve(root, schema);
    if let Some(branches) = schema
        .get("anyOf")
        .or_else(|| schema.get("oneOf"))
        .and_then(Value::as_array)
    {
        let mut all = Vec::new();
        for branch in branches {
            let branch = resolve(root, branch);
            if branch.get("type").and_then(Value::as_str) == Some("object")
                || branch.get("properties").is_some()
            {
                all.extend(declared(root, branch)?);
            }
        }
        return (!all.is_empty()).then_some(all);
    }
    let closed = schema.get("additionalProperties") == Some(&Value::Bool(false));
    let open = schema.get("additionalProperties").is_some() && !closed;
    match schema.get("properties").and_then(Value::as_object) {
        _ if open => None,
        Some(props) => Some(props.iter().map(|(k, v)| (k.as_str(), v)).collect()),
        // A struct with no fields that refuses the rest: takes nothing.
        None if closed => Some(Vec::new()),
        None => None,
    }
}

fn walk(root: &Value, schema: &Value, value: &Value, at: &str) -> Option<String> {
    let schema = resolve(root, schema);
    match value {
        Value::Object(map) => {
            let props = declared(root, schema)?;
            for (key, inner) in map {
                let Some((_, inner_schema)) = props.iter().find(|(name, _)| name == key) else {
                    let mut names: Vec<&str> = props.iter().map(|(name, _)| *name).collect();
                    names.sort_unstable();
                    names.dedup();
                    let place = if at.is_empty() {
                        String::new()
                    } else {
                        format!(" in `{at}`")
                    };
                    let takes = if names.is_empty() {
                        "it takes no arguments".to_owned()
                    } else {
                        format!("it takes {}", names.join(", "))
                    };
                    return Some(format!("unknown argument `{key}`{place}: {takes}"));
                };
                let path = if at.is_empty() {
                    key.clone()
                } else {
                    format!("{at}.{key}")
                };
                if let Some(why) = walk(root, inner_schema, inner, &path) {
                    return Some(why);
                }
            }
            None
        }
        Value::Array(items) => {
            let item_schema = schema.get("items")?;
            items
                .iter()
                .enumerate()
                .find_map(|(i, item)| walk(root, item_schema, item, &format!("{at}[{i}]")))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::unknown;

    #[test]
    fn names_the_argument_and_what_the_tool_takes() {
        let schema = json!({"type": "object", "properties": {"a": {}, "b": {}}});
        let why = unknown(&schema, &json!({"a": 1, "c": 2})).unwrap();
        assert_eq!(why, "unknown argument `c`: it takes a, b");
        assert!(unknown(&schema, &json!({"a": 1})).is_none());
    }

    #[test]
    fn follows_refs_arrays_and_branches() {
        let schema = json!({
            "type": "object",
            "properties": {"items": {"type": "array", "items": {"$ref": "#/$defs/Item"}}},
            "$defs": {"Item": {"anyOf": [
                {"type": "object", "properties": {"x": {}}},
                {"type": "object", "properties": {"y": {}}}
            ]}}
        });
        assert!(unknown(&schema, &json!({"items": [{"x": 1}, {"y": 2}]})).is_none());
        let why = unknown(&schema, &json!({"items": [{"z": 1}]})).unwrap();
        assert!(
            why.starts_with("unknown argument `z` in `items[0]`"),
            "{why}"
        );
    }

    #[test]
    fn leaves_open_maps_alone() {
        let schema = json!({"type": "object", "properties": {
            "areas": {"type": "object", "additionalProperties": {"type": "number"}}
        }});
        assert!(unknown(&schema, &json!({"areas": {"Sala": 12}})).is_none());
    }

    #[test]
    fn an_argument_is_refused_with_an_action_it_does_not_belong_to() {
        let schema = json!({"type": "object", "properties": {
            "action": {"enum": ["rotate", "mirror", "array", "route", "cable"]},
            "copy": {"description": "rotate/mirror: keep the originals"},
            "dx": {"description": "array: step per copy"},
            "kind": {"description": "For `cable` and `route`: what the run carries"},
            "depth": {"description": "For `route` of sewer: the height free"},
            "ids": {"description": "The elements: all of them"}
        }});
        let why = unknown(&schema, &json!({"action": "rotate", "dx": 1})).unwrap();
        assert_eq!(why, "`dx` is for `array`, not `rotate`");
        assert!(unknown(&schema, &json!({"action": "mirror", "copy": true})).is_none());
        assert!(unknown(&schema, &json!({"action": "route", "kind": "power"})).is_none());
        assert!(unknown(&schema, &json!({"action": "cable", "kind": "power"})).is_none());
        let why = unknown(&schema, &json!({"action": "cable", "depth": 20})).unwrap();
        assert_eq!(why, "`depth` is for `route`, not `cable`");
        assert!(unknown(&schema, &json!({"action": "array", "ids": ["f1"]})).is_none());
    }

    /// The values that are one of two shapes answer with both, not with
    /// serde's "did not match any variant".
    #[test]
    fn a_value_of_the_wrong_shape_says_the_right_ones() {
        use newera_core::{Document, SharedDocument};
        let doc = || SharedDocument::new(Document::default());
        let why = crate::call(doc(), "move", json!({"ids": ["w1"], "dx": 1, "dry": 3}))
            .expect_err("a number is no dry run");
        assert!(why.contains("true, false or \"summary\""), "{why}");
        let why = crate::call(
            doc(),
            "place",
            json!({"items": [{"cat": "sofa", "at": [0, 0], "facing": 5}]}),
        )
        .expect_err("a number is no facing");
        assert!(why.contains("a side (+x -x +y -y)"), "{why}");
    }

    #[test]
    fn a_tool_without_arguments_says_so() {
        let schema = json!({"type": "object", "properties": {}});
        let why = unknown(&schema, &json!({"x": 1})).unwrap();
        assert_eq!(why, "unknown argument `x`: it takes no arguments");
    }
}
