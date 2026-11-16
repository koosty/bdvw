//! Graph view: the dependency DAG rendered from `bd graph --all --json`
//! (bdui-rs-7zq). Nodes are absolutely-positioned divs over a canvas that
//! paints edges (solid blocks / dotted parent-child / dashed related) with
//! arrowheads. Layout is the pure `graph::layout_graph`.

use crate::app::state_color;
use crate::graph::{
    collapse_graph, layout_graph, EdgeStyle, Graph, GraphData, COL_GAP, NODE_H, NODE_W, ROW_GAP,
};
use gpui_kit::component::ActiveTheme;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Bounds, Context, InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Render,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

/// Zoom clamp, matching the prototype's zoomAt (0.15..=2.5).
pub const ZOOM_MIN: f32 = 0.15;
pub const ZOOM_MAX: f32 = 2.5;
/// Fit upscale cap, matching the prototype's fitGraph.
pub const FIT_CAP: f32 = 1.1;

/// Pure pan/zoom state: world -> screen is
/// `sx = pan_x + wx * zoom`, `sy = pan_y + wy * zoom`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub pan_x: f32,
    pub pan_y: f32,
    pub zoom: f32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self { pan_x: 0.0, pan_y: 0.0, zoom: 1.0 }
    }
}

impl Viewport {
    /// Drag delta in screen pixels.
    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.pan_x += dx;
        self.pan_y += dy;
    }

    /// Zoom by `factor` anchored at screen point (mx, my): the world point
    /// under the cursor stays under the cursor. Clamp [ZOOM_MIN, ZOOM_MAX].
    pub fn zoom_at(&mut self, (mx, my): (f32, f32), factor: f32) {
        let new_zoom = (self.zoom * factor).clamp(ZOOM_MIN, ZOOM_MAX);
        let ratio = new_zoom / self.zoom;
        self.pan_x = mx - (mx - self.pan_x) * ratio;
        self.pan_y = my - (my - self.pan_y) * ratio;
        self.zoom = new_zoom;
    }

    /// Inverse of the world->screen map.
    pub fn screen_to_world(&self, (sx, sy): (f32, f32)) -> (f32, f32) {
        ((sx - self.pan_x) / self.zoom, (sy - self.pan_y) / self.zoom)
    }

    /// Fit world box (bw, bh) into viewport (w, h), centered; upscale
    /// capped at FIT_CAP. Mirrors the prototype's fitGraph.
    pub fn fit(bw: f32, bh: f32, w: f32, h: f32) -> Self {
        let zoom = (w / bw).min(h / bh).min(FIT_CAP);
        Self {
            pan_x: (w - bw * zoom) / 2.0,
            pan_y: (h - bh * zoom) / 2.0,
            zoom,
        }
    }
}

/// Fetched once per Store refresh by the shell; layout recomputed then.
pub struct GraphView {
    /// Raw parsed graph data; collapse + layout derive from this.
    data: Option<GraphData>,
    graph: Option<Graph>,
    error: Option<String>,
    viewport: Viewport,
    /// True until the first successful render has fitted the viewport.
    fitted: bool,
    /// Pan drag bookkeeping: last screen position of the pressed button.
    drag_last: Option<(f32, f32)>,
    /// Selected node id for j/k navigation and visual emphasis.
    selected: Option<String>,
    /// Node under the current press, pending click-vs-drag resolution:
    /// a press that moves >3px pans, otherwise it is a click on release.
    pending_click: Option<String>,
    /// Press origin in screen pixels for the slop test.
    press_origin: (f32, f32),
    /// Epics collapsed into single nodes; session-persistent (bdui-rs-kdr).
    collapsed: std::collections::HashSet<String>,
    /// Hovered node id; prototype focus dims non-neighbors until exit.
    hovered: Option<String>,
    /// Collapsed-child counts by node id for the current render.
    child_counts: std::collections::HashMap<String, usize>,
}

impl GraphView {
    pub fn new() -> Self {
        Self {
            data: None,
            graph: None,
            error: None,
            viewport: Viewport::default(),
            fitted: false,
            drag_last: None,
            selected: None,
            pending_click: None,
            press_origin: (0.0, 0.0),
            collapsed: std::collections::HashSet::new(),
            hovered: None,
            child_counts: std::collections::HashMap::new(),
        }
    }

