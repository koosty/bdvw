//! Graph data model and deterministic layered layout.
//!
//! Source of truth: `bd graph --all --json` — one record per connected
//! component with full issue records and dependency edges. Layout is pure:
//! same input, same pixel positions (ticket bdui-rs-7zq).
//!
//! Direction semantics (CONTEXT.md): a dependency entry `issue_id` is
//! blocked by `depends_on_id`. Arrows point blocker -> blocked, matching
//! bd's own "execution order" rendering.

use crate::model::Bead;
use serde::Deserialize;

/// One raw dependency from bd: `issue_id` depends on `depends_on_id`.
#[derive(Debug, Clone, Deserialize)]
pub struct RawDep {
    pub issue_id: String,
    pub depends_on_id: String,
    #[serde(rename = "type")]
    pub kind: String,
}

/// One connected component from `bd graph --all --json`.
#[derive(Debug, Deserialize)]
pub struct Component {
    #[serde(rename = "Issues")]
    pub issues: Vec<Bead>,
    #[serde(rename = "Dependencies")]
    #[serde(default)]
    pub dependencies: Vec<RawDep>,
}

/// Edge rendered blocker -> blocked.
#[derive(Debug, Clone, PartialEq)]
pub struct DepEdge {
    pub from: String,
    pub to: String,
    pub kind: String,
}

/// Canvas line style per dependency type (CONTEXT.md edge vocabulary).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeStyle {
    /// blocks
    Solid,
    /// parent-child (fewer, denser dots)
    Dotted,
    /// related | tracks | discovered-from
    Dashed,
}

impl EdgeStyle {
    pub fn from_kind(kind: &str) -> EdgeStyle {
        match kind {
            "blocks" => EdgeStyle::Solid,
            "parent-child" => EdgeStyle::Dotted,
            _ => EdgeStyle::Dashed,
        }
    }
}

/// Direction the arrow relationship flows for layout purposes: only
/// `blocks` constrains execution order.
fn constrains(kind: &str) -> bool {
    kind == "blocks"
}

/// Parsed, merged graph: all components' nodes and edges.
#[derive(Debug, Clone, Default)]
pub struct GraphData {
    pub nodes: Vec<Bead>,
    pub edges: Vec<DepEdge>,
}

/// A graph node carrying collapse metadata: `child_count` is Some(n) on
/// epics whose subtree is currently hidden.
#[derive(Debug, Clone)]
pub struct CollapsedNode {
    pub bead: Bead,
    /// Hidden direct+descendant count when this epic is collapsed.
    pub child_count: Option<usize>,
}

/// Graph after epic collapse: hidden subtree nodes removed, their outside
/// edges rewritten to point at the epic root, epics tagged with child counts.
#[derive(Debug, Clone, Default)]
pub struct CollapsedGraph {
    pub nodes: Vec<CollapsedNode>,
    pub edges: Vec<DepEdge>,
}

