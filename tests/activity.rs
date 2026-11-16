//! S1: activity feed merge/group/order (bdui-rs-2u6).

use bdui::activity::{
    activity_feed, day_of, event_label, parse_events, time_of, ActivityEvent,
};
use bdui::model::Bead;
use serde_json::json;

fn event(id: &str, issue: &str, kind: &str, actor: &str, at: &str) -> ActivityEvent {
    serde_json::from_value(json!({
        "id": id,
        "issue_id": issue,
        "event_type": kind,
        "actor": actor,
        "created_at": at,
    }))
    .unwrap()
}

fn bead(id: &str, title: &str) -> Bead {
    serde_json::from_value(json!({
        "id": id,
        "title": title,
        "status": "open",
        "priority": 2,
        "issue_type": "task",
    }))
    .unwrap()
}

#[test]
fn parses_real_event_shapes() {
    // created/claimed carry old/new values; label_added carries comment.
    let json = r#"[
        {"id":"a","issue_id":"x","event_type":"label_added","actor":"k",
         "comment":"label x","created_at":"2026-11-16T07:00:00Z"},
        {"id":"b","issue_id":"x","event_type":"closed","actor":"k",
         "old_value":"","new_value":"Closed","created_at":"2026-11-16T07:01:00Z"}
    ]"#;
    let evs = parse_events(json).unwrap();
    assert_eq!(evs.len(), 2);
    assert_eq!(evs[0].kind, "label_added");
    assert_eq!(evs[0].comment, "label x");
    assert_eq!(evs[0].new_value, "");
    assert_eq!(evs[1].new_value, "Closed");
}

#[test]
fn feed_sorts_newest_first_and_groups_by_day() {
    let beads = vec![bead("x", "Tree view"), bead("y", "Board view")];
    let events = vec![
        event("e1", "x", "created", "koosty", "2026-11-15T09:00:00Z"),
        event("e2", "y", "claimed", "agent", "2026-11-16T08:00:00Z"),
        event("e3", "x", "closed", "koosty", "2026-11-16T09:30:00Z"),
        event("e4", "x", "created", "koosty", "2026-11-14T23:00:00Z"),
    ];
    let days = activity_feed(&beads, events);
    assert_eq!(days.len(), 3, "three distinct days");
    assert_eq!(days[0].day, "2026-11-16");
    assert_eq!(days[1].day, "2026-11-15");
    assert_eq!(days[2].day, "2026-11-14");
    // Newest first inside the day.
    assert_eq!(days[0].entries[0].event.id, "e3");
    assert_eq!(days[0].entries[1].event.id, "e2");
    // Titles resolve from the bead set.
    assert_eq!(days[0].entries[0].title.as_deref(), Some("Tree view"));
    assert_eq!(days[0].entries[1].title.as_deref(), Some("Board view"));
}

#[test]
fn feed_dedupes_by_event_id() {
    let beads = vec![bead("x", "X")];
    let e = event("dup", "x", "created", "k", "2026-11-16T08:00:00Z");
    let days = activity_feed(&beads, vec![e.clone(), e]);
    assert_eq!(days[0].entries.len(), 1);
}

#[test]
fn feed_keeps_unknown_issues() {
    // An event for a bead no longer in the list still shows.
    let days = activity_feed(&[], vec![event("e1", "gone", "closed", "k", "2026-11-16T08:00:00Z")]);
    assert_eq!(days.len(), 1);
    assert!(days[0].entries[0].title.is_none());
}

#[test]
fn stamps_split() {
    assert_eq!(day_of("2026-11-16T07:01:32Z"), "2026-11-16");
    assert_eq!(time_of("2026-11-16T07:01:32Z"), "07:01");
    // Unparseable stamps pass through instead of vanishing.
    assert_eq!(day_of("garbage"), "garbage");
    assert_eq!(time_of("garbage"), "garbage");
}

#[test]
fn labels_map_kinds() {
    assert_eq!(event_label("created"), "created");
    assert_eq!(event_label("claimed"), "claimed");
    assert_eq!(event_label("closed"), "closed");
    assert_eq!(event_label("label_added"), "labeled");
    assert_eq!(event_label("dependency_added"), "dependency added");
    assert_eq!(event_label("mystery_event"), "mystery_event");
}

fn stamped(id: &str, updated_at: &str) -> Bead {
    serde_json::from_value(json!({
        "id": id,
        "title": id,
        "status": "open",
        "priority": 2,
        "issue_type": "task",
        "updated_at": updated_at,
    }))
    .unwrap()
}

#[test]
fn cache_refetches_only_new_or_changed_beads() {
    use bdui::activity::HistoryCache;
    let mut cache = HistoryCache::default();
    let beads = vec![stamped("a", "t1"), stamped("b", "t1")];
    assert_eq!(cache.stale(&beads).len(), 2, "cold cache fetches everything");

    cache.insert("a".into(), "t1".into(), vec![event("e1", "a", "created", "k", "t1")]);
    cache.insert("b".into(), "t1".into(), vec![]);
    assert!(cache.stale(&beads).is_empty(), "unchanged stamps are memo hits");

    // Bead b moved on; a is untouched.
    let moved = vec![stamped("a", "t1"), stamped("b", "t2")];
    assert_eq!(cache.stale(&moved), vec![("b".to_string(), "t2".to_string())]);
}

#[test]
fn cache_prunes_departed_beads() {
    use bdui::activity::HistoryCache;
    let mut cache = HistoryCache::default();
    cache.insert("a".into(), "t1".into(), vec![event("e1", "a", "created", "k", "t1")]);
    cache.insert("gone".into(), "t1".into(), vec![event("e2", "gone", "created", "k", "t1")]);
    cache.prune(&[stamped("a", "t1")]);
    let events = cache.events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].issue_id, "a");
}

// S2: history transport against the real `bd` CLI.
#[test]
fn history_events_from_real_bd() {
    use bdui::client::{BeadsClient, CliClient};
    use std::process::Command;

    let dir = tempfile::tempdir().expect("tempdir");
    let run = |args: &[&str]| {
        let out = Command::new("bd")
            .args(args)
            .current_dir(dir.path())
            .output()
            .expect("run bd");
        assert!(
            out.status.success(),
            "bd {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
        out
    };
    run(&["init"]);
    let id = String::from_utf8(run(&["q", "Activity target"]).stdout)
        .expect("utf8")
        .trim()
        .to_string();
    run(&["update", &id, "--claim"]);
    let client = CliClient::in_dir(dir.path());

    let events = client.history(&id).expect("history");
    assert!(
        events.iter().any(|e| e.kind == "created"),
        "creation is journaled"
    );
    assert!(
        events.iter().any(|e| e.kind == "claimed"),
        "claim is journaled"
    );
    // Every event names its bead and an actor.
    assert!(events.iter().all(|e| e.issue_id == id));
    assert!(events.iter().all(|e| !e.actor.is_empty()));

    // Feed over the workspace: both events land on one day. Same-second
    // ties have no reliable order (bd's history output is not ordered),
    // so this asserts membership, not sequence.
    let beads = client.list().expect("list");
    let days = activity_feed(&beads, events);
    assert_eq!(days.len(), 1);
    assert_eq!(days[0].entries.len(), 2);
    let kinds: Vec<&str> = days[0].entries.iter().map(|e| e.event.kind.as_str()).collect();
    assert!(kinds.contains(&"created"));
    assert!(kinds.contains(&"claimed"));
}
