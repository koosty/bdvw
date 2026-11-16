//! App shell: three-pane layout (Sidebar, top bar, drawer column)
//! with the Ready list as the home view. Ticket bdui-rs-oga.

use crate::model::{Bead, State};
use gpui_kit::AppContext as _;
use crate::store::Store;
use gpui_kit::component::sidebar::{
    Sidebar, SidebarFooter, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::ActiveTheme;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px, rgb,
};

/// Colors keyed to derived state (CONTEXT.md: State), not priority.
/// Dark-mode-first palette from the prototype's tokens.
pub fn state_color(state: State) -> gpui_kit::Hsla {
    match state {
        State::Ready => rgb(0x3ccf7e).into(),   // --ready (dark)
        State::InProgress => rgb(0x6aa0ff).into(), // --progress
        State::Blocked => rgb(0xff6b5e).into(),  // --blocked
        State::Deferred => rgb(0xb794f4).into(), // --deferred
        State::Closed => rgb(0x6b7486).into(),   // --closed
    }
}

/// P0–P4 chip colors (see prototype --p0…--p4 tokens).
pub fn priority_color(priority: u8) -> gpui_kit::Hsla {
    match priority {
        0 => rgb(0xff6b5e).into(),
        1 => rgb(0xffa24a).into(),
        2 => rgb(0xf0c23a).into(),
        3 => rgb(0x6aa0ff).into(),
        _ => rgb(0x6b7486).into(),
    }
}

pub struct AppShell {
    store: Entity<Store>,
    drawer: Entity<crate::drawer::Drawer>,
    /// Command palette overlay (bdui-rs-ue2).
    palette: Entity<crate::palette_ui::Palette>,
    /// Index into the ready list for j/k navigation. None = no selection.
    selected_row: Option<usize>,
    /// Pending `g` prefix: the next key jump targets a view.
    g_prefix: bool,
    /// Which view the main pane shows; only Ready is rendered today.
    active_view: crate::palette::ViewKind,
    focus_handle: gpui_kit::FocusHandle,
}

/// Transport for the shell's initial load. The Store owns refresh state;
/// the shell needs the client once, at startup.
pub trait BeadsClientShim {
    fn refresh(&self, store: &mut Store, cx: &mut Context<Store>);
}

impl AppShell {
    pub fn new(
        store: Entity<Store>,
        client: std::rc::Rc<dyn BeadsClientShim>,
        drawer: Entity<crate::drawer::Drawer>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        store.update(cx, |store, cx| client.refresh(store, cx));
        let handle = cx.focus_handle();
        window.focus(&handle, cx);
        let palette = cx.new(|cx| crate::palette_ui::Palette::new(window, cx));
        // Palette picks dispatch to the shell.
        cx.subscribe(&palette, |shell, _, event, cx| {
            let crate::palette_ui::PaletteEvent::Pick(action) = event;
            shell_palette_pick(shell, action.clone(), cx);
        })
        .detach();
        // Keep palette bead jumps in sync with store refreshes.
        cx.observe(&store, |shell, store, cx| {
            let beads = store.read(cx).ready().to_vec();
            let palette = shell.palette.clone();
            palette.update(cx, |palette, cx| palette.set_beads(beads, cx));
        })
        .detach();

        Self {
            store,
            drawer,
            palette,
            selected_row: None,
            g_prefix: false,
            active_view: crate::palette::ViewKind::Ready,
            focus_handle: handle,
        }
    }

