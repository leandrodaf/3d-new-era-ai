//! The rules behind the reviews, read when they are needed.
//!
//! A review's description says what it answers. What each check demands — a
//! standard's tables, what every layout problem means, how a score is weighed —
//! lives here, so an agent pays for it when it asks instead of on every
//! conversation. The same texts are resources (`newera://rules/<topic>`) for
//! the clients that read those.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ErrorData, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use super::NewEraMcp;
use super::reply::invalid;

/// `(topic, title, text)`.
pub(crate) const TOPICS: &[(&str, &str, &str)] = &[
    (
        "electrical",
        "Electrical and telecom rules",
        include_str!("rules/electrical.md"),
    ),
    (
        "plumbing",
        "Plumbing rules",
        include_str!("rules/plumbing.md"),
    ),
    ("layout", "Layout problems", include_str!("rules/layout.md")),
    (
        "ergonomics",
        "How ergonomics scores",
        include_str!("rules/ergonomics.md"),
    ),
];

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RulesParams {
    /// Which review's rules.
    #[schemars(extend("enum" = ["electrical", "plumbing", "layout", "ergonomics"]))]
    topic: String,
}

#[tool_router(router = rules_router, vis = "pub(crate)")]
impl NewEraMcp {
    #[tool(
        description = "The rules behind a review, as Markdown: what `electrical` and `plumbing` check against NBR 5410, 16264, 5626 and 8160 (outlets per room, loads, wire sections, DR, the panel, Wi-Fi, drains, diameters, traps, pipe runs), what each `check_layout` problem means and how to fix it, and how `ergonomics` scores (weights, source tiers, scope, city). For designing to a standard, or explaining a finding."
    )]
    #[allow(clippy::unused_self)] // tool methods need the receiver
    pub(crate) fn rules(
        &self,
        Parameters(p): Parameters<RulesParams>,
    ) -> Result<String, ErrorData> {
        TOPICS
            .iter()
            .find(|(topic, ..)| *topic == p.topic)
            .map(|(.., text)| (*text).to_owned())
            .ok_or_else(|| {
                invalid(format!(
                    "topic: electrical, plumbing, layout or ergonomics (not {})",
                    p.topic
                ))
            })
    }
}

#[cfg(test)]
mod tests {
    /// Every topic answers, and the text names the standard or the problems
    /// it is about.
    #[test]
    fn every_topic_answers() {
        for (topic, needle) in [
            ("electrical", "NBR 5410"),
            ("plumbing", "NBR 8160"),
            ("layout", "blocks_door"),
            ("ergonomics", "tier"),
        ] {
            let doc = newera_core::SharedDocument::new(newera_core::Document::default());
            let answer = crate::call(doc, "rules", serde_json::json!({"topic": topic}))
                .expect("rules answer");
            let text = serde_json::to_string(&answer).unwrap();
            assert!(text.contains(needle), "{topic}: {text}");
        }
    }
}
