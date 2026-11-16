//! S1: tree hierarchy derivation, filters, and row assembly
//! (bdui-rs-8db).

use bdui::model::Bead;
use bdui::tree::{tree_rows, Filters, Hierarchy};
use serde_json::json;
use std::collections::HashSet;

fn bead_json(id: &str, title: &str, status: &str, priority: u8) -> serde_json::Value {
    json!({
        "id": id,
        "title": title,
        "status": status,
        "priority": priority,
        "issue_type": "task",
    })
}

fn bead(id: &str, title: &str, status: &str, priority: u8) -> Bead {
    serde_json::from_value(bead_json(id, title, status, priority)).unwrap()
}

/// Parent edge via the list row's inline dependencies array.
fn child_of(mut b: Bead, parent: &str) -> Bead {
    b.raw_deps = vec![bdui::graph::RawDep {
        issue_id: b.id.clone(),
        depends_on_id: parent.to_string(),
        kind: "parent-child".to_string(),
    }];
    b
}

fn derive(mut beads: Vec<Bead>) -> Vec<Bead> {
    bdui::model::derive_state(&mut beads);
    beads
}

#[test]
fn hierarchy_from_parent_child_edges() {
    let beads = derive(vec![
        bead("epic", "E", "open", 2),
        child_of(bead("c1", "C1", "open", 2), "epic"),
        child_of(bead("c1x", "C1x", "open", 2), "c1"),
    ]);
    let h = Hierarchy::from_beads(&beads);
    assert_eq!(h.parent_of("c1"), Some("epic"));
    assert_eq!(h.parent_of("c1x"), Some("c1"));
    assert_eq!(h.parent_of("epic"), None);
    assert_eq!(h.children_of("epic"), &["c1".to_string()]);
    assert_eq!(h.descendants_of("epic").len(), 2);
}

#[test]
fn hierarchy_id_suffix_fallback() {
    // No explicit edge; "abc.2" becomes a child of "abc" because the ID
    // prefix exists (prototype derive rule).
    let beads = derive(vec![
        bead("abc", "Root", "open", 2),
        bead("abc.2", "Kid", "open", 2),
        bead("zzz.9", "No parent", "open", 2),
    ]);
    let h = Hierarchy::from_beads(&beads);
    assert_eq!(h.parent_of("abc.2"), Some("abc"));
    assert_eq!(h.parent_of("zzz.9"), None, "prefix not in set");
}

#[test]
fn numeric_child_sort() {
    let mut kids: Vec<String> = ["a.10", "a.2", "a.1"].iter().map(|s| s.to_string()).collect();
    kids.sort_by(|a, b| bdui::tree::cmp_numeric_ids(a, b));
    assert_eq!(kids, vec!["a.1", "a.2", "a.10"]);
}

#[test]
fn rows_show_caret_and_progress() {
    let beads = derive(vec![
        bead("epic", "E", "open", 2),
        child_of(bead("c1", "C1", "closed", 2), "epic"),
        child_of(bead("c2", "C2", "open", 2), "epic"),
        child_of(bead("c3", "C3", "closed", 2), "epic"),
    ]);
    let h = Hierarchy::from_beads(&beads);
    let f = Filters { show_closed: true, ..Filters::default() };
    let rows = tree_rows(&beads, &h, &HashSet::new(), &f);
    assert_eq!(rows.len(), 4, "root + 3 children");
    assert_eq!(rows[0].bead.id, "epic");
    assert!(rows[0].has_children);
    assert!(rows[0].expanded);
    assert_eq!(rows[0].progress, Some((2, 3)), "two closed of three");
    assert_eq!(rows[1].depth, 1);
    assert!(!rows[1].has_children);
}

#[test]
fn collapse_hides_descendants() {
    let beads = derive(vec![
        bead("epic", "E", "open", 2),
        child_of(bead("c1", "C1", "open", 2), "epic"),
        child_of(bead("c1x", "C1x", "open", 2), "c1"),
    ]);
    let h = Hierarchy::from_beads(&beads);
    let collapsed: HashSet<String> = ["epic".to_string()].into();
    let rows = tree_rows(&beads, &h, &collapsed, &Filters::default());
    assert_eq!(rows.len(), 1, "only the root remains");
    assert!(rows[0].has_children);
    assert!(!rows[0].expanded, "caret shows collapsed");
    // Progress is still computed from hidden descendants.
    assert!(rows[0].progress.is_some());
}