    /// Fetch + lay out. Called by the shell when the view activates and on
    /// store refresh while active.
    pub fn reload(&mut self, data: &GraphData, cx: &mut Context<Self>) {
        if data.nodes.is_empty() {
            self.data = None;
            self.graph = None;
            self.error = Some("No issues to graph.".to_string());
        } else {
            self.data = Some(data.clone());
            self.error = None;
            self.relayout();
        }
        cx.notify();
    }

    /// Collapse epics then lay out; call after any collapse-set change.
    fn relayout(&mut self) {
        let Some(data) = &self.data else { return };
        let collapsed = collapse_graph(data, &self.collapsed);
        let graph_data = GraphData {
            nodes: collapsed.nodes.iter().map(|n| n.bead.clone()).collect(),
            edges: collapsed.edges.clone(),
        };
        self.child_counts = collapsed
            .nodes
            .iter()
            .filter_map(|n| n.child_count.map(|c| (n.bead.id.clone(), c)))
            .collect();
        self.graph = Some(layout_graph(&graph_data));
    }

    /// Toggle a node's collapsed state and re-lay out deterministically.
    pub fn toggle_collapse(&mut self, id: &str, cx: &mut Context<Self>) {
        if !self.collapsed.remove(id) {
            self.collapsed.insert(id.to_string());
        }
        self.relayout();
        cx.notify();
    }

    /// Fetch failed surface: transport error from the shell's client call.
    pub fn reload_failed(&mut self, message: String, cx: &mut Context<Self>) {
        self.data = None;
        self.graph = None;
        self.error = Some(message);
        cx.notify();
    }
}

impl Default for GraphView {
    fn default() -> Self {
        Self::new()
    }
}

