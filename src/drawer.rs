//! Detail drawer: right-hand column over the main pane. Ticket bdui-rs-it5.
//! The selected bead id and its load state live in this Drawer entity. The
//! full record loads off the UI thread through the client seam on open.

use crate::app::{priority_color, state_color};
use crate::model::{BeadDetail, Comment, DepLink, State};
use gpui_kit::AppContext as _;
use gpui_kit::component::clipboard::Clipboard;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Context, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use gpui_kit::component::theme::Theme;
use std::sync::Arc;

/// Transport for drawer loads. `Send + Sync` so loads run off the UI thread.
pub trait DetailClient: Send + Sync {
    fn detail(&self, id: &str) -> Result<BeadDetail, String>;
}

/// Pure load state for the drawer (bdui-rs-A2). The last good detail survives
/// failed reloads, and results from a superseded request are discarded.
#[derive(Debug, Default)]
pub struct DetailSlot {
    selected: Option<String>,
    detail: Option<BeadDetail>,
    error: Option<String>,
    seq: u64,
}

impl DetailSlot {
    /// Select `id` and return the token its load must present. Re-selecting
    /// the same id keeps the previous detail on screen while it reloads.
    pub fn begin(&mut self, id: &str) -> u64 {
        if self.selected.as_deref() != Some(id) {
            self.detail = None;
        }
        self.selected = Some(id.to_string());
        self.seq += 1;
        self.seq
    }

    /// Token for reloading the current selection, if any.
    pub fn begin_refresh(&mut self) -> Option<(String, u64)> {
        let id = self.selected.clone()?;
        self.seq += 1;
        Some((id, self.seq))
    }

    /// Apply a load result if its token is still current. A failure keeps
    /// the previous detail and records the error. Returns whether it applied.
    pub fn apply(&mut self, seq: u64, result: Result<BeadDetail, String>) -> bool {
        if seq != self.seq {
            return false;
        }
        match result {
            Ok(detail) => {
                self.detail = Some(detail);
                self.error = None;
            }
            Err(message) => self.error = Some(message),
        }
        true
    }

    pub fn close(&mut self) {
        self.selected = None;
        self.detail = None;
        self.error = None;
        self.seq += 1;
    }

    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    pub fn detail(&self) -> Option<&BeadDetail> {
        self.detail.as_ref()
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

/// Right-pane drawer state. Owns selection so the shell's list keeps its
/// scroll position ("never unseats the list").
pub struct Drawer {
    slot: DetailSlot,
    client: Arc<dyn DetailClient>,
}

impl Drawer {
    pub fn new(client: Arc<dyn DetailClient>) -> Self {
        Self { slot: DetailSlot::default(), client }
    }

    /// Re-fetch the detail for the currently selected bead (poll path).
    pub fn refresh(&mut self, cx: &mut Context<Drawer>) {
        if let Some((id, seq)) = self.slot.begin_refresh() {
            self.load(id, seq, cx);
        }
    }

    /// The bead currently open, if any.
    pub fn open_id(&self) -> Option<&str> {
        self.slot.selected()
    }

    /// Select a bead and load its full record off the UI thread.
    pub fn open(&mut self, id: &str, cx: &mut Context<Drawer>) {
        let seq = self.slot.begin(id);
        self.load(id.to_string(), seq, cx);
        cx.notify();
    }

    pub fn close(&mut self, cx: &mut Context<Drawer>) {
        self.slot.close();
        cx.notify();
    }

    /// Fetch on the background executor; apply on the UI thread if the
    /// request is still current.
    fn load(&self, id: String, seq: u64, cx: &mut Context<Drawer>) {
        let client = self.client.clone();
        cx.spawn(async move |drawer, cx| {
            let result = cx
                .background_spawn(async move { client.detail(&id) })
                .await;
            let _ = drawer.update(cx, |drawer, cx| {
                if drawer.slot.apply(seq, result) {
                    cx.notify();
                }
            });
        })
        .detach();
    }
}


impl Render for Drawer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Drawer>) -> impl IntoElement {
        let theme = cx.theme().clone();

        match (self.slot.selected(), self.slot.detail()) {
            (_, Some(_)) => {
                let banner = self
                    .slot
                    .error()
                    .map(|e| SharedString::from(format!("Refresh failed: {e}")));
                let detail = self.render_detail(&theme, window, cx);
                match banner {
                    None => detail,
                    Some(text) => div()
                        .w(px(430.))
                        .h_full()
                        .flex()
                        .flex_col()
                        .child(div().p_2().text_xs().text_color(theme.danger).child(text))
                        .child(detail)
                        .into_any_element(),
                }
            }
            (Some(id), None) => {
                let text = match self.slot.error() {
                    Some(e) => format!("Could not load {id}: {e}"),
                    None => format!("Loading {id}…"),
                };
                div()
                    .w(px(430.))
                    .h_full()
                    .overflow_hidden()
                    .border_l_1()
                    .border_color(theme.border)
                    .bg(theme.background)
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child("Detail")
                    .child(SharedString::from(text))
                    .into_any_element()
            }
            _ => div().into_any_element(),
        }
    }
}

impl Drawer {
    fn render_detail(
        &self,
        theme: &Theme,
        window: &mut Window,
        cx: &mut Context<Drawer>,
    ) -> gpui_kit::AnyElement {
        let detail = self.slot.detail().expect("checked in render");
        // Chips open the linked bead in this drawer.
        let chip_drawer = cx.entity();
        let open_in_drawer = move |id: &str, window: &mut Window, cx: &mut gpui_kit::App| {
            let id = id.to_string();
            let chip_drawer = chip_drawer.clone();
            window.defer(cx, move |_, cx| {
                chip_drawer.update(cx, |drawer, cx| drawer.open(&id, cx));
            });
        };
        let _ = window;
        render_detail(detail, theme, open_in_drawer)
    }
}