#[test]
fn filter_keeps_ancestors_of_matches() {
    let mut labels = bead("epic", "E", "open", 2);
    labels.labels = vec!["infra".into()];
    let beads = derive(vec![
        labels,
        child_of(bead("c1", "C1", "open", 2), "epic"),
        child_of(bead("c2", "Has infra", "open", 2), "epic"),
    ]);
    let h = Hierarchy::from_beads(&beads);
    let filters = Filters { query: "infra".into(), ..Filters::default() };
    let rows = tree_rows(&beads, &h, &HashSet::new(), &filters);
    let ids: Vec<&str> = rows.iter().map(|r| r.bead.id.as_str()).collect();
    // c2 matches; its parent epic is kept as context; c1 is pruned.
    assert!(ids.contains(&"epic"));
    assert!(ids.contains(&"c2"));
    assert!(!ids.contains(&"c1"));
}

#[test]
fn filter_prunes_unmatched_roots() {
    let beads = derive(vec![
        bead("a", "Alpha", "open", 2),
        bead("b", "Beta", "open", 2),
    ]);
    let h = Hierarchy::from_beads(&beads);
    let filters = Filters {
        query: "beta".into(),
        ..Default::default()
    };
    let rows = tree_rows(&beads, &h, &HashSet::new(), &filters);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].bead.id, "b");
}

#[test]
fn closed_hidden_by_default() {
    let beads = derive(vec![
        bead("a", "Alpha", "closed", 2),
        bead("b", "Beta", "open", 2),
    ]);
    let h = Hierarchy::from_beads(&beads);
    let rows = tree_rows(&beads, &h, &HashSet::new(), &Filters::default());
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].bead.id, "b");

    let f = Filters { show_closed: true, ..Filters::default() };
    let rows = tree_rows(&beads, &h, &HashSet::new(), &f);
    assert_eq!(rows.len(), 2);
}

#[test]
fn assignee_and_priority_filters() {
    let mut a = bead("a", "Alpha", "open", 1);
    a.assignee = Some("koosty".into());
    let mut b = bead("b", "Beta", "open", 2);
    b.assignee = None;
    let beads = derive(vec![a, b]);
    let h = Hierarchy::from_beads(&beads);

    let f = Filters {
        assignee: Some(None),
        ..Default::default()
    };
    assert_eq!(tree_rows(&beads, &h, &HashSet::new(), &f).len(), 1);

    let f = Filters {
        priority: Some(1),
        ..Default::default()
    };
    let rows = tree_rows(&beads, &h, &HashSet::new(), &f);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].bead.id, "a");
}

#[test]
fn cycle_does_not_hang() {
    // Corrupt data: two beads claim each other as parents.
    let mut a = bead("a", "A", "open", 2);
    a.raw_deps = vec![bdui::graph::RawDep {
        issue_id: "a".into(),
        depends_on_id: "b".into(),
        kind: "parent-child".into(),
    }];
    let mut b = bead("b", "B", "open", 2);
    b.raw_deps = vec![bdui::graph::RawDep {
        issue_id: "b".into(),
        depends_on_id: "a".into(),
        kind: "parent-child".into(),
    }];
    let beads = derive(vec![a, b]);
    let h = Hierarchy::from_beads(&beads);
    // Must terminate and emit both as rows.
    let rows = tree_rows(&beads, &h, &HashSet::new(), &Filters::default());
    assert_eq!(rows.len(), 2);
}

// S2: hierarchy from the real `bd` CLI with parent-child links.
#[test]
fn hierarchy_from_real_bd_parent_child() {
    use bdui::client::BeadsClient;
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
    let epic = String::from_utf8(run(&["q", "Epic work", "--type", "epic"]).stdout)
        .expect("utf8")
        .trim()
        .to_string();
    let task = String::from_utf8(run(&["q", "Task under epic"]).stdout)
        .expect("utf8")
        .trim()
        .to_string();
    run(&["link", &task, &epic, "--type", "parent-child"]);

    let client = bdui::client::CliClient::in_dir(dir.path());
    let beads = client.list().expect("list");
    assert_eq!(beads.len(), 2);
    let h = Hierarchy::from_beads(&beads);
    assert_eq!(h.parent_of(&task), Some(epic.as_str()));
    assert_eq!(h.children_of(&epic), std::slice::from_ref(&task));

    // Rows: root epic, then the task under it, with progress 0/1.
    let rows = tree_rows(&beads, &h, &HashSet::new(), &Filters::default());
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].bead.id, epic);
    assert_eq!(rows[1].bead.id, task);
    assert_eq!(rows[1].depth, 1);
    assert_eq!(rows[0].progress, Some((0, 1)));
}
