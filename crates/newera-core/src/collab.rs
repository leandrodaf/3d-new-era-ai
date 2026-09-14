//! Several people (or agents) on the same project: who is connected, where
//! their cursor is, what they selected and how much they changed. Presence
//! lives beside the document, is never saved and never enters undo history.

use serde::Serialize;

use crate::{ElementId, LevelId, Point2};

/// Sessions not heard from for this long are dropped.
pub const SESSION_TIMEOUT_MS: u64 = 30_000;

/// Distinct, readable colors handed out to sessions in turn.
const COLORS: [[u8; 3]; 8] = [
    [0xE0, 0x4F, 0x5F],
    [0x2E, 0x86, 0xDE],
    [0x27, 0xAE, 0x60],
    [0xF3, 0x9C, 0x12],
    [0x8E, 0x44, 0xAD],
    [0x16, 0xA0, 0x85],
    [0xD3, 0x54, 0x00],
    [0x34, 0x49, 0x5E],
];

/// One connected collaborator.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub color: [u8; 3],
    /// Pointer on the plan, cm.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<Point2>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<LevelId>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub selection: Vec<ElementId>,
    /// Command batches applied by this session.
    pub edits: u64,
    /// Document revision after its last edit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_revision: Option<u64>,
    /// Last time it was heard from, ms since the Unix epoch.
    pub seen_ms: u64,
}

/// What a session reports about itself.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Presence {
    pub cursor: Option<Point2>,
    pub level: Option<LevelId>,
    pub selection: Vec<ElementId>,
}

/// Where the HTTP API of this process is, for plugins calling back into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerInfo {
    /// Base URL, e.g. `http://127.0.0.1:7878`.
    pub url: String,
    pub token: Option<String>,
}

/// Every session of a document.
#[derive(Debug, Clone, Default)]
pub struct Sessions {
    list: Vec<Session>,
    next: u64,
    /// Changes on every join, leave or presence update.
    generation: u64,
}

impl Sessions {
    /// Registers a collaborator and returns its session.
    pub fn join(&mut self, name: &str, now_ms: u64) -> Session {
        self.next += 1;
        let index = usize::try_from(self.next - 1).unwrap_or(0) % COLORS.len();
        let name = name.trim();
        let session = Session {
            id: format!("s{}", self.next),
            name: if name.is_empty() {
                format!("Pessoa {}", self.next)
            } else {
                name.chars().take(40).collect()
            },
            color: COLORS[index],
            cursor: None,
            level: None,
            selection: Vec::new(),
            edits: 0,
            last_revision: None,
            seen_ms: now_ms,
        };
        self.list.push(session.clone());
        self.generation += 1;
        session
    }

    /// Updates what a session shows; `false` when it is unknown.
    pub fn update(&mut self, id: &str, presence: Presence, now_ms: u64) -> bool {
        let Some(session) = self.list.iter_mut().find(|s| s.id == id) else {
            return false;
        };
        session.cursor = presence.cursor;
        session.level = presence.level;
        session.selection = presence.selection;
        session.seen_ms = now_ms;
        self.generation += 1;
        true
    }

    /// Counts an edit by `id` that left the document at `revision`.
    pub fn record_edit(&mut self, id: &str, revision: u64, now_ms: u64) -> bool {
        let Some(session) = self.list.iter_mut().find(|s| s.id == id) else {
            return false;
        };
        session.edits += 1;
        session.last_revision = Some(revision);
        session.seen_ms = now_ms;
        self.generation += 1;
        true
    }

    pub fn leave(&mut self, id: &str) -> bool {
        let before = self.list.len();
        self.list.retain(|s| s.id != id);
        let left = self.list.len() != before;
        if left {
            self.generation += 1;
        }
        left
    }

    /// Drops sessions silent for longer than [`SESSION_TIMEOUT_MS`].
    pub fn expire(&mut self, now_ms: u64) {
        let before = self.list.len();
        self.list
            .retain(|s| now_ms.saturating_sub(s.seen_ms) <= SESSION_TIMEOUT_MS);
        if self.list.len() != before {
            self.generation += 1;
        }
    }

    pub fn get(&self, id: &str) -> Option<&Session> {
        self.list.iter().find(|s| s.id == id)
    }

    pub fn list(&self) -> &[Session] {
        &self.list
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }
}

/// Milliseconds since the Unix epoch.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WallId;

    #[test]
    fn sessions_join_update_edit_and_expire() {
        let mut sessions = Sessions::default();
        let ana = sessions.join("Ana", 1_000);
        let bot = sessions.join("  ", 1_000);
        assert_ne!(ana.id, bot.id);
        assert_ne!(ana.color, bot.color);
        assert_eq!(bot.name, "Pessoa 2");

        let generation = sessions.generation();
        assert!(sessions.update(
            &ana.id,
            Presence {
                cursor: Some(Point2::new(120.0, 80.0)),
                selection: vec![ElementId::Wall(WallId(1))],
                ..Presence::default()
            },
            5_000,
        ));
        assert!(sessions.generation() > generation);
        assert!(!sessions.update("s99", Presence::default(), 5_000));
        assert!(sessions.record_edit(&ana.id, 7, 6_000));
        let ana = sessions.get(&ana.id).unwrap();
        assert_eq!((ana.edits, ana.last_revision), (1, Some(7)));

        // Bot was last seen at 1 s; at 32 s it is gone, Ana (6 s) stays.
        sessions.expire(32_000);
        assert_eq!(sessions.list().len(), 1);
        assert!(sessions.leave("s1"));
        assert!(sessions.list().is_empty());
        let json = serde_json::to_value(sessions.join("Caio", 0)).unwrap();
        assert_eq!(json["name"], "Caio");
        assert!(json.get("cursor").is_none());
    }
}