fn render_detail<O>(detail: &BeadDetail, theme: &Theme, open: O) -> gpui_kit::AnyElement
where
    O: Fn(&str, &mut Window, &mut gpui_kit::App) + Clone + 'static,
{
    let bead = &detail.bead;

    div()
        .id("drawer-scroll")
        .w(px(430.))
        .h_full()
        .overflow_y_scroll()
        .border_l_1()
        .border_color(theme.border)
        .bg(theme.background)
        .p_4()
        .flex()
        .flex_col()
        .gap_3()
        .child(render_id_row(&bead.id, theme))
        .child(
            div()
                .text_lg()
                .line_height(gpui_kit::relative(1.3))
                .child(bead.title.clone()),
        )
        .child(render_chips(bead, theme))
        .children(section("Blocked by", render_links(&detail.dependencies, theme, &open), theme))
        .children(section("Blocks", render_links(&detail.dependents, theme, &open), theme))
        .children(section("Description", text_block(
            detail.description.as_deref(),
            theme,
        ), theme))
        .children(section("Design", text_block(detail.design.as_deref(), theme), theme))
        .children(section("Acceptance criteria", text_block(
            detail.acceptance_criteria.as_deref(),
            theme,
        ), theme))
        .children(section("Notes", text_block(detail.notes.as_deref(), theme), theme))
        .children(section("Comments", render_comments(&detail.comments, theme), theme))
        .into_any_element()
}

fn render_id_row(id: &str, theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(SharedString::from(id.to_string())),
        )
        .child(Clipboard::new("copy-id").value(SharedString::from(id.to_string())))
        .child(
            Clipboard::new("copy-show")
                .value(SharedString::from(format!("bd show {id}")))
                .tooltip("bd show command"),
        )
}

fn render_chips(bead: &crate::model::Bead, theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .flex_wrap()
        .gap_1()
        .child(state_pill(bead.state))
        .child(
            div()
                .text_xs()
                .font_weight(gpui_kit::FontWeight::BOLD)
                .text_color(priority_color(bead.priority))
                .child(format!("P{}", bead.priority)),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(bead.issue_type.clone()),
        )
        .when_some(bead.assignee.clone(), |row, who| {
            row.child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(who),
            )
        })
        .children(
            bead.labels
                .iter()
                .map(|l| {
                    div()
                        .text_xs()
                        .px_2()
                        .rounded_full()
                        .bg(theme.secondary)
                        .text_color(theme.secondary_foreground)
                        .child(l.clone())
                })
                .collect::<Vec<_>>(),
        )
}

fn state_pill(state: State) -> impl IntoElement {
    let color = state_color(state);
    div()
        .flex()
        .items_center()
        .gap_1()
        .px_2()
        .rounded_full()
        .text_xs()
        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
        .text_color(color)
        .bg(color.opacity(0.14))
        .child(div().size(px(7.)).rounded_full().bg(color))
        .child(state.to_string())
}

/// A section renders only when it has content.
fn section(
    title: &str,
    body: Option<impl IntoElement>,
    theme: &Theme,
) -> Option<impl IntoElement> {
    let body = body?;
    Some(
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .text_color(theme.muted_foreground)
                    .child(title.to_uppercase()),
            )
            .child(body),
    )
}

fn text_block(text: Option<&str>, theme: &Theme) -> Option<impl IntoElement> {
    text.map(|t| {
        div()
            .text_sm()
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(theme.border)
            .child(t.to_string())
    })
}

/// Clickable dependency/dependent chips.
fn render_links<O>(links: &[DepLink], theme: &Theme, open: &O) -> Option<impl IntoElement>
where
    O: Fn(&str, &mut Window, &mut gpui_kit::App) + Clone + 'static,
{
    if links.is_empty() {
        return None;
    }
    let chips = links
        .iter()
        .map(|l| {
            let color = state_color(l.state());
            div()
                .id(l.id.clone())
                .flex()
                .items_center()
                .gap_2()
                .w_full()
                .px_2()
                .py_1()
                .rounded_md()
                .border_1()
                .border_color(theme.border)
                .hover(|s| s.border_color(color))
                .on_click({
                    let id = l.id.clone();
                    let open = open.clone();
                    move |_, window, cx| open(&id, window, cx)
                })
                .child(div().size(px(8.)).rounded_full().bg(color))
                .child(
                    div()
                        .text_xs()
                        .flex_1()
                        .overflow_x_hidden()
                        .child(SharedString::from(format!("{} {}", l.id, l.title))),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(SharedString::from(l.dependency_type.clone())),
                )
        })
        .collect::<Vec<_>>();
    Some(div().flex().flex_col().gap_1().children(chips))
}

fn render_comments(comments: &[Comment], theme: &Theme) -> Option<impl IntoElement> {
    if comments.is_empty() {
        return None;
    }
    let blocks = comments
        .iter()
        .map(|c| {
            div()
                .border_l_2()
                .border_color(theme.border)
                .pl_3()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(SharedString::from(format!(
                            "{} · {}",
                            c.author,
                            c.created_at
                        ))),
                )
                .child(div().text_sm().child(c.text.clone()))
        })
        .collect::<Vec<_>>();
    Some(div().flex().flex_col().gap_2().children(blocks))
}
