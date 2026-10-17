//! Covers: measured size/clipping, popped exits, gestures/drag cancellation,
//! scroll bindings, shared position/crossfade, reorder and vector/style drawing.
//! Real: CE layout, prepaint, paint and input dispatch with deterministic clocks.
//! Doubles: CE TestPlatform window/display/GPU. Requires: native-tests feature.
#[path = "common/activation.rs"]
mod activation;
use gpui::{
    AppContext, Context, Modifiers, MouseButton, Render, Role, ScrollHandle, TestAppContext,
    VisualTestContext, Window, div, point, prelude::*, px, size,
};
use gpui_ce_motion::*;
use std::{cell::Cell, rc::Rc, time::Duration};
fn linear() -> Transition {
    Transition::Tween(Duration::from_secs(1), Easing::Linear)
}
fn shape(height: f64) -> PathShape {
    PathShape::new([
        PathCommand::Move([0., 0.]),
        PathCommand::Quadratic([50., height], [100., 0.]),
    ])
    .unwrap()
}
struct Fixture {
    mode: u8,
    height: f32,
    width: f32,
    axis: SizeAxis,
    open: bool,
    swapped: bool,
    reduced: bool,
    gestures: GestureState,
    pop: PopLayout,
    scroll: ScrollHandle,
    shared: SharedLayout<u8>,
    reorder: ReorderGroup<u8>,
    vector: VectorState,
    clicks: Rc<Cell<usize>>,
}
impl Render for Fixture {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        cx.set_reduce_motion(self.reduced);
        let mut root = div().size_full().flex().flex_col().items_start();
        match self.mode {
            0 => {
                let clicks = self.clicks.clone();
                root = root
                    .child(
                        AutoSize::new(
                            "auto",
                            div()
                                .id("content")
                                .debug_selector(|| "content".into())
                                .w(px(self.width))
                                .h(px(self.height))
                                .on_mouse_down(MouseButton::Left, move |_, _, _| {
                                    clicks.set(clicks.get() + 1)
                                }),
                        )
                        .width(self.width)
                        .axis(self.axis)
                        .transition(linear()),
                    )
                    .child(
                        div()
                            .id("sibling")
                            .debug_selector(|| "sibling".into())
                            .w(px(100.))
                            .h(px(10.)),
                    );
            }
            1 => {
                let mut row = div()
                    .id("pop-row")
                    .when(self.swapped, |d| d.justify_end())
                    .gap(px(10.))
                    .border_1()
                    .relative()
                    .flex()
                    .items_start()
                    .p(px(10.))
                    .w(px(200.))
                    .h(px(100.));
                for id in 0..3usize {
                    let card = div()
                        .id(("pop-card", id))
                        .debug_selector(move || format!("pop-{id}"))
                        .w(px(40.))
                        .h(px(30.))
                        .flex_shrink_0();
                    row = row.child(
                        self.pop
                            .item(("pop-item", id), id != 0 || self.open, card)
                            .transition(linear()),
                    );
                }
                root = root.child(self.pop.container("container", row));
            }
            2 => {
                let clicks = self.clicks.clone();
                root = root.child(
                    self.gestures
                        .bind(
                            "gesture",
                            div()
                                .id("button")
                                .role(Role::Button)
                                .aria_label("Drag control")
                                .child("Drag control")
                                .debug_selector(|| "gesture".into())
                                .w(px(80.))
                                .h(px(30.))
                                .on_click(move |_, _, _| clicks.set(clicks.get() + 1)),
                            Motion::new(Target::default()).transition(Transition::Instant),
                            GestureTargets::default()
                                .while_hover(Target::default().opacity(0.5))
                                .while_press(Target::default().opacity(0.2)),
                        )
                        .enabled(self.open),
                );
            }
            3 => {
                root = root
                    .child(ScrollMotion::new(
                        "scroll-motion",
                        self.scroll.clone(),
                        Axis::Y,
                        Target::default().x(0.),
                        Target::default().x(100.),
                        div()
                            .id("indicator")
                            .debug_selector(|| "indicator".into())
                            .w(px(20.))
                            .h(px(20.)),
                    ))
                    .child(
                        div()
                            .id("scroll")
                            .w(px(200.))
                            .h(px(100.))
                            .overflow_scroll()
                            .track_scroll(&self.scroll)
                            .child(div().w(px(200.)).h(px(400.)).flex_shrink_0()),
                    );
            }
            4 => {
                let row = div()
                    .relative()
                    .w(px(300.))
                    .h(px(100.))
                    .child(
                        self.shared
                            .member(
                                0,
                                "shared-a",
                                !self.swapped,
                                div()
                                    .id("a")
                                    .debug_selector(|| "shared-a".into())
                                    .absolute()
                                    .left(px(0.))
                                    .w(px(40.))
                                    .h(px(30.)),
                            )
                            .transition(linear()),
                    )
                    .child(
                        self.shared
                            .member(
                                0,
                                "shared-b",
                                self.swapped,
                                div()
                                    .id("b")
                                    .debug_selector(|| "shared-b".into())
                                    .absolute()
                                    .left(px(200.))
                                    .w(px(40.))
                                    .h(px(30.)),
                            )
                            .transition(linear()),
                    );
                root = root.child(row);
            }
            5 => {
                let mut column = div().flex().flex_col().items_start();
                for key in self.reorder.order().iter().copied() {
                    column = column.child(
                        self.reorder
                            .item(
                                ("reorder", usize::from(key)),
                                key,
                                div()
                                    .id(("card", usize::from(key)))
                                    .role(Role::Button)
                                    .aria_label(format!("Item {key}"))
                                    .child(format!("Item {key}"))
                                    .debug_selector(move || format!("reorder-{key}"))
                                    .w(px(100.))
                                    .h(px(30.))
                                    .flex_shrink_0(),
                            )
                            .transition(linear()),
                    );
                }
                root = root.child(column);
            }
            6 => {
                root = root.child(
                    self.vector
                        .element(
                            "vector",
                            shape(if self.swapped { 80. } else { 0. }),
                            [120., 100.],
                            Color::rgb(0xffffff),
                        )
                        .unwrap()
                        .reveal(
                            StrokeReveal::new(if self.open { 1. } else { 0.5 }, 0., 1.).unwrap(),
                        )
                        .transition(linear()),
                );
            }
            _ => unreachable!(),
        }
        root
    }
}
fn setup(cx: &mut TestAppContext, mode: u8) -> gpui::WindowHandle<Fixture> {
    let window = cx.open_window(size(px(400.), px(300.)), |_, _| Fixture {
        mode,
        height: 40.,
        width: 100.,
        axis: SizeAxis::Height,
        open: true,
        swapped: false,
        reduced: false,
        gestures: GestureState::default()
            .with_drag(Drag::new([0., 0.], [0., 0.], [100., 0.]))
            .in_view(0.5, false),
        pop: PopLayout::default(),
        scroll: ScrollHandle::new(),
        shared: SharedLayout::default(),
        reorder: ReorderGroup::new(vec![1, 2, 3], Axis::Y).unwrap(),
        vector: VectorState::new(shape(0.)),
        clicks: Rc::new(Cell::new(0)),
    });
    cx.update_window(window.into(), |_, w, _| activation::activate(w))
        .unwrap();
    cx.run_until_parked();
    draw(cx, window);
    window
}
fn draw(cx: &mut TestAppContext, window: gpui::WindowHandle<Fixture>) {
    cx.update_window(window.into(), |_, w, cx| {
        w.simulate_next_frame(cx);
        w.draw(cx).clear(cx);
    })
    .unwrap();
}
fn bounds(
    cx: &TestAppContext,
    window: gpui::WindowHandle<Fixture>,
    key: &'static str,
) -> gpui::Bounds<gpui::Pixels> {
    VisualTestContext::from_window(window.into(), cx)
        .debug_bounds(key)
        .unwrap()
}
#[gpui::test]
fn intrinsic_size_reflows_siblings_clips_input_and_retargets_without_jumping(
    cx: &mut TestAppContext,
) {
    let window = setup(cx, 0);
    assert_eq!(bounds(cx, window, "sibling").origin.y, px(40.));
    window
        .update(cx, |v, _, cx| {
            v.height = 100.;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    assert_eq!(bounds(cx, window, "sibling").origin.y, px(40.));
    let clicks = window.update(cx, |v, _, _| v.clicks.clone()).unwrap();
    VisualTestContext::from_window(window.into(), cx).simulate_mouse_down(
        point(px(10.), px(80.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    assert_eq!(clicks.get(), 0);
    cx.executor().advance_clock(Duration::from_millis(500));
    draw(cx, window);
    assert_eq!(bounds(cx, window, "sibling").origin.y, px(70.));
    window
        .update(cx, |v, _, cx| {
            v.height = 160.;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    assert_eq!(bounds(cx, window, "sibling").origin.y, px(70.));
    window
        .update(cx, |v, _, cx| {
            v.reduced = true;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    assert_eq!(bounds(cx, window, "sibling").origin.y, px(160.));
}
#[gpui::test]
fn popped_exit_preserves_bounds_and_allows_immediate_sibling_reflow(cx: &mut TestAppContext) {
    let window = setup(cx, 1);
    let old = bounds(cx, window, "pop-0");
    assert_eq!(old.origin.x, px(11.));
    window
        .update(cx, |v, _, cx| {
            v.open = false;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    assert_eq!(bounds(cx, window, "pop-0"), old);
    assert_eq!(bounds(cx, window, "pop-1").origin.x, px(61.));
    cx.executor().advance_clock(Duration::from_millis(500));
    draw(cx, window);
    assert_eq!(bounds(cx, window, "pop-1").origin.x, px(36.));
    cx.executor().advance_clock(Duration::from_secs(1));
    draw(cx, window);
    assert_eq!(bounds(cx, window, "pop-1").origin.x, px(11.));
    assert_eq!(bounds(cx, window, "pop-0"), old);
}
#[gpui::test]
fn native_gestures_bind_hover_press_drag_outside_release_and_keyboard(cx: &mut TestAppContext) {
    let window = setup(cx, 2);
    let state = window.update(cx, |v, _, _| v.gestures.clone()).unwrap();
    assert!(state.interaction().in_view);
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_mouse_move(point(px(10.), px(10.)), None, Modifiers::default());
    assert!(state.interaction().hovered);
    visual.simulate_mouse_down(
        point(px(10.), px(10.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    assert!(state.interaction().pressed);
    assert!(state.interaction().dragging);
    draw(cx, window);
    assert_eq!(state.presented().alpha(), 0.2);
    cx.executor().advance_clock(Duration::from_millis(50));
    visual.simulate_mouse_move(
        point(px(200.), px(10.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    draw(cx, window);
    assert!(state.presented().x_px() > 0.);
    visual.simulate_mouse_up(
        point(px(250.), px(10.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    assert!(!state.interaction().dragging);
    assert!(!state.interaction().pressed);
    visual.simulate_keystrokes("right");
    cx.executor().advance_clock(Duration::from_secs(3));
    draw(cx, window);
    assert!(state.presented().x_px() <= 100.);
}
#[gpui::test]
fn drag_cancels_on_escape_blur_and_window_deactivation(cx: &mut TestAppContext) {
    let window = setup(cx, 2);
    let state = window.update(cx, |v, _, _| v.gestures.clone()).unwrap();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_mouse_down(
        point(px(10.), px(10.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    assert!(state.interaction().dragging);
    visual.simulate_keystrokes("escape");
    assert!(!state.interaction().dragging);
    visual.simulate_mouse_down(
        point(px(10.), px(10.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    assert!(state.interaction().dragging);
    let other_focus = window
        .update(cx, |_, w, cx| {
            let focus = cx.focus_handle();
            focus.focus(w, cx);
            cx.notify();
            focus
        })
        .unwrap();
    draw(cx, window);
    assert!(!state.interaction().dragging);
    drop(other_focus);
    visual.simulate_mouse_down(
        point(px(10.), px(10.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    assert!(state.interaction().dragging);
    visual.deactivate_window();
    draw(cx, window);
    assert!(!state.interaction().dragging);
    assert!(!state.interaction().pressed);
}
#[gpui::test]
fn native_scroll_binding_reads_real_extents_without_host_metric_calculation(
    cx: &mut TestAppContext,
) {
    let window = setup(cx, 3);
    let scroll = window.update(cx, |v, _, _| v.scroll.clone()).unwrap();
    assert_eq!(native_scroll_progress(&scroll, Axis::Y), 0.);
    window
        .update(cx, |v, _, cx| {
            v.scroll.set_offset(point(px(0.), px(-150.)));
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    assert!((native_scroll_progress(&scroll, Axis::Y) - 0.5).abs() < 1e-6);
    assert_eq!(bounds(cx, window, "indicator").origin.x, px(50.));
}
#[gpui::test]
fn shared_position_crossfade_retargets_across_separate_native_ids(cx: &mut TestAppContext) {
    let window = setup(cx, 4);
    assert_eq!(bounds(cx, window, "shared-a").origin.x, px(0.));
    window
        .update(cx, |v, _, cx| {
            v.swapped = true;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    assert_eq!(bounds(cx, window, "shared-b").origin.x, px(0.));
    cx.executor().advance_clock(Duration::from_millis(500));
    draw(cx, window);
    assert_eq!(bounds(cx, window, "shared-b").origin.x, px(100.));
    assert_eq!(bounds(cx, window, "shared-a").origin.x, px(100.));
    window
        .update(cx, |v, _, cx| {
            v.swapped = false;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    assert_eq!(bounds(cx, window, "shared-a").origin.x, px(100.));
    cx.executor().advance_clock(Duration::from_secs(2));
    draw(cx, window);
    assert_eq!(bounds(cx, window, "shared-a").origin.x, px(0.));
    let shared = window.update(cx, |v, _, _| v.shared.clone()).unwrap();
    assert_eq!(shared.len(), 1);
    shared.remove(&0);
    assert!(shared.is_empty());
}
#[gpui::test]
fn reorder_uses_actual_geometry_and_commits_pointer_and_keyboard_order(cx: &mut TestAppContext) {
    let window = setup(cx, 5);
    let group = window.update(cx, |v, _, _| v.reorder.clone()).unwrap();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_mouse_down(
        point(px(10.), px(15.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    assert!(group.is_dragging());
    visual.simulate_mouse_move(
        point(px(10.), px(75.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    assert_eq!(&*group.order(), &[2, 3, 1]);
    draw(cx, window);
    assert_eq!(bounds(cx, window, "reorder-1").origin.y, px(60.));
    visual.simulate_mouse_up(
        point(px(200.), px(200.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    assert!(!group.is_dragging());
    visual.simulate_keystrokes("alt-up");
    assert_eq!(&*group.order(), &[2, 1, 3]);
    assert!(group.set_order(vec![1, 1]).is_err());
    assert_eq!(&*group.order(), &[2, 1, 3]);
}
#[gpui::test]
fn reorder_escape_restores_original_order(cx: &mut TestAppContext) {
    let window = setup(cx, 5);
    let group = window.update(cx, |v, _, _| v.reorder.clone()).unwrap();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_mouse_down(
        point(px(10.), px(15.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    visual.simulate_mouse_move(
        point(px(10.), px(75.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    assert_eq!(&*group.order(), &[2, 3, 1]);
    visual.simulate_keystrokes("escape");
    assert_eq!(&*group.order(), &[1, 2, 3]);
    assert!(!group.is_dragging());
}
#[gpui::test]
fn vector_morph_and_reveal_tessellate_and_stop_scheduling_after_reduced_motion(
    cx: &mut TestAppContext,
) {
    let window = setup(cx, 6);
    assert_eq!(window.update(cx, |v, _, _| v.vector.error()).unwrap(), None);
    window
        .update(cx, |v, _, cx| {
            v.swapped = true;
            v.open = false;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    cx.executor().advance_clock(Duration::from_millis(500));
    draw(cx, window);
    assert_eq!(window.update(cx, |v, _, _| v.vector.error()).unwrap(), None);
    window
        .update(cx, |v, _, cx| {
            v.reduced = true;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    assert_eq!(
        cx.update_window(window.into(), |_, w, cx| w.simulate_next_frame(cx))
            .unwrap(),
        0
    );
}

#[gpui::test]
fn unanimated_measured_axes_do_not_schedule_unnecessary_frames(cx: &mut TestAppContext) {
    let window = setup(cx, 0);
    window
        .update(cx, |v, _, cx| {
            v.width = 140.;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    assert_eq!(bounds(cx, window, "content").size.width, px(140.));
    assert_eq!(
        cx.update_window(window.into(), |_, w, cx| w.simulate_next_frame(cx))
            .unwrap(),
        0
    );
    window
        .update(cx, |v, _, cx| {
            v.axis = SizeAxis::Width;
            v.height = 100.;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    assert_eq!(bounds(cx, window, "sibling").origin.y, px(100.));
    assert_eq!(
        cx.update_window(window.into(), |_, w, cx| w.simulate_next_frame(cx))
            .unwrap(),
        0
    );
}
#[gpui::test]
fn first_active_shared_member_snaps_even_when_inactive_member_is_traversed_first(
    cx: &mut TestAppContext,
) {
    let window = setup(cx, 4);
    window
        .update(cx, |v, _, cx| {
            v.shared.remove(&0);
            v.swapped = true;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    assert_eq!(bounds(cx, window, "shared-b").origin.x, px(200.));
}

#[gpui::test]
fn keyboard_motion_reaches_a_control_with_text_descendants(cx: &mut TestAppContext) {
    let window = setup(cx, 2);
    let state = window.update(cx, |v, _, _| v.gestures.clone()).unwrap();
    let focus = state.focus_handle().unwrap();
    cx.update_window(window.into(), |_, w, cx| focus.focus(w, cx))
        .unwrap();
    draw(cx, window);
    VisualTestContext::from_window(window.into(), cx).simulate_keystrokes("right");
    cx.executor().advance_clock(Duration::from_secs(3));
    draw(cx, window);
    assert_eq!(state.presented().x_px(), 10.);
}

#[gpui::test]
fn disabling_native_bindings_cancels_drag_and_blocks_child_activation(cx: &mut TestAppContext) {
    let window = setup(cx, 2);
    let state = window.update(cx, |v, _, _| v.gestures.clone()).unwrap();
    let clicks = window.update(cx, |v, _, _| v.clicks.clone()).unwrap();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_mouse_down(
        point(px(10.), px(10.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    assert!(state.interaction().dragging);
    window
        .update(cx, |v, _, cx| {
            v.open = false;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    assert!(!state.interaction().dragging);
    visual.simulate_mouse_up(
        point(px(10.), px(10.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    visual.simulate_click(point(px(10.), px(10.)), Modifiers::default());
    assert_eq!(clicks.get(), 0);
    assert!(!state.interaction().pressed);
}
#[gpui::test]
fn reorder_callbacks_only_fire_for_changes_and_can_reenter_the_group(cx: &mut TestAppContext) {
    let window = setup(cx, 5);
    let group = window.update(cx, |v, _, _| v.reorder.clone()).unwrap();
    let count = Rc::new(Cell::new(0));
    let calls = count.clone();
    let observer = group.clone();
    group.clone().on_change(move |order, _, _| {
        assert_eq!(&*observer.order(), order);
        observer.set_order(order.to_vec()).unwrap();
        calls.set(calls.get() + 1);
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_click(point(px(10.), px(15.)), Modifiers::default());
    assert_eq!(count.get(), 0);
    visual.simulate_keystrokes("alt-down");
    assert_eq!(count.get(), 1);
    assert_eq!(&*group.order(), &[2, 1, 3]);
    // Break this test's deliberate callback/group self-reference after checking reentry.
    group.on_change(|_, _, _| {});
}

#[gpui::test]
fn restoring_a_popped_item_preserves_its_displayed_position(cx: &mut TestAppContext) {
    let window = setup(cx, 1);
    window
        .update(cx, |v, _, cx| {
            v.swapped = true;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    cx.executor().advance_clock(Duration::from_millis(500));
    draw(cx, window);
    let presented = bounds(cx, window, "pop-0").origin;
    window
        .update(cx, |v, _, cx| {
            v.open = false;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    assert_eq!(bounds(cx, window, "pop-0").origin, presented);
    cx.executor().advance_clock(Duration::from_millis(500));
    draw(cx, window);
    window
        .update(cx, |v, _, cx| {
            v.open = true;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    assert_eq!(bounds(cx, window, "pop-0").origin, presented);
}