/// Collapse every epic in `collapsed` (by id) into a single node.
///
/// - The hidden set is the full descendant closure over `parent-child`
///   edges pointing at a collapsed epic (nested children included).
/// - An edge between two hidden nodes disappears; an edge between a hidden
///   node and a visible one is re-anchored to the collapsed epic root.
///   Re-anchored `blocks` edges keep blocking the epic (bd's layered model
///   treats the epic as the unit of work).
/// - Collapsed epics get `child_count` = hidden-subtree size.
pub fn collapse_graph(data: &GraphData, collapsed: &std::collections::HashSet<String>) -> CollapsedGraph {
    // Descendents of each collapsed epic over parent-child edges:
    // dep entry "issue_id child depends_on_id parent" means child has parent.
    let mut children_of: std::collections::HashMap<&str, Vec<&str>> =
        std::collections::HashMap::new();
    for e in &data.edges {
        if e.kind == "parent-child" {
            children_of.entry(e.from.as_str()).or_default().push(e.to.as_str());
        }
    }
    let in_graph =
        |id: &str| data.nodes.iter().any(|n| n.id == id);

    // Hidden = descendant closure of collapsed epics.
    let mut hidden: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for root in collapsed {
        let mut stack = children_of
            .get(root.as_str())
            .map(|v| v.as_slice())
            .unwrap_or(&[])
            .to_vec();
        while let Some(id) = stack.pop() {
            if !in_graph(id) || hidden.contains(id) {
                continue;
            }
            hidden.insert(id);
            if let Some(kids) = children_of.get(id) {
                stack.extend(kids.iter().copied());
            }
        }
    }

    // Root epic of a hidden node: nearest collapsed ancestor.
    let root_of = |id: &str| -> Option<&str> {
        if !hidden.contains(id) {
            return None;
        }
        let mut current = id;
        loop {
            let parent = data.edges.iter().find_map(|e| {
                (e.kind == "parent-child" && e.to == current)
                    .then_some(e.from.as_str())
            });
            match parent {
                Some(p) if hidden.contains(p) => current = p,
                Some(p) if collapsed.contains(p) => return Some(p),
                _ => return None,
            }
        }
    };

    let nodes = data
        .nodes
        .iter()
        .filter(|n| !hidden.contains(n.id.as_str()))
        .map(|bead| CollapsedNode {
            bead: bead.clone(),
            child_count: (bead.issue_type == "epic" && collapsed.contains(&bead.id))
                .then(|| {
                    // Count hidden descendants rooted here.
                    let mut count = 0usize;
                    let mut stack = children_of
                        .get(bead.id.as_str())
                        .map(|v| v.as_slice())
                        .unwrap_or(&[])
                        .to_vec();
                    let mut seen = std::collections::HashSet::new();
                    while let Some(id) = stack.pop() {
                        if !in_graph(id) || !seen.insert(id) {
                            continue;
                        }
                        if root_of(id) == Some(bead.id.as_str()) {
                            count += 1;
                        }
                        if let Some(kids) = children_of.get(id) {
                            stack.extend(kids.iter().copied());
                        }
                    }
                    count
                }),
        })
        .collect();

    // Edges: drop hidden-hidden; re-anchor hidden-visible to root epic.
    let edges: Vec<DepEdge> = data
        .edges
        .iter()
        .filter_map(|e| {
            let reanchor = |who: &str| {
                if hidden.contains(who) {
                    root_of(who).map(|r| r.to_string())
                } else {
                    None
                }
            };
            let from = reanchor(&e.from).unwrap_or_else(|| e.from.clone());
            let to = reanchor(&e.to).unwrap_or_else(|| e.to.clone());
            if from == to {
                return None; // internal to the collapsed subtree
            }
            Some(DepEdge { from, to, kind: e.kind.clone() })
        })
        .collect();

    // Visible nodes whose edges were re-anchored keep their other edges;
    // dedup identical contracted edges (same from/to/kind).
    let mut seen = std::collections::HashSet::new();
    let edges = edges
        .into_iter()
        .filter(|e| seen.insert((e.from.clone(), e.to.clone(), e.kind.clone())))
        .collect();

    CollapsedGraph { nodes, edges }
}

pub fn parse_components(json: &str) -> Result<GraphData, String> {
    let components: Vec<Component> =
        serde_json::from_str(json).map_err(|e| format!("graph json: {e}"))?;
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    for c in components {
        for bead in c.issues {
            if !nodes.iter().any(|b: &Bead| b.id == bead.id) {
                nodes.push(bead);
            }
        }
        for d in c.dependencies {
            // Edge points from blocker to blocked.
            edges.push(DepEdge {
                from: d.depends_on_id,
                to: d.issue_id,
                kind: d.kind,
            });
        }
    }
    Ok(GraphData { nodes, edges })
}

/// A laid-out node: bead plus its column and top-left pixel position.
#[derive(Debug, Clone)]
pub struct LaidNode {
    pub bead: Bead,
    /// Longest-path column: 0 = no blockers, n = deepest chain.
    pub layer: usize,
    pub x: f32,
    pub y: f32,
}

/// Geometry constants from the prototype: node size and travel gaps.
pub const NODE_W: f32 = 220.0;
pub const NODE_H: f32 = 46.0;
pub const COL_GAP: f32 = 90.0;
pub const ROW_GAP: f32 = 14.0;

