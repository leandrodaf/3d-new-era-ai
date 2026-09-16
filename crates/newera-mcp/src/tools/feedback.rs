//! What the tools could do better, told by whoever uses them.
//!
//! A note is a small report, not a complaint: a fix made from "measure is
//! wrong" is as likely to break the case that works as to mend the one that
//! did not — the friction log this tool replaces shows fixes that did both.
//! So every note carries the case that produced it, the literal answer, what
//! was true, what it cost, the change that would have helped, and what must
//! keep working when that change is made.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ErrorData, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use super::NewEraMcp;
use super::reply::invalid;

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct FeedbackParams {
    /// `friction` (it worked, at a cost), `bug` (it answered wrong) or
    /// `idea` (something missing).
    kind: Option<String>,
    /// The tool it is about, e.g. `measure`.
    tool: Option<String>,
    /// What you were trying to do, and the situation in the plan: the
    /// pieces, rooms and numbers involved.
    #[serde(default)]
    goal: String,
    /// The exact call, with its arguments.
    #[serde(default)]
    tried: String,
    /// What came back, literally — the part of the reply that is the proof.
    #[serde(default)]
    got: String,
    /// What was true instead, and how you found out (another tool, a render,
    /// a probe) — the reference the fix will be checked against.
    #[serde(default)]
    expected: String,
    /// What it cost: the workaround, the extra calls, the wrong conclusion
    /// reached before the right one, the distortion made to the plan.
    #[serde(default)]
    cost: String,
    /// The change that would have shortened the way, as concretely as you
    /// can: a field, a parameter, a rule.
    #[serde(default)]
    would_help: String,
    /// What works today and must keep working with that change — the cases
    /// where the current behaviour is right. "none known" is an answer.
    #[serde(default)]
    must_keep: String,
}

/// Shortest a part of a note can be and still say something, in characters.
const AT_LEAST: usize = 10;

#[tool_router(router = feedback_router, vis = "pub(crate)")]
impl NewEraMcp {
    #[tool(
        description = "Report to the developers what a tool could do better, as it happens: a reply that answered less than was asked, a detour, a wrong conclusion reached before the right one, a fix that would distort the plan, something missing. It is a report the fix will be made from, so give the whole case — a vague note gets fixed in a way that breaks what already works. Every part is required: goal (the task and the situation: pieces, rooms, numbers), tried (the exact call with arguments), got (the literal reply), expected (what was true and how you found out), cost (the workaround, extra calls, wrong conclusion), would_help (the concrete change), must_keep (what works today and must not get worse with that change; \"none known\" is an answer). kind friction|bug|idea, tool. Kept on this machine and, while the user's telemetry is on, sent to the project. Reply {kept, sent}; then carry on with the task."
    )]
    #[allow(clippy::unused_self)] // tool methods need the receiver
    pub(crate) fn feedback(
        &self,
        Parameters(p): Parameters<FeedbackParams>,
    ) -> Result<String, ErrorData> {
        let kind = p.kind.as_deref().map_or("friction", str::trim);
        if !["friction", "bug", "idea"].contains(&kind) {
            return Err(invalid(format!("kind: friction, bug or idea (not {kind})")));
        }
        let parts = [
            ("goal", &p.goal, "the task and the situation in the plan"),
            ("tried", &p.tried, "the exact call with its arguments"),
            ("got", &p.got, "the literal reply"),
            (
                "expected",
                &p.expected,
                "what was true, and how you found out",
            ),
            (
                "cost",
                &p.cost,
                "the workaround, extra calls or wrong conclusion",
            ),
            ("would_help", &p.would_help, "the concrete change"),
            (
                "must_keep",
                &p.must_keep,
                "what works today and must not get worse (\"none known\" is an answer)",
            ),
        ];
        let thin: Vec<String> = parts
            .iter()
            .filter(|(_, value, _)| value.trim().chars().count() < AT_LEAST)
            .map(|(name, _, what)| format!("{name} ({what})"))
            .collect();
        if !thin.is_empty() {
            return Err(invalid(format!(
                "a note is fixed from its details, and one without them gets fixed in a way that breaks what works; missing or too short: {}",
                thin.join("; ")
            )));
        }
        let note = newera_telemetry::Note {
            kind: kind.to_owned(),
            tool: p
                .tool
                .map(|t| t.trim().to_owned())
                .filter(|t| !t.is_empty()),
            goal: p.goal.trim().to_owned(),
            tried: p.tried.trim().to_owned(),
            got: p.got.trim().to_owned(),
            expected: p.expected.trim().to_owned(),
            cost: p.cost.trim().to_owned(),
            would_help: p.would_help.trim().to_owned(),
            must_keep: p.must_keep.trim().to_owned(),
        };
        let delivery = newera_telemetry::note(&note)
            .map_err(|e| invalid(format!("the note could not be kept: {e}")))?;
        Ok(serde_json::json!({
            "kept": newera_telemetry::notes_path().display().to_string(),
            "sent": delivery == newera_telemetry::Delivery::Sent,
        })
        .to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::server;

    #[test]
    fn a_note_needs_the_whole_case_and_says_where_it_went() {
        let dir = std::env::temp_dir().join(format!("newera-mcp-notes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        newera_telemetry::use_config_dir(dir.clone());
        let s = server();
        let feedback = |json: &str| s.feedback(Parameters(serde_json::from_str(json).unwrap()));

        // A one-liner is refused, naming every part it lacks.
        let err = feedback(r#"{"tool":"measure","would_help":"a sonda ler as chapas"}"#)
            .unwrap_err()
            .message
            .to_string();
        for part in ["goal", "tried", "got", "expected", "cost", "must_keep"] {
            assert!(err.contains(part), "{part}: {err}");
        }
        assert!(!err.contains("would_help ("), "{err}");

        let reply: serde_json::Value = serde_json::from_str(
            &feedback(
                r#"{"kind":"bug","tool":"measure",
                    "goal":"conferir a passagem de 50 cm entre o vassoureiro e o tanque",
                    "tried":"measure(axis=\"x\", at=450, range=[612,650], z=[0,280])",
                    "got":"spans: [[612,613,\"f1080\"],[613,650,null,\"\"]]",
                    "expected":"o vassoureiro f988 ocupa 615-645; measure(from=f988) acerta",
                    "cost":"a cota d106 foi acusada como stale e quase foi corrigida para o número errado",
                    "would_help":"a sonda ler as partes finas de todo grupo, como from já faz",
                    "must_keep":"tapetes e vidros finos soltos continuam fora da sonda"}"#,
            )
            .unwrap(),
        )
        .unwrap();
        // No reporter runs in tests, so nothing can have been sent.
        assert_eq!(reply["sent"], false, "{reply}");
        let kept = std::fs::read_to_string(dir.join("notes.jsonl")).unwrap();
        let note: newera_telemetry::Note =
            serde_json::from_str(kept.lines().last().unwrap()).unwrap();
        assert_eq!(note.kind, "bug");
        assert!(note.must_keep.contains("tapetes"), "{kept}");
        assert!(
            note.text().contains("Não pode piorar: tapetes"),
            "{}",
            note.text()
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
