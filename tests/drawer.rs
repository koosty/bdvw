//! S1: DetailSlot load state (bdui-rs-A2). The drawer's async loads are
//! applied through this pure slot, so token rules are tested here.

use bdui::drawer::DetailSlot;
use bdui::model::{parse_detail, BeadDetail};

fn detail(id: &str) -> BeadDetail {
    parse_detail(&format!(
        r#"[{{"id":"{id}","title":"T {id}","status":"open","priority":2,"issue_type":"task"}}]"#
    ))
    .expect("fixture parses")
}

#[test]
fn failed_reload_keeps_last_good_detail() {
    let mut slot = DetailSlot::default();
    let s1 = slot.begin("a");
    assert!(slot.apply(s1, Ok(detail("a"))));

    let (id, s2) = slot.begin_refresh().expect("selection exists");
    assert_eq!(id, "a");
    assert!(slot.apply(s2, Err("bd show failed".into())));

    assert_eq!(slot.detail().map(|d| d.id.as_str()), Some("a"));
    assert_eq!(slot.error(), Some("bd show failed"));
}

#[test]
fn successful_reload_clears_error() {
    let mut slot = DetailSlot::default();
    let s1 = slot.begin("a");
    slot.apply(s1, Err("boom".into()));
    let (_, s2) = slot.begin_refresh().expect("selection exists");
    assert!(slot.apply(s2, Ok(detail("a"))));
    assert_eq!(slot.error(), None);
}

#[test]
fn superseded_result_is_discarded() {
    let mut slot = DetailSlot::default();
    let s_a = slot.begin("a");
    let _s_b = slot.begin("b");
    assert!(!slot.apply(s_a, Ok(detail("a"))), "a's load finished after b was chosen");
    assert_eq!(slot.selected(), Some("b"));
    assert!(slot.detail().is_none());
}

#[test]
fn reselecting_same_bead_keeps_detail_while_reloading() {
    let mut slot = DetailSlot::default();
    let s1 = slot.begin("a");
    slot.apply(s1, Ok(detail("a")));
    slot.begin("a");
    assert_eq!(slot.detail().map(|d| d.id.as_str()), Some("a"));
}

#[test]
fn switching_beads_drops_previous_detail() {
    let mut slot = DetailSlot::default();
    let s1 = slot.begin("a");
    slot.apply(s1, Ok(detail("a")));
    slot.begin("b");
    assert!(slot.detail().is_none());
}

#[test]
fn close_invalidates_in_flight_load() {
    let mut slot = DetailSlot::default();
    let s1 = slot.begin("a");
    slot.close();
    assert!(!slot.apply(s1, Ok(detail("a"))));
    assert_eq!(slot.selected(), None);
    assert!(slot.detail().is_none());
}
