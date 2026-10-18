//! Covers the Kit adapter through real layout, executor time, and reduced motion.
//! Real: portable engine, Kit layout/paint preparation, and native element styles.
//! Doubles: TestPlatform display/GPU. Requires: native-tests.

use gpui_ce_motion::{Easing, Motion, Target, Transition};
use gpui_kit::test::TestWindowExt;

use gpui_kit::{
    AppContext, Bounds, Context, Entity, IntoElement, Point, Render, TestAppContext,
    TestSupportExt, Window, WindowBounds, WindowHandle, WindowOptions, base::Root, div, prelude::*,
    px, size,
};

use gpui_motion_kit::{Frame, MotionState, ValueState};
use std::time::Duration;

struct Probe {
    state: MotionState,
    scalar: ValueState,
    x: f32,
    present: bool,
    frame: Option<Frame>,
    value: f64,
}

impl Render for Probe {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let transition = Transition::Tween(Duration::from_millis(200), Easing::Linear);

        let frame = self.state.frame(
            Motion::new(Target::default().x(self.x))
                .exit(Target::default().x(self.x).opacity(0.))
                .present(self.present)
                .transition(transition),
            window,
            cx,
        );

        self.frame = Some(frame);
        self.value = self
            .scalar
            .frame(self.x.into(), transition, window, cx)
            .value();

        div().size_full().child(
            frame.render(|frame| {
                frame.apply(div().id("probe").test_support().w(px(40.)).h(px(30.)))
            }),
        )
    }
}

fn setup(cx: &mut TestAppContext) -> (WindowHandle<Root>, Entity<Probe>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(false);
        let (window, view) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    Point::default(),
                    size(px(400.), px(300.)),
                ))),
                ..Default::default()
            },
            cx,
            |_, cx| {
                cx.new(|_| Probe {
                    state: MotionState::default(),
                    scalar: ValueState::new(0.),
                    x: 0.,
                    present: true,
                    frame: None,
                    value: 0.,
                })
            },
        )
        .unwrap();

        let handle = window.downcast::<Root>().unwrap();

        cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
            .unwrap();

        (handle, view)
    })
}

#[gpui_kit::test]
fn uses_executor_time_and_preserves_presentation_on_reversal(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx);

    cx.update(|cx| {
        view.update(cx, |probe, cx| {
            probe.x = 100.;
            cx.notify();
        })
    });
    cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
        .unwrap();
    cx.executor().advance_clock(Duration::from_millis(80));
    let before = cx
        .update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);

            window.find("probe").bounds().origin.x
        })
        .unwrap();

    assert!((f32::from(before) - 40.).abs() < 1.);
    cx.update(|cx| {
        assert!((view.read(cx).value - 40.).abs() < 1.);
        view.update(cx, |probe, cx| {
            probe.x = -40.;
            cx.notify();
        });
    });
    let reversed = cx
        .update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);

            window.find("probe").bounds().origin.x
        })
        .unwrap();

    assert!((f32::from(reversed - before)).abs() < 1.);
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
        .unwrap();
    cx.update(|cx| {
        assert!(!view.read(cx).frame.unwrap().is_animating());
        assert!((view.read(cx).value + 40.).abs() < 0.01);
    });
}

#[gpui_kit::test]
fn reduced_motion_finishes_exit_and_omits_native_input_target(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx);

    cx.update(|cx| {
        cx.set_reduce_motion(true);
        view.update(cx, |probe, cx| {
            probe.present = false;
            probe.x = 120.;
            cx.notify();
        });
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("probe").is_none());
    })
    .unwrap();
    cx.update(|cx| {
        let frame = view.read(cx).frame.unwrap();

        assert!(!frame.is_mounted());
        assert!(!frame.is_animating());
        assert_eq!(view.read(cx).value, 120.);
    });
}
