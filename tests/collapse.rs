//! S1 seam: epic collapse over fixture graph data — subtree hiding, edge
//! contraction to the epic, child counts.

use bdvw::graph::{collapse_graph, parse_components};
use serde_json::json;
use std::collections::HashSet;

fn issue(id: &str, title: &str, issue_type: &str) -> serde_json::Value {
    json!({
        "id": id,
        "title": title,
        "status": "open",
        "priority": 2,
        "issue_type": issue_type,
        "created_at": "2026-01-01T00:00:00Z",
        "dependency_count": 0,
        "dependent_count": 0,
        "comment_count": 0
    })
}

fn component(issues: Vec<serde_json::Value>, deps: Vec<serde_json::Value>) -> serde_json::Value {
    json!({ "Issues": issues, "Dependencies": deps })
}

fn dep(id: &str, depends_on: &str, kind: &str) -> serde_json::Value {
    json!({ "issue_id": id, "depends_on_id": depends_on, "type": kind })
}

fn data(issues: Vec<serde_json::Value>, deps: Vec<serde_json::Value>) -> bdvw::graph::GraphData {
    parse_components(&json!([component(issues, deps)]).to_string()).expect("parse")
}

/// epic P has children c1, c2; c2 blocks task t.
fn epic_fixture() -> bdvw::graph::GraphData {
    data(
        vec![
            issue("p", "Epic", "epic"),
            issue("c1", "Child one", "task"),
            issue("c2", "Child two", "task"),
            issue("t", "Task", "task"),
        ],
        vec![
            dep("c1", "p", "parent-child"),
            dep("c2", "p", "parent-child"),
            dep("t", "c2", "blocks"),
        ],
    )
}

#[test]
fn collapse_hides_subtree_and_counts_children() {
    let d = epic_fixture();
    let collapsed: HashSet<String> = ["p".into()].into();
    let out = collapse_graph(&d, &collapsed);
    assert!(!out.nodes.iter().any(|n| n.bead.id == "c1"));
    assert!(!out.nodes.iter().any(|n| n.bead.id == "c2"));
    assert!(out.nodes.iter().any(|n| n.bead.id == "p"));
    assert!(out.nodes.iter().any(|n| n.bead.id == "t"));
    // Child count attaches to the epic.
    assert_eq!(
        out.nodes
            .iter()
            .find(|n| n.bead.id == "p")
            .unwrap()
            .child_count,
        Some(2)
    );
}

#[test]
fn collapse_contracts_outside_edges_to_epic() {
    let d = epic_fixture();
    let collapsed: HashSet<String> = ["p".into()].into();
    let out = collapse_graph(&d, &collapsed);
    // t->c2 blocks edge becomes t->p.
    assert!(!out
        .edges
        .iter()
        .any(|e| e.from == "c2" && e.to == "t" && e.kind == "blocks"));
    // The child's outside edge is re-anchored: t is now blocked by the epic.
    let rewritten: Vec<&bdvw::graph::DepEdge> = out.edges.iter().filter(|e| e.to == "t").collect();
    assert_eq!(rewritten.len(), 1, "t has exactly one incident edge");
    assert_eq!(rewritten[0].from, "p");
    assert_eq!(rewritten[0].kind, "blocks");
}

#[test]
fn collapse_hides_internal_edges() {
    // Two epics each with a child, children linked.
    let d = data(
        vec![
            issue("e1", "Epic one", "epic"),
            issue("k1", "Kid one", "task"),
            issue("e2", "Epic two", "epic"),
            issue("k2", "Kid two", "task"),
        ],
        vec![
            dep("k1", "e1", "parent-child"),
            dep("k2", "e2", "parent-child"),
            dep("k2", "k1", "related"),
        ],
    );
    let collapsed: HashSet<String> = ["e1".into()].into();
    let out = collapse_graph(&d, &collapsed);
    // k1 hidden: the related edge k1-k2 attaches to e1 instead.
    assert!(!out.nodes.iter().any(|n| n.bead.id == "k1"));
    assert!(out.edges.iter().any(|e| e.from == "e1" && e.to == "k2"));
    // e2 subtree untouched.
    assert!(out.nodes.iter().any(|n| n.bead.id == "k2"));
}

#[test]
fn nested_subtree_collapses_fully() {
    // p -> c1 -> gc1 parent chain; collapse p hides gc1 too.
    let d = data(
        vec![
            issue("p", "Epic", "epic"),
            issue("c1", "Child", "task"),
            issue("gc1", "Grandchild", "task"),
            issue("t", "Task", "task"),
        ],
        vec![
            dep("c1", "p", "parent-child"),
            dep("gc1", "c1", "parent-child"),
        ],
    );
    let collapsed: HashSet<String> = ["p".into()].into();
    let out = collapse_graph(&d, &collapsed);
    assert!(!out.nodes.iter().any(|n| n.bead.id == "c1"));
    assert!(!out.nodes.iter().any(|n| n.bead.id == "gc1"));
    assert_eq!(
        out.nodes
            .iter()
            .find(|n| n.bead.id == "p")
            .unwrap()
            .child_count,
        Some(2),
        "count covers the whole hidden subtree"
    );
}

#[test]
fn empty_collapse_set_is_identity() {
    let d = epic_fixture();
    let out = collapse_graph(&d, &HashSet::new());
    assert_eq!(out.nodes.len(), d.nodes.len());
    assert_eq!(out.edges.len(), d.edges.len());
}

#[test]
fn collapsing_task_without_children_is_noop() {
    let d = epic_fixture();
    let collapsed: HashSet<String> = ["t".into()].into();
    let out = collapse_graph(&d, &collapsed);
    assert_eq!(out.nodes.len(), d.nodes.len(), "no children to hide");
    assert!(out.nodes.iter().all(|n| n.child_count.is_none()));
}

#[test]
fn expand_restores_children() {
    let d = epic_fixture();
    // Collapsing with an empty set is the expanded form of the same data.
    let restored = collapse_graph(&d, &HashSet::new());
    assert_eq!(restored.nodes.len(), d.nodes.len());
    assert!(restored.nodes.iter().all(|n| n.child_count.is_none()));
}

#[test]
fn two_collapsed_epics_both_count() {
    let d = data(
        vec![
            issue("e1", "Epic one", "epic"),
            issue("e2", "Epic two", "epic"),
            issue("k1", "Kid one", "task"),
            issue("k2", "Kid two", "task"),
        ],
        vec![
            dep("k1", "e1", "parent-child"),
            dep("k2", "e2", "parent-child"),
        ],
    );
    let collapsed: HashSet<String> = ["e1".into(), "e2".into()].into();
    let out = collapse_graph(&d, &collapsed);
    let count = |id: &str| {
        out.nodes
            .iter()
            .find(|n| n.bead.id == id)
            .unwrap()
            .child_count
            .unwrap()
    };
    assert_eq!(count("e1"), 1);
    assert_eq!(count("e2"), 1);
}
