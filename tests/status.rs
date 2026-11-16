#![cfg(test)]
//! S1: statuses outside the five mapped ones keep their identity
//! (bdvw-rs drawer/board status handling). bd built-ins `pinned` and
//! `hooked`, plus `bd config status.custom` values, must not read as Ready.

use bdvw::board::{board_columns, state_label};
use bdvw::model::{parse_summary, Bead, State};
use bdvw::ready::{tab_count, tab_rows, ReadyTab};

fn fixture() -> &'static str {
    r#"[
      {"id":"bd-s1","title":"Plain open","status":"open","priority":2,"issue_type":"task",
       "created_at":"2026-11-16T02:18:58Z","updated_at":"2026-11-16T02:18:58Z"},
      {"id":"bd-s2","title":"Pinned bead","status":"pinned","priority":1,"issue_type":"task",
       "created_at":"2026-11-16T02:18:58Z","updated_at":"2026-11-16T02:18:58Z"},
      {"id":"bd-s3","title":"Hooked bead","status":"hooked","priority":3,"issue_type":"task",
       "created_at":"2026-11-16T02:18:58Z","updated_at":"2026-11-16T02:18:58Z"},
      {"id":"bd-s4","title":"Custom triage","status":"triage","priority":2,"issue_type":"task",
       "created_at":"2026-11-16T02:18:58Z","updated_at":"2026-11-16T02:18:58Z"},
      {"id":"bd-s5","title":"Deferred bead","status":"deferred","priority":2,"issue_type":"task",
       "created_at":"2026-11-16T02:18:58Z","updated_at":"2026-11-16T02:18:58Z"},
      {"id":"bd-s6","title":"Closed bead","status":"closed","priority":2,"issue_type":"task",
       "created_at":"2026-11-16T02:18:58Z","updated_at":"2026-11-16T02:18:58Z"}
    ]"#
}

fn beads() -> Vec<Bead> {
    parse_summary(fixture()).expect("fixture parses")
}

#[test]
fn unmapped_statuses_keep_their_text() {
    let beads = beads();
    let by = |id: &str| beads.iter().find(|b| b.id == id).unwrap().state.clone();
    assert_eq!(by("bd-s1"), State::Ready);
    assert_eq!(by("bd-s2"), State::Custom("pinned".into()));
    assert_eq!(by("bd-s3"), State::Custom("hooked".into()));
    assert_eq!(by("bd-s4"), State::Custom("triage".into()));
    // Mapped statuses are untouched by the fallback.
    assert_eq!(by("bd-s5"), State::Deferred);
    assert_eq!(by("bd-s6"), State::Closed);
}

#[test]
fn custom_states_display_and_label_raw_status() {
    let custom = State::Custom("pinned".into());
    assert_eq!(custom.to_string(), "pinned");
    assert_eq!(state_label(&custom), "pinned");
    assert_eq!(state_label(&State::InProgress), "In progress");
}

#[test]
fn ready_tab_includes_custom_without_calling_them_ready() {
    let beads = beads();
    // Ready tab counts the open bead plus the three unmapped ones.
    assert_eq!(tab_count(&beads, ReadyTab::Ready), 4);
    let rows = tab_rows(&beads, ReadyTab::Ready);
    let ids: Vec<&str> = rows.iter().map(|b| b.id.as_str()).collect();
    // Sorted by priority: pinned P1, open P2, triage P2, hooked P3.
    assert_eq!(ids, ["bd-s2", "bd-s1", "bd-s4", "bd-s3"]);
    // Their states stay distinct; only the tab membership widened.
    assert_eq!(rows[0].state, State::Custom("pinned".into()));
}

#[test]
fn board_gives_each_custom_status_its_own_column() {
    let cols = board_columns(&beads(), &Default::default());
    let labels: Vec<String> = cols.iter().map(|c| c.label.clone()).collect();
    // Mapped columns first, then custom in first-seen order. Closed is
    // hidden by default filters, so it is absent here.
    assert_eq!(
        labels,
        [
            "Ready",
            "In progress",
            "Blocked",
            "Deferred",
            "pinned",
            "hooked",
            "triage"
        ]
    );
    let ready = cols.iter().find(|c| c.state == State::Ready).unwrap();
    assert_eq!(
        ready
            .cards
            .iter()
            .map(|c| c.bead.id.as_str())
            .collect::<Vec<_>>(),
        ["bd-s1"],
        "custom beads must not land in the Ready column"
    );
    let pinned = cols.iter().find(|c| c.label == "pinned").unwrap();
    assert_eq!(pinned.cards.len(), 1);
    assert_eq!(pinned.cards[0].bead.id, "bd-s2");
}

#[test]
fn closed_column_trails_custom_columns_when_shown() {
    let filters = bdvw::tree::Filters {
        show_closed: true,
        ..Default::default()
    };
    let cols = board_columns(&beads(), &filters);
    let labels: Vec<String> = cols.iter().map(|c| c.label.clone()).collect();
    assert_eq!(
        labels,
        [
            "Ready",
            "In progress",
            "Blocked",
            "Deferred",
            "pinned",
            "hooked",
            "triage",
            "Closed"
        ]
    );
}
