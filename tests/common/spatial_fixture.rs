//! Real native layout/input; TestPlatform substitutes fonts and GPU presentation.
use crate::{activation, motion, ui};

use gpui_ce_motion::{
    AutoScroll, Color, Drag, DragEvent, Easing, FixedNumber, GesturePhase, GestureTargets, Motion,
    NumberFormat, PinchGesture, ReorderAxis, RichText, ScrollFrame, Target, TextPreset, TextSpan,
    Transition,
};

use motion::{
    Axis, GestureState, MotionAnchor, NumberMotionState, ReorderGroup, ScrollTracker,
    SharedOverlay, TextMotionState,
};

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use ui::{
    AppContext, Context, IntoElement, Modifiers, MouseButton, Render, Role, ScrollHandle,
    TestAppContext, VisualTestContext, Window, div, point, prelude::*, px, size,
};

fn linear() -> Transition {
    Transition::Tween(Duration::from_secs(1), Easing::Linear)
}

struct Fixture {
    mode: u8,
    viewport: MotionAnchor,
    target: MotionAnchor,
    scroll: ScrollHandle,
    tracker: ScrollTracker,
    scroll_frame: Option<ScrollFrame>,
    height: f32,
    drag: GestureState,
    events: Rc<RefCell<Vec<DragEvent>>>,
    pinches: Rc<RefCell<Vec<PinchGesture>>>,
    grid: ReorderGroup<u8>,
    overlay: SharedOverlay<u8>,
    switched: bool,
    text: TextMotionState,
    number: NumberMotionState,
    format: NumberFormat,
    reduced: bool,
    clicks: Rc<Cell<usize>>,
}

impl Render for Fixture {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        cx.set_reduce_motion(self.reduced);
        let mut root = div().relative().size_full();

        match self.mode {
            0 => {
                self.scroll_frame = Some(self.tracker.frame(window, cx));
                root = root.child(
                    self.viewport.element(
                        "viewport",
                        div()
                            .id("scroll")
                            .w(px(200.))
                            .h(px(self.height))
                            .overflow_scroll()
                            .track_scroll(&self.scroll)
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(div().h(px(100.)).flex_shrink_0())
                                    .child(
                                        self.target
                                            .element("target", div().h(px(100.)).flex_shrink_0()),
                                    )
                                    .child(div().h(px(400.)).flex_shrink_0()),
                            ),
                    ),
                );
            }
            1 | 5 => {
                let drag = self.drag.clone();

                root = root
                    .child(
                        self.viewport.element(
                            "constraint",
                            div().w(px(self.height)).h(px(100.)).child(
                                self.drag
                                    .bind(
                                        "drag",
                                        div()
                                            .id("control")
                                            .role(Role::Button)
                                            .child("Drag")
                                            .w(px(40.))
                                            .h(px(30.)),
                                        Motion::new(Target::default()),
                                        GestureTargets::default(),
                                    )
                                    .drag_listener(false),
                            ),
                        ),
                    )
                    .child(
                        div()
                            .id("handle")
                            .absolute()
                            .left(px(220.))
                            .top(px(0.))
                            .w(px(30.))
                            .h(px(30.))
                            .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                                drag.start_drag(
                                    [f64::from(event.position.x), f64::from(event.position.y)],
                                    window,
                                    cx,
                                );
                            }),
                    );
            }
            2 => {
                let mut grid = div().flex().flex_wrap().w(px(100.)).flex_shrink_0();

                for key in self.grid.order().iter().copied() {
                    grid = grid.child(
                        self.grid
                            .item(
                                ("grid", usize::from(key)),
                                key,
                                div()
                                    .id(("item", usize::from(key)))
                                    .role(Role::Button)
                                    .child(format!("{key}"))
                                    .w(px(50.))
                                    .h(px(50.))
                                    .flex_shrink_0(),
                            )
                            .transition(linear()),
                    );
                }

                root = root.child(
                    self.viewport.element(
                        "grid-root",
                        div()
                            .id("grid-scroll")
                            .w(px(100.))
                            .h(px(100.))
                            .overflow_scroll()
                            .track_scroll(&self.scroll)
                            .child(grid),
                    ),
                );
            }
            3 => {
                self.overlay
                    .select(0, if self.switched { "right" } else { "left" });
                let clicks = self.clicks.clone();

                root = root
                    .child(
                        div()
                            .absolute()
                            .left(px(10.))
                            .top(px(20.))
                            .w(px(80.))
                            .h(px(60.))
                            .overflow_hidden()
                            .child(self.overlay.anchor(0, "left", div().w(px(40.)).h(px(30.)))),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(230.))
                            .top(px(80.))
                            .p(px(20.))
                            .w(px(80.))
                            .h(px(60.))
                            .overflow_hidden()
                            .child(self.overlay.anchor(0, "right", div().w(px(40.)).h(px(30.)))),
                    )
                    .child(
                        self.overlay
                            .element(
                                0,
                                "floating",
                                div()
                                    .id("float-control")
                                    .debug_selector(|| "floating".into())
                                    .w(px(40.))
                                    .h(px(30.))
                                    .on_mouse_down(MouseButton::Left, move |_, _, _| {
                                        clicks.set(clicks.get() + 1)
                                    }),
                            )
                            .transition(linear()),
                    );
            }
            4 => {
                let rich = RichText::new([
                    TextSpan::new("Café "),
                    TextSpan::new("motion")
                        .font_weight(700.)
                        .color(Color::rgb(if self.switched { 0xff8800 } else { 0x4488ff })),
                ]);

                root = root
                    .text_size(px(30.))
                    .child(
                        div()
                            .id("rich-box")
                            .debug_selector(|| "rich-box".into())
                            .child(
                                self.text
                                    .rich_element("rich", &rich)
                                    .preset(TextPreset::WipeDown)
                                    .animate_initial(false)
                                    .animate_font_size(
                                        if self.switched { 60. } else { 30. },
                                        linear(),
                                    )
                                    .color_transition(linear()),
                            ),
                    )
                    .child(self.number.formatted_element(
                        "number",
                        FixedNumber::new(if self.switched { 120900 } else { 99999 }, 2).unwrap(),
                        &self.format,
                    ));
            }
            _ => unreachable!(),
        }

        root
    }
}

