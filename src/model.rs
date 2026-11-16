//! Issue model: parsed bead records plus derived state.
//! Vocabulary per CONTEXT.md: State is the viewer's derived five-value
//! classification; status is the stored bd field.

use serde::Deserialize;
use std::fmt;


/// Stored bd status values.
pub const STATUS_OPEN: &str = "open";
pub const STATUS_IN_PROGRESS: &str = "in_progress";
pub const STATUS_BLOCKED: &str = "blocked";
pub const STATUS_DEFERRED: &str = "deferred";
pub const STATUS_CLOSED: &str = "closed";

/// Derived viewer state. A bead with status `open` and open blockers
/// shows as Blocked (CONTEXT.md: State vs status).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum State {
    #[default]
    Ready,
    InProgress,
    Blocked,
    Deferred,
    Closed,
}

impl State {
    /// Map a stored status to the derived state, before blocker analysis.
    fn from_status(status: &str) -> Option<Self> {
        match status {
            STATUS_OPEN => Some(Self::Ready),
            STATUS_IN_PROGRESS => Some(Self::InProgress),
            STATUS_BLOCKED => Some(Self::Blocked),
            STATUS_DEFERRED => Some(Self::Deferred),
            STATUS_CLOSED => Some(Self::Closed),
            _ => None,
        }
    }
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Ready => "ready",
            Self::InProgress => "in progress",
            Self::Blocked => "blocked",
            Self::Deferred => "deferred",
            Self::Closed => "closed",
        };
        f.write_str(s)
    }
}

/// One bead as reported by `bd list --json` (summary row) or
/// `bd show <id> --json --long` (full record; extra fields tolerated).
#[derive(Debug, Clone, Deserialize)]
pub struct Bead {
    pub id: String,
    pub title: String,
    pub status: String,
    #[serde(default = "default_priority")]
    pub priority: u8,
    #[serde(default)]
    pub issue_type: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub assignee: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub dependency_count: usize,
    #[serde(default)]
    pub dependent_count: usize,
    #[serde(default)]
    pub comment_count: usize,

    /// Derived; populated by `derive_state`, not present in bd JSON.
    #[serde(skip)]
    pub state: State,
}

fn default_priority() -> u8 {
    2
}

impl Bead {
    /// Ready-work ranking: priority ascending (P0 first), ID as tiebreak.
    pub fn ready_rank(a: &Bead, b: &Bead) -> std::cmp::Ordering {
        a.priority
            .cmp(&b.priority)
            .then_with(|| a.id.cmp(&b.id))
    }
}

/// Parse a `bd list --json` array and derive each bead's state.
///
/// Derivation rules (CONTEXT.md "State"):
/// - closed status => Closed
/// - blocked status => Blocked
/// - open status with open blockers => Blocked (blocker-aware ready semantics,
///   matching `bd ready`: only open beads with no active blockers are ready)
/// - otherwise => Ready / InProgress / Deferred per status
///
/// Summary rows do not name which dependencies are open, so a summary
/// `dependency_count > 0` on an open bead is refined later by the Store
/// when dependency records are loaded; at summary level an open bead
/// stays Ready and `bd ready --json` remains the authority for readiness.
pub fn parse_summary(json: &str) -> Result<Vec<Bead>, String> {
    let mut beads: Vec<Bead> = serde_json::from_str(json).map_err(|e| e.to_string())?;
    derive_state(&mut beads);
    Ok(beads)
}

/// Compute derived state for each bead from its stored status.
fn derive_state(beads: &mut [Bead]) {
    for bead in beads.iter_mut() {
        bead.state = State::from_status(&bead.status).unwrap_or(State::Ready);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_status_defaults_to_ready() {
        let mut b = Bead {
            id: "bd-x".into(),
            title: "t".into(),
            status: "weird".into(),
            priority: 2,
            issue_type: "task".into(),
            created_at: String::new(),
            updated_at: String::new(),
            assignee: None,
            labels: vec![],
            dependency_count: 0,
            dependent_count: 0,
            comment_count: 0,
            state: State::Ready,
        };
        let slice = std::slice::from_mut(&mut b);
        derive_state(slice);
        assert_eq!(b.state, State::Ready);
    }
}
