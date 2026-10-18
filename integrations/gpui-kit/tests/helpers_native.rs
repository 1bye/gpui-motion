//! Native adapter contracts: real layout, hitboxes, input and deterministic clocks.
//! TestPlatform substitutes fonts, window/display and GPU presentation.
use gpui_kit as ui;
use gpui_motion_kit as motion;

#[path = "../../../tests/common/advanced_fixture.rs"]
mod fixture;

mod activation {
    pub fn activate(window: &gpui_kit::Window) {
        window.activate_window();
    }
}

#[gpui_kit::test]
fn intrinsic_size_reflows_siblings_clips_input_and_retargets_without_jumping(
    cx: &mut ui::TestAppContext,
) {
    fixture::intrinsic_size_reflows_siblings_clips_input_and_retargets_without_jumping(cx);
}

#[gpui_kit::test]
fn popped_exit_preserves_bounds_and_allows_immediate_sibling_reflow(cx: &mut ui::TestAppContext) {
    fixture::popped_exit_preserves_bounds_and_allows_immediate_sibling_reflow(cx);
}

#[gpui_kit::test]
fn native_gestures_bind_hover_press_drag_outside_release_and_keyboard(cx: &mut ui::TestAppContext) {
    fixture::native_gestures_bind_hover_press_drag_outside_release_and_keyboard(cx);
}

#[gpui_kit::test]
fn drag_cancels_on_escape_blur_and_window_deactivation(cx: &mut ui::TestAppContext) {
    fixture::drag_cancels_on_escape_blur_and_window_deactivation(cx);
}

#[gpui_kit::test]
fn native_scroll_binding_reads_real_extents_without_host_metric_calculation(
    cx: &mut ui::TestAppContext,
) {
    fixture::native_scroll_binding_reads_real_extents_without_host_metric_calculation(cx);
}

#[gpui_kit::test]
fn shared_position_crossfade_retargets_across_separate_native_ids(cx: &mut ui::TestAppContext) {
    fixture::shared_position_crossfade_retargets_across_separate_native_ids(cx);
}

#[gpui_kit::test]
fn reorder_uses_actual_geometry_and_commits_pointer_and_keyboard_order(
    cx: &mut ui::TestAppContext,
) {
    fixture::reorder_uses_actual_geometry_and_commits_pointer_and_keyboard_order(cx);
}

#[gpui_kit::test]
fn reorder_escape_restores_original_order(cx: &mut ui::TestAppContext) {
    fixture::reorder_escape_restores_original_order(cx);
}

#[gpui_kit::test]
fn vector_morph_and_reveal_tessellate_and_stop_scheduling_after_reduced_motion(
    cx: &mut ui::TestAppContext,
) {
    fixture::vector_morph_and_reveal_tessellate_and_stop_scheduling_after_reduced_motion(cx);
}

#[gpui_kit::test]
fn unanimated_measured_axes_do_not_schedule_unnecessary_frames(cx: &mut ui::TestAppContext) {
    fixture::unanimated_measured_axes_do_not_schedule_unnecessary_frames(cx);
}

#[gpui_kit::test]
fn first_active_shared_member_snaps_even_when_inactive_member_is_traversed_first(
    cx: &mut ui::TestAppContext,
) {
    fixture::first_active_shared_member_snaps_even_when_inactive_member_is_traversed_first(cx);
}

#[gpui_kit::test]
fn keyboard_motion_reaches_a_control_with_text_descendants(cx: &mut ui::TestAppContext) {
    fixture::keyboard_motion_reaches_a_control_with_text_descendants(cx);
}

#[gpui_kit::test]
fn disabling_native_bindings_cancels_drag_and_blocks_child_activation(cx: &mut ui::TestAppContext) {
    fixture::disabling_native_bindings_cancels_drag_and_blocks_child_activation(cx);
}

#[gpui_kit::test]
fn reorder_callbacks_only_fire_for_changes_and_can_reenter_the_group(cx: &mut ui::TestAppContext) {
    fixture::reorder_callbacks_only_fire_for_changes_and_can_reenter_the_group(cx);
}

#[gpui_kit::test]
fn restoring_a_popped_item_preserves_its_displayed_position(cx: &mut ui::TestAppContext) {
    fixture::restoring_a_popped_item_preserves_its_displayed_position(cx);
}

#[gpui_kit::test]
fn scroll_resize_refreshes_metrics_and_smoothing_retargets_continuously(
    cx: &mut ui::TestAppContext,
) {
    fixture::scroll_resize_refreshes_metrics_and_smoothing_retargets_continuously(cx);
}

#[gpui_kit::test]
fn visibility_uses_ancestor_clipping_and_preserves_once_policy(cx: &mut ui::TestAppContext) {
    fixture::visibility_uses_ancestor_clipping_and_preserves_once_policy(cx);
}
