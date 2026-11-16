//! S1: Ready view tabs and suggested-next (pure logic).

use bdui::model::{derive_state, Bead};
use bdui::ready::{suggested_next, tab_count, tab_rows, ReadyTab};
use serde_json::json;

fn bead(id: &str, status: &str, priority: u8, updated_at: &str) -> Bead {
    serde_json::from_value(json!({
        "id": id,
        "title": id,
        "status": status,
        "priority": priority,
        "issue_type": "task",
        "updated_at": updated_at,
    }))
    .expect("valid bead json")
}

fn derived(mut beads: Vec<Bead>) -> Vec<Bead> {
    derive_state(&mut beads);
    beads
}

fn ids(beads: &[Bead]) -> Vec<&str> {
    beads.iter().map(|b| b.id.as_str()).collect()
}

#[test]
fn tab_rows_keep_only_the_tab_state_in_ready_rank_order() {
    let beads = derived(vec![
        bead("low", "open", 2, "2026-01-01T00:00:00Z"),
        bead("high", "open", 0, "2026-01-01T00:00:00Z"),
        bead("running", "in_progress", 1, "2026-01-01T00:00:00Z"),
    ]);

    assert_eq!(ids(&tab_rows(&beads, ReadyTab::Ready)), ["high", "low"]);
    assert_eq!(ids(&tab_rows(&beads, ReadyTab::InProgress)), ["running"]);
    assert!(tab_rows(&beads, ReadyTab::Blocked).is_empty());
}

#[test]
fn tab_count_matches_tab_rows_for_every_tab() {
    let beads = derived(vec![
        bead("a", "open", 1, "2026-01-01T00:00:00Z"),
        bead("b", "in_progress", 1, "2026-01-01T00:00:00Z"),
        bead("c", "blocked", 1, "2026-01-01T00:00:00Z"),
        bead("d", "closed", 1, "2026-01-01T00:00:00Z"),
        bead("e", "closed", 1, "2026-01-01T00:00:00Z"),
    ]);

    for tab in ReadyTab::ALL {
        assert_eq!(tab_count(&beads, tab), tab_rows(&beads, tab).len(), "{tab:?}");
    }
    assert_eq!(tab_count(&beads, ReadyTab::Closed), 2);
}

#[test]
fn closed_tab_orders_most_recently_updated_first() {
    let beads = derived(vec![
        bead("older", "closed", 0, "2026-01-01T00:00:00Z"),
        bead("newer", "closed", 4, "2026-02-01T00:00:00Z"),
    ]);

    assert_eq!(ids(&tab_rows(&beads, ReadyTab::Closed)), ["newer", "older"]);
}

#[test]
fn suggested_next_is_the_top_ready_row() {
    let beads = derived(vec![
        bead("low", "open", 3, "2026-01-01T00:00:00Z"),
        bead("top", "open", 0, "2026-01-01T00:00:00Z"),
    ]);
    let ready = tab_rows(&beads, ReadyTab::Ready);

    assert_eq!(suggested_next(&ready).map(|b| b.id.as_str()), Some("top"));
}

#[test]
fn suggested_next_is_none_when_nothing_is_ready() {
    let beads = derived(vec![bead("busy", "in_progress", 0, "2026-01-01T00:00:00Z")]);
    let ready = tab_rows(&beads, ReadyTab::Ready);

    assert!(suggested_next(&ready).is_none());
}
