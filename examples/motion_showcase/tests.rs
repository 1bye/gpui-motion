//! Covers showcase layout regressions using the actual Studio view.
//! Real: CE layout/prepaint and motion. Doubles: CE TestPlatform display/GPU.
//! Requires: demo,native-tests; these checks do not measure GPU presentation.
use super::*;
use gpui::{TestAppContext, VisualTestContext, WindowHandle};

fn activate(cx: &mut TestAppContext, window: WindowHandle<Studio>) {
    window
        .update(cx, |_, w, cx| {
            cx.set_reduce_motion(false);
            w.activate_window();
        })
        .unwrap();
    cx.run_until_parked();
    draw(cx, window);
}

fn click(cx: &TestAppContext, window: WindowHandle<Studio>, selector: &'static str) {
    let center = bounds(cx, window, selector).center();
    VisualTestContext::from_window(window.into(), cx)
        .simulate_click(center, gpui::Modifiers::default());
}

fn draw(cx: &mut TestAppContext, window: WindowHandle<Studio>) {
    cx.update_window(window.into(), |_, w, cx| {
        w.simulate_next_frame(cx);
        w.draw(cx).clear(cx);
    })
    .unwrap();
}

fn setup(cx: &mut TestAppContext, scene: usize, alternate: bool) -> WindowHandle<Studio> {
    let window = cx.open_window(size(px(1000.), px(600.)), |w, cx| {
        cx.set_reduce_motion(true);
        let mut studio = Studio::new(w, cx);
        studio.scene = scene;
        studio.rendered_scene = scene;
        studio.alternate = alternate;
        studio.spring = Transition::Tween(ms(1000), Easing::Linear);
        studio
    });
    draw(cx, window);
    window
}

fn bounds(
    cx: &TestAppContext,
    window: WindowHandle<Studio>,
    selector: &'static str,
) -> Bounds<gpui::Pixels> {
    VisualTestContext::from_window(window.into(), cx)
        .debug_bounds(selector)
        .expect("rendered showcase element")
}

