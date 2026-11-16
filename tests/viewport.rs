//! S1 seam: pan/zoom viewport math, independent of any gpui types.
//! Semantics mirror the prototype's fitGraph/zoomAt exactly.

use bdui::graph_view::{Viewport, ZOOM_MAX, ZOOM_MIN};

/// Prototype wheel: one notch multiplies zoom by ~this.
const WHEEL_STEP: f32 = 1.1;

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

#[test]
fn default_viewport_is_identity() {
    let v = Viewport::default();
    assert_eq!(v.pan_x, 0.0);
    assert_eq!(v.pan_y, 0.0);
    assert!(close(v.zoom, 1.0));
}

#[test]
fn pan_accumulates() {
    let mut v = Viewport::default();
    v.pan(10.0, -5.0);
    v.pan(1.0, 2.0);
    assert!(close(v.pan_x, 11.0));
    assert!(close(v.pan_y, -3.0));
}

#[test]
fn zoom_anchored_at_cursor_keeps_world_point_under_cursor() {
    let mut v = Viewport::default();
    v.pan(40.0, 20.0);
    let cursor = (300.0, 150.0);
    let world_before = v.screen_to_world(cursor);
    v.zoom_at(cursor, 1.25);
    let world_after = v.screen_to_world(cursor);
    assert!(close(world_before.0, world_after.0));
    assert!(close(world_before.1, world_after.1));
}

#[test]
fn zoom_is_clamped() {
    let mut v = Viewport { zoom: ZOOM_MAX, ..Default::default() };
    v.zoom_at((0.0, 0.0), 2.0);
    assert!(close(v.zoom, ZOOM_MAX));
    v.zoom = ZOOM_MIN;
    v.zoom_at((0.0, 0.0), 0.5);
    assert!(close(v.zoom, ZOOM_MIN));
}

#[test]
fn zoom_factor_round_trips() {
    let mut v = Viewport::default();
    v.zoom_at((100.0, 100.0), WHEEL_STEP);
    assert!(v.zoom > 1.0);
    v.zoom_at((100.0, 100.0), 1.0 / WHEEL_STEP);
    assert!(close(v.zoom, 1.0));
}

/// Prototype fit: k = min(1.1, min(w/bw, h/bh)), content centered.
#[test]
fn fit_scales_into_bounds_and_centers() {
    // World 1000x500 into a 500x300 viewport: k = 0.5 (width-bound).
    let v = Viewport::fit(1000.0, 500.0, 500.0, 300.0);
    assert!(close(v.zoom, 0.5));
    assert!(close(v.pan_x, 0.0)); // (500 - 1000*0.5) / 2
    assert!(close(v.pan_y, 25.0)); // (300 - 500*0.5) / 2
}

#[test]
fn fit_caps_upscale_at_1_point_1() {
    // Tiny world in a big viewport: no more than 110%.
    let v = Viewport::fit(100.0, 100.0, 1000.0, 1000.0);
    assert!(close(v.zoom, 1.1));
    assert!(close(v.pan_x, 445.0)); // (1000 - 100*1.1) / 2
}

#[test]
fn fit_with_exact_width_keeps_k_at_one() {
    let v = Viewport::fit(200.0, 400.0, 200.0, 1000.0);
    assert!(close(v.zoom, 1.0));
    assert!(close(v.pan_x, 0.0));
    assert!(close(v.pan_y, 300.0)); // (1000 - 400) / 2
}
