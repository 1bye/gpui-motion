//! Exercises the production gallery through native Kit controls and input.
use super::*;
use gpui_kit::{
    Entity, ScrollDelta, TestAppContext, WindowHandle, base::Root, test::TestWindowExt,
};

fn open(
    cx: &mut TestAppContext,
    scene: usize,
    reduced: bool,
) -> (WindowHandle<Root>, Entity<Gallery>) {
    let result = cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(reduced);
        let (window, view) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(1020.), px(760.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| {
                cx.new(|cx| {
                    let mut view = Gallery::new(window, cx);

                    view.scene = scene;

                    view
                })
            },
        )
        .unwrap();

        let handle = window.downcast::<Root>().unwrap();

        cx.update_window(handle.into(), |_, window, cx| {
            window.activate_window();
            window.render_frame(cx);
        })
        .unwrap();

        (handle, view)
    });

    cx.run_until_parked();
    cx.update_window(result.0.into(), |_, window, cx| window.render_frame(cx))
        .unwrap();

    result
}

fn advance(cx: &mut TestAppContext, handle: WindowHandle<Root>, millis: u64) {
    cx.executor().advance_clock(Duration::from_millis(millis));
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.simulate_next_frame(cx);
        window.render_frame(cx);
    })
    .unwrap();
}

#[gpui_kit::test]
fn styles_and_measured_content_respond_to_the_real_button(cx: &mut TestAppContext) {
    for scene in [0, 3] {
        let (handle, view) = open(cx, scene, false);
        let selector = if scene == 0 {
            "styled-card"
        } else {
            "content-sibling"
        };
        let before = cx
            .update_window(handle.into(), |_, window, _| window.find(selector).bounds())
            .unwrap();

        cx.update_window(handle.into(), |_, window, cx| {
            window.click("change-state", cx)
        })
        .unwrap();
        advance(cx, handle, 130);
        let mid = cx
            .update_window(handle.into(), |_, window, _| window.find(selector).bounds())
            .unwrap();

        assert_ne!(mid, before);
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("change-state", cx)
        })
        .unwrap();
        let reversed = cx
            .update_window(handle.into(), |_, window, _| window.find(selector).bounds())
            .unwrap();

        assert_eq!(
            reversed, mid,
            "retargeting starts at the current presentation"
        );
        advance(cx, handle, 2000);
        assert!(!view.read_with(cx, |view, _| view.expanded));
    }
}

#[gpui_kit::test]
fn observed_drag_and_reorder_elements_keep_native_focus_and_keyboard_input(
    cx: &mut TestAppContext,
) {
    let (handle, view) = open(cx, 1, true);

    cx.update_window(handle.into(), |_, window, cx| {
        let from = window.find("drag-card").bounds().center();

        window.drag(from, from + point(px(100.), px(60.)), cx);
        assert_eq!(window.find("drag-card").focused(), Some(true));
        window.press("right", cx);
    })
    .unwrap();
    assert!(!view.read_with(cx, |view, _| view.gesture.interaction().dragging));
    assert!(view.read_with(cx, |view, _| view.gesture.presented().x_px()) > 0.);

    let (handle, view) = open(cx, 4, true);

    cx.update_window(handle.into(), |_, window, cx| {
        let from = window.find(("sort-row", 1usize)).bounds().center();
        let to = window.find(("sort-row", 3usize)).bounds().center();

        window.drag(from, to, cx);
    })
    .unwrap();
    let pointer_order = view.read_with(cx, |view, _| view.reorder.order().to_vec());

    assert_eq!(pointer_order, vec![2, 3, 1]);
    cx.update_window(handle.into(), |_, window, cx| window.press("alt-up", cx))
        .unwrap();
    assert_eq!(
        view.read_with(cx, |view, _| view.reorder.order().to_vec()),
        vec![2, 1, 3]
    );
}

#[gpui_kit::test]
fn manual_scroll_updates_measured_progress_and_in_view_state(cx: &mut TestAppContext) {
    let (handle, view) = open(cx, 2, false);

    cx.update_window(handle.into(), |_, window, cx| {
        window.scroll(
            "manual-scroll",
            ScrollDelta::Pixels(point(px(0.), px(-180.))),
            cx,
        );
    })
    .unwrap();
    advance(cx, handle, 500);
    assert!(view.read_with(cx, |view, _| view.scroll.offset().y) < px(0.));
    assert!(view.read_with(cx, |view, _| view.visibility.interaction().in_view));
    cx.update_window(handle.into(), |_, window, _| {
        assert!(window.find("scroll-progress").bounds().size.width > px(100.));
    })
    .unwrap();
}

#[gpui_kit::test]
fn popped_rows_remove_restore_and_reflow_through_native_controls(cx: &mut TestAppContext) {
    let (handle, _) = open(cx, 4, true);
    let before = cx
        .update_window(handle.into(), |_, window, _| {
            window.find(("pop-card", 1usize)).bounds().origin.y
        })
        .unwrap();

    cx.update_window(handle.into(), |_, window, cx| {
        window.click("remove-row", cx);
        assert!(window.try_find(("pop-card", 0usize)).is_none());
        assert!(window.find(("pop-card", 1usize)).bounds().origin.y < before);
        window.click("remove-row", cx);
        assert!(window.find(("pop-card", 0usize)).visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn shared_and_svg_transitions_stop_under_reduced_motion(cx: &mut TestAppContext) {
    for scene in [5, 7] {
        let (handle, view) = open(cx, scene, false);

        cx.update_window(handle.into(), |_, window, cx| {
            window.click("change-state", cx)
        })
        .unwrap();
        advance(cx, handle, 100);
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("reduced-motion", cx)
        })
        .unwrap();
        advance(cx, handle, 1);
        assert!(view.read_with(cx, |view, _| view.expanded));
        cx.update_window(handle.into(), |_, window, cx| {
            assert!(cx.reduce_motion());
            assert_eq!(window.simulate_next_frame(cx), 0);
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn playback_buttons_pause_seek_resume_and_replay_the_real_players(cx: &mut TestAppContext) {
    let (handle, view) = open(cx, 6, false);

    advance(cx, handle, 200);
    cx.update_window(handle.into(), |_, window, cx| window.click("pause", cx))
        .unwrap();
    let before = cx
        .update_window(handle.into(), |_, window, _| {
            window.find("player-card").bounds()
        })
        .unwrap();

    advance(cx, handle, 200);
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(window.find("player-card").bounds(), before);
        window.click("seek", cx);
    })
    .unwrap();
    assert!(!view.read_with(cx, |view, _| view.player.is_playing()));
    cx.update_window(handle.into(), |_, window, cx| window.click("play", cx))
        .unwrap();
    advance(cx, handle, 100);
    assert!(view.read_with(cx, |view, _| view.player.is_playing()));
    cx.update_window(handle.into(), |_, window, cx| window.click("replay", cx))
        .unwrap();
    advance(cx, handle, 1300);
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(window.simulate_next_frame(cx), 0)
    })
    .unwrap();
}
