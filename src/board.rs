//! Board columns (bdui-rs-nx8): kanban by derived state. Pure; S1-tested.
//!
//! Card facts the prototype shows but raw list rows lack — open blockers
//! and unblocks — are derived here from the row's inline dependency edges
//! plus the bead set (target status lookup). Epics with children are
//! excluded, matching the prototype's isEpicParent rule.

use crate::model::{Bead, State};
use crate::tree::Filters;
use std::collections::HashMap;

/// Card facts derived per bead before column assembly.
#[derive(Debug, Clone)]
pub struct BoardCard {
    pub bead: Bead,
    /// Blockers (deps of type `blocks`) that are not closed.
    pub open_blockers: Vec<String>,
    /// Count of open beads this card unblocks (other beads blocked by it).
    pub unblocks: usize,
}

/// One kanban column.
#[derive(Debug, Clone)]
pub struct BoardColumn {
    pub state: State,
    pub label: String,
    pub cards: Vec<BoardCard>,
}

pub fn state_label(state: &State) -> String {
    match state {
        State::Ready => "Ready".to_string(),
        State::InProgress => "In progress".to_string(),
        State::Blocked => "Blocked".to_string(),
        State::Deferred => "Deferred".to_string(),
        State::Closed => "Closed".to_string(),
        State::Custom(raw) => raw.clone(),
    }
}

/// Prototype isEpicParent: an epic that actually has children in the set.
/// (issue_type == "epic" and at least one parent-child edge pointing at it).
fn is_epic_parent(bead: &Bead, children: &HashMap<&str, usize>) -> bool {
    bead.issue_type == "epic" && children.get(bead.id.as_str()).copied().unwrap_or(0) > 0
}

/// Derive open blockers and unblocks for every bead from inline dep edges.
///
/// `blocks` edges read `depends_on_id` blocks `issue_id`: target blocks
/// the source. A blocker is open when its status is not `closed`; `bd`
/// list rows do not carry the target, so status comes from the set.
pub fn derive_cards(beads: &[Bead]) -> Vec<BoardCard> {
    let status: HashMap<&str, &str> = beads
        .iter()
        .map(|b| (b.id.as_str(), b.status.as_str()))
        .collect();
    let mut children: HashMap<&str, usize> = HashMap::new();
    for b in beads {
        for dep in &b.raw_deps {
            if dep.kind == "parent-child" && status.contains_key(dep.depends_on_id.as_str()) {
                *children.entry(dep.depends_on_id.as_str()).or_insert(0) += 1;
            }
        }
    }

    let mut blockers: HashMap<&str, Vec<String>> = HashMap::new();
    let mut unblocks: HashMap<&str, usize> = HashMap::new();
    for b in beads {
        for dep in &b.raw_deps {
            if dep.kind != "blocks" {
                continue;
            }
            let target_open = status
                .get(dep.depends_on_id.as_str())
                .map(|s| *s != "closed")
                .unwrap_or(false);
            if target_open {
                blockers
                    .entry(b.id.as_str())
                    .or_default()
                    .push(dep.depends_on_id.clone());
                *unblocks.entry(dep.depends_on_id.as_str()).or_insert(0) += 1;
            }
        }
    }

    beads
        .iter()
        .map(|b| BoardCard {
            open_blockers: blockers.get(b.id.as_str()).cloned().unwrap_or_default(),
            unblocks: unblocks.get(b.id.as_str()).copied().unwrap_or(0),
            bead: b.clone(),
        })
        .filter(|c| !is_epic_parent(&c.bead, &children))
        .collect()
}

/// Card ordering: priority ascending, then more-unblocks first, then newer
/// updated_at first (prototype cmpP).
pub fn card_rank(a: &BoardCard, b: &BoardCard) -> std::cmp::Ordering {
    a.bead
        .priority
        .cmp(&b.bead.priority)
        .then_with(|| b.unblocks.cmp(&a.unblocks))
        .then_with(|| b.bead.updated_at.cmp(&a.bead.updated_at))
}

/// Assemble the board: one column per derived state over the filtered,
/// epic-parent-free cards. Columns present: Ready, In progress, Blocked,
/// Deferred (when any card), Closed (filter toggle gates the cards; the
/// column appears only when the toggle is on and cards exist).
pub fn board_columns(beads: &[Bead], filters: &Filters) -> Vec<BoardColumn> {
    let cards = derive_cards(beads)
        .into_iter()
        .filter(|c| filters.matches(&c.bead))
        .collect::<Vec<_>>();

    let make_col = |state: State, cards: &[BoardCard]| -> BoardColumn {
        let mut list: Vec<BoardCard> = cards
            .iter()
            .filter(|c| c.bead.state == state)
            .cloned()
            .collect();
        if state == State::Closed {
            // Closed column: most recently first, capped at 30 (prototype
            // board). bd list rows expose no closed-at, so updated_at
            // stands in as the recency key.
            list.sort_by(|a, b| b.bead.updated_at.cmp(&a.bead.updated_at));
            list.truncate(30);
        } else {
            list.sort_by(card_rank);
        }
        let label = state_label(&state);
        BoardColumn {
            state,
            label,
            cards: list,
        }
    };

    let mut cols = vec![
        make_col(State::Ready, &cards),
        make_col(State::InProgress, &cards),
        make_col(State::Blocked, &cards),
    ];
    if cards.iter().any(|c| c.bead.state == State::Deferred) {
        cols.push(make_col(State::Deferred, &cards));
    }
    // Unmapped statuses keep their own column, after the mapped ones, in
    // first-seen order so the board is deterministic.
    let mut custom: Vec<State> = Vec::new();
    for card in &cards {
        if matches!(card.bead.state, State::Custom(_)) && !custom.contains(&card.bead.state) {
            custom.push(card.bead.state.clone());
        }
    }
    for state in custom {
        cols.push(make_col(state, &cards));
    }
    if cards.iter().any(|c| c.bead.state == State::Closed) {
        cols.push(make_col(State::Closed, &cards));
    }
    cols
}
