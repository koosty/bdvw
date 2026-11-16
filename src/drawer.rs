//! Detail drawer: right-hand column over the main pane. Ticket bdui-rs-it5.
//! The selected bead id and its load state live in this Drawer entity. The
//! full record loads off the UI thread through the client seam on open.

use crate::app::{priority_color, state_color};
use crate::model::{Bead, BeadDetail, Comment, DepLink, State};
use crate::stamp;
use gpui_kit::AppContext as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::theme::Theme;
use gpui_kit::{
    ClipboardItem, Context, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Drawer>) -> impl IntoElement {
        let theme = cx.theme().clone();

        match (self.slot.selected(), self.slot.detail()) {
            (_, Some(_)) => {
                let banner = self
                    .slot
                    .error()
                    .map(|e| SharedString::from(format!("Refresh failed: {e}")));
                let detail = self.render_detail(&theme, cx);
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
    fn render_detail(&self, theme: &Theme, cx: &mut Context<Drawer>) -> gpui_kit::AnyElement {
        let detail = self.slot.detail().expect("checked in render");
        let drawer = cx.entity();
        // Links open the linked bead in this drawer.
        let open = {
            let drawer = drawer.clone();
            move |id: &str, window: &mut Window, cx: &mut gpui_kit::App| {
                let id = id.to_string();
                let drawer = drawer.clone();
                window.defer(cx, move |_, cx| {
                    drawer.update(cx, |drawer, cx| drawer.open(&id, cx));
                });
            }
        };
        let close = move |window: &mut Window, cx: &mut gpui_kit::App| {
            let drawer = drawer.clone();
            window.defer(cx, move |_, cx| {
                drawer.update(cx, |drawer, cx| drawer.close(cx));
            });
        };
        render_detail(detail, theme, open, close)
    }
}

fn render_detail<O, C>(detail: &BeadDetail, theme: &Theme, open: O, close: C) -> gpui_kit::AnyElement
where
    O: Fn(&str, &mut Window, &mut gpui_kit::App) + Clone + 'static,
    C: Fn(&mut Window, &mut gpui_kit::App) + 'static,
{
    let bead = &detail.bead;
    let now = stamp::now_epoch();
    let blocked_by: Vec<&DepLink> = detail.dependencies.iter().filter(|l| is_blocking(l)).collect();
    let blocks: Vec<&DepLink> = detail.dependents.iter().filter(|l| is_blocking(l)).collect();
    let related: Vec<&DepLink> = detail
        .dependencies
        .iter()
        .chain(detail.dependents.iter())
        .filter(|l| !is_blocking(l))
        .collect();

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
        .gap_4()
        .child(header(&bead.id, theme, close))
        .child(
            div()
                .text_lg()
                .font_weight(gpui_kit::FontWeight::BOLD)
                .line_height(gpui_kit::relative(1.3))
                .child(bead.title.clone()),
        )
        .child(chips(bead, theme))
        .children(section("Description", text_block(detail.description.as_deref(), theme), theme))
        .children(section("Blocked by", render_links(blocked_by, theme, &open), theme))
        .children(section("Blocks", render_links(blocks, theme, &open), theme))
        .children(section("Related", render_links(related, theme, &open), theme))
        .children(section("Design", text_block(detail.design.as_deref(), theme), theme))
        .children(section(
            "Acceptance criteria",
            text_block(detail.acceptance_criteria.as_deref(), theme),
            theme,
        ))
        .children(section("Notes", text_block(detail.notes.as_deref(), theme), theme))
        .children(section("Comments", render_comments(&detail.comments, theme), theme))
        .children(section("Details", Some(details(bead, theme, now)), theme))
        .children(section("Commands", Some(commands(&bead.id, theme)), theme))
        .into_any_element()
}

/// `blocks` edges drive the Blocked by and Blocks sections; all others are related.
fn is_blocking(link: &DepLink) -> bool {
    link.dependency_type == DepLink::BLOCKS
}

/// Title bar: id, copy-id, and close.
fn header<C>(id: &str, theme: &Theme, close: C) -> impl IntoElement
where
    C: Fn(&mut Window, &mut gpui_kit::App) + 'static,
{
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .font_family(theme.mono_font_family.clone())
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(id.to_string()),
        )
        .child(copy_action(
            "copy-id".to_string(),
            "Copy ID",
            id.to_string(),
            theme.foreground,
        ))
        .child(div().flex_1())
        .child(
            div()
                .id("drawer-close")
                .size(px(28.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_md()
                .text_lg()
                .text_color(theme.muted_foreground)
                .cursor_pointer()
                .hover(|s| s.bg(theme.secondary))
                .on_click(move |_, window, cx| close(window, cx))
                .child("×"),
        )
}

/// Text button that writes `value` to the clipboard on click.
fn copy_action(id: String, label: &str, value: String, color: gpui_kit::Hsla) -> impl IntoElement {
    div()
        .id(id)
        .px_2()
        .py_1()
        .rounded_md()
        .text_xs()
        .text_color(color)
        .cursor_pointer()
        .hover(move |s| s.bg(color.opacity(0.12)))
        .on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(value.clone())))
        .child(label.to_string())
}

fn chips(bead: &Bead, theme: &Theme) -> impl IntoElement {
    let priority = priority_color(bead.priority);
    let who = bead.assignee.clone().unwrap_or_else(|| "unassigned".to_string());
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap_1()
        .child(state_pill(bead.state))
        .child(
            div()
                .text_xs()
                .font_weight(gpui_kit::FontWeight::BOLD)
                .px_1()
                .rounded_sm()
                .border_1()
                .border_color(priority)
                .text_color(priority)
                .child(format!("P{}", bead.priority)),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(bead.issue_type.to_uppercase()),
        )
        .child(pill(who, theme))
        .children(bead.labels.iter().map(|l| pill(l.clone(), theme)).collect::<Vec<_>>())
}

fn pill(text: String, theme: &Theme) -> impl IntoElement {
    div()
        .text_xs()
        .px_2()
        .rounded_full()
        .bg(theme.secondary)
        .text_color(theme.secondary_foreground)
        .child(text)
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
            .gap_2()
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
            .p_3()
            .rounded_lg()
            .bg(theme.secondary)
            .child(t.to_string())
    })
}

