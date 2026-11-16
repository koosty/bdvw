//! Ready view tabs and suggested-next selection (pure; no UI).
//!
//! The Ready view shows one of four state tabs. Counts and rows come from the
//! same filtered set, so the tab strip and the list always agree.

use crate::model::{Bead, State};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReadyTab {
    #[default]
    Ready,
    InProgress,
    Blocked,
    Closed,
}

impl ReadyTab {
    pub const ALL: [ReadyTab; 4] = [
        ReadyTab::Ready,
        ReadyTab::InProgress,
        ReadyTab::Blocked,
        ReadyTab::Closed,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Ready => "Ready",
            Self::InProgress => "In progress",
            Self::Blocked => "Blocked",
            Self::Closed => "Closed",
        }
    }

    pub fn state(self) -> State {
        match self {
            Self::Ready => State::Ready,
            Self::InProgress => State::InProgress,
            Self::Blocked => State::Blocked,
            Self::Closed => State::Closed,
        }
    }
}

/// Number of beads in `tab`. `beads` is the already-filtered set.
pub fn tab_count(beads: &[Bead], tab: ReadyTab) -> usize {
    beads.iter().filter(|b| b.state == tab.state()).count()
}

/// Rows for `tab`: open tabs by ready rank, Closed by most recent update.
/// `beads` is the already-filtered set.
pub fn tab_rows(beads: &[Bead], tab: ReadyTab) -> Vec<Bead> {
    let mut rows: Vec<Bead> = beads
        .iter()
        .filter(|b| b.state == tab.state())
        .cloned()
        .collect();
    match tab {
        ReadyTab::Closed => rows.sort_by(|a, b| b.updated_at.cmp(&a.updated_at)),
        _ => rows.sort_by(Bead::ready_rank),
    }
    rows
}

/// The bead the prototype suggests claiming: the first Ready row.
/// `ready` must already be in ready-rank order (as `tab_rows` returns it).
pub fn suggested_next(ready: &[Bead]) -> Option<&Bead> {
    ready.first()
}
