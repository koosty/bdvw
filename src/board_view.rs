//! Board view (bdui-rs-nx8): kanban columns by derived state with the
//! shared filter surface applied upstream (shell passes filtered beads).
//! Click opens the drawer; j/k move a per-view selection across cards in
//! reading order (column by column).

use crate::board::{board_columns, BoardColumn, BoardCard};
use crate::model::State;
use crate::tree::Filters;
use gpui_kit::component::ActiveTheme;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, Context, EventEmitter, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

/// Clicking a card opens the detail drawer.
pub enum BoardViewEvent {
    CardClicked(String),
}

pub struct BoardView {
    columns: Vec<BoardColumn>,
    /// Flat reading order of card ids (column by column) for j/k.
    flat: Vec<String>,
    selected: Option<usize>,
    filters: Filters,
    error: Option<String>,
}

impl BoardView {
    pub fn new() -> Self {
        Self {
            columns: Vec::new(),
            flat: Vec::new(),
            selected: None,
            filters: Filters::default(),
            error: None,
        }
    }

    /// Push a store snapshot: filtered beads in, columns out.
    pub fn set_data(&mut self, beads: Vec<crate::model::Bead>, filters: Filters, cx: &mut Context<Self>) {
        if beads.is_empty() {
            self.columns = Vec::new();
            self.flat = Vec::new();
            self.error = Some("No issues to show.".into());
            cx.notify();
            return;
        }
        self.error = None;
        self.filters = filters.clone();
        self.columns = board_columns(&beads, &filters);
        self.flat = self
            .columns
            .iter()
            .flat_map(|c| c.cards.iter().map(|card| card.bead.id.clone()))
            .collect();
        if let Some(sel) = self.selected {
            if sel >= self.flat.len() {
                self.selected = self.flat.len().checked_sub(1);
            }
        }
        cx.notify();
    }

    /// j/k-aware move; wraps at the ends.
    pub fn move_selection(&mut self, delta: i32, cx: &mut Context<Self>) {
        if self.flat.is_empty() {
            return;
        }
        let cur = self.selected.unwrap_or(0) as i32;
        let len = self.flat.len() as i32;
        self.selected = Some((cur + delta).rem_euclid(len) as usize);
        cx.notify();
    }

    /// Enter activates the selected card.
    pub fn activate(&mut self, cx: &mut Context<Self>) {
        if let Some(i) = self.selected {
            if let Some(id) = self.flat.get(i) {
                cx.emit(BoardViewEvent::CardClicked(id.clone()));
            }
        }
    }

    fn render_card(
        &self,
        card: &BoardCard,
        idx: usize,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let bead = &card.bead;
        let color = crate::app::state_color(bead.state);
        let id = bead.id.clone();

        let mut card_el = div()
            .id(SharedString::from(format!("board-card-{}", bead.id)))
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .w(px(220.))
            .rounded_md()
            .border_1()
            .border_color(theme.border)
            .bg(theme.secondary)
            .when(selected, |el| {
                el.border_color(theme.primary).shadow_md()
            })
            .hover(|s| s.border_color(theme.accent.opacity(0.5)))
            .on_click(cx.listener(move |_, _, _, cx| {
                cx.emit(BoardViewEvent::CardClicked(id.clone()));
            }))
            // Header: priority, id, type.
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
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
                            .ml_auto()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(bead.issue_type.clone()),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .text_ellipsis()
                    .overflow_hidden()
                    .child(bead.title.clone()),
            );

        // Blocked cards name their open blockers (prototype "waiting on").
        if bead.state == State::Blocked && !card.open_blockers.is_empty() {
            card_el = card_el.child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(SharedString::from(format!(
                        "waiting on {}",
                        card.open_blockers.join(", ")
                    ))),
            );
        }

        // Footer: labels, unblocks count, assignee.
        card_el = card_el.child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .flex_wrap()
                .children(
                    bead.labels.iter().take(3).map(|l| {
                        div()
                            .text_xs()
                            .px_1()
                            .rounded_full()
                            .bg(theme.background)
                            .text_color(theme.muted_foreground)
                            .child(l.clone())
                    }),
                )
                .when(card.unblocks > 0, |el| {
                    el.child(
                        div()
                            .text_xs()
                            .text_color(color)
                            .child(SharedString::from(format!(
                                "unblocks {}",
                                card.unblocks
                            ))),
                    )
                })
                .child(div().ml_auto().when(bead.assignee.is_some(), |el| {
                    el.text_xs()
                        .text_color(theme.muted_foreground)
                        .child(SharedString::from(
                            bead.assignee.clone().unwrap_or_default(),
                        ))
                })),
        );
        let _ = idx;
        card_el.into_any_element()
    }
}

impl Render for BoardView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let selected_id = self.selected.and_then(|i| self.flat.get(i)).cloned();

        let body = if let Some(err) = &self.error {
            div()
                .p_6()
                .text_color(theme.muted_foreground)
                .child(SharedString::from(err.clone()))
                .into_any_element()
        } else if self.columns.iter().all(|c| c.cards.is_empty()) {
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
            // Columns laid out horizontally; each scrolls vertically.
            let cols: Vec<AnyElement> = self
                .columns
                .iter()
                .map(|col| {
                    let color = crate::app::state_color(col.state);
                    let cards: Vec<AnyElement> = col
                        .cards
                        .iter()
                        .enumerate()
                        .map(|(i, card)| {
                            let is_selected = selected_id.as_deref() == Some(card.bead.id.as_str());
                            self.render_card(card, i, is_selected, cx)
                        })
                        .collect();
                    div()
                        .id(SharedString::from(format!(
                            "board-col-{}",
                            col.label
                        )))
                        .flex()
                        .flex_col()
                        .gap_2()
                        .w(px(240.))
                        .min_h(px(80.))
                        .px_2()
                        .py_2()
                        .rounded_md()
                        .bg(theme.background)
                        // Header: dot, label, count.
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .size(px(8.))
                                        .rounded_full()
                                        .bg(color),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                        .child(col.label),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child(SharedString::from(
                                            col.cards.len().to_string(),
                                        )),
                                ),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!(
                                    "board-col-scroll-{}",
                                    col.label
                                )))
                                .flex()
                                .flex_col()
                                .gap_2()
                                .overflow_y_scroll()
                                .children(cards),
                        )
                        .into_any_element()
                })
                .collect();

            div()
                .id("board-root")
                .flex_1()
                .flex()
                .items_start()
                .gap_2()
                .p_3()
                .overflow_x_scroll()
                .children(cols)
                .into_any_element()
        };

        div()
            .id("board-body")
            .flex()
            .flex_col()
            .flex_1()
            .child(body)
    }
}

impl EventEmitter<BoardViewEvent> for BoardView {}

impl Default for BoardView {
    fn default() -> Self {
        Self::new()
    }
}
