//! Pure palette logic: command inventory, fuzzy matching, ranking.
//!
//! Kept free of any gpui types so it can be tested without a window (S1 seam).

use crate::model::Bead;

/// A navigable view in the shell. Board/Tree/Graph/Activity/Memory land in
/// later slices; Ready is the only one rendered today.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewKind {
    Ready,
    Tree,
    Graph,
    Board,
    Activity,
    Memory,
}

impl ViewKind {
    pub fn name(self) -> &'static str {
        match self {
            ViewKind::Ready => "Ready",
            ViewKind::Tree => "Tree",
            ViewKind::Graph => "Graph",
            ViewKind::Board => "Board",
            ViewKind::Activity => "Activity",
            ViewKind::Memory => "Memory",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Ready" => Some(ViewKind::Ready),
            "Tree" => Some(ViewKind::Tree),
            "Graph" => Some(ViewKind::Graph),
            "Board" => Some(ViewKind::Board),
            "Activity" => Some(ViewKind::Activity),
            "Memory" => Some(ViewKind::Memory),
            _ => None,
        }
    }
}

/// Everything the palette can do: jump to a view or toggle the theme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteCommand {
    View(ViewKind),
    ToggleTheme,
}

impl PaletteCommand {
    pub fn label(&self) -> String {
        match self {
            PaletteCommand::View(v) => format!("Go to {}", v.name()),
            PaletteCommand::ToggleTheme => "Toggle theme".to_string(),
        }
    }

    fn keywords(&self) -> &'static str {
        match self {
            PaletteCommand::View(v) => match v {
                ViewKind::Ready => "ready list",
                ViewKind::Tree => "tree",
                ViewKind::Graph => "graph dag",
                ViewKind::Board => "board kanban",
                ViewKind::Activity => "activity",
                ViewKind::Memory => "memory messages",
            },
            PaletteCommand::ToggleTheme => "theme dark light toggle",
        }
    }
}

pub fn palette_commands() -> Vec<PaletteCommand> {
    let mut cmds = [
        ViewKind::Ready,
        ViewKind::Tree,
        ViewKind::Graph,
        ViewKind::Board,
        ViewKind::Activity,
        ViewKind::Memory,
    ]
    .into_iter()
    .map(PaletteCommand::View)
    .collect::<Vec<_>>();
    cmds.push(PaletteCommand::ToggleTheme);
    cmds
}

/// One row in the palette list. Either a command or a bead jump.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteMatch {
    score: i64,
    id: String,
    label: String,
    title: Option<String>,
}

impl PaletteMatch {
    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }
}

/// Subsequence fuzzy match. Higher score = better; prefixes of any word score
/// highest, contiguous runs beat scattered letters, earlier matches beat later.
pub fn fuzzy_match(query: &str, text: &str) -> Option<i64> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Some(0);
    }
    let hay = text.to_lowercase();
    let hay_contains_word_prefix = hay
        .split(|c: char| !c.is_alphanumeric())
        .any(|w| w.starts_with(&query));
    if hay.contains(&query) {
        let pos = hay.find(&query).unwrap() as i64;
        // Contiguous substring: good. Word start: best.
        return Some(1_000_000 - pos * 10 + if hay_contains_word_prefix { 500_000 } else { 0 });
    }
    // Subsequence scan.
    let mut score = 0i64;
    let mut last: Option<usize> = None;
    let mut hay_chars = hay.char_indices();
    for q in query.chars() {
        let (idx, _) = loop {
            let (i, c) = hay_chars.next()?;
            if c == q {
                break (i, c);
            }
        };
        let contiguous = last == Some(idx.saturating_sub(1));
        score += if contiguous { 12 } else { 3 };
        score -= idx as i64 / 16;
        last = Some(idx);
    }
    if score <= 0 {
        return None;
    }
    Some(score)
}

/// Rank palette matches for a query: static commands plus bead jumps
/// (ID first, then title). Beads tie-broken by ready_rank order (as passed in).
pub fn palette_matches(
    query: &str,
    commands: &[PaletteCommand],
    beads: &[Bead],
) -> Vec<PaletteMatch> {
    let mut hits: Vec<PaletteMatch> = commands
        .iter()
        .filter_map(|c| {
            let label = c.label();
            fuzzy_match(query, &label)
                .or_else(|| fuzzy_match(query, c.keywords()))
                .map(|score| PaletteMatch {
                    score,
                    id: String::new(),
                    label,
                    title: None,
                })
        })
        .chain(beads.iter().filter_map(|b| {
            let id_hit = fuzzy_match(query, &b.id);
            let title_hit = fuzzy_match(query, &b.title);
            // Prefer the better of ID or title match; ID wins exact ties.
            let (score, title) = match (id_hit, title_hit) {
                (Some(i), Some(t)) => (
                    i.max(t) + if i >= t { 100 } else { 0 },
                    Some(b.title.clone()),
                ),
                (Some(i), None) => (i + 100, None),
                (None, Some(t)) => (t, Some(b.title.clone())),
                (None, None) => return None,
            };
            Some(PaletteMatch {
                score,
                id: b.id.clone(),
                label: b.id.clone(),
                title,
            })
        }))
        .collect();
    hits.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.label.cmp(&b.label)));
    hits.dedup_by(|a, b| a.label == b.label && a.id == b.id);
    hits
}
