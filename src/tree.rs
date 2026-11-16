//! Tree hierarchy (bdui-rs-8db): parent-child derivation + renderable row
//! list with caret/progress, over already-parsed beads. Pure; S1-tested.

use crate::model::Bead;

/// Parent map: child id -> parent id. Children: parent id -> child ids,
/// numerically sorted (bd-a.2 before bd-a.10).
#[derive(Debug, Default, Clone)]
pub struct Hierarchy {
    pub parent: std::collections::HashMap<String, String>,
    pub children: std::collections::HashMap<String, Vec<String>>,
}

/// Numeric ID comparison: segments between dots compare numerically
/// ("a.10" sorts after "a.2"); other text compares lexically.
pub fn cmp_numeric_ids(a: &str, b: &str) -> std::cmp::Ordering {
    /// Compare one dot segment: numerically when both are numbers.
    fn cmp_seg(pa: &str, pb: &str) -> std::cmp::Ordering {
        match (pa.parse::<u64>(), pb.parse::<u64>()) {
            (Ok(x), Ok(y)) => x.cmp(&y),
            _ => pa.cmp(pb),
        }
    }
    for (pa, pb) in a.split('.').zip(b.split('.')) {
        let ord = cmp_seg(pa, pb);
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
    }
    a.split('.').count().cmp(&b.split('.').count())
}

impl Hierarchy {
    /// Derive parent-child from explicit dep edges, then the ID-suffix
    /// rule when no edge names a parent (prototype derive()).
    pub fn from_beads(beads: &[Bead]) -> Self {
        use std::collections::HashMap;
        let mut by_id: HashMap<&str, &Bead> = HashMap::new();
        for b in beads {
            by_id.insert(b.id.as_str(), b);
        }
        let mut parent: HashMap<String, String> = HashMap::new();
        for b in beads {
            for dep in &b.raw_deps {
                if dep.kind == "parent-child" {
                    let pid = &dep.depends_on_id;
                    if by_id.contains_key(pid.as_str()) && pid != &b.id {
                        parent.insert(b.id.clone(), pid.clone());
                    }
                }
            }
            if !parent.contains_key(&b.id) {
                // ID-suffix fallback: "abc.3" is a child of "abc" when it
                // exists in the bead set.
                if let Some(dot) = b.id.rfind('.') {
                    if dot > 0 {
                        let pid = &b.id[..dot];
                        if by_id.contains_key(pid) {
                            parent.insert(b.id.clone(), pid.to_string());
                        }
                    }
                }
            }
        }
        let mut children: HashMap<String, Vec<String>> = HashMap::new();
        for (child, pid) in &parent {
            if *child != *pid {
                children.entry(pid.clone()).or_default().push(child.clone());
            }
        }
        for kids in children.values_mut() {
            kids.sort_by(|a, b| cmp_numeric_ids(a, b));
        }
        Self { parent, children }
    }

    pub fn parent_of(&self, id: &str) -> Option<&str> {
        self.parent.get(id).map(|s| s.as_str())
    }

