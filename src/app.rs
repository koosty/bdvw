//! App shell: three-pane layout (Sidebar, top bar, drawer column)
//! with the Ready list as the home view. Ticket bdui-rs-oga.

use crate::client::BeadsClient as _;
use crate::model::{Bead, State};
use crate::store::Store;
use gpui_kit::assets::IconName;
use gpui_kit::component::clipboard::Clipboard;
use gpui_kit::component::sidebar::{
    Sidebar, SidebarFooter, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem,
};
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::TitleBar;
use gpui_kit::component::WindowExt as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::AppContext as _;
use gpui_kit::{
    div, px, rgb, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window,
};

/// Colors keyed to derived state (CONTEXT.md: State), not priority.
/// Dark-mode-first palette from the prototype's tokens.
pub fn state_color(state: &State) -> gpui_kit::Hsla {
    match state {
        State::Ready => rgb(0x3ccf7e).into(),      // --ready (dark)
        State::InProgress => rgb(0x6aa0ff).into(), // --progress
        State::Blocked => rgb(0xff6b5e).into(),    // --blocked
        State::Deferred => rgb(0xb794f4).into(),   // --deferred
        State::Closed => rgb(0x6b7486).into(),     // --closed
        // Unmapped status: neutral, so it never reads as ready work.
        State::Custom(_) => rgb(0x9aa4b2).into(),
    }
}

/// Colors for activity event kinds, distinct from the state palette so a
/// timeline reads at a glance in both themes.
pub fn event_color(kind: &str) -> gpui_kit::Hsla {
    match kind {
        "created" => rgb(0x6aa0ff).into(),
        "claimed" => rgb(0x3ccf7e).into(),
        "closed" => rgb(0x6b7486).into(),
        "reopened" => rgb(0xffb454).into(),
        "dependency_added" => rgb(0xb794f4).into(),
        "dependency_removed" => rgb(0xff6b5e).into(),
        "label_added" | "label_removed" => rgb(0x8ab4f8).into(),
        _ => rgb(0x9aa4b2).into(),
    }
}

/// Issue-type label color: bugs, epics, features get state hues; others stay muted.
pub fn type_color(issue_type: &str, muted: gpui_kit::Hsla) -> gpui_kit::Hsla {
    match issue_type {
        "bug" => state_color(&State::Blocked),
        "epic" => state_color(&State::Deferred),
        "feature" => state_color(&State::InProgress),
        _ => muted,
    }
}

/// Flip between light and dark. Shared by the topbar button and the palette.
fn toggle_theme(cx: &mut gpui_kit::App) {
    let mode = if gpui_kit::component::theme::Theme::global(cx).mode.is_dark() {
        gpui_kit::component::theme::ThemeMode::Light
    } else {
        gpui_kit::component::theme::ThemeMode::Dark
    };
    gpui_kit::component::theme::Theme::change(mode, None, cx);
    cx.refresh_windows();
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
    /// Graph view; reloaded when the view activates.
    graph_view: Entity<crate::graph_view::GraphView>,
    /// Tree view; refed from the store on every change (bdui-rs-8db).
    tree_view: Entity<crate::tree_view::TreeView>,
    /// Board view; same feed as the tree (bdui-rs-nx8).
    board_view: Entity<crate::board_view::BoardView>,
    /// Activity feed; refetched while Activity is active (bdui-rs-2u6).
    activity_view: Entity<crate::activity_view::ActivityView>,
    /// One activity fetch at a time while the view is active.
    activity_in_flight: bool,
    /// Set when the feed is requested mid-sweep; the sweep reruns on completion.
    activity_dirty: bool,
    /// Per-bead history memo (bdui-rs-2u6): refetch only changed beads.
    activity_cache: crate::activity::HistoryCache,
    /// Search input state for the shared filter surface.
    search_input: gpui_kit::Entity<gpui_kit::component::input::InputState>,
    drawer: Entity<crate::drawer::Drawer>,
    /// Command palette overlay (bdui-rs-ue2).
    palette: Entity<crate::palette_ui::Palette>,
    /// Index into the ready list for j/k navigation. None = no selection.
    selected_row: Option<usize>,
    /// Active tab of the Ready view.
    ready_tab: crate::ready::ReadyTab,
    /// Pending `g` prefix: the next key jump targets a view.
    g_prefix: bool,
    /// Which view the main pane shows; only Ready is rendered today.
    active_view: crate::palette::ViewKind,
    client: std::rc::Rc<dyn BeadsClientShim>,
    focus_handle: gpui_kit::FocusHandle,
}