fn setup(cx: &mut TestAppContext, mode: u8) -> ui::WindowHandle<Fixture> {
    let window = cx.open_window(size(px(400.), px(300.)), |_, _| {
        let viewport = MotionAnchor::default();
        let scroll = ScrollHandle::new();
        let target = MotionAnchor::default().scroll_source(scroll.clone());

        let drag = GestureState::default()
            .with_drag(
                Drag::unconstrained([0.; 2])
                    .elasticity(0.)
                    .direction_lock(5.),
            )
            .constraints(viewport.clone());

        let events = Rc::new(RefCell::new(Vec::new()));
        let output = events.clone();
        let reentrant = drag.clone();

        // The reentry fixture deliberately captures its state and clears the
        // callback at test completion. Other scenes must not retain that cycle.
        let drag = if mode == 1 {
            drag.on_drag(move |event, _, _| {
                let _ = reentrant.interaction();

                output.borrow_mut().push(event);
            })
        } else {
            drag
        };

        let pinches = Rc::new(RefCell::new(Vec::new()));
        let output = pinches.clone();
        let drag = drag.on_pinch(move |event, _, _| output.borrow_mut().push(event));

        Fixture {
            mode,
            tracker: ScrollTracker::new(scroll.clone(), viewport.clone(), target.clone(), Axis::Y),
            target,
            height: if mode == 0 { 100. } else { 200. },
            drag,
            events,
            pinches,
            grid: ReorderGroup::spatial((0..8).collect(), ReorderAxis::Auto)
                .unwrap()
                .autoscroll(
                    scroll.clone(),
                    viewport.clone(),
                    AutoScroll::new(20., 200.).unwrap(),
                ),
            viewport,
            scroll,
            scroll_frame: None,
            overlay: SharedOverlay::default(),
            switched: false,
            text: TextMotionState::default(),
            number: NumberMotionState::default(),
            format: NumberFormat::default().prefix("€").grouping(",", 3, 3),
            reduced: false,
            clicks: Rc::new(Cell::new(0)),
        }
    });

    cx.update_window(window.into(), |_, window, _| activation::activate(window))
        .unwrap();
    cx.run_until_parked();
    draw(cx, window);
    draw(cx, window);

    window
}

