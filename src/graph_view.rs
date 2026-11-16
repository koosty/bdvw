//! Graph view: the dependency DAG rendered from `bd graph --all --json`
//! (bdui-rs-7zq). Nodes are absolutely-positioned divs over a canvas that
//! paints edges (solid blocks / dotted parent-child / dashed related) with
//! arrowheads. Layout is the pure `graph::layout_graph`.

use crate::app::state_color;
use crate::graph::{layout_graph, EdgeStyle, Graph, GraphData, COL_GAP, NODE_H, NODE_W, ROW_GAP};
use gpui_kit::component::ActiveTheme;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Bounds, Context, InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Render,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

/// Fetched once per Store refresh by the shell; layout recomputed then.
pub struct GraphView {
    graph: Option<Graph>,
    error: Option<String>,
}

impl GraphView {
    pub fn new() -> Self {
        Self { graph: None, error: None }
    }

    /// Fetch + lay out. Called by the shell when the view activates and on
    /// store refresh while active.
    pub fn reload(&mut self, data: &GraphData, cx: &mut Context<Self>) {
        if data.nodes.is_empty() {
            self.graph = None;
            self.error = Some("No issues to graph.".to_string());
        } else {
            self.graph = Some(layout_graph(data));
            self.error = None;
        }
        cx.notify();
    }

    /// Fetch failed surface: transport error from the shell's client call.
    pub fn reload_failed(&mut self, message: String, cx: &mut Context<Self>) {
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

        // Canvas size from the laid-out extents.
        let width = graph
            .nodes
            .iter()
            .map(|n| n.x + NODE_W)
            .fold(0.0_f32, f32::max)
            + COL_GAP;
        let height = graph
            .nodes
            .iter()
            .map(|n| n.y + NODE_H)
            .fold(0.0_f32, f32::max)
            + ROW_GAP;

        // Edge paint closure: straight lines blocker-right -> blocked-left.
        let edges = graph.edges.clone();

        let edge_fg = theme.foreground.opacity(0.55);
        let edge_muted = theme.muted_foreground.opacity(0.55);
        let edge_canvas = gpui_kit::canvas(
            |_bounds: Bounds<Pixels>, _window: &mut Window, _cx: &mut gpui_kit::App| {
                // Prepaint: nothing; edge list is already captured.
            },
            move |_bounds: Bounds<Pixels>, (): (), window: &mut Window, _cx: &mut gpui_kit::App| {
                for e in &edges {
                    let color = match e.style {
                        EdgeStyle::Solid => edge_fg,
                        _ => edge_muted,
                    };
                    let mut path = gpui_kit::PathBuilder::stroke(px(1.5));
                    path = match e.style {
                        EdgeStyle::Solid => path,
                        EdgeStyle::Dotted => path.dash_array(&[px(1.5), px(3.0)]),
                        EdgeStyle::Dashed => path.dash_array(&[px(5.0), px(4.0)]),
                    };
                    path.move_to(gpui_kit::Point::new(px(e.x1), px(e.y1)));
                    let mid_x = px((e.x1 + e.x2) / 2.0);
                    path.line_to(gpui_kit::Point::new(mid_x, px(e.y1)));
                    path.line_to(gpui_kit::Point::new(mid_x, px(e.y2)));
                    path.line_to(gpui_kit::Point::new(px(e.x2), px(e.y2)));
                    if let Ok(path) = path.build() {
                        window.paint_path(path, color);
                    }
                    // Arrowhead: filled triangle pointing right at (x2, y2).
                    let tip = gpui_kit::Point::new(px(e.x2), px(e.y2));
                    let back = px(e.x2 - 7.0);
                    let up = gpui_kit::Point::new(back, px(e.y2 - 3.5));
                    let down = gpui_kit::Point::new(back, px(e.y2 + 3.5));
                    let mut head = gpui_kit::PathBuilder::fill();
                    head.move_to(tip);
                    head.line_to(up);
                    head.line_to(down);
                    if let Ok(head) = head.build() {
                        window.paint_path(head, color);
                    }
                }
            },
        );

        let theme2 = theme.clone();
        // Non-blocks edges carry their true kind as a label at the elbow,
        // so related/tracks/discovered-from are honestly named (no hover
        // tooltips on canvas paths; the legend plus this label covers it).
        let kind_labels = graph
            .edges
            .iter()
            .filter(|e| e.kind != "blocks")
            .map(|e| {
                let mid_x = (e.x1 + e.x2) / 2.0;
                let mid_y = (e.y1 + e.y2) / 2.0;
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
                let title = bead.title.clone();
                let id = bead.id.clone();
                div()
                    .id(gpui_kit::SharedString::from(format!("graph-node-{id}")))
                    .absolute()
                    .left(px(n.x))
                    .top(px(n.y))
                    .w(px(NODE_W))
                    .h(px(NODE_H))
                    .rounded_md()
                    .border_1()
                    .border_color(border)
                    // Epics keep the card outline tint; regular tasks fill.
                    .bg(if bead.issue_type == "epic" {
                        theme2.background
                    } else {
                        theme2.secondary
                    })
                    .when(bead.issue_type == "epic", |el| {
                        el.border_l_2().border_color(color)
                    })
                    .px_2()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .overflow_hidden()
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
            })
            .collect::<Vec<_>>();

        div()
            .id("graph-scroll")
            .flex_1()
            .relative()
            .overflow_scroll()
            .child(
                div()
                    .id("graph-canvas-wrap")
                    .relative()
                    .w(px(width))
                    .h(px(height))
                    .child(edge_canvas)
                    .children(node_rows)
                    .children(kind_labels),
            )
            .child(render_legend(&theme))
            .into_any_element()
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