/// Deterministic layered layout.
///
/// - layer(node) = 1 + max(layer(blockers)) over `blocks` edges only;
///   roots sit in layer 0. Cycles cannot form (bd enforces a DAG) but a
///   guard caps recursion defensively.
/// - Within a column: priority ascending, then ID ascending (Bead order
///   in the input is ignored for positioning).
/// - Positions: x = layer * (NODE_W + COL_GAP), y = index-in-column *
///   (NODE_H + ROW_GAP).
pub fn layout_graph(data: &GraphData) -> Graph {
    // blockers-of map: blocked -> [(blocker, constrains)]
    let mut blockers: std::collections::HashMap<&str, Vec<(&str, bool)>> =
        std::collections::HashMap::new();
    for e in &data.edges {
        blockers
            .entry(e.to.as_str())
            .or_default()
            .push((e.from.as_str(), constrains(&e.kind)));
    }

    // Longest-path layer via memoized DFS.
    let mut layer_of: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    fn depth<'a>(
        id: &'a str,
        blockers: &std::collections::HashMap<&'a str, Vec<(&'a str, bool)>>,
        memo: &mut std::collections::HashMap<&'a str, usize>,
        stack: &mut Vec<&'a str>,
    ) -> usize {
        if let Some(&l) = memo.get(id) {
            return l;
        }
        if stack.contains(&id) {
            return 0; // cycle guard: treat as root
        }
        stack.push(id);
        let mut l = 0;
        if let Some(bs) = blockers.get(id) {
            for (b, constrains) in bs {
                if *constrains {
                    l = l.max(depth(b, blockers, memo, stack) + 1);
                }
            }
        }
        stack.pop();
        memo.insert(id, l);
        l
    }

    for n in &data.nodes {
        depth(&n.id, &blockers, &mut layer_of, &mut Vec::new());
    }

    // Column buckets sorted by (priority, id).
    let mut columns: std::collections::BTreeMap<usize, Vec<&Bead>> =
        std::collections::BTreeMap::new();
    for n in &data.nodes {
        columns
            .entry(layer_of[n.id.as_str()])
            .or_default()
            .push(n);
    }
    for col in columns.values_mut() {
        col.sort_by(|a, b| a.ready_rank_cmp(b));
    }

    let mut nodes = Vec::new();
    let mut position_of: std::collections::HashMap<String, (f32, f32)> =
        std::collections::HashMap::new();
    for (&layer, col) in columns.iter() {
        for (i, bead) in col.iter().enumerate() {
            let x = layer as f32 * (NODE_W + COL_GAP);
            let y = i as f32 * (NODE_H + ROW_GAP);
            position_of.insert(bead.id.clone(), (x, y));
            nodes.push(LaidNode {
                bead: (*bead).clone(),
                layer,
                x,
                y,
            });
        }
    }

    // Edge endpoints: source right-center, target left-center.
    let mut edges = Vec::new();
    for e in &data.edges {
        let (Some(&(fx, fy)), Some(&(tx, ty))) =
            (position_of.get(&e.from), position_of.get(&e.to))
        else {
            continue; // edge to a node outside the snapshot
        };
        edges.push(LaidEdge {
            from: e.from.clone(),
            to: e.to.clone(),
            kind: e.kind.clone(),
            style: EdgeStyle::from_kind(&e.kind),
            x1: fx + NODE_W,
            y1: fy + NODE_H / 2.0,
            x2: tx,
            y2: ty + NODE_H / 2.0,
        });
    }

    Graph { nodes, edges }
}

/// Deterministic render model: positioned nodes and styled edges.
#[derive(Debug, Clone, Default)]
pub struct Graph {
    pub nodes: Vec<LaidNode>,
    pub edges: Vec<LaidEdge>,
}

/// Edge with computed endpoints (blocker right-center -> blocked left-center).
#[derive(Debug, Clone, PartialEq)]
pub struct LaidEdge {
    pub from: String,
    pub to: String,
    pub kind: String,
    pub style: EdgeStyle,
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_style_mapping() {
        assert_eq!(EdgeStyle::from_kind("blocks"), EdgeStyle::Solid);
        assert_eq!(EdgeStyle::from_kind("parent-child"), EdgeStyle::Dotted);
        assert_eq!(EdgeStyle::from_kind("related"), EdgeStyle::Dashed);
        assert_eq!(EdgeStyle::from_kind("tracks"), EdgeStyle::Dashed);
        assert_eq!(EdgeStyle::from_kind("discovered-from"), EdgeStyle::Dashed);
    }
}
