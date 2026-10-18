//! Covers the same production text renderer against Kit's native GPUI types.
//! Real: Kit adapter/layout preparation and executor. Doubles: fonts, display, GPU.
//! Requires: native-tests.
use gpui_kit as ui;
use gpui_motion_kit as motion;

#[path = "../../../tests/common/text_fixture.rs"]
mod fixture;

#[gpui_kit::test]
fn clock_and_interruption(cx: &mut gpui_kit::TestAppContext) {
    fixture::clock_and_interruption(cx);
}

#[gpui_kit::test]
fn unicode_wrap_and_mask(cx: &mut gpui_kit::TestAppContext) {
    fixture::unicode_wrap_and_mask(cx);
}

#[gpui_kit::test]
fn native_controls(cx: &mut gpui_kit::TestAppContext) {
    fixture::native_controls(cx);
}

#[gpui_kit::test]
fn reduced_and_scrubbed(cx: &mut gpui_kit::TestAppContext) {
    fixture::reduced_and_scrubbed(cx);
}

#[gpui_kit::test]
fn digit_place_value(cx: &mut gpui_kit::TestAppContext) {
    fixture::digit_place_value(cx);
}

#[gpui_kit::test]
fn centered_text_does_not_jump_when_its_width_changes(cx: &mut gpui_kit::TestAppContext) {
    fixture::centered_text_does_not_jump_when_its_width_changes(cx);
}

#[test]
fn semantic_text_has_one_accessible_label() {
    fixture::semantic_text_has_one_accessible_label();
}

#[gpui_kit::test]
fn rolling_and_sliding_digits_clear_tall_line_boxes(cx: &mut gpui_kit::TestAppContext) {
    fixture::rolling_and_sliding_digits_clear_tall_line_boxes(cx);
}
