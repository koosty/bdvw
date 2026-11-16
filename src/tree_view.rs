//! Tree view (bdui-rs-8db): hierarchy rows with carets, epic progress
//! bars, expand/collapse-all, and j/k/Enter navigation. Data comes from
//! the shell (store snapshot + hierarchy) so the store stays the single
//! source of truth; collapse state lives here per session.

use crate::model::Bead;
use crate::tree::{tree_rows, Filters, Hierarchy, TreeRow};
use gpui_kit::component::ActiveTheme;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, Context, EventEmitter, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use std::collections::HashSet;

/// Clicking a row (or a dependency chip elsewhere) opens the drawer.
pub enum TreeViewEvent {
    NodeClicked(String),
}

pub struct TreeView {
    /// Last snapshot pushed by the shell on store change.
    rows: Vec<TreeRow>,
    hierarchy: Hierarchy,
    beads: Vec<Bead>,
    collapsed: HashSet<String>,
    selected: Option<usize>,
    filters: Filters,
    error: Option<String>,
    /// Caret mouse_down suppresses the row click beneath it.
    last_caret_id: Option<String>,
}

impl TreeView {
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            hierarchy: Hierarchy::default(),
            beads: Vec::new(),
            collapsed: HashSet::new(),
            selected: None,
            filters: Filters::default(),
            error: None,
            last_caret_id: None,
        }
    }

    /// Push a store snapshot: recompute hierarchy + rows.
    pub fn set_data(
        &mut self,
        beads: Vec<Bead>,
        filters: Filters,
        cx: &mut Context<Self>,
    ) {
        if beads.is_empty() {
            self.beads = beads;
            self.rows = Vec::new();
            self.error = Some("No issues to show.".into());
            cx.notify();
            return;
        }
        self.error = None;
        self.beads = beads.clone();
        self.filters = filters;
        self.hierarchy = Hierarchy::from_beads(&beads);
        self.recompute();
        cx.notify();
    }

    fn recompute(&mut self) {
        // Keep caret collapse stable across refreshes; drop ids that no
        // longer exist so the set cannot grow stale.
        let live: HashSet<String> =
            self.beads.iter().map(|b| b.id.clone()).collect();
        self.collapsed.retain(|id| live.contains(id));
        self.rows = tree_rows(
            &self.beads,
            &self.hierarchy,
            &self.collapsed,
            &self.filters,
        );
        if let Some(sel) = self.selected {
            if sel >= self.rows.len() {
                self.selected = self.rows.len().checked_sub(1);
            }
        }
    }

    /// Toggle one node's expansion.
    pub fn toggle(&mut self, id: &str, cx: &mut Context<Self>) {
        if !self.collapsed.remove(id) {
            self.collapsed.insert(id.to_string());
        }
        self.recompute();
        cx.notify();
    }

    pub fn expand_all(&mut self, cx: &mut Context<Self>) {
        self.collapsed.clear();
        self.recompute();
        cx.notify();
    }

    pub fn collapse_all(&mut self, cx: &mut Context<Self>) {
        // Collapse every node that has children.
        self.collapsed = self
            .hierarchy
            .children
            .keys()
            .cloned()
            .collect::<HashSet<_>>();
        self.recompute();
        cx.notify();
    }

    /// j/k-aware move by delta; wraps at the ends.
    pub fn move_selection(&mut self, delta: i32, cx: &mut Context<Self>) {
        if self.rows.is_empty() {
            return;
        }
        let cur = self.selected.unwrap_or(0) as i32;
        let len = self.rows.len() as i32;
        let next = (cur + delta).rem_euclid(len) as usize;
        self.selected = Some(next);
        cx.notify();
    }

    /// Enter activates the selected row.
    pub fn activate(&mut self, cx: &mut Context<Self>) {
        if let Some(i) = self.selected {
            if let Some(row) = self.rows.get(i) {
                let id = row.bead.id.clone();
                cx.emit(TreeViewEvent::NodeClicked(id));
            }
        }
    }

    fn render_row(
        &self,
        row: &TreeRow,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let bead = &row.bead;
        let color = crate::app::state_color(&bead.state);
        let toggle_id = bead.id.clone();
        let click_id = bead.id.clone();
        let caret = if row.has_children {
            Some(if row.expanded { "\u{25BE}" } else { "\u{25B8}" })
        } else {
            None
        };

        div()
            .id(SharedString::from(format!("tree-row-{}", bead.id)))
            .flex()
            .items_center()
            .gap_1()
            .pl(px(8. + row.depth as f32 * 18.))
            .pr_3()
            .py_1()
            .rounded_md()
            .when(selected, |el| {
                el.bg(theme.accent.opacity(0.12))
                    .border_l_2()
                    .border_color(theme.accent)
            })
            .hover(|s| s.bg(theme.accent.opacity(0.06)))
            // The whole row opens the drawer…
            .on_click(cx.listener(move |this, _, _, cx| {
                // A caret toggle suppresses the row click underneath it.
                if this.last_caret_id.as_deref() == Some(click_id.as_str()) {
                    this.last_caret_id = None;
                    return;
                }
                cx.emit(TreeViewEvent::NodeClicked(click_id.clone()));
            }))
            // …the caret only toggles.
            .child(
                div()
                    .id(SharedString::from(format!("tree-caret-{}", bead.id)))
                    .w(px(18.))
                    .text_xs()
                    .text_color(if row.has_children {
                        theme.foreground
                    } else {
                        theme.muted_foreground.opacity(0.3)
                    })
                    .when(row.has_children, |el| {
                        el.hover(|s| s.bg(theme.accent.opacity(0.1)))
                            .rounded_sm()
                            .on_mouse_down(
                                gpui_kit::MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    this.last_caret_id = Some(toggle_id.clone());
                                    this.toggle(&toggle_id, cx);
                                }),
                            )
                    })
                    .child(SharedString::from(
                        caret.unwrap_or("\u{00B7}").to_string(),
                    )),
            )
            .child(div().size(px(9.)).rounded_full().bg(color))
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui_kit::FontWeight::BOLD)
                    .text_color(crate::app::priority_color(bead.priority))
                    .child(format!("P{}", bead.priority)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(bead.id.clone()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .overflow_x_hidden()
                    .text_ellipsis()
                    .child(bead.title.clone()),
            )
            .children(row.progress.map(|(done, total)| {
                // Progress bar: fill by closed fraction, "done/total" text.
                let frac = if total > 0 {
                    done as f32 / total as f32
                } else {
                    0.0
                };
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .w(px(54.))
                            .h(px(4.))
                            .rounded_full()
                            .bg(theme.secondary)
                            .overflow_hidden()
                            .child(
                                div()
                                    .w(px(54. * frac))
                                    .h(px(4.))
                                    .bg(crate::app::state_color(
                                        &crate::model::State::Closed,
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(SharedString::from(format!("{done}/{total}"))),
                    )
            }))
            .into_any_element()
    }
}

impl Render for TreeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let selected = self.selected;
        let rows_len = self.rows.len();

        let mut body: gpui_kit::AnyElement = if let Some(err) = &self.error {
            div()
                .p_6()
                .text_color(theme.muted_foreground)
                .child(SharedString::from(err.clone()))
                .into_any_element()
        } else if rows_len == 0 {
            div()
                .p_6()
                .text_color(theme.muted_foreground)
                .child(if self.filters.query.is_empty() {
                    "No issues match the current filters."
                } else {
                    "Nothing matches this search."
                })
                .into_any_element()
        } else {
            let rows: Vec<AnyElement> = self
                .rows
                .iter()
                .enumerate()
                .map(|(i, row)| {
                    self.render_row(row, Some(i) == selected, cx)
                })
                .collect();
            div()
                .id("tree-list")
                .flex_1()
                .overflow_y_scroll()
                .p_2()
                .children(rows)
                .into_any_element()
        };

        // Toolbar: Expand all / Collapse all (prototype treebar).
        let toolbar = div()
            .id("tree-toolbar")
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .id("btn-expand-all")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .text_xs()
                    .border_1()
                    .border_color(theme.border)
                    .hover(|s| s.bg(theme.accent.opacity(0.08)))
                    .on_click(cx.listener(|this, _, _, cx| this.expand_all(cx)))
                    .child("Expand all"),
            )
            .child(
                div()
                    .id("btn-collapse-all")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .text_xs()
                    .border_1()
                    .border_color(theme.border)
                    .hover(|s| s.bg(theme.accent.opacity(0.08)))
                    .on_click(cx.listener(|this, _, _, cx| this.collapse_all(cx)))
                    .child("Collapse all"),
            );

        body = div()
            .id("tree-body")
            .flex()
            .flex_col()
            .flex_1()
            .child(toolbar)
            .child(body)
            .into_any_element();

        body
    }
}

impl EventEmitter<TreeViewEvent> for TreeView {}

impl Default for TreeView {
    fn default() -> Self {
        Self::new()
    }
}