/// Transport for the shell's initial load. The Store owns refresh state;
/// the shell needs the client once, at startup.
pub trait BeadsClientShim {
    fn refresh(&self, store: &mut Store, cx: &mut Context<Store>);
    /// `bd graph --all --json`, parsed. The graph view fetches on activation.
    fn graph(&self) -> Result<crate::graph::GraphData, String>;
    /// The shared CLI client for the 2s poll loop (bdui-rs-7lr). The client
    /// itself serializes invocations.
    fn poll_client(&self) -> std::sync::Arc<crate::client::CliClient>;
}

impl AppShell {
    pub fn new(
        store: Entity<Store>,
        client: std::rc::Rc<dyn BeadsClientShim>,
        drawer: Entity<crate::drawer::Drawer>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let graph_view = cx.new(|_| crate::graph_view::GraphView::new());
        let tree_view = cx.new(|_| crate::tree_view::TreeView::new());
        // Tree row clicks open the detail drawer.
        cx.subscribe(&tree_view, |shell, _, event, cx| {
            let crate::tree_view::TreeViewEvent::NodeClicked(id) = event;
            let drawer = shell.drawer.clone();
            drawer.update(cx, |drawer, cx| drawer.open(id, cx));
        })
        .detach();
        let board_view = cx.new(|_| crate::board_view::BoardView::new());
        // Board card clicks open the detail drawer.
        cx.subscribe(&board_view, |shell, _, event, cx| {
            let crate::board_view::BoardViewEvent::CardClicked(id) = event;
            let drawer = shell.drawer.clone();
            drawer.update(cx, |drawer, cx| drawer.open(id, cx));
        })
        .detach();
        let activity_view = cx.new(|_| crate::activity_view::ActivityView::new());
        // Activity entries open the drawer on their bead.
        cx.subscribe(&activity_view, |shell, _, event, cx| {
            let crate::activity_view::ActivityViewEvent::EntryClicked(id) = event;
            let drawer = shell.drawer.clone();
            drawer.update(cx, |drawer, cx| drawer.open(id, cx));
        })
        .detach();
        // Graph node clicks open the detail drawer.
        cx.subscribe(&graph_view, |shell, _, event, cx| {
            if let crate::graph_view::GraphViewEvent::NodeClicked(id) = event {
                let drawer = shell.drawer.clone();
                drawer.update(cx, |drawer, cx| drawer.open(id, cx));
            }
        })
        .detach();
        let handle = cx.focus_handle();
        window.focus(&handle, cx);
        let palette = cx.new(|cx| crate::palette_ui::Palette::new(handle.clone(), cx));
        // Palette picks dispatch to the shell.
        cx.subscribe(&palette, |shell, _, event, cx| {
            let crate::palette_ui::PaletteEvent::Pick(action) = event;
            shell_palette_pick(shell, action.clone(), cx);
        })
        .detach();
        // Keep palette bead jumps in sync with store refreshes, and reload
        // the graph when its data may have changed while it is the active
        // view (the store observe fires on every poll apply).
        cx.observe(&store, |shell, store, cx| {
            let beads = store.read(cx).ready().to_vec();
            let palette = shell.palette.clone();
            palette.update(cx, |palette, cx| palette.set_beads(beads, cx));
            // The tree and board always track the store, not just when
            // visible: filters and data stay live across view jumps.
            {
                let all = store.read(cx).beads().to_vec();
                let filters = store.read(cx).filters().clone();
                let tree_view = shell.tree_view.clone();
                tree_view.update(cx, |tv, cx| tv.set_data(all.clone(), filters.clone(), cx));
                let board_view = shell.board_view.clone();
                board_view.update(cx, |bv, cx| bv.set_data(all, filters, cx));
            }
            if let crate::palette::ViewKind::Graph = shell.active_view {
                let graph_view = shell.graph_view.clone();
                let client = shell.client.clone();
                let data = client.graph();
                graph_view.update(cx, |gv, cx| match data {
                    Ok(data) => gv.reload(&data, cx),
                    Err(e) => gv.reload_failed(e, cx),
                });
            }
            if let crate::palette::ViewKind::Activity = shell.active_view {
                let client = shell.client.clone();
                shell.refresh_activity(client.as_ref(), cx);
            }
        })
        .detach();
        // Initial load runs after the observer exists so its notify reaches
        // the tree and board; a load before subscribing never delivers data.
        store.update(cx, |store, cx| client.refresh(store, cx));

        // Shared filter surface: search input state drives Store.filters
        // (bdui-rs-8db).
        let search_input = cx.new(|cx| {
            gpui_kit::component::input::InputState::new(window, cx)
                .placeholder("Search id, title, label, assignee…")
        });
        cx.subscribe(
            &search_input,
            |shell, input, event: &gpui_kit::component::input::InputEvent, cx| {
                if matches!(event, gpui_kit::component::input::InputEvent::Change) {
                    let q = input.read(cx).value().to_string();
                    let store = shell.store.clone();
                    store.update(cx, |store, cx| {
                        let mut f = store.filters().clone();
                        f.query = q;
                        store.set_filters(f, cx);
                    });
                }
            },
        )
        .detach();

        // Live-sync poll (bdui-rs-7lr): every 2s fetch the list off-thread,
        // diff into the store, refresh the open drawer's detail. The CLI
        // client serializes invocations internally.
        {
            let client = std::sync::Arc::clone(&client.poll_client());
            cx.spawn(async move |shell, cx| loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(2))
                    .await;
                let fetch = std::sync::Arc::clone(&client);
                let fetched = cx.background_spawn(async move { fetch.list() }).await;
                let shell = shell.clone();
                cx.update(|cx| {
                    let Some(shell) = shell.upgrade() else { return };
                    shell.update(cx, |shell, cx| match fetched {
                        Ok(beads) => {
                            let store = shell.store.clone();
                            let drawer = shell.drawer.clone();
                            let changed =
                                store.update(cx, |store, cx| store.apply_beads(beads, cx));
                            if changed {
                                if let Some(_id) = drawer.read(cx).open_id() {
                                    drawer.update(cx, |drawer, cx| drawer.refresh(cx));
                                }
                            }
                        }
                        Err(e) => {
                            let store = shell.store.clone();
                            store.update(cx, |store, cx| store.set_sync_error(e.to_string(), cx));
                        }
                    })
                });
            })
            .detach();
        }

        Self {
            store,
            graph_view,
            tree_view,
            board_view,
            activity_view,
            activity_in_flight: false,
            activity_dirty: false,
            activity_cache: crate::activity::HistoryCache::default(),
            search_input,
            drawer,
            palette,
            selected_row: None,
            ready_tab: crate::ready::ReadyTab::default(),
            g_prefix: false,
            active_view: crate::palette::ViewKind::Ready,
            client,
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

        // While a text input (filter search) is focused, plain keys type
        // text; only palette/dismiss chords stay global (bdui-rs-8db).
        if window.has_focused_input(cx) && !(mods.platform || mods.control) {
            return;
        }

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

        // Esc closes the drawer from every view. The view branches below
        // return early and would otherwise swallow it.
        if key == "escape" {
            let drawer = self.drawer.clone();
            window.defer(cx, move |_, cx| {
                drawer.update(cx, |drawer, cx| drawer.close(cx));
            });
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
                let client = self.client.clone();
                self.go_to_view(view, client.as_ref(), cx);
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

        // On the Graph view, j/k drive node selection instead.
        if self.active_view == crate::palette::ViewKind::Graph {
            let gv = self.graph_view.clone();
            match key {
                "j" => gv.update(cx, |gv, cx| gv.move_selection(true, cx)),
                "k" => gv.update(cx, |gv, cx| gv.move_selection(false, cx)),
                "enter" => {
                    let id = gv.read(cx).selected().map(|s| s.to_string());
                    if let Some(id) = id {
                        let drawer = self.drawer.clone();
                        window.defer(cx, move |_, cx| {
                            drawer.update(cx, |drawer, cx| drawer.open(&id, cx));
                        });
                    }
                }
                _ => {}
            }
            return;
        }

        // Activity view owns j/k/Enter over its entries (bdui-rs-2u6).
        if self.active_view == crate::palette::ViewKind::Activity {
            let activity = self.activity_view.clone();
            match key {
                "j" => activity.update(cx, |av, cx| av.move_selection(1, cx)),
                "k" => activity.update(cx, |av, cx| av.move_selection(-1, cx)),
                "enter" => activity.update(cx, |av, cx| av.activate(cx)),
                _ => {}
            }
            return;
        }

        // Board view owns j/k/Enter over its cards (bdui-rs-nx8).
        if self.active_view == crate::palette::ViewKind::Board {
            let board = self.board_view.clone();
            match key {
                "j" => board.update(cx, |bv, cx| bv.move_selection(1, cx)),
                "k" => board.update(cx, |bv, cx| bv.move_selection(-1, cx)),
                "enter" => board.update(cx, |bv, cx| bv.activate(cx)),
                _ => {}
            }
            return;
        }

        // Tree view owns j/k/Enter over its rows (bdui-rs-8db).
        if self.active_view == crate::palette::ViewKind::Tree {
            let tree = self.tree_view.clone();
            match key {
                "j" => tree.update(cx, |tv, cx| tv.move_selection(1, cx)),
                "k" => tree.update(cx, |tv, cx| tv.move_selection(-1, cx)),
                "enter" => tree.update(cx, |tv, cx| tv.activate(cx)),
                _ => {}
            }
            return;
        }

        let ready = self.ready_rows(cx);
        match key {
            "j" => {
                if ready.is_empty() {
                    return;
                }
                let next = self
                    .selected_row
                    .map_or(0, |i| (i + 1).min(ready.len() - 1));
                self.selected_row = Some(next);
                cx.notify();
            }
            "k" => {
                let prev = self.selected_row.map_or(0, |i| i.saturating_sub(1));
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
            _ => {}
        }
    }

    /// Switch the main pane to a view. Ready and Graph render; the rest
    /// are named markers until their slices land. Entering Graph fetches
    /// fresh `bd graph --all --json`.
    fn go_to_view(
        &mut self,
        view: crate::palette::ViewKind,
        client: &dyn BeadsClientShim,
        cx: &mut Context<Self>,
    ) {
        if view == crate::palette::ViewKind::Graph {
            let data = client.graph();
            let gv = self.graph_view.clone();
            gv.update(cx, |gv, cx| match data {
                Ok(data) => gv.reload(&data, cx),
                Err(e) => gv.reload_failed(e, cx),
            });
        }
        if view == crate::palette::ViewKind::Activity {
            self.refresh_activity(client, cx);
        }
        self.active_view = view;
        cx.notify();
    }

    /// Refresh the activity feed (bdui-rs-2u6). Only beads whose `updated_at`
    /// moved since the last sweep are refetched; the rest come from the memo.
    /// One sweep in flight at a time. A request that arrives mid-sweep sets
    /// `activity_dirty` and reruns on completion, so no store change is lost.
    fn refresh_activity(&mut self, client: &dyn BeadsClientShim, cx: &mut Context<Self>) {
        if self.activity_in_flight {
            self.activity_dirty = true;
            return;
        }
        self.activity_in_flight = true;
        let beads = self.store.read(cx).beads().to_vec();
        let stale = self.activity_cache.stale(&beads);
        let cache = std::mem::take(&mut self.activity_cache);
        let poll = client.poll_client();
        let activity = self.activity_view.clone();
        let total = stale.len();
        if total > 0 {
            activity.update(cx, |av, cx| av.set_progress(0, total, cx));
        }
        cx.spawn(async move |shell, cx| {
            let mut cache = cache;
            for (done, (id, updated_at)) in stale.into_iter().enumerate() {
                let poll = std::sync::Arc::clone(&poll);
                let fetch_id = id.clone();
                // Failed fetches stay unmemoized and retry next sweep.
                let fetched = cx
                    .background_spawn(async move { poll.history(&fetch_id) })
                    .await;
                if let Ok(evs) = fetched {
                    cache.insert(id, updated_at, evs);
                }
                cx.update(|cx| {
                    activity.update(cx, |av, cx| av.set_progress(done + 1, total, cx));
                });
            }
            let shell = shell.clone();
            cx.update(|cx| {
                let Some(shell) = shell.upgrade() else { return };
                shell.update(cx, |shell, cx| {
                    shell.activity_in_flight = false;
                    let mut cache = cache;
                    cache.prune(&beads);
                    let events = cache.events();
                    shell.activity_cache = cache;
                    activity.update(cx, |av, cx| av.set_feed(&beads, events, cx));
                    if std::mem::take(&mut shell.activity_dirty) {
                        let client = shell.client.clone();
                        shell.refresh_activity(client.as_ref(), cx);
                    }
                })
            });
        })
        .detach();
    }
}

impl Render for AppShell {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        // The Ready list respects the shared filter surface (bdui-rs-8db).
        let ready = self.ready_rows(cx);

        div()
            .id("shell-root")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::handle_key))
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(TitleBar::new())
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.render_sidebar(cx))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .child(self.render_topbar(cx))
                            .child(
                                div()
                                    .id("main-pane-slot")
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_h_0()
                                    .child(self.render_main_pane(&ready, cx)),
                            ),
                    )
                    .child(self.drawer.clone()),
            )
            .child(self.palette.clone())
    }
}

