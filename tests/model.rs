#![cfg(test)]
//! Seam S1: parse + derive over fixture JSON captured from `bd list --json`.
//! Expected states are hand-labeled, not recomputed.

use bdui::model::{parse_summary, Bead, State};

/// Fixture: recorded shape of `bd list --json` rows (verified against a
/// real workspace, 2026-11-16). Five beads covering every state.
fn fixture() -> &'static str {
    r#"[
      {"id":"bd-w1","title":"Plain open bead","status":"open","priority":2,"issue_type":"task",
       "created_at":"2026-11-16T02:18:58Z","updated_at":"2026-11-16T02:18:58Z",
       "dependency_count":1,"dependent_count":0,"comment_count":0},
      {"id":"bd-w2","title":"In progress bead","status":"in_progress","priority":1,"issue_type":"task",
       "created_at":"2026-11-16T02:18:58Z","updated_at":"2026-11-16T02:18:58Z",
       "dependency_count":0,"dependent_count":1,"comment_count":0},
      {"id":"bd-w3","title":"Blocked storage status","status":"blocked","priority":0,"issue_type":"bug",
       "created_at":"2026-11-16T02:18:58Z","updated_at":"2026-11-16T02:18:58Z",
       "dependency_count":2,"dependent_count":0,"comment_count":0},
      {"id":"bd-w4","title":"Deferred bead","status":"deferred","priority":3,"issue_type":"task",
       "created_at":"2026-11-16T02:18:58Z","updated_at":"2026-11-16T02:18:58Z",
       "dependency_count":0,"dependent_count":0,"comment_count":0},
      {"id":"bd-w5","title":"Closed bead","status":"closed","priority":2,"issue_type":"chore",
       "created_at":"2026-11-16T02:18:58Z","updated_at":"2026-11-16T02:18:58Z",
       "dependency_count":0,"dependent_count":1,"comment_count":0}
    ]"#
}

fn parse() -> Vec<Bead> {
    parse_summary(fixture()).expect("fixture parses")
}

#[test]
fn states_follow_from_status() {
    let beads = parse();
    let by = |id: &str| beads.iter().find(|b| b.id == id).unwrap();
    // Hand-labeled expectations, one per stored status.
    assert_eq!(by("bd-w1").state, State::Ready, "open with no blockers is ready");
    assert_eq!(by("bd-w2").state, State::InProgress);
    assert_eq!(by("bd-w3").state, State::Blocked);
    assert_eq!(by("bd-w4").state, State::Deferred);
    assert_eq!(by("bd-w5").state, State::Closed);
}

#[test]
fn ready_ranking_is_priority_then_id() {
    let mut beads = parse();
    beads.sort_by(Bead::ready_rank);
    // P0 blocked excluded: ranking applies to ready beads only.
    let ready: Vec<&str> = beads
        .iter()
        .filter(|b| b.state == State::Ready)
        .map(|b| b.id.as_str())
        .collect();
    assert_eq!(ready, ["bd-w1"]);
}
