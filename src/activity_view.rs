//! Activity view (bdui-rs-2u6): day-grouped event feed across all beads.
//! Clicking an entry opens the drawer. The shell refreshes the feed when
//! the store changes while this view is active (poll cadence).

use crate::activity::{activity_feed, event_label, time_of, ActivityDay, ActivityEvent};
use crate::model::Bead;
use gpui_kit::component::ActiveTheme;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, Context, EventEmitter, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

/// Clicking an entry opens the drawer on that bead.
pub enum ActivityViewEvent {
    EntryClicked(String),
}

pub struct ActivityView {
    days: Vec<ActivityDay>,
    selected: Option<(usize, usize)>,
    /// True once any feed has been produced (first fetch done).
    loaded: bool,
    error: Option<String>,
    /// Sweep progress as (done, total) beads; None when idle.
    progress: Option<(usize, usize)>,
}

impl ActivityView {
    pub fn new() -> Self {
        Self {
            days: Vec::new(),
            selected: None,
            loaded: false,
            error: None,
            progress: None,
        }
    }

    /// Push a fresh feed: merge events over the current bead titles.
    pub fn set_feed(
        &mut self,
        beads: &[Bead],
        events: Vec<ActivityEvent>,
        cx: &mut Context<Self>,
    ) {
        self.loaded = true;
        self.error = None;
        self.progress = None;
        self.days = activity_feed(beads, events);
        if let Some((d, e)) = self.selected {
            if d >= self.days.len() || e >= self.days.get(d).map_or(0, |x| x.entries.len()) {
                self.selected = None;
            }
        }
        cx.notify();
    }

    /// Sweep progress: `done` of `total` beads fetched.
    pub fn set_progress(&mut self, done: usize, total: usize, cx: &mut Context<Self>) {
        self.progress = Some((done, total));
        cx.notify();
    }

    /// Transport-level failure: keep the last good feed, surface the error.
    pub fn feed_failed(&mut self, message: String, cx: &mut Context<Self>) {
        self.error = Some(message);
        self.loaded = true;
        cx.notify();
    }

    /// Flat index over (day, entry) for j/k.
    fn len(&self) -> usize {
        self.days.iter().map(|d| d.entries.len()).sum()
    }

    fn locate(&self, flat: usize) -> Option<(usize, usize)> {
        let mut left = flat;
        for (d, day) in self.days.iter().enumerate() {
            if left < day.entries.len() {
                return Some((d, left));
            }
            left -= day.entries.len();
        }
        None
    }

    /// j/k-aware move; wraps at the ends.
    pub fn move_selection(&mut self, delta: i32, cx: &mut Context<Self>) {
        if self.len() == 0 {
            return;
        }
        let cur = self
            .selected
            .map(|(d, e)| {
                self.days[..d].iter().map(|x| x.entries.len()).sum::<usize>() + e
            })
            .unwrap_or(0) as i32;
        let next = (cur + delta).rem_euclid(self.len() as i32) as usize;
        self.selected = self.locate(next);
        cx.notify();
    }

    /// Enter activates the selected entry.
    pub fn activate(&mut self, cx: &mut Context<Self>) {
        if let Some((d, e)) = self.selected {
            if let Some(entry) = self.days.get(d).and_then(|day| day.entries.get(e)) {
                cx.emit(ActivityViewEvent::EntryClicked(entry.event.issue_id.clone()));
            }
        }
    }

    fn render_entry(
        &self,
        day_idx: usize,
        entry_idx: usize,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let entry = &self.days[day_idx].entries[entry_idx];
        let id = entry.event.issue_id.clone();
        let badge_id = id.clone();
        let stamp = time_of(&entry.event.created_at);

        div()
            .id(SharedString::from(format!(
                "activity-entry-{}",
                entry.event.id
            )))
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .rounded_md()
            .when(selected, |el| {
                el.bg(theme.accent.opacity(0.12))
                    .border_l_2()
                    .border_color(theme.accent)
            })
            .hover(|s| s.bg(theme.accent.opacity(0.06)))
            .on_click(cx.listener(move |_, _, _, cx| {
                cx.emit(ActivityViewEvent::EntryClicked(badge_id.clone()));
            }))
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .w(px(44.))
                    .child(SharedString::from(stamp.to_string())),
            )
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui_kit::FontWeight::BOLD)
                    .text_color(theme.accent)
                    .child(SharedString::from(event_label(&entry.event.kind).to_string())),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(SharedString::from(if entry.event.actor.is_empty() {
                        "unknown".to_string()
                    } else {
                        entry.event.actor.clone()
                    })),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(SharedString::from(id.clone())),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .overflow_x_hidden()
                    .text_ellipsis()
                    .child(SharedString::from(
                        entry
                            .title
                            .clone()
                            .unwrap_or_else(|| "(".to_string() + &id.clone() + ")"),
                    )),
            )
            .into_any_element()
    }
}

impl Render for ActivityView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let selected = self.selected;

        let mut children: Vec<AnyElement> = Vec::new();
        if let Some(err) = self.error.clone() {
            children.push(
                div()
                    .mx_3()
                    .my_1()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .text_color(theme.danger)
                    .rounded_md()
                    .bg(theme.danger.opacity(0.1))
                    .child(SharedString::from(err))
                    .into_any_element(),
            );
        }
        if let Some((done, total)) = self.progress {
            children.push(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(SharedString::from(format!("Fetching history {done}/{total}")))
                    .into_any_element(),
            );
        }
        if !self.loaded {
            children.push(
                div()
                    .p_6()
                    .text_color(theme.muted_foreground)
                    .child("Loading activity…")
                    .into_any_element(),
            );
        } else if self.days.is_empty() {
            children.push(
                div()
                    .p_6()
                    .text_color(theme.muted_foreground)
                    .child("No activity yet.")
                    .into_any_element(),
            );
        } else {
            for (d, day) in self.days.clone().iter().enumerate() {
                children.push(
                    div()
                        .id(SharedString::from(format!("activity-day-{}", day.day)))
                        .px_2()
                        .py_1()
                        .text_xs()
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .text_color(theme.muted_foreground)
                        .bg(theme.background)
                        .child(SharedString::from(day.day.clone()))
                        .into_any_element(),
                );
                for (e, _) in day.entries.iter().enumerate() {
                    children.push(self.render_entry(d, e, selected == Some((d, e)), cx));
                }
            }
        }

        div()
            .id("activity-list")
            .flex_1()
            .overflow_y_scroll()
            .p_2()
            .children(children)
    }
}

impl EventEmitter<ActivityViewEvent> for ActivityView {}

impl Default for ActivityView {
    fn default() -> Self {
        Self::new()
    }
}