impl Render for GraphView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let Some(graph) = &self.graph else {
            let msg = self.error.clone().unwrap_or_else(|| "Loading graph…".into());
            return div()
                .flex()
                .flex_1()
                .items_center()
                .justify_center()
                .text_color(theme.muted_foreground)
                .child(msg)
                .into_any_element();
        };

        // World extents.
        let world_w = graph
            .nodes
            .iter()
            .map(|n| n.x + NODE_W)
            .fold(0.0_f32, f32::max)
            + COL_GAP;
        let world_h = graph
            .nodes
            .iter()
            .map(|n| n.y + NODE_H)
            .fold(0.0_f32, f32::max)
            + ROW_GAP;

        let vp = self.viewport;
        let selected = self.selected.clone();

        // Prototype focus: hovering a node keeps it, its adjacency (over
        // blocks/parent edges), and rel-edge endpoints; everything else
        // dims. Incident edges highlight.
        let hovered = self.hovered.clone();
        let focus_set: std::collections::HashSet<String> = hovered
            .as_deref()
            .map(|id| {
                let mut near = std::collections::HashSet::new();
                near.insert(id.to_string());
                for e in &graph.edges {
                    // Prototype: adjacency over structural edges, plus both
                    // endpoints of incident rel edges — together, every
                    // edge touching the hovered node keeps its endpoints.
                    if e.from == id || e.to == id {
                        near.insert(e.from.clone());
                        near.insert(e.to.clone());
                    }
                }
                near
            })
            .unwrap_or_default();

        // First open: the canvas prepaint reports the pane bounds on the
        // first frame and fits the viewport there. One identity-viewport
        // frame renders before the fit lands.
        let entity = cx.entity();
        let fitted = std::rc::Rc::new(std::cell::Cell::new(self.fitted));
        let fit_params = (world_w, world_h);
        let fit_canvas = gpui_kit::canvas(
            move |bounds: Bounds<Pixels>, _window: &mut Window, cx: &mut gpui_kit::App| {
                if !fitted.get() {
                    fitted.set(true);
                    let (bw, bh) = fit_params;
                    let fit = Viewport::fit(bw, bh, bounds.size.width.as_f32(), bounds.size.height.as_f32());
                    entity.update(cx, |this, cx| {
                        this.viewport = fit;
                        this.fitted = true;
                        cx.notify();
                    });
                }
            },
            |_, _, _, _| {},
        );

        let edges = graph.edges.clone();
        let edge_fg = theme.foreground.opacity(0.55);
        let edge_muted = theme.muted_foreground.opacity(0.55);
        let hovered_edge = hovered.clone();
        let edge_accent = theme.accent;
        let edge_canvas = gpui_kit::canvas(
            |_bounds: Bounds<Pixels>, _window: &mut Window, _cx: &mut gpui_kit::App| {},
            move |_bounds: Bounds<Pixels>, (): (), window: &mut Window, _cx: &mut gpui_kit::App| {
                for e in &edges {
                    let incident = hovered_edge
                        .as_deref()
                        .is_some_and(|id| e.from == id || e.to == id);
                    let color = match (&hovered_edge, incident) {
                        // Focus: incident edges take the accent and stay
                        // opaque; everything else fades to 0.07.
                        (Some(_), true) => edge_accent,
                        (Some(_), false) => {
                            match e.style {
                                EdgeStyle::Solid => edge_fg,
                                _ => edge_muted,
                            }
                            .opacity(0.13)
                        }
                        (None, _) => match e.style {
                            EdgeStyle::Solid => edge_fg,
                            _ => edge_muted,
                        },
                    };
                    // World -> screen.
                    let (x1, y1) = (vp.pan_x + e.x1 * vp.zoom, vp.pan_y + e.y1 * vp.zoom);
                    let (x2, y2) = (vp.pan_x + e.x2 * vp.zoom, vp.pan_y + e.y2 * vp.zoom);
                    let mut path = gpui_kit::PathBuilder::stroke(px(1.5));
                    path = match e.style {
                        EdgeStyle::Solid => path,
                        EdgeStyle::Dotted => path.dash_array(&[px(1.5), px(3.0)]),
                        EdgeStyle::Dashed => path.dash_array(&[px(5.0), px(4.0)]),
                    };
                    path.move_to(gpui_kit::Point::new(px(x1), px(y1)));
                    let mid_x = px((x1 + x2) / 2.0);
                    path.line_to(gpui_kit::Point::new(mid_x, px(y1)));
                    path.line_to(gpui_kit::Point::new(mid_x, px(y2)));
                    path.line_to(gpui_kit::Point::new(px(x2), px(y2)));
                    if let Ok(path) = path.build() {
                        window.paint_path(path, color);
                    }
                    // Arrowhead at the target, scaled with zoom.
                    let head = 7.0 * vp.zoom;
                    let tip = gpui_kit::Point::new(px(x2), px(y2));
                    let back = px(x2 - head);
                    let up = gpui_kit::Point::new(back, px(y2 - head / 2.0));
                    let down = gpui_kit::Point::new(back, px(y2 + head / 2.0));
                    let mut head_path = gpui_kit::PathBuilder::fill();
                    head_path.move_to(tip);
                    head_path.line_to(up);
                    head_path.line_to(down);
                    if let Ok(head_path) = head_path.build() {
                        window.paint_path(head_path, color);
                    }
                }
            },
        );

        let theme2 = theme.clone();
        // Non-blocks edges carry their kind as a label at the elbow. Labels
        // for non-incident edges dim with focus like their edges.
        let kind_labels = graph
            .edges
            .iter()
            .filter(|e| e.kind != "blocks")
            .map(|e| {
                let mid_x = vp.pan_x + ((e.x1 + e.x2) / 2.0) * vp.zoom;
                let mid_y = vp.pan_y + ((e.y1 + e.y2) / 2.0) * vp.zoom;
                let incident = hovered
                    .as_deref()
                    .is_some_and(|id| e.from == id || e.to == id);
                div()
                    .id(gpui_kit::SharedString::from(format!(
                        "graph-edge-label-{}-{}",
                        e.from, e.to
                    )))
                    .absolute()
                    .left(px(mid_x - 34.0))
                    .top(px(mid_y - 8.0))
                    .px_1()
                    .text_xs()
                    .opacity(if hovered.is_some() && !incident {
                        0.13
                    } else {
                        1.0
                    })
                    .text_color(theme2.muted_foreground)
                    .child(gpui_kit::SharedString::from(e.kind.clone()))
            })
            .collect::<Vec<_>>();

        let node_rows = graph
            .nodes
            .iter()
            .map(|n| {
                let bead = &n.bead;
                let color = state_color(bead.state);
                let border = if bead.issue_type == "epic" {
                    color.opacity(0.9)
                } else {
                    theme2.border
                };
                let is_selected = selected.as_deref() == Some(bead.id.as_str());
                let in_focus = hovered.is_none()
                    || focus_set.contains(bead.id.as_str());
                let child_count = self.child_counts.get(&bead.id).copied();
                let title = bead.title.clone();
                let id = bead.id.clone();
                let badge_id = id.clone();
                let hover_id = id.clone();
                let chevron_id = id.clone();
                let is_epic = bead.issue_type == "epic";
                let sx = vp.pan_x + n.x * vp.zoom;
                let sy = vp.pan_y + n.y * vp.zoom;
                div()
                    .id(gpui_kit::SharedString::from(format!("graph-node-{id}")))
                    .absolute()
                    .left(px(sx))
                    .top(px(sy))
                    .w(px(NODE_W * vp.zoom))
                    .h(px(NODE_H * vp.zoom))
                    .rounded_md()
                    // Focus dims non-neighbors (prototype: .22 opacity).
                    .opacity(if in_focus { 1.0 } else { 0.22 })
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        this.hovered = hovered.then(|| hover_id.clone());
                        cx.notify();
                    }))
                    .border_1()
                    .border_color(if is_selected {
                        theme2.accent
                    } else {
                        border
                    })
                    .when(is_selected, |el| el.border_2())
                    // Epics keep the card outline tint; regular tasks fill.
                    .bg(if bead.issue_type == "epic" {
                        theme2.background
                    } else {
                        theme2.secondary
                    })
                    .when(bead.issue_type == "epic", |el| {
                        el.border_l_2().border_color(color)
                    })
                    .when(is_selected, |el| {
                        el.shadow_md().border_color(theme2.accent)
                    })
                    .px_2()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .overflow_hidden()
                    .on_mouse_down(
                        gpui_kit::MouseButton::Left,
                        cx.listener(move |this, event: &gpui_kit::MouseDownEvent, _, cx| {
                            // Press selects immediately; whether it counts
                            // as a click (opens the drawer) is decided on
                            // release by the press-move distance.
                            this.selected = Some(badge_id.clone());
                            this.pending_click = Some(badge_id.clone());
                            this.press_origin = (
                                event.position.x.as_f32(),
                                event.position.y.as_f32(),
                            );
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(color)
                            .child(gpui_kit::SharedString::from(format!(
                                "{} · P{}",
                                id, bead.priority
                            ))),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_ellipsis()
                            .child(gpui_kit::SharedString::from(title)),
                    )
                    .when(is_epic, |el| {
                        // Chevron + hidden-child count; click toggles
                        // collapse. Sits in the node's top-right corner.
                        el.child(
                            div()
                                .id(gpui_kit::SharedString::from(format!(
                                    "graph-collapse-{chevron_id}"
                                )))
                                .absolute()
                                .right_1()
                                .top_1()
                                .px_1()
                                .rounded_sm()
                                .text_xs()
                                .text_color(theme2.accent)
                                .on_mouse_down(
                                    gpui_kit::MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        // The node press also fired; the
                                        // click would open the drawer, so
                                        // cancel it — this press toggles.
                                        this.pending_click = None;
                                        this.toggle_collapse(&chevron_id, cx);
                                    }),
                                )
                                .child(gpui_kit::SharedString::from(format!(
                                    "{}{}",
                                    if child_count.is_some() { "▾" } else { "▸" },
                                    child_count
                                        .map(|c| format!(" {c}"))
                                        .unwrap_or_default()
                                ))),
                        )
                    })
            })
            .collect::<Vec<_>>();

        div()
            .id("graph-viewport")
            .flex_1()
            .relative()
            .overflow_hidden()
            .on_scroll(cx.listener(|this, event: &gpui_kit::ScrollEvent, _window, cx| {
                let position = (event.position.x.as_f32(), event.position.y.as_f32());
                // Wheel zooms, matching the prototype's wheel handler.
                let factor = match &event.delta {
                    gpui_kit::ScrollDelta::Lines(d) => (-d.y * 0.1).exp(),
                    gpui_kit::ScrollDelta::Pixels(d) => (-d.y.as_f32() * 0.0015).exp(),
                };
                this.viewport.zoom_at(position, factor);
                cx.notify();
            }))
            .on_mouse_down(
                gpui_kit::MouseButton::Left,
                cx.listener(|this, event: &gpui_kit::MouseDownEvent, _, cx| {
                    // Begin a pan drag; the move handler pans until release.
                    this.drag_last =
                        Some((event.position.x.as_f32(), event.position.y.as_f32()));
                    if this.pending_click.is_none() {
                        this.press_origin = (
                            event.position.x.as_f32(),
                            event.position.y.as_f32(),
                        );
                    }
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(
                |this, event: &gpui_kit::MouseMoveEvent, _, cx| {
                if event.pressed_button == Some(gpui_kit::MouseButton::Left) {
                    if let Some((lx, ly)) = this.drag_last {
                        let (mx, my) =
                            (event.position.x.as_f32(), event.position.y.as_f32());
                        this.viewport.pan(mx - lx, my - ly);
                        this.drag_last = Some((mx, my));
                        cx.notify();
                    }
                }
                },
            ))
            .on_mouse_up(
                gpui_kit::MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.drag_last = None;
                    if let Some(id) = this.pending_click.take() {
                        cx.emit(GraphViewEvent::NodeClicked(id));
                    }
                    cx.notify();
                }),
            )
            .child(
                div()
                    .id("graph-canvas-wrap")
                    .relative()
                    .size_full()
                    .child(fit_canvas)
                    .child(edge_canvas)
                    .children(node_rows)
                    .children(kind_labels),
            )
            .child(render_legend(&theme))
            .into_any_element()
    }
}

