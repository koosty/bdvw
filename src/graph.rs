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
