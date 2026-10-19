//! Covers: CE batch clock, scheduling, interruption, native frames, and cleanup.
//! Real: CE layout and the shared engine. Doubles: TestPlatform. Requires: native-tests.
use gpui as ui;
use gpui_ce_motion as motion;

#[path = "common/motions_fixture.rs"]
mod fixture;

#[gpui::test]
fn batched_clock_interruption_and_scheduling(cx: &mut ui::TestAppContext) {
    fixture::batched_clock_interruption_and_scheduling(cx);
}

#[gpui::test]
fn reduced_motion_exit_cleanup_and_omission(cx: &mut ui::TestAppContext) {
    fixture::reduced_motion_exit_cleanup_and_omission(cx);
}

#[gpui::test]
fn single_entry_and_explicit_samples_return_native_frames(cx: &mut ui::TestAppContext) {
    fixture::single_entry_and_explicit_samples_return_native_frames(cx);
}