fn draw(cx: &mut TestAppContext, window: ui::WindowHandle<Fixture>) {
    cx.update_window(window.into(), |_, window, cx| {
        window.simulate_next_frame(cx);
        window.draw(cx).clear(cx);
    })
    .unwrap();
}

pub fn scroll_intersections_follow_current_native_offsets_and_resizing(cx: &mut TestAppContext) {
    let window = setup(cx, 0);

    assert_eq!(
        window
            .update(cx, |v, _, _| v.scroll_frame.unwrap().progress)
            .unwrap(),
        0.
    );
    window
        .update(cx, |v, _, cx| {
            v.scroll.set_offset(point(px(0.), px(-100.)));
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    assert_eq!(
        window
            .update(cx, |v, _, _| v.scroll_frame.unwrap().progress)
            .unwrap(),
        0.5
    );
    window
        .update(cx, |v, _, cx| {
            v.height = 200.;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    draw(cx, window);
    assert!(
        (window
            .update(cx, |v, _, _| v.scroll_frame.unwrap().progress)
            .unwrap()
            - 2. / 3.)
            .abs()
            < 1e-6
    );
    window
        .update(cx, |v, w, cx| {
            v.target.clear();
            assert!(!v.tracker.frame(w, cx).ready);
        })
        .unwrap();
}

pub fn external_handle_measured_bounds_direction_lock_and_reentrant_events(
    cx: &mut TestAppContext,
) {
    let window = setup(cx, 1);
    let state = window.update(cx, |v, _, _| v.drag.clone()).unwrap();
    let mut visual = VisualTestContext::from_window(window.into(), cx);

    visual.simulate_mouse_down(
        point(px(230.), px(10.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    assert!(state.interaction().dragging);
    visual.simulate_mouse_move(
        point(px(430.), px(50.)),
        Some(MouseButton::Left),
        Modifiers::default(),
    );
    draw(cx, window);
    assert_eq!(state.presented().x_px(), 160.);
    assert_eq!(state.presented().y_px(), 0.);
    visual.simulate_mouse_up(
        point(px(430.), px(50.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    let events = window.update(cx, |v, _, _| v.events.clone()).unwrap();

    assert_eq!(
        events.borrow().first().unwrap().phase,
        GesturePhase::Started
    );
    assert_eq!(events.borrow().last().unwrap().phase, GesturePhase::Ended);
    assert_eq!(events.borrow()[1].locked_axis, Some(0));
    window
        .update(cx, |v, _, cx| {
            v.height = 100.;
            v.reduced = true;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    draw(cx, window);
    assert_eq!(state.presented().x_px(), 60.);
    let _ = state.on_drag(|_, _, _| {});
}

pub fn spatial_reorder_rolls_back_and_edge_scroll_stops_on_release(cx: &mut TestAppContext) {
    let window = setup(cx, 2);
    let group = window.update(cx, |v, _, _| v.grid.clone()).unwrap();
    let mut visual = VisualTestContext::from_window(window.into(), cx);

    visual.simulate_mouse_down(
        point(px(10.), px(10.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    visual.simulate_mouse_move(
        point(px(60.), px(60.)),
        Some(MouseButton::Left),
        Modifiers::default(),
    );
    assert_eq!(&*group.order(), &[1, 2, 3, 0, 4, 5, 6, 7]);
    visual.simulate_keystrokes("escape");
    assert_eq!(&*group.order(), &[0, 1, 2, 3, 4, 5, 6, 7]);
    visual.simulate_mouse_up(
        point(px(60.), px(60.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    draw(cx, window);
    cx.executor().advance_clock(Duration::from_secs(2));
    draw(cx, window);
    visual.simulate_mouse_down(
        point(px(10.), px(10.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    visual.simulate_mouse_move(
        point(px(10.), px(99.)),
        Some(MouseButton::Left),
        Modifiers::default(),
    );
    draw(cx, window);

    for _ in 0..5 {
        cx.executor().advance_clock(Duration::from_millis(32));
        draw(cx, window);
    }

    let scroll = window.update(cx, |v, _, _| v.scroll.clone()).unwrap();

    assert!(
        scroll.offset().y < px(0.),
        "offset {:?}, max {:?}, dragging {}",
        scroll.offset(),
        scroll.max_offset(),
        group.is_dragging()
    );
    visual.simulate_mouse_up(
        point(px(10.), px(99.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    let offset = scroll.offset();

    cx.executor().advance_clock(Duration::from_secs(1));
    draw(cx, window);
    assert_eq!(scroll.offset(), offset);
}

pub fn window_overlay_crosses_clipped_containers_and_keeps_input_at_presentation(
    cx: &mut TestAppContext,
) {
    let window = setup(cx, 3);

    let bounds = |cx: &TestAppContext| {
        VisualTestContext::from_window(window.into(), cx)
            .debug_bounds("floating")
            .unwrap()
    };

    assert_eq!(bounds(cx).origin, point(px(10.), px(20.)));
    window
        .update(cx, |v, _, cx| {
            v.switched = true;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    assert_eq!(bounds(cx).origin, point(px(10.), px(20.)));
    cx.executor().advance_clock(Duration::from_millis(500));
    draw(cx, window);
    assert_eq!(bounds(cx).origin, point(px(130.), px(60.)));
    VisualTestContext::from_window(window.into(), cx).simulate_mouse_down(
        point(px(140.), px(65.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    assert_eq!(window.update(cx, |v, _, _| v.clicks.get()).unwrap(), 1);
    window
        .update(cx, |v, _, cx| {
            v.switched = false;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    assert_eq!(bounds(cx).origin, point(px(130.), px(60.)));
    window
        .update(cx, |v, _, cx| {
            v.switched = true;
            v.reduced = true;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    assert_eq!(bounds(cx).origin, point(px(250.), px(100.)));
}

pub fn rich_text_decimal_numbers_and_style_playback_use_native_layout(cx: &mut TestAppContext) {
    let window = setup(cx, 4);

    window
        .update(cx, |v, _, cx| {
            v.switched = true;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    cx.executor().advance_clock(Duration::from_millis(100));
    draw(cx, window);
    let _before_pause = VisualTestContext::from_window(window.into(), cx)
        .debug_bounds("rich-box")
        .unwrap();

    window.update(cx, |v, w, cx| v.text.pause(w, cx)).unwrap();
    draw(cx, window);
    cx.executor().advance_clock(Duration::from_secs(2));
    draw(cx, window);
    assert!(!window.update(cx, |v, _, _| v.text.is_animating()).unwrap());
    window
        .update(cx, |v, w, cx| {
            v.text.play(w, cx);
            v.reduced = true;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    window
        .update(cx, |v, _, _| {
            assert!(v.text.error().is_none());
            assert!(v.number.text_state().error().is_none());
        })
        .unwrap();
}

pub fn pinch_dispatch_tracks_lifecycle_and_cancellation(cx: &mut TestAppContext) {
    let window = setup(cx, 5);

    for (phase, delta) in [
        (ui::TouchPhase::Started, 0.),
        (ui::TouchPhase::Moved, 0.25),
        (ui::TouchPhase::Ended, 0.),
    ] {
        cx.update_window(window.into(), |_, w, cx| {
            w.dispatch_event(
                ui::PlatformInput::Pinch(ui::PinchEvent {
                    position: point(px(10.), px(10.)),
                    delta,
                    phase,
                    modifiers: Modifiers::default(),
                }),
                cx,
            );
        })
        .unwrap();
    }

    let pinches = window.update(cx, |v, _, _| v.pinches.clone()).unwrap();

    assert_eq!(pinches.borrow().len(), 3);
    assert_eq!(pinches.borrow()[1].scale, 1.25);
    assert_eq!(pinches.borrow()[2].phase, GesturePhase::Ended);
}

pub fn formatted_numbers_expose_the_readable_amount_to_accessibility(_: &mut TestAppContext) {
    use ui::Element as _;

    let number = NumberMotionState::default();

    let format = NumberFormat::default()
        .prefix("€")
        .decimal(",")
        .grouping(" ", 3, 3);

    let element = number
        .formatted_element("balance", FixedNumber::new(120900, 2).unwrap(), &format)
        .min_digits(6)
        .into_element();

    let mut node = ui::accesskit::Node::new(Role::Label);

    element.write_a11y_info(&mut node);

    assert_eq!(node.value(), Some("€1 209,00"));
}

pub fn grid_keyboard_reorder_uses_neighbors_on_both_axes(cx: &mut TestAppContext) {
    let window = setup(cx, 2);
    let group = window.update(cx, |view, _, _| view.grid.clone()).unwrap();
    let mut visual = VisualTestContext::from_window(window.into(), cx);

    visual.simulate_mouse_down(
        point(px(10.), px(10.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    visual.simulate_mouse_up(
        point(px(10.), px(10.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    visual.simulate_keystrokes("alt-right");

    assert_eq!(&*group.order(), &[1, 0, 2, 3, 4, 5, 6, 7]);

    draw(cx, window);
    visual.simulate_keystrokes("alt-down");

    assert_eq!(&*group.order(), &[1, 2, 3, 0, 4, 5, 6, 7]);
    assert!(!group.is_dragging());
}

pub fn grid_pointer_batches_keep_the_measured_destination_stable(cx: &mut TestAppContext) {
    let window = setup(cx, 2);
    let group = window.update(cx, |view, _, _| view.grid.clone()).unwrap();

    VisualTestContext::from_window(window.into(), cx).simulate_mouse_down(
        point(px(10.), px(10.)),
        MouseButton::Left,
        Modifiers::default(),
    );

    cx.update_window(window.into(), |_, w, cx| {
        for x in [60., 61., 62.] {
            w.dispatch_event(
                ui::PlatformInput::MouseMove(ui::MouseMoveEvent {
                    position: point(px(x), px(60.)),
                    pressed_button: Some(MouseButton::Left),
                    modifiers: Modifiers::default(),
                }),
                cx,
            );

            assert_eq!(&*group.order(), &[1, 2, 3, 0, 4, 5, 6, 7]);
        }
    })
    .unwrap();

    draw(cx, window);
    VisualTestContext::from_window(window.into(), cx).simulate_mouse_move(
        point(px(63.), px(60.)),
        Some(MouseButton::Left),
        Modifiers::default(),
    );

    assert_eq!(&*group.order(), &[1, 2, 3, 0, 4, 5, 6, 7]);

    VisualTestContext::from_window(window.into(), cx).simulate_mouse_up(
        point(px(63.), px(60.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    draw(cx, window);

    cx.update_window(window.into(), |_, w, cx| {
        for key in ["alt-up", "alt-left"] {
            w.dispatch_event(
                ui::PlatformInput::KeyDown(ui::KeyDownEvent {
                    keystroke: ui::Keystroke::parse(key).unwrap(),
                    is_held: false,
                    prefer_character_input: false,
                }),
                cx,
            );
        }
    })
    .unwrap();

    assert_eq!(&*group.order(), &[0, 1, 2, 3, 4, 5, 6, 7]);
}

pub fn external_order_changes_wait_for_fresh_measurements_before_acquiring(
    cx: &mut TestAppContext,
) {
    let window = setup(cx, 2);

    window
        .update(cx, |view, w, cx| {
            view.grid.set_order(vec![0]).unwrap();
            assert!(!view.grid.start_drag(&0, [10., 10.], w, cx));
            cx.notify();
        })
        .unwrap();
    draw(cx, window);

    window
        .update(cx, |view, w, cx| {
            assert!(view.grid.start_drag(&0, [10., 10.], w, cx));
        })
        .unwrap();
}
