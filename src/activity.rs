//! Activity feed (bdui-rs-2u6): `bd history <id> --events --json`
//! events across all beads, merged, deduped, sorted newest-first, grouped
//! by day. Pure; S1-tested.
//!
//! Field tolerance: bd's audit events vary by kind (e.g. `label_added`
//! carries `comment` instead of `new_value`), so every optional field has
//! a serde default and parse never fails on shape.

use crate::model::Bead;
use serde::Deserialize;

/// One audit event from `bd history <id> --events --json`.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ActivityEvent {
    pub id: String,
    pub issue_id: String,
    #[serde(rename = "event_type")]
    pub kind: String,
    #[serde(default)]
    pub actor: String,
    #[serde(default)]
    pub old_value: String,
    #[serde(default)]
    pub new_value: String,
    #[serde(default)]
    pub comment: String,
    #[serde(default)]
    pub created_at: String,
}

/// Parse one bead's `--events --json` array.
pub fn parse_events(json: &str) -> Result<Vec<ActivityEvent>, String> {
    serde_json::from_str(json).map_err(|e| e.to_string())
}

/// Human label per kind; unknown kinds pass through verbatim.
pub fn event_label(kind: &str) -> &str {
    match kind {
        "created" => "created",
        "claimed" => "claimed",
        "closed" => "closed",
        "label_added" => "labeled",
        "dependency_added" => "dependency added",
        other => other,
    }
}

/// Day component of a `2026-11-16T07:01:32Z` stamp; falls back to the
/// whole string for unparseable input so nothing is silently dropped.
pub fn day_of(stamp: &str) -> &str {
    stamp.split_once('T').map(|(d, _)| d).unwrap_or(stamp)
}

/// Time-of-day component (`HH:MM`) of the same stamp.
pub fn time_of(stamp: &str) -> &str {
    stamp
        .split_once('T')
        .and_then(|(_, t)| t.get(..5))
        .unwrap_or(stamp)
}

/// One rendered feed entry.
#[derive(Debug, Clone, PartialEq)]
pub struct ActivityEntry {
    pub event: ActivityEvent,
    /// Bead title at last sync; lookup misses keep the id alone.
    pub title: Option<String>,
}

/// One day group, newest day first, entries newest-first inside.
#[derive(Debug, Clone, PartialEq)]
pub struct ActivityDay {
    pub day: String,
    pub entries: Vec<ActivityEntry>,
}

/// Merge per-bead histories into a day-grouped feed. Duplicate event ids
/// (same bead fetched twice) drop; ordering is by created_at descending,
/// falling back to id for stamps that tie or fail to compare.
pub fn activity_feed(beads: &[Bead], events: Vec<ActivityEvent>) -> Vec<ActivityDay> {
    let title = |id: &str| {
        beads
            .iter()
            .find(|b| b.id == id)
            .map(|b| b.title.clone())
    };

    let mut seen = std::collections::HashSet::new();
    let mut merged: Vec<ActivityEntry> = events
        .into_iter()
        .filter(|e| seen.insert(e.id.clone()))
        .map(|event| {
            let title = title(&event.issue_id);
            ActivityEntry { event, title }
        })
        .collect();

    // ISO stamps sort lexically. Ties keep the incoming order (bd returns
    // a bead's events newest-first), so same-second mutations stay in
    // bd's sequencing rather than falling to an arbitrary id compare.
    merged.sort_by(|a, b| b.event.created_at.cmp(&a.event.created_at));

    let mut days: Vec<ActivityDay> = Vec::new();
    for entry in merged {
        let day = day_of(&entry.event.created_at).to_string();
        match days.last_mut() {
            Some(last) if last.day == day => last.entries.push(entry),
            _ => days.push(ActivityDay {
                day,
                entries: vec![entry],
            }),
        }
    }
    days
}
