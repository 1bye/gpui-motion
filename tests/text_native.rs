//! Covers the CE text adapter, reflow, controls, and number slots.
//! Real: CE adapter/layout preparation and executor. Doubles: fonts, display, GPU.
//! Requires: native-tests.
use gpui as ui;
use gpui_ce_motion as motion;
#[path = "common/text_fixture.rs"]
mod fixture;
#[gpui::test]
fn clock_and_interruption(cx: &mut gpui::TestAppContext) {
    fixture::clock_and_interruption(cx);
}
#[gpui::test]
fn unicode_wrap_and_mask(cx: &mut gpui::TestAppContext) {
    fixture::unicode_wrap_and_mask(cx);
}
#[gpui::test]
fn native_controls(cx: &mut gpui::TestAppContext) {
    fixture::native_controls(cx);
}
#[gpui::test]
fn reduced_and_scrubbed(cx: &mut gpui::TestAppContext) {
    fixture::reduced_and_scrubbed(cx);
}
#[gpui::test]
fn digit_place_value(cx: &mut gpui::TestAppContext) {
    fixture::digit_place_value(cx);
}

#[gpui::test]
fn centered_text_does_not_jump_when_its_width_changes(cx: &mut gpui::TestAppContext) {
    fixture::centered_text_does_not_jump_when_its_width_changes(cx);
}

#[test]
fn semantic_text_has_one_accessible_label() {
    fixture::semantic_text_has_one_accessible_label();
}

#[gpui::test]
fn rolling_and_sliding_digits_clear_tall_line_boxes(cx: &mut gpui::TestAppContext) {
    fixture::rolling_and_sliding_digits_clear_tall_line_boxes(cx);
}