#[gpui::test]
fn collapsing_card_surface_follows_the_animated_reservation(cx: &mut TestAppContext) {
    let window = setup(cx, 1, true);
    let expanded = bounds(cx, window, "expansion-card").size.height;
    window
        .update(cx, |studio, _, cx| {
            cx.set_reduce_motion(false);
            studio.alternate = false;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    let first = bounds(cx, window, "expansion-card").size.height;
    assert!(
        (f32::from(first - expanded)).abs() < 1.,
        "card jumped from {expanded:?} to {first:?}"
    );
    cx.executor().advance_clock(ms(500));
    draw(cx, window);
    let halfway = bounds(cx, window, "expansion-card").size.height;
    assert!(
        halfway < first - px(20.),
        "card should visibly shrink over time"
    );
    window
        .update(cx, |studio, _, cx| {
            studio.alternate = true;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    let interrupted = bounds(cx, window, "expansion-card").size.height;
    assert!(
        (f32::from(interrupted - halfway)).abs() < 1.,
        "reversal must preserve the displayed height"
    );
}

#[gpui::test]
fn removing_a_card_does_not_jump_the_centered_container(cx: &mut TestAppContext) {
    let window = setup(cx, 2, false);
    let anchor = bounds(cx, window, "exit-card-0").origin;
    let moving = bounds(cx, window, "exit-card-2").origin;
    window
        .update(cx, |studio, _, cx| {
            cx.set_reduce_motion(false);
            studio.alternate = true;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    assert_eq!(bounds(cx, window, "exit-card-0").origin, anchor);
    assert_eq!(bounds(cx, window, "exit-card-2").origin, moving);
    cx.executor().advance_clock(ms(500));
    draw(cx, window);
    assert_eq!(bounds(cx, window, "exit-card-0").origin, anchor);
    assert!(bounds(cx, window, "exit-card-2").origin.x < moving.x);
    let interrupted = bounds(cx, window, "exit-card-2").origin;
    window
        .update(cx, |studio, _, cx| {
            studio.alternate = false;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    assert_eq!(bounds(cx, window, "exit-card-0").origin, anchor);
    assert_eq!(bounds(cx, window, "exit-card-2").origin, interrupted);
}

#[gpui::test]
fn keyframe_dot_is_centered_on_its_track(cx: &mut TestAppContext) {
    let window = setup(cx, 5, false);
    let track = bounds(cx, window, "vector-track");
    let dot = bounds(cx, window, "vector-dot");
    assert_eq!(dot.center().y, track.center().y);
    assert_eq!(
        window
            .update(cx, |studio, _, _| studio.vector.error())
            .unwrap(),
        None
    );
}

#[gpui::test]
fn showcase_wave_tessellates_through_morph_reveal_and_interruption(cx: &mut TestAppContext) {
    let window = setup(cx, 5, false);
    for (step, alternate) in [true, true, false, false, true, false]
        .into_iter()
        .enumerate()
    {
        window
            .update(cx, |studio, _, cx| {
                cx.set_reduce_motion(false);
                studio.alternate = alternate;
                cx.notify();
            })
            .unwrap();
        draw(cx, window);
        cx.executor().advance_clock(ms(200));
        draw(cx, window);
        assert_eq!(
            window
                .update(cx, |studio, _, _| studio.vector.error())
                .unwrap(),
            None,
            "tessellation at step {step}"
        );
    }
}

#[gpui::test]
fn scroll_experiment_yields_to_presenter_until_continue(cx: &mut TestAppContext) {
    let window = setup(cx, 4, false);
    activate(cx, window);
    click(cx, window, "play");
    draw(cx, window);
    window
        .update(cx, |s, _, _| {
            assert!(s.exploring);
            assert!(!s.touring);
        })
        .unwrap();

    click(cx, window, "scroll-down");
    draw(cx, window);
    let offset = window.update(cx, |s, _, _| s.scroll.offset()).unwrap();
    assert!(offset.y < px(0.));
    for _ in 0..3 {
        cx.executor().advance_clock(ms(500));
        draw(cx, window);
        assert_eq!(
            window.update(cx, |s, _, _| s.scroll.offset()).unwrap(),
            offset
        );
    }
    click(cx, window, "scroll-up");
    draw(cx, window);
    assert_eq!(
        window.update(cx, |s, _, _| s.scroll.offset().y).unwrap(),
        px(0.)
    );

    VisualTestContext::from_window(window.into(), cx).simulate_event(gpui::ScrollWheelEvent {
        position: bounds(cx, window, "playlist").center(),
        delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(-60.))),
        modifiers: gpui::Modifiers::default(),
        touch_phase: gpui::TouchPhase::Moved,
    });
    draw(cx, window);
    assert!(window.update(cx, |s, _, _| s.scroll.offset().y).unwrap() < px(0.));

    click(cx, window, "play");
    draw(cx, window);
    window
        .update(cx, |s, _, _| {
            assert!(!s.exploring);
            assert!(s.touring);
            assert_eq!(s.scene, 5);
        })
        .unwrap();
}

#[gpui::test]
fn tour_pauses_when_it_reaches_the_scroll_experiment(cx: &mut TestAppContext) {
    let window = setup(cx, 3, false);
    activate(cx, window);
    window
        .update(cx, |s, _, cx| {
            let now = s.epoch.elapsed();
            s.clock.seek(Duration::from_secs(24), now);
            s.clock.play(now);
            s.touring = true;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    window
        .update(cx, |s, _, _| {
            assert_eq!(s.scene, 4);
            assert!(s.exploring);
            assert!(!s.touring);
        })
        .unwrap();
}

#[gpui::test]
fn tabs_keep_selection_when_dragged_and_support_keyboard_reorder(cx: &mut TestAppContext) {
    let window = setup(cx, 6, false);
    activate(cx, window);
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    let from = bounds(cx, window, "tab-2").center();
    let to = bounds(cx, window, "tab-0").center();
    let neighbor = bounds(cx, window, "tab-0").origin;
    let group = window.update(cx, |s, _, _| s.tabs.clone()).unwrap();
    visual.simulate_mouse_down(from, gpui::MouseButton::Left, gpui::Modifiers::default());
    assert!(group.is_dragging());
    assert_eq!(window.update(cx, |s, _, _| s.active_tab).unwrap(), 2);
    visual.simulate_mouse_move(to, gpui::MouseButton::Left, gpui::Modifiers::default());
    draw(cx, window);
    assert_eq!(&*group.order(), &[2, 0, 1, 3]);
    assert_eq!(bounds(cx, window, "tab-0").origin, neighbor);
    cx.executor().advance_clock(ms(500));
    draw(cx, window);
    assert!(bounds(cx, window, "tab-0").origin.x > neighbor.x);
    visual.simulate_mouse_up(to, gpui::MouseButton::Left, gpui::Modifiers::default());
    draw(cx, window);
    assert!(!group.is_dragging());
    assert_eq!(window.update(cx, |s, _, _| s.active_tab).unwrap(), 2);
    visual.simulate_keystrokes("alt-right");
    draw(cx, window);
    assert_eq!(&*group.order(), &[0, 2, 1, 3]);
    assert_eq!(window.update(cx, |s, _, _| s.scene).unwrap(), 6);
}

#[gpui::test]
fn tabs_escape_restores_order_and_enter_selects_focused_tab(cx: &mut TestAppContext) {
    let window = setup(cx, 6, false);
    activate(cx, window);
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    let from = bounds(cx, window, "tab-3").center();
    let to = bounds(cx, window, "tab-0").center();
    let group = window.update(cx, |s, _, _| s.tabs.clone()).unwrap();
    visual.simulate_mouse_down(from, gpui::MouseButton::Left, gpui::Modifiers::default());
    visual.simulate_mouse_move(to, gpui::MouseButton::Left, gpui::Modifiers::default());
    assert_eq!(&*group.order(), &[3, 0, 1, 2]);
    visual.simulate_keystrokes("escape");
    assert_eq!(&*group.order(), &[0, 1, 2, 3]);
    assert!(!group.is_dragging());
    // Leave focus on Notes while changing the selected content programmatically.
    window
        .update(cx, |s, _, cx| {
            s.active_tab = 0;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    visual.simulate_keystrokes("enter");
    assert_eq!(window.update(cx, |s, _, _| s.active_tab).unwrap(), 3);
}