    /// Global keys: cmd/ctrl+K and `/` open the palette; `g` then
    /// r/t/g/b/a/m jumps views; j/k move the row selection; Enter opens the
    /// drawer; Esc closes drawer, then palette.
    fn handle_key(
        &mut self,
        event: &gpui_kit::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = event.keystroke.key.as_str();
        let mods = &event.keystroke.modifiers;

        // Cmd/Ctrl+K: open the palette from anywhere.
        if (mods.platform || mods.control) && key == "k" {
            let palette = self.palette.clone();
            window.defer(cx, move |window, cx| {
                palette.update(cx, |palette, cx| palette.open(window, cx));
            });
            return;
        }
        if *mods != gpui_kit::Modifiers::default() {
            return;
        }

        // While the palette is open its overlay owns focus and key handling;
        // nothing reaches the shell root.
        if self.palette.read(cx).is_open() {
            return;
        }

        // `g` prefix mode: next key picks a view.
        if self.g_prefix {
            self.g_prefix = false;
            let view = match key {
                "r" => Some(crate::palette::ViewKind::Ready),
                "t" => Some(crate::palette::ViewKind::Tree),
                "g" => Some(crate::palette::ViewKind::Graph),
                "b" => Some(crate::palette::ViewKind::Board),
                "a" => Some(crate::palette::ViewKind::Activity),
                "m" => Some(crate::palette::ViewKind::Memory),
                _ => None,
            };
            if let Some(view) = view {
                self.go_to_view(view, cx);
            }
            cx.notify();
            return;
        }
        if key == "g" {
            self.g_prefix = true;
            cx.notify();
            return;
        }
        if key == "/" {
            let palette = self.palette.clone();
            window.defer(cx, move |window, cx| {
                palette.update(cx, |palette, cx| palette.open(window, cx));
            });
            return;
        }

        let ready = self.store.read(cx).ready();
        match key {
            "j" => {
                if ready.is_empty() {
                    return;
                }
                let next = self.selected_row.map_or(0, |i| (i + 1).min(ready.len() - 1));
                self.selected_row = Some(next);
                cx.notify();
            }
            "k" => {
                let prev = self
                    .selected_row
                    .map_or(0, |i| i.saturating_sub(1));
                self.selected_row = Some(prev);
                cx.notify();
            }
            "enter" => {
                if let Some(&i) = self.selected_row.as_ref() {
                    if let Some(bead) = ready.get(i) {
                        let id = bead.id.clone();
                        let drawer = self.drawer.clone();
                        window.defer(cx, move |_, cx| {
                            drawer.update(cx, |drawer, cx| drawer.open(&id, cx));
                        });
                    }
                }
            }
            // Palette handles its own Esc; this closes the drawer when no
            // palette is open (the is_open early-return above covers that).
            "escape" => {
                let drawer = self.drawer.clone();
                window.defer(cx, move |_, cx| {
                    drawer.update(cx, |drawer, cx| drawer.close(cx));
                });
            }
            _ => {}
        }
    }

    /// Switch the main pane to a view. Only Ready renders today; the rest
    /// are named markers until their slices land.
    fn go_to_view(&mut self, view: crate::palette::ViewKind, cx: &mut Context<Self>) {
        match view {
            crate::palette::ViewKind::Ready
            | crate::palette::ViewKind::Tree
            | crate::palette::ViewKind::Graph
            | crate::palette::ViewKind::Board
            | crate::palette::ViewKind::Activity
            | crate::palette::ViewKind::Memory => {
                self.active_view = view;
            }
        }
        cx.notify();
    }

}

impl Render for AppShell {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let ready = self.store.read(cx).ready();

        div()
            .id("shell-root")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::handle_key))
            .flex()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(self.render_sidebar(cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_full()
                    .child(self.render_topbar(cx))
                    .child(self.render_ready_list(&ready, cx)),
            )
            .child(self.palette.clone())
    }
}

