//! S1: board column assembly (bdui-rs-nx8).

use bdui::board::{board_columns, derive_cards, state_label};
use bdui::model::{Bead, State};
use bdui::tree::Filters;
use serde_json::json;

fn bead(id: &str, title: &str, status: &str, priority: u8) -> Bead {
    serde_json::from_value(json!({
        "id": id,
        "title": title,
        "status": status,
        "priority": priority,
        "issue_type": "task",
    }))
    .expect("valid bead json")
}

fn blocked_by(mut b: Bead, blocker: &str) -> Bead {
    b.raw_deps.push(bdui::graph::RawDep {
        issue_id: b.id.clone(),
        depends_on_id: blocker.to_string(),
        kind: "blocks".into(),
    });
    b
}

fn child_of(mut b: Bead, parent: &str) -> Bead {
    b.raw_deps.push(bdui::graph::RawDep {
        issue_id: b.id.clone(),
        depends_on_id: parent.to_string(),
        kind: "parent-child".into(),
    });
    b
}

fn derive(mut beads: Vec<Bead>) -> Vec<Bead> {
    bdui::model::derive_state(&mut beads);
    beads
}

#[test]
fn cards_derive_open_blockers() {
    let beads = derive(vec![
        blocked_by(bead("x", "X", "open", 2), "open-b"),
        blocked_by(bead("y", "Y", "open", 2), "closed-b"),
        bead("open-b", "Open blocker", "open", 2),
        bead("closed-b", "Closed blocker", "closed", 2),
    ]);
    let cards = derive_cards(&beads);
    let x = cards.iter().find(|c| c.bead.id == "x").unwrap();
    let y = cards.iter().find(|c| c.bead.id == "y").unwrap();
    // Only the open blocker is listed as blocking.
    assert_eq!(x.open_blockers, vec!["open-b".to_string()]);
    assert!(y.open_blockers.is_empty(), "closed blockers do not count");
    // open-b unblocks x; closed-b unblocks nobody while closed.
    let ob = cards.iter().find(|c| c.bead.id == "open-b").unwrap();
    assert_eq!(ob.unblocks, 1);
    let cb = cards.iter().find(|c| c.bead.id == "closed-b").unwrap();
    assert_eq!(cb.unblocks, 0);
}

#[test]
fn epic_parents_are_excluded() {
    let mut epic = bead("epic", "E", "open", 2);
    epic.issue_type = "epic".into();
    let beads = derive(vec![
        epic.clone(),
        child_of(bead("c1", "C1", "open", 2), "epic"),
    ]);
    let cards = derive_cards(&beads);
    assert!(cards.iter().all(|c| c.bead.id != "epic"));
    assert!(cards.iter().any(|c| c.bead.id == "c1"));
    // A childless epic stays on the board.
    let mut loner = bead("loner", "L", "open", 2);
    loner.issue_type = "epic".into();
    let cards = derive_cards(&derive(vec![loner]));
    assert_eq!(cards.len(), 1);
}

#[test]
fn columns_by_state_with_labels() {
    let beads = derive(vec![
        bead("r", "Ready card", "open", 2),
        bead("p", "WIP", "in_progress", 2),
        bead("b", "Blocked", "blocked", 2),
        bead("d", "Deferred", "deferred", 2),
        bead("c", "Done", "closed", 2),
    ]);
    let f = Filters { show_closed: true, ..Filters::default() };
    let cols = board_columns(&beads, &f);
    let labels: Vec<String> = cols.iter().map(|c| c.label.clone()).collect();
    assert_eq!(
        labels,
        vec!["Ready", "In progress", "Blocked", "Deferred", "Closed"]
    );
    for col in &cols {
        assert_eq!(col.cards.len(), 1, "{} column", col.label);
    }
}

#[test]
fn deferred_and_closed_columns_appear_conditionally() {
    let beads = derive(vec![bead("r", "Ready", "open", 2)]);
    let f = Filters { show_closed: true, ..Filters::default() };
    let cols = board_columns(&beads, &f);
    assert_eq!(cols.len(), 3, "base columns only");

    let f = Filters::default();
    let beads2 = derive(vec![
        bead("r", "Ready", "open", 2),
        bead("c", "Done", "closed", 2),
    ]);
    let cols = board_columns(&beads2, &f);
    assert!(!cols.iter().any(|c| c.state == State::Closed));
}

#[test]
fn filter_gates_cards() {
    let beads = derive(vec![
        bead("a", "Alpha infra", "open", 2),
        bead("b", "Beta", "open", 2),
    ]);
    let f = Filters {
        query: "infra".into(),
        ..Filters::default()
    };
    let cols = board_columns(&beads, &f);
    let ready = &cols[0];
    assert_eq!(ready.cards.len(), 1);
    assert_eq!(ready.cards[0].bead.id, "a");
}

#[test]
fn order_priority_then_unblocks() {
    let beads = derive(vec![
        bead("a", "A", "open", 2),
        blocked_by(bead("x", "X", "open", 2), "a"),
        blocked_by(bead("y", "Y", "open", 2), "a"),
        bead("p0", "P0 card", "open", 0),
    ]);
    let cols = board_columns(&beads, &Filters::default());
    let ids: Vec<&str> =
        cols[0].cards.iter().map(|c| c.bead.id.as_str()).collect();
    // P0 first; then a (unblocks 2) before x/y (unblocks 0).
    assert_eq!(ids, vec!["p0", "a", "x", "y"]);
}

#[test]
fn closed_column_capped_and_recent_first() {
    let mut beads = Vec::new();
    for (i, ts) in (0..40).map(|i| (i, format!("2026-01-01T{:02}:00:00Z", i))) {
        let mut b = bead(&format!("c{i}"), "Closed", "closed", 2);
        b.updated_at = ts;
        beads.push(b);
    }
    let beads = derive(beads);
    let f = Filters { show_closed: true, ..Filters::default() };
    let cols = board_columns(&beads, &f);
    let closed = cols.iter().find(|c| c.state == State::Closed).unwrap();
    assert_eq!(closed.cards.len(), 30, "prototype caps closed at 30");
    // Most recently updated first: c39 has the latest timestamp.
    assert_eq!(closed.cards[0].bead.id, "c39");
}

#[test]
fn labels_cover_states() {
    assert_eq!(state_label(&State::Ready), "Ready");
    assert_eq!(state_label(&State::InProgress), "In progress");
    assert_eq!(state_label(&State::Blocked), "Blocked");
    assert_eq!(state_label(&State::Deferred), "Deferred");
    assert_eq!(state_label(&State::Closed), "Closed");
}
