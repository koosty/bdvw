//! Command palette overlay: Cmd/Ctrl+K or `/` opens it; fuzzy matches come
//! from the pure `palette` module (S1). The overlay owns keyboard focus:
//! typed characters build the query, j/k and arrows move the selection,
//! Enter picks, Esc closes. No text-input widget, so every key is ours.

use crate::model::Bead;
use crate::palette::{palette_commands, palette_matches, PaletteCommand, PaletteMatch, ViewKind};
use gpui_kit::component::ActiveTheme;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, Context, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    Window, div, px, rgb,
};

/// What the palette asks the shell to do when a match is picked.
#[derive(Debug, Clone)]
pub enum PaletteAction {
    OpenBead(String),
    GoToView(ViewKind),
    ToggleTheme,
}

/// Events the palette emits to its owner.
pub enum PaletteEvent {
    /// User picked the selected match (Enter).
    Pick(PaletteAction),
}

pub struct Palette {
    open: bool,
    query: String,
    matches: Vec<PaletteMatch>,
    selected: Option<usize>,
    /// Snapshot of the beads to jump to; the shell pushes it on refresh.
    beads: Vec<Bead>,
    commands: Vec<PaletteCommand>,
    focus_handle: gpui_kit::FocusHandle,
}

impl gpui_kit::EventEmitter<PaletteEvent> for Palette {}

impl Palette {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            open: false,
            query: String::new(),
            matches: Vec::new(),
            selected: None,
            beads: Vec::new(),
            commands: palette_commands(),
            focus_handle: cx.focus_handle(),
        };
        this.recompute();
        this
    }

    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = true;
        self.query.clear();
        self.recompute();
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.open = false;
        self.query.clear();
        self.matches.clear();
        self.selected = None;
        cx.notify();
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Shell pushes the current bead list so ID/title jumps stay fresh.
    pub fn set_beads(&mut self, beads: Vec<Bead>, cx: &mut Context<Self>) {
        self.beads = beads;
        self.recompute();
        cx.notify();
    }

    fn recompute(&mut self) {
        self.matches = palette_matches(&self.query, &self.commands, &self.beads);
        self.selected = self.matches.first().map(|_| 0);
    }

    /// Key pressed while the palette is open (overlay owns focus).
    /// Returns true if handled; the shell ignores anything unhandled.
    pub fn handle_key(
        &mut self,
        event: &gpui_kit::KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.open {
            return false;
        }
        let ks = &event.keystroke;
        if ks.modifiers != gpui_kit::Modifiers::default() {
            return true; // swallow modified keys while open
        }
        match ks.key.as_str() {
            "escape" => {
                self.close(cx);
                true
            }
            "down" | "j" => {
                let len = self.matches.len();
                if len > 0 {
                    self.selected =
                        Some(self.selected.map_or(0, |i| (i + 1).min(len - 1)));
                    cx.notify();
                }
                true
            }
            "up" | "k" => {
                let len = self.matches.len();
                if len > 0 {
                    self.selected = Some(self.selected.map_or(0, |i| i.saturating_sub(1)));
                    cx.notify();
                }
                true
            }
            "enter" => {
                let action = self.selected_action();
                if let Some(action) = action {
                    cx.emit(PaletteEvent::Pick(action));
                }
                cx.notify();
                true
            }
            "backspace" => {
                self.query.pop();
                self.recompute();
                cx.notify();
                true
            }
            _ => {
                if let Some(ch) = ks.key_char.as_deref() {
                    // Single printable characters extend the query.
                    let mut chars = ch.chars();
                    if let (Some(first), None) = (chars.next(), chars.next()) {
                        self.query.push(first);
                        self.recompute();
                        cx.notify();
                    }
                }
                true // the palette consumes all plain keys while open
            }
        }
    }

    fn selected_action(&self) -> Option<PaletteAction> {
        let m = self.matches.get(self.selected?)?;
        if m.id().is_empty() {
            self.commands
                .iter()
                .find(|c| c.label() == m.label())
                .map(|c| match c {
                    PaletteCommand::View(v) => PaletteAction::GoToView(*v),
                    PaletteCommand::ToggleTheme => PaletteAction::ToggleTheme,
                })
        } else {
            Some(PaletteAction::OpenBead(m.id().to_string()))
        }
    }
}

impl Render for Palette {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        if !self.open {
            return div().into_any_element();
        }

        let selected = self.selected;
        let rows = self
            .matches
            .iter()
            .take(10)
            .enumerate()
            .map(|(i, m)| render_match_row(m, Some(i) == selected, &theme))
            .collect::<Vec<_>>();

        div()
            .id("palette-overlay")
            .on_key_down(cx.listener(|this, event, window, cx| {
                this.handle_key(event, window, cx);
            }))
            .absolute()
            .size_full()
            .top_0()
            .left_0()
            .bg(rgb(0x000000).opacity(0.35))
            .flex()
            .justify_center()
            .items_start()
            .pt(px(120.))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                this.close(cx);
            }))
            .child(
                div()
                    .id("palette-panel")
                    .track_focus(&self.focus_handle)
                    .w(px(560.))
                    .max_h(px(420.))
                    .overflow_hidden()
                    .rounded_lg()
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.background)
                    .flex()
                    .flex_col()
                    .on_mouse_down(MouseButton::Left, |_, _, _| {
                        // swallow overlay close for clicks inside the panel
                    })
                    .child(
                        div()
                            .id("palette-query")
                            .p_3()
                            .border_b_1()
                            .border_color(theme.border)
                            .text_sm()
                            .when(self.query.is_empty(), |q| {
                                q.text_color(theme.muted_foreground)
                                    .child(SharedString::from("Search beads, views, actions…"))
                            })
                            .when(!self.query.is_empty(), |q| {
                                q.child(SharedString::from(self.query.clone()))
                            }),
                    )
                    .child(
                        div()
                            .id("palette-matches")
                            .flex_1()
                            .overflow_y_scroll()
                            .p_1()
                            .children(rows),
                    ),
            )
            .into_any_element()
    }
}

fn render_match_row(m: &PaletteMatch, selected: bool, theme: &gpui_kit::component::theme::Theme) -> AnyElement {
    div()
        .id(SharedString::from(format!(
            "palette-row-{}-{}",
            m.id(),
            m.label()
        )))
        .flex()
        .items_center()
        .gap_2()
        .px_3()
        .py_2()
        .rounded_md()
        .when(selected, |row| row.bg(theme.accent.opacity(0.12)))
        .child(div().text_sm().child(match m.title() {
            // Bead jump: "ID title"; command: plain label.
            Some(title) => SharedString::from(format!("{}  {}", m.id(), title)),
            None => SharedString::from(m.label().to_string()),
        }))
        .into_any_element()
}