impl AppShell {
    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        Sidebar::new("sidebar")
            .header(
                SidebarHeader::new().child(
                    div()
                        .text_sm()
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .child("Beads Viewer"),
                ),
            )
            .child(
                SidebarGroup::new("Views").child(
                    SidebarMenu::new().child(
                        SidebarMenuItem::new("Ready")
                            .icon(IconName::ListTodo)
                            .active(true)
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.notify();
                            })),
                    ),
                ),
            )
            .footer(
                SidebarFooter::new().child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("0 saved filters"),
                ),
            )
    }

    fn render_topbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let store = self.store.read(cx);
        let dot = if store.is_synced() {
            state_color(State::Ready)
        } else {
            theme.muted_foreground
        };

        div()
            .flex()
            .items_center()
            .gap_3()
            .px_4()
            .h(px(44.))
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .child("Ready"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(format!("{} beads", store.beads().len())),
            )
            .flex_1()
            .child(div())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(div().size(px(7.)).rounded_full().bg(dot))
                    .child(SharedString::from(store.sync_label().to_string())),
            )
    }

    fn render_ready_list(&self, ready: &[Bead], cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        if ready.is_empty() {
            return div().p_6().text_color(theme.muted_foreground).child(
                if self.store.read(cx).is_synced() {
                    "No ready work. All issues have blocking dependencies."
                } else {
                    "Loading…"
                },
            )
            .into_any_element();
        }

        // Simple scroll container for v1; VirtualList swap-in comes with the
        // live-sync slice when refresh timing matters.
        let selected = self.selected_row;
        let rows = ready
            .iter()
            .enumerate()
            .map(|(i, bead)| self.render_row(bead, Some(i) == selected, i, cx))
            .collect::<Vec<_>>();

        div()
            .id("ready-list")
            .flex_1()
            .overflow_y_scroll()
            .p_3()
            .children(rows)
            .into_any_element()
    }

    fn render_row(
        &self,
        bead: &Bead,
        selected: bool,
        idx: usize,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let theme = cx.theme();
        let color = state_color(bead.state);

        let id = bead.id.clone();
        let row_index = idx;
        div()
            .id(bead.id.clone())
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .py_2()
            .rounded_md()
            .when(selected, |row| {
                row.bg(theme.accent.opacity(0.12))
                    .border_l_2()
                    .border_color(theme.accent)
            })
            .hover(|s| s.bg(theme.accent.opacity(0.06)))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.selected_row = Some(row_index);
                let drawer = this.drawer.clone();
                let id = id.clone();
                window.defer(cx, move |_, cx| {
                    drawer.update(cx, |drawer, cx| drawer.open(&id, cx));
                });
            }))
            .child(div().size(px(9.)).rounded_full().bg(color))
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
            .children(
                bead.labels
                    .iter()
                    .take(2)
                    .map(|l| {
                        div()
                            .text_xs()
                            .px_2()
                            .py_0()
                            .rounded_full()
                            .bg(theme.secondary)
                            .text_color(theme.muted_foreground)
                            .child(l.clone())
                    })
                    .collect::<Vec<_>>(),
            )
            .when(bead.dependent_count > 0, |row| {
                row.child(
                    div()
                        .text_xs()
                        .text_color(state_color(State::Ready))
                        .bg(state_color(State::Ready).opacity(0.12))
                        .px_2()
                        .rounded_full()
                        .child(format!("unblocks {}", bead.dependent_count)),
                )
            })
            .when(bead.assignee.as_deref().is_some_and(|a| !a.is_empty()), |row| {
                row.child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(bead.assignee.clone().unwrap_or_default()),
                )
            })
            .into_any_element()
    }
}


/// Helper indirection: run a palette pick (closes palette, opens drawer/view).
fn shell_palette_pick(
    shell: &mut AppShell,
    action: crate::palette_ui::PaletteAction,
    cx: &mut Context<AppShell>,
) {
    let palette = shell.palette.clone();
    palette.update(cx, |palette, cx| palette.close(cx));
    match action {
        crate::palette_ui::PaletteAction::OpenBead(id) => {
            let drawer = shell.drawer.clone();
            drawer.update(cx, |drawer, cx| drawer.open(&id, cx));
        }
        crate::palette_ui::PaletteAction::GoToView(view) => {
            // Re-borrow shell through the field; run when window available.
            shell.active_view = view;
            cx.notify();
        }
        crate::palette_ui::PaletteAction::ToggleTheme => {
            let mode = if gpui_kit::component::theme::Theme::global(cx).mode.is_dark() {
                gpui_kit::component::theme::ThemeMode::Light
            } else {
                gpui_kit::component::theme::ThemeMode::Dark
            };
            gpui_kit::component::theme::Theme::change(mode, None, cx);
            cx.notify();
        }
    }
}