/// Events the graph view emits to its owner (the shell).
pub enum GraphViewEvent {
    /// A node was clicked; the shell opens the detail drawer.
    NodeClicked(String),
    /// The pane size was measured for the first-open fit.
    FirstFitReady,
}

impl gpui_kit::EventEmitter<GraphViewEvent> for GraphView {}

// Navigation: j/k move selection through the deterministic node order.
impl GraphView {
    /// Move selection down/up in layout order; returns the newly selected id.
    pub fn move_selection(&mut self, down: bool, cx: &mut Context<Self>) {
        let Some(graph) = &self.graph else { return };
        let ids: Vec<&str> = graph.nodes.iter().map(|n| n.bead.id.as_str()).collect();
        if ids.is_empty() {
            return;
        }
        let idx = match &self.selected {
            Some(id) => ids.iter().position(|i| *i == id.as_str()),
            None => None,
        };
        let next = match (idx, down) {
            (None, _) => 0,
            (Some(i), true) => (i + 1).min(ids.len() - 1),
            (Some(i), false) => i.saturating_sub(1),
        };
        self.selected = Some(ids[next].to_string());
        cx.notify();
    }

    /// Currently selected node id.
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }
}

/// Legend naming the real dependency vocabulary (CONTEXT.md).
fn render_legend(theme: &gpui_kit::component::theme::Theme) -> gpui_kit::AnyElement {
    let label_of = |style: EdgeStyle| match style {
        EdgeStyle::Solid => "blocks",
        EdgeStyle::Dotted => "parent-child",
        EdgeStyle::Dashed => "related · tracks · discovered-from",
    };
    let line = |style: EdgeStyle| {
        let line_color = theme.foreground.opacity(0.55);
        let id = gpui_kit::SharedString::from(format!("legend-{}", label_of(style)));
        let label: gpui_kit::SharedString = label_of(style).into();
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(
                div()
                    .id(id)
                    .w(px(18.))
                    .h(px(2.))
                    .bg(line_color)
                    .when(style == EdgeStyle::Dashed, |el| el.opacity(0.55))
                    .when(style == EdgeStyle::Dotted, |el| el.opacity(0.85)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(label),
            )
    };
    div()
        .id("graph-legend")
        .absolute()
        .bottom_2()
        .left_2()
        .flex()
        .gap_3()
        .px_2()
        .py_1()
        .rounded_md()
        .bg(theme.background.opacity(0.85))
        .border_1()
        .border_color(theme.border)
        .child(line(EdgeStyle::Solid))
        .child(line(EdgeStyle::Dotted))
        .child(line(EdgeStyle::Dashed))
        .into_any_element()
}
