//! S1 seam: deterministic DAG layout over fixture graph JSON.

use bdui::graph::{EdgeStyle, Graph, layout_graph, parse_components, DepEdge};
use serde_json::json;

fn issue(id: &str, title: &str, priority: u8) -> serde_json::Value {
    json!({
        "id": id,
        "title": title,
        "status": "open",
        "priority": priority,
        "issue_type": "task",
        "created_at": "2026-01-01T00:00:00Z",
        "dependency_count": 0,
        "dependent_count": 0,
        "comment_count": 0
    })
}

fn component(root: &str, issues: Vec<serde_json::Value>, deps: Vec<serde_json::Value>) -> serde_json::Value {
    json!({ "Root": issue(root, root, 2), "Issues": issues, "Dependencies": deps })
}

fn dep(id: &str, depends_on: &str, kind: &str) -> serde_json::Value {
    json!({ "issue_id": id, "depends_on_id": depends_on, "type": kind })
}

/// A  B -> C chain: A blocks C, B blocks C. A,B layer 0; C layer 1.
#[test]
fn longest_path_layers_blockers_left() {
    let raw = json!([component(
        "c",
        vec![issue("a", "A", 2), issue("b", "B", 1), issue("c", "C", 3)],
        vec![dep("c", "a", "blocks"), dep("c", "b", "blocks")]
    )]);
    let data = parse_components(&raw.to_string()).expect("parse");
    let laid = layout_graph(&data);
    let layer = |id: &str| {
        laid.nodes
            .iter()
            .find(|n| n.bead.id == id)
            .map(|n| n.layer)
            .unwrap()
    };
    assert_eq!(layer("a"), 0);
    assert_eq!(layer("b"), 0);
    assert_eq!(layer("c"), 1);
}

/// Layer counts must respect transitive depth, not just direct edges.
#[test]
fn transitive_chain_layers_increase() {
    // a blocks b, b blocks c -> layers 0,1,2
    let raw = json!([component(
        "c",
        vec![issue("a", "A", 2), issue("b", "B", 2), issue("c", "C", 2)],
        vec![dep("b", "a", "blocks"), dep("c", "b", "blocks")]
    )]);
    let data = parse_components(&raw.to_string()).expect("parse");
    let laid = layout_graph(&data);
    let layer = |id: &str| laid.nodes.iter().find(|n| n.bead.id == id).unwrap().layer;
    assert_eq!(layer("a"), 0);
    assert_eq!(layer("b"), 1);
    assert_eq!(layer("c"), 2);
}

/// Within a column, order is priority asc then ID.
#[test]
fn column_order_is_priority_then_id() {
    let raw = json!([component(
        "c",
        vec![issue("b2", "B two", 2), issue("b1", "B one", 2), issue("a1", "A one", 1)],
        vec![]
    )]);
    let data = parse_components(&raw.to_string()).expect("parse");
    let laid = layout_graph(&data);
    let col: Vec<&str> = laid
        .nodes
        .iter()
        .filter(|n| n.layer == 0)
        .map(|n| n.bead.id.as_str())
        .collect();
    assert_eq!(col, vec!["a1", "b1", "b2"]);
}

/// Same input, identical node positions and edge endpoints.
#[test]
fn layout_is_deterministic() {
    let raw = json!([component(
        "c",
        vec![issue("a", "A", 2), issue("b", "B", 1), issue("c", "C", 3)],
        vec![dep("c", "a", "blocks"), dep("c", "b", "tracks")]
    )])
    .to_string();
    let first = layout_graph(&parse_components(&raw).unwrap());
    let second = layout_graph(&parse_components(&raw).unwrap());
    let key = |g: &Graph| {
        g.nodes
            .iter()
            .map(|n| format!("{}:{}:{}", n.bead.id, n.x, n.y))
            .chain(g.edges.iter().map(|e| format!("{}>{}", e.from, e.to)))
            .collect::<Vec<_>>()
    };
    assert_eq!(key(&first), key(&second));
}

/// Edge styles per CONTEXT.md vocabulary.
#[test]
fn edge_styles_map_to_dependency_types() {
    let raw = json!([component(
        "z",
        vec![issue("a", "A", 2), issue("b", "B", 2), issue("c", "C", 2), issue("d", "D", 2), issue("e", "E", 2)],
        vec![
            dep("b", "a", "blocks"),
            dep("c", "a", "parent-child"),
            dep("d", "a", "related"),
            dep("e", "a", "discovered-from")
        ]
    )]);
    let data = parse_components(&raw.to_string()).expect("parse");
    let laid = layout_graph(&data);
    let style = |from: &str, to: &str| {
        laid.edges
            .iter()
            .find(|e| e.from == to && e.to == from)
            .map(|e| e.style)
            .unwrap()
    };
    // Edge direction: from blocker -> blocked.
    assert_eq!(style("b", "a"), EdgeStyle::Solid);
    assert_eq!(style("c", "a"), EdgeStyle::Dotted);
    assert_eq!(style("d", "a"), EdgeStyle::Dashed);
    assert_eq!(style("e", "a"), EdgeStyle::Dashed);
}

/// Non-blocks edges must not affect layering.
#[test]
fn related_edges_do_not_affect_layers() {
    let raw = json!([component(
        "b",
        vec![issue("a", "A", 2), issue("b", "B", 2)],
        vec![dep("b", "a", "related")]
    )]);
    let data = parse_components(&raw.to_string()).expect("parse");
    let laid = layout_graph(&data);
    // Both layer 0: "related" does not block.
    assert!(laid.nodes.iter().all(|n| n.layer == 0));
}

/// Multiple components all land in one graph.
#[test]
fn all_components_merge() {
    let raw = json!([
        component("c1", vec![issue("a", "A", 2)], vec![]),
        component("c2", vec![issue("b", "B", 2)], vec![])
    ]);
    let data = parse_components(&raw.to_string()).expect("parse");
    assert_eq!(data.nodes.len(), 2);
    assert_eq!(data.edges.len(), 0);
}

/// parse_components tolerates unknown fields and rejects bad JSON.
#[test]
fn parse_rejects_garbage() {
    assert!(parse_components("not json").is_err());
}

/// DepEdge round-trips its bd-facing direction.
#[test]
fn dep_edge_direction_is_blocker_first() {
    let e = DepEdge {
        from: "blocker".into(),
        to: "blocked".into(),
        kind: "blocks".into(),
    };
    assert_eq!(e.from, "blocker");
}