impl AppShell {
    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let shell = cx.entity().clone();
        let (ready_count, sync_label) = {
            let store = self.store.read(cx);
            (
                crate::ready::tab_count(&store.filtered(), crate::ready::ReadyTab::Ready),
                store.sync_label(),
            )
        };
        let source = std::env::current_dir()
            .ok()
            .and_then(|dir| {
                dir.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "bd".to_string());

        let views = [
            (
                crate::palette::ViewKind::Ready,
                "Ready",
                IconName::CircleCheck,
            ),
            (crate::palette::ViewKind::Tree, "Tree", IconName::FolderOpen),
            (crate::palette::ViewKind::Graph, "Graph", IconName::Network),
            (
                crate::palette::ViewKind::Board,
                "Board",
                IconName::LayoutDashboard,
            ),
            (
                crate::palette::ViewKind::Activity,
                "Activity",
                IconName::Bell,
            ),
        ];
        let menu = views
            .into_iter()
            .fold(SidebarMenu::new(), |menu, (view, label, icon)| {
                let shell = shell.clone();
                let item = SidebarMenuItem::new(label)
                    .icon(icon)
                    .active(self.active_view == view)
                    .on_click(move |_, _, cx| {
                        shell.update(cx, |this, cx| {
                            let client = this.client.clone();
                            this.go_to_view(view, client.as_ref(), cx);
                        });
                    });
                let item = if view == crate::palette::ViewKind::Ready {
                    item.suffix(move |_, _| div().text_xs().child(ready_count.to_string()))
                } else {
                    item
                };
                menu.child(item)
            });

        Sidebar::new("sidebar")
            .header(
                SidebarHeader::new().child(
                    div()
                        .text_sm()
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .child("Beads Viewer"),
                ),
            )
            .child(SidebarGroup::new("Views").child(menu))
            .footer(
                SidebarFooter::new().child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(SharedString::from(format!("bd · {source}")))
                        .child(SharedString::from(sync_label))
                        .child("⌘K palette · / search")
                        .child("j/k move · g then r t g b a views"),
                ),
            )
    }

    /// The active view pane: Ready list or Graph. Others render a marker
    /// until their slices land.
    fn render_main_pane(&self, ready: &[Bead], cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        match self.active_view {
            crate::palette::ViewKind::Ready => self.render_ready_list(ready, cx).into_any_element(),
            crate::palette::ViewKind::Graph => self.graph_view.clone().into_any_element(),
            crate::palette::ViewKind::Tree => self.tree_view.clone().into_any_element(),
            crate::palette::ViewKind::Board => self.board_view.clone().into_any_element(),
            crate::palette::ViewKind::Activity => self.activity_view.clone().into_any_element(),
            view => {
                let theme = cx.theme().clone();
                div()
                    .id(SharedString::from(format!("view-marker-{view:?}")))
                    .flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .text_color(theme.muted_foreground)
                    .child(SharedString::from(format!(
                        "{view:?} view lands in a later slice"
                    )))
                    .into_any_element()
            }
        }
    }

    fn render_topbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (dot, bead_count, sync_label) = {
            let store = self.store.read(cx);
            (
                if store.is_synced() {
                    state_color(&State::Ready)
                } else {
                    theme.muted_foreground
                },
                store.beads().len(),
                store.sync_label(),
            )
        };

        let shell = cx.entity().clone();
        let dark = gpui_kit::component::theme::Theme::global(cx).mode.is_dark();
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
                    .child(format!("{bead_count} beads")),
            )
            // Shared filter surface (bdui-rs-8db).
            .child(
                div()
                    .id("filter-search")
                    .flex()
                    .items_center()
                    .w(px(240.))
                    .child(gpui_kit::component::input::Input::new(&self.search_input)),
            )
            .child(self.render_filter_menus(cx))
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(div().size(px(7.)).rounded_full().bg(dot))
                    .child(SharedString::from(sync_label)),
            )
            .child(
                gpui_kit::component::button::Button::new("open-palette")
                    .icon(IconName::SquareTerminal)
                    .outline()
                    .compact()
                    .on_click(move |_, window, cx| {
                        shell.update(cx, |this, cx| {
                            let palette = this.palette.clone();
                            palette.update(cx, |palette, cx| palette.open(window, cx));
                        });
                    }),
            )
            .child(
                gpui_kit::component::button::Button::new("toggle-theme")
                    .icon(if dark { IconName::Sun } else { IconName::Moon })
                    .outline()
                    .compact()
                    .on_click(|_, _, cx| toggle_theme(cx)),
            )
    }

    /// Type/priority/assignee filter popovers + closed toggle. Options are
    /// derived from the current bead set like the prototype's select
    /// population.
    fn render_filter_menus(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let theme = cx.theme().clone();
        let (types, who, filters, show_closed) = {
            let store = self.store.read(cx);
            let filters = store.filters().clone();
            let types: Vec<String> = {
                let mut v: Vec<String> = store
                    .beads()
                    .iter()
                    .map(|b| b.issue_type.clone())
                    .filter(|t| !t.is_empty())
                    .collect();
                v.sort();
                v.dedup();
                v
            };

            let who: Vec<String> = {
                let mut v: Vec<String> = store
                    .beads()
                    .iter()
                    .filter_map(|b| b.assignee.clone())
                    .collect();
                v.sort();
                v.dedup();
                v
            };
            (types, who, filters.clone(), filters.show_closed)
        };
        let store_handle = self.store.clone();

        let type_label = filters
            .issue_type
            .clone()
            .unwrap_or_else(|| "All types".into());
        let prio_label = filters
            .priority
            .map(|p| format!("P{p}"))
            .unwrap_or_else(|| "Any priority".into());
        let who_label = match &filters.assignee {
            None => "Anyone".to_string(),
            Some(None) => "Unassigned".to_string(),
            Some(Some(w)) => w.clone(),
        };
        let _ = show_closed;

        // Option rows via a generic helper (impl Trait cannot be a closure
        // parameter).
        // Option lists build with plain loops: filter_option needs &mut cx
        // per call, which closures over .map cannot provide.
        let mut type_opts: Vec<gpui_kit::AnyElement> = vec![Self::filter_option(
            cx,
            "All types",
            filters.issue_type.is_none(),
            store_handle.clone(),
            |f| f.issue_type = None,
        )];
        for t in &types {
            let t = t.clone();
            type_opts.push(Self::filter_option(
                cx,
                &t.clone(),
                filters.issue_type.as_deref() == Some(t.as_str()),
                store_handle.clone(),
                move |f| f.issue_type = Some(t.clone()),
            ));
        }

        let mut prio_opts: Vec<gpui_kit::AnyElement> = Vec::new();
        for p in 0..=4u8 {
            let store = store_handle.clone();
            prio_opts.push(
                div()
                    .id(SharedString::from(format!("filter-opt-P{p}")))
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .text_xs()
                    .when(filters.priority == Some(p), |el| {
                        el.bg(theme.accent.opacity(0.12))
                            .text_color(theme.foreground)
                    })
                    .hover(|s| s.bg(theme.accent.opacity(0.06)))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        store.update(cx, |store, cx| {
                            let mut f = store.filters().clone();
                            f.priority = Some(p);
                            store.set_filters(f, cx);
                        });
                    }))
                    .child(SharedString::from(format!("P{p}")))
                    .into_any_element(),
            );
        }
        prio_opts.push(Self::filter_option(
            cx,
            "Any priority",
            filters.priority.is_none(),
            store_handle.clone(),
            |f| f.priority = None,
        ));

        let mut who_opts: Vec<gpui_kit::AnyElement> = vec![
            Self::filter_option(
                cx,
                "Anyone",
                filters.assignee.is_none(),
                store_handle.clone(),
                |f| f.assignee = None,
            ),
            Self::filter_option(
                cx,
                "Unassigned",
                filters.assignee == Some(None),
                store_handle.clone(),
                |f| f.assignee = Some(None),
            ),
        ];
        for w in &who {
            let w = w.clone();
            who_opts.push(Self::filter_option(
                cx,
                &w.clone(),
                matches!(&filters.assignee, Some(Some(x)) if x == &w),
                store_handle.clone(),
                move |f| f.assignee = Some(Some(w.clone())),
            ));
        }

        // Popover triggers must implement Selectable: Button does.
        let trigger = |id: &'static str, label: String| {
            gpui_kit::component::button::Button::new(id)
                .label(gpui_kit::SharedString::from(label))
                .outline()
                .compact()
        };

        let type_menu: gpui_kit::AnyElement = div()
            .flex()
            .flex_col()
            .p_1()
            .w(px(150.))
            .children(type_opts)
            .into_any_element();
        let prio_menu: gpui_kit::AnyElement = div()
            .flex()
            .flex_col()
            .p_1()
            .w(px(140.))
            .children(prio_opts)
            .into_any_element();
        let who_menu: gpui_kit::AnyElement = div()
            .flex()
            .flex_col()
            .p_1()
            .w(px(150.))
            .children(who_opts)
            .into_any_element();

        let closed_toggle = {
            let theme = cx.theme();
            let store = store_handle.clone();
            let on = filters.show_closed;
            div()
                .id("filter-closed-toggle")
                .px_2()
                .py_1()
                .rounded_md()
                .text_xs()
                .border_1()
                .border_color(if on { theme.accent } else { theme.border })
                .when(on, |el| {
                    el.bg(theme.accent.opacity(0.12))
                        .text_color(theme.foreground)
                })
                .hover(|s| s.bg(theme.accent.opacity(0.08)))
                .on_click(cx.listener(move |_, _, _, cx| {
                    store.update(cx, |store, cx| {
                        let mut f = store.filters().clone();
                        f.show_closed = !f.show_closed;
                        store.set_filters(f, cx);
                    });
                }))
                .child("Closed")
        };

        div()
            .id("filter-menus")
            .flex()
            .items_center()
            .gap_2()
            .child(
                gpui_kit::component::popover::Popover::new("filter-type")
                    .trigger(trigger("filter-type-trigger", type_label))
                    .child(type_menu),
            )
            .child(
                gpui_kit::component::popover::Popover::new("filter-prio")
                    .trigger(trigger("filter-prio-trigger", prio_label))
                    .child(prio_menu),
            )
            .child(
                gpui_kit::component::popover::Popover::new("filter-who")
                    .trigger(trigger("filter-who-trigger", who_label))
                    .child(who_menu),
            )
            .child(closed_toggle)
            .into_any_element()
    }

    /// Rows for the Ready view's active tab, after the shared filters.
    fn ready_rows(&self, cx: &Context<Self>) -> Vec<Bead> {
        let visible = self.store.read(cx).filtered_for_tabs();
        crate::ready::tab_rows(&visible, self.ready_tab)
    }

    fn select_ready_tab(&mut self, tab: crate::ready::ReadyTab, cx: &mut Context<Self>) {
        self.ready_tab = tab;
        self.selected_row = None;
        cx.notify();
    }

    /// Ready view: state tabs, suggested-next card, then the active tab's rows.
    fn render_ready_list(&self, ready: &[Bead], cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (counts, synced) = {
            let store = self.store.read(cx);
            let visible = store.filtered_for_tabs();
            let counts = crate::ready::ReadyTab::ALL
                .map(|tab| (tab, crate::ready::tab_count(&visible, tab)));
            (counts, store.is_synced())
        };

        let tabs = counts.map(|(tab, count)| {
            let on = tab == self.ready_tab;
            div()
                .id(SharedString::from(format!("ready-tab-{}", tab.label())))
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py_1()
                .rounded_full()
                .border_1()
                .border_color(if on { theme.accent } else { theme.border })
                .when(on, |el| el.bg(theme.accent.opacity(0.12)))
                .text_sm()
                .text_color(if on {
                    theme.foreground
                } else {
                    theme.muted_foreground
                })
                .hover(|s| s.bg(theme.accent.opacity(0.06)))
                .on_click(cx.listener(move |this, _, _, cx| this.select_ready_tab(tab, cx)))
                .child(
                    div()
                        .size(px(7.))
                        .rounded_full()
                        .bg(state_color(&tab.state())),
                )
                .child(tab.label())
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(count.to_string()),
                )
        });

        // Suggested next only makes sense on the Ready tab, where `ready` is
        // in ready-rank order.
        let suggested = if self.ready_tab == crate::ready::ReadyTab::Ready {
            crate::ready::suggested_next(ready).map(|bead| {
                let accent = state_color(&State::Ready);
                let claim = format!("bd update {} --claim", bead.id);
                div()
                    .id("suggested-next")
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_3()
                    .py_2()
                    .rounded_lg()
                    .border_1()
                    .border_color(accent.opacity(0.4))
                    .bg(accent.opacity(0.08))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui_kit::FontWeight::BOLD)
                            .text_color(accent)
                            .child("SUGGESTED NEXT"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_family(theme.mono_font_family.clone())
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
                    .child(
                        Clipboard::new(SharedString::from(format!("claim-{}", bead.id)))
                            .value(SharedString::from(claim)),
                    )
            })
        } else {
            None
        };

        let list = if ready.is_empty() {
            let message = if !synced {
                "Loading…"
            } else if self.ready_tab == crate::ready::ReadyTab::Ready {
                "No ready work. All issues have blocking dependencies."
            } else {
                "Nothing in this tab."
            };
            div()
                .p_6()
                .text_color(theme.muted_foreground)
                .child(message)
                .into_any_element()
        } else {
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
                .min_h_0()
                .overflow_y_scroll()
                .border_1()
                .border_color(theme.border)
                .rounded_lg()
                .children(rows)
                .into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .p_3()
            .gap_3()
            .child(div().flex().flex_wrap().gap_2().children(tabs))
            .children(suggested)
            .child(list)
    }

    /// One popover option row; click applies `set` to the store filters.
    fn filter_option<F>(
        cx: &mut Context<Self>,
        label: &str,
        current: bool,
        store: gpui_kit::Entity<Store>,
        set: F,
    ) -> gpui_kit::AnyElement
    where
        F: Fn(&mut crate::tree::Filters) + 'static,
    {
        let theme = cx.theme();
        div()
            .id(SharedString::from(format!("filter-opt-{label}")))
            .px_2()
            .py_1()
            .rounded_sm()
            .text_xs()
            .when(current, |el| {
                el.bg(theme.accent.opacity(0.12))
                    .text_color(theme.foreground)
            })
            .hover(|s| s.bg(theme.accent.opacity(0.06)))
            .on_click(cx.listener(move |_, _, _, cx| {
                store.update(cx, |store, cx| {
                    let mut f = store.filters().clone();
                    set(&mut f);
                    store.set_filters(f, cx);
                });
            }))
            .child(SharedString::from(label.to_string()))
            .into_any_element()
    }

    fn render_row(
        &self,
        bead: &Bead,
        selected: bool,
        idx: usize,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let theme = cx.theme().clone();
        let color = state_color(&bead.state);
        let priority = priority_color(bead.priority);

        let id = bead.id.clone();
        let row_index = idx;
        div()
            .id(bead.id.clone())
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .py(px(6.))
            .when(idx > 0, |row| row.border_t_1().border_color(theme.border))
            .when(selected, |row| {
                row.bg(theme.primary.opacity(0.12))
                    .border_l_2()
                    .border_color(theme.primary)
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
                    .text_color(priority)
                    .border_1()
                    .border_color(priority)
                    .rounded_sm()
                    .px_1()
                    .child(format!("P{}", bead.priority)),
            )
            .child(
                div()
                    .text_xs()
                    .font_family(theme.mono_font_family.clone())
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
                        .text_color(state_color(&State::Ready))
                        .bg(state_color(&State::Ready).opacity(0.12))
                        .px_2()
                        .rounded_full()
                        .child(format!("unblocks {}", bead.dependent_count)),
                )
            })
            .when(!bead.issue_type.is_empty(), |row| {
                row.child(
                    div()
                        .text_xs()
                        .text_color(type_color(&bead.issue_type, theme.muted_foreground))
                        .child(bead.issue_type.to_uppercase()),
                )
            })
            .when(
                bead.assignee.as_deref().is_some_and(|a| !a.is_empty()),
                |row| {
                    row.child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(bead.assignee.clone().unwrap_or_default()),
                    )
                },
            )
            .into_any_element()
    }
}

/// Helper indirection: run a palette pick (closes palette, opens drawer/view).
fn shell_palette_pick(
    shell: &mut AppShell,
    action: crate::palette_ui::PaletteAction,
    cx: &mut Context<AppShell>,
) {
    match action {
        crate::palette_ui::PaletteAction::OpenBead(id) => {
            let drawer = shell.drawer.clone();
            drawer.update(cx, |drawer, cx| drawer.open(&id, cx));
        }
        crate::palette_ui::PaletteAction::GoToView(view) => {
            let client = shell.client.clone();
            shell.go_to_view(view, client.as_ref(), cx);
        }
        crate::palette_ui::PaletteAction::ToggleTheme => {
            toggle_theme(cx);
            cx.notify();
        }
    }
}
