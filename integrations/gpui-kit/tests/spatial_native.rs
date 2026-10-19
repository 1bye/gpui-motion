//! Native input/geometry integration for the expanded shared engine.
use gpui_kit as ui;
use gpui_motion_kit as motion;

#[path = "../../../tests/common/spatial_fixture.rs"]
mod fixture;

mod activation {
    pub fn activate(window: &gpui_kit::Window) {
        window.activate_window();
    }
}

#[gpui_kit::test]
fn scroll_intersections_follow_current_native_offsets_and_resizing(cx: &mut ui::TestAppContext) {
    fixture::scroll_intersections_follow_current_native_offsets_and_resizing(cx);
}

#[gpui_kit::test]
fn external_handle_measured_bounds_direction_lock_and_reentrant_events(
    cx: &mut ui::TestAppContext,
) {
    fixture::external_handle_measured_bounds_direction_lock_and_reentrant_events(cx);
}

#[gpui_kit::test]
fn spatial_reorder_rolls_back_and_edge_scroll_stops_on_release(cx: &mut ui::TestAppContext) {
    fixture::spatial_reorder_rolls_back_and_edge_scroll_stops_on_release(cx);
}

#[gpui_kit::test]
fn window_overlay_crosses_clipped_containers_and_keeps_input_at_presentation(
    cx: &mut ui::TestAppContext,
) {
    fixture::window_overlay_crosses_clipped_containers_and_keeps_input_at_presentation(cx);
}

#[gpui_kit::test]
fn rich_text_decimal_numbers_and_style_playback_use_native_layout(cx: &mut ui::TestAppContext) {
    fixture::rich_text_decimal_numbers_and_style_playback_use_native_layout(cx);
}

#[gpui_kit::test]
fn pinch_dispatch_tracks_lifecycle_and_cancellation(cx: &mut ui::TestAppContext) {
    fixture::pinch_dispatch_tracks_lifecycle_and_cancellation(cx);
}

#[gpui_kit::test]
fn kit_touch_drag_claims_moves_and_cancels_a_measured_control(cx: &mut ui::TestAppContext) {
    use ui::{AppContext, IntoElement, Render, div, point, prelude::*, px, size};

    struct TouchView(motion::GestureState);

    impl Render for TouchView {
        fn render(&mut self, _: &mut ui::Window, _: &mut ui::Context<Self>) -> impl IntoElement {
            self.0.bind(
                "touch",
                div().id("touch-child").w(px(100.)).h(px(50.)),
                motion::core::Motion::new(motion::core::Target::default()),
                motion::core::GestureTargets::default(),
            )
        }
    }
    let state = motion::GestureState::default()
        .with_drag(motion::core::Drag::unconstrained([0.; 2]))
        .touch_drag(true);

    let copy = state.clone();
    let window = cx.open_window(size(px(400.), px(300.)), |_, _| TouchView(copy));

    cx.update_window(window.into(), |_, w, _| w.activate_window())
        .unwrap();
    cx.run_until_parked();
    cx.update_window(window.into(), |_, w, cx| {
        w.simulate_next_frame(cx);
        w.draw(cx).clear(cx);
    })
    .unwrap();

    for (phase, x) in [
        (ui::TouchPhase::Started, 10.),
        (ui::TouchPhase::Moved, 80.),
        (ui::TouchPhase::Cancelled, 80.),
    ] {
        cx.update_window(window.into(), |_, w, cx| {
            w.dispatch_event(
                ui::PlatformInput::TouchDrag(ui::TouchDragEvent {
                    phase,
                    start_position: point(px(10.), px(10.)),
                    position: point(px(x), px(10.)),
                }),
                cx,
            );
            w.simulate_next_frame(cx);
            w.draw(cx).clear(cx);
        })
        .unwrap();

        if phase == ui::TouchPhase::Moved {
            assert!(state.presented().x_px() > 0.);
            assert!(state.interaction().dragging);
        }
    }

    assert!(!state.interaction().dragging);
}

#[gpui_kit::test]
fn kit_touch_reorder_releases_the_owned_item_after_crossing_other_tiles(
    cx: &mut ui::TestAppContext,
) {
    use ui::{AppContext, IntoElement, Render, div, point, prelude::*, px, size};

    struct Board(motion::ReorderGroup<u8>);

    impl Render for Board {
        fn render(&mut self, _: &mut ui::Window, _: &mut ui::Context<Self>) -> impl IntoElement {
            let mut row = div().flex();

            for key in self.0.order().iter().copied() {
                row = row.child(self.0.item(
                    ("item", usize::from(key)),
                    key,
                    div().id(("child", usize::from(key))).w(px(50.)).h(px(50.)),
                ));
            }

            row
        }
    }

    let group = motion::ReorderGroup::spatial(vec![0, 1, 2], motion::core::ReorderAxis::X).unwrap();
    let copy = group.clone();
    let window = cx.open_window(size(px(300.), px(200.)), |_, _| Board(copy));

    cx.update_window(window.into(), |_, w, _| w.activate_window())
        .unwrap();
    cx.run_until_parked();
    cx.update_window(window.into(), |_, w, cx| {
        w.simulate_next_frame(cx);
        w.draw(cx).clear(cx);
    })
    .unwrap();

    for (phase, x) in [
        (ui::TouchPhase::Started, 10.),
        (ui::TouchPhase::Moved, 110.),
        (ui::TouchPhase::Ended, 110.),
    ] {
        cx.update_window(window.into(), |_, w, cx| {
            w.dispatch_event(
                ui::PlatformInput::TouchDrag(ui::TouchDragEvent {
                    phase,
                    start_position: point(px(10.), px(10.)),
                    position: point(px(x), px(10.)),
                }),
                cx,
            );
            w.simulate_next_frame(cx);
            w.draw(cx).clear(cx);
        })
        .unwrap();
    }

    assert_eq!(&*group.order(), &[1, 2, 0]);
    assert!(!group.is_dragging());
}

#[gpui_kit::test]
fn formatted_numbers_expose_the_readable_amount_to_accessibility(cx: &mut ui::TestAppContext) {
    fixture::formatted_numbers_expose_the_readable_amount_to_accessibility(cx);
}

#[gpui_kit::test]
fn grid_keyboard_reorder_uses_neighbors_on_both_axes(cx: &mut ui::TestAppContext) {
    fixture::grid_keyboard_reorder_uses_neighbors_on_both_axes(cx);
}

#[gpui_kit::test]
fn grid_pointer_batches_keep_the_measured_destination_stable(cx: &mut ui::TestAppContext) {
    fixture::grid_pointer_batches_keep_the_measured_destination_stable(cx);
}

#[gpui_kit::test]
fn external_order_changes_wait_for_fresh_measurements_before_acquiring(
    cx: &mut ui::TestAppContext,
) {
    fixture::external_order_changes_wait_for_fresh_measurements_before_acquiring(cx);
}