    pub fn children_of(&self, id: &str) -> &[String] {
        self.children.get(id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// All descendants (transitive children) of `id`, depth-first.
    pub fn descendants_of(&self, id: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut stack: Vec<String> = self.children_of(id).to_vec();
        while let Some(cur) = stack.pop() {
            if !out.contains(&cur) {
                out.push(cur.clone());
                stack.extend(self.children_of(&cur).iter().cloned());
            }
        }
        out
    }
}

/// A renderable tree row (indent + optional children indicator).
#[derive(Debug, Clone)]
pub struct TreeRow {
    pub bead: Bead,
    pub depth: usize,
    pub has_children: bool,
    pub expanded: bool,
    /// Descendants closed / total; None when the row has no children.
    pub progress: Option<(usize, usize)>,
}

/// Filters (ticket): type, priority, assignee, closed toggle, and query
/// across id/title/label/assignee. "all" / "none" match the prototype.
#[derive(Debug, Default, Clone)]
pub struct Filters {
    pub query: String,
    /// None = all types.
    pub issue_type: Option<String>,
    /// None = all priorities.
    pub priority: Option<u8>,
    /// None = anyone; Some(None) = unassigned.
    pub assignee: Option<Option<String>>,
    /// Closed visibility toggle (true = show). Default-hidden.
    pub show_closed: bool,
}

impl Filters {
    /// Prototype `visible()`: closed filter applied to the bead's derived
    /// state, then type/priority/assignee, then every whitespace-separated
    /// query word matching id/title/labels/assignee.
    pub fn matches(&self, bead: &Bead) -> bool {
        if !self.show_closed && bead.state == crate::model::State::Closed {
            return false;
        }
        if let Some(t) = &self.issue_type {
            if &bead.issue_type != t {
                return false;
            }
        }
        if let Some(p) = self.priority {
            if bead.priority != p {
                return false;
            }
        }
        match &self.assignee {
            // Unassigned filter: only unassigned beads pass.
            Some(None) if bead.assignee.is_some() => return false,
            // Named assignee filter: exact match required.
            Some(Some(who)) if bead.assignee.as_deref() != Some(who.as_str()) => {
                return false;
            }
            _ => {}
        }
        if !self.query.trim().is_empty() {
            let hay = format!(
                "{} {} {} {}",
                bead.id,
                bead.title,
                bead.labels.join(" "),
                bead.assignee.as_deref().unwrap_or("")
            )
            .to_lowercase();
            for word in self.query.to_lowercase().split_whitespace() {
                if !hay.contains(word) {
                    return false;
                }
            }
        }
        true
    }
}

/// Rows for the tree view: filters prune, ancestors of matches are kept,
/// epics show done/total progress, collapsed ids hide descendants.
/// Cycle-safe: a node is visited once even with corrupt parent cycles.
pub fn tree_rows(
    beads: &[Bead],
    hierarchy: &Hierarchy,
    collapsed: &std::collections::HashSet<String>,
    filters: &Filters,
) -> Vec<TreeRow> {
    use std::collections::HashSet;
    // Pass 1: which beads pass the filter on their own.
    let matched: HashSet<&str> = beads
        .iter()
        .filter(|b| filters.matches(b))
        .map(|b| b.id.as_str())
        .collect();
    let index: std::collections::HashMap<&str, &Bead> =
        beads.iter().map(|b| (b.id.as_str(), b)).collect();

    // Pass 2: memoized keep — a node is kept when it matched or when any
    // descendant matched (prototype `has`). `visiting` breaks cycles.
    fn keep_fn<'b>(
        id: &str,
        index: &std::collections::HashMap<&'b str, &'b Bead>,
        hierarchy: &Hierarchy,
        matched: &HashSet<&str>,
        visiting: &mut HashSet<String>,
        cache: &mut std::collections::HashMap<String, bool>,
    ) -> bool {
        if let Some(v) = cache.get(id) {
            return *v;
        }
        if !index.contains_key(id) {
            return false;
        }
        if !visiting.insert(id.to_string()) {
            // Cycle: ignore back-edges.
            return false;
        }
        let mut kept = matched.contains(id);
        for kid in hierarchy.children_of(id) {
            if keep_fn(kid, index, hierarchy, matched, visiting, cache) {
                kept = true;
            }
        }
        visiting.remove(id);
        cache.insert(id.to_string(), kept);
        kept
    }
    let mut keep: HashSet<&str> = HashSet::new();
    let mut cache: std::collections::HashMap<String, bool> = std::collections::HashMap::new();
    for b in beads {
        let mut visiting = HashSet::new();
        if keep_fn(
            b.id.as_str(),
            &index,
            hierarchy,
            &matched,
            &mut visiting,
            &mut cache,
        ) {
            keep.insert(b.id.as_str());
        }
    }

    // Roots: kept beads whose parent is not kept, or whose parent chain
    // loops back into itself (corrupt cycles still surface as roots).
    let cyclic = |id: &str| -> bool {
        let mut cur = hierarchy.parent_of(id).map(|p| p.to_string());
        let start = id.to_string();
        for _ in 0..beads.len() + 1 {
            match cur {
                Some(p) if p == start => return true,
                Some(p) => cur = hierarchy.parent_of(&p).map(|x| x.to_string()),
                None => return false,
            }
        }
        false
    };
    let mut roots: Vec<&Bead> = beads
        .iter()
        .filter(|b| {
            keep.contains(b.id.as_str())
                && (cyclic(&b.id) || !hierarchy.parent_of(&b.id).is_some_and(|p| keep.contains(p)))
        })
        .collect();
    roots.sort_by(|a, b| Bead::ready_rank(a, b));

    // Depth-first expansion, skipping collapsed ids.
    let mut rows: Vec<TreeRow> = Vec::new();
    #[allow(clippy::too_many_arguments)]
    fn walk<'b>(
        id: &'b str,
        depth: usize,
        index: &std::collections::HashMap<&str, &'b Bead>,
        hierarchy: &Hierarchy,
        keep: &HashSet<&str>,
        collapsed: &std::collections::HashSet<String>,
        beads: &'b [Bead],
        rows: &mut Vec<TreeRow>,
        seen: &mut HashSet<String>,
    ) {
        let Some(bead) = index.get(id) else { return };
        if !seen.insert(id.to_string()) {
            return;
        }
        let kid_ids: Vec<&str> = hierarchy
            .children_of(id)
            .iter()
            .map(|s| s.as_str())
            .filter(|k| keep.contains(*k))
            .collect();
        let has_children = !kid_ids.is_empty();
        let expanded = has_children && !collapsed.contains(id);
        let progress = if has_children {
            let desc: Vec<String> = hierarchy.descendants_of(id);
            let status = |kid: &str| {
                beads
                    .iter()
                    .find(|b| b.id == kid)
                    .map(|b| b.status == "closed")
                    .unwrap_or(false)
            };
            let done = desc.iter().filter(|k| status(k)).count();
            Some((done, desc.len()))
        } else {
            None
        };
        rows.push(TreeRow {
            bead: (*bead).clone(),
            depth,
            has_children,
            expanded,
            progress,
        });
        // State scope only for the closed check above.
        if expanded {
            for kid in kid_ids {
                walk(
                    kid,
                    depth + 1,
                    index,
                    hierarchy,
                    keep,
                    collapsed,
                    beads,
                    rows,
                    seen,
                );
            }
        }
    }
    let mut seen = HashSet::new();
    for root in roots {
        walk(
            &root.id, 0, &index, hierarchy, &keep, collapsed, beads, &mut rows, &mut seen,
        );
    }
    rows
}