/// Clickable dependency chips; each opens its bead in this drawer.
fn render_links<O>(links: Vec<&DepLink>, theme: &Theme, open: &O) -> Option<impl IntoElement>
where
    O: Fn(&str, &mut Window, &mut gpui_kit::App) + Clone + 'static,
{
    if links.is_empty() {
        return None;
    }
    let rows = links
        .into_iter()
        .map(|l| {
            let color = state_color(l.state());
            let open = open.clone();
            let id = l.id.clone();
            div()
                .id(l.id.clone())
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py_2()
                .rounded_md()
                .border_1()
                .border_color(theme.border)
                .cursor_pointer()
                .hover(|s| s.border_color(color))
                .on_click(move |_, window, cx| open(&id, window, cx))
                .child(div().size(px(8.)).rounded_full().bg(color))
                .child(
                    div()
                        .font_family(theme.mono_font_family.clone())
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(l.id.clone()),
                )
                .child(div().text_sm().flex_1().overflow_x_hidden().child(l.title.clone()))
        })
        .collect::<Vec<_>>();
    Some(div().flex().flex_col().gap_2().children(rows))
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

fn details(bead: &Bead, theme: &Theme, now: i64) -> impl IntoElement {
    let created = match stamp::ago(&bead.created_at, now) {
        Some(ago) => format!("{} · {ago}", stamp::absolute(&bead.created_at)),
        None => stamp::absolute(&bead.created_at),
    };
    let updated = stamp::ago(&bead.updated_at, now).unwrap_or_else(|| bead.updated_at.clone());
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(detail_row("Status", bead.status.clone(), theme))
        .child(detail_row("Created", created, theme))
        .child(detail_row("Updated", updated, theme))
}

fn detail_row(label: &str, value: String, theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .gap_3()
        .text_sm()
        .child(
            div()
                .w(px(72.))
                .text_color(theme.muted_foreground)
                .child(label.to_string()),
        )
        .child(div().flex_1().child(value))
}

fn commands(id: &str, theme: &Theme) -> impl IntoElement {
    let rows = [
        format!("bd show {id}"),
        format!("bd update {id} --claim"),
        format!("bd close {id} \"Done\""),
    ]
    .into_iter()
    .map(|cmd| command_row(cmd, theme))
    .collect::<Vec<_>>();
    div().flex().flex_col().gap_2().children(rows)
}

fn command_row(cmd: String, theme: &Theme) -> impl IntoElement {
    let copy = copy_action(
        format!("copy-{cmd}"),
        "copy",
        cmd.clone(),
        gpui_kit::rgb(0x9ca3af).into(),
    );
    div()
        .flex()
        .items_center()
        .gap_3()
        .px_3()
        .py_2()
        .rounded_md()
        .bg(gpui_kit::rgb(0x111318))
        .child(
            div()
                .flex_1()
                .overflow_x_hidden()
                .font_family(theme.mono_font_family.clone())
                .text_xs()
                .text_color(gpui_kit::rgb(0xf5f5f5))
                .child(cmd),
        )
        .child(copy)
}
