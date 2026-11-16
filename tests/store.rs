//! S1: store diff + sync-label derivation (bdvw-rs-7lr).

use bdvw::model::Bead;
use serde_json::json;

fn bead(id: &str, status: &str) -> Bead {
    serde_json::from_value(json!({
        "id": id,
        "title": format!("T {id}"),
        "issue_type": "task",
        "status": status,
        "priority": 2,
    }))
    .expect("valid bead json")
}

#[test]
fn differs_detects_status_change() {
    let a = vec![bead("x", "open")];
    let b = vec![bead("x", "closed")];
    assert!(!bdvw::store::beads_differ(&a, &a));
    assert!(bdvw::store::beads_differ(&a, &b));
}

#[test]
fn differs_false_when_rank_sorted_identically() {
    // Both sides pass through the same ready_rank sort, so identical data
    // arrives in identical order and is not a change.
    let a = vec![bead("x", "open"), bead("y", "open")];
    let b = a.clone();
    assert!(!bdvw::store::beads_differ(&a, &b));
}

#[test]
fn differs_detects_add_remove_priority() {
    assert!(bdvw::store::beads_differ(&[bead("x", "open")], &[]));
    assert!(bdvw::store::beads_differ(&[], &[bead("x", "open")]));
    let mut p3 = bead("x", "open");
    p3.priority = 3;
    assert!(bdvw::store::beads_differ(&[bead("x", "open")], &[p3]));
}

#[test]
fn sync_label_stages() {
    use bdvw::store::Store;
    // Loading: no success yet.
    let loading = Store::sync_label_from(false, None, None);
    assert_eq!(loading, "loading…");
    // Error dominates recency.
    let error = Store::sync_label_from(true, Some("boom"), Some(std::time::Instant::now()));
    assert!(error.starts_with("error: "));
    // Live: recently synced shows "live · 0s"; 3s elapsed shows "live · 3s".
    let now = std::time::Instant::now();
    let fresh = Store::sync_label_from(true, None, Some(now));
    assert!(fresh.starts_with("live · 0s"));
    let stale = Store::sync_label_from(true, None, Some(now - std::time::Duration::from_secs(3)));
    assert_eq!(stale, "live · 3s");
}
