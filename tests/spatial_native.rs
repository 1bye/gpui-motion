//! Native input/geometry integration for the expanded shared engine.
use gpui as ui;
use gpui_ce_motion as motion;

#[path = "common/activation.rs"]
mod activation;
#[path = "common/spatial_fixture.rs"]
mod fixture;

#[gpui::test]
fn scroll_intersections_follow_current_native_offsets_and_resizing(cx: &mut ui::TestAppContext) {
    fixture::scroll_intersections_follow_current_native_offsets_and_resizing(cx);
}

#[gpui::test]
fn external_handle_measured_bounds_direction_lock_and_reentrant_events(
    cx: &mut ui::TestAppContext,
) {
    fixture::external_handle_measured_bounds_direction_lock_and_reentrant_events(cx);
}

#[gpui::test]
fn spatial_reorder_rolls_back_and_edge_scroll_stops_on_release(cx: &mut ui::TestAppContext) {
    fixture::spatial_reorder_rolls_back_and_edge_scroll_stops_on_release(cx);
}

#[gpui::test]
fn window_overlay_crosses_clipped_containers_and_keeps_input_at_presentation(
    cx: &mut ui::TestAppContext,
) {
    fixture::window_overlay_crosses_clipped_containers_and_keeps_input_at_presentation(cx);
}

#[gpui::test]
fn rich_text_decimal_numbers_and_style_playback_use_native_layout(cx: &mut ui::TestAppContext) {
    fixture::rich_text_decimal_numbers_and_style_playback_use_native_layout(cx);
}

#[gpui::test]
fn pinch_dispatch_tracks_lifecycle_and_cancellation(cx: &mut ui::TestAppContext) {
    fixture::pinch_dispatch_tracks_lifecycle_and_cancellation(cx);
}

#[gpui::test]
fn formatted_numbers_expose_the_readable_amount_to_accessibility(cx: &mut ui::TestAppContext) {
    fixture::formatted_numbers_expose_the_readable_amount_to_accessibility(cx);
}

#[gpui::test]
fn grid_keyboard_reorder_uses_neighbors_on_both_axes(cx: &mut ui::TestAppContext) {
    fixture::grid_keyboard_reorder_uses_neighbors_on_both_axes(cx);
}

#[gpui::test]
fn grid_pointer_batches_keep_the_measured_destination_stable(cx: &mut ui::TestAppContext) {
    fixture::grid_pointer_batches_keep_the_measured_destination_stable(cx);
}

#[gpui::test]
fn external_order_changes_wait_for_fresh_measurements_before_acquiring(
    cx: &mut ui::TestAppContext,
) {
    fixture::external_order_changes_wait_for_fresh_measurements_before_acquiring(cx);
}
