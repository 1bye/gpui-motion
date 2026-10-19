//! Shared native contract: real adapters/layout, with TestPlatform time and display.
use crate::{motion, ui};
use gpui_ce_motion::{Motion, Target};
use motion::{Frame, Motions};
use std::time::Duration;

use ui::{
    AppContext, Context, IntoElement, Render, TestAppContext, Window, WindowHandle, div,
    prelude::*, px, size,
};

struct Probe {
    motions: Motions<u8>,
    target: f32,
    present: bool,
    reverse_order: bool,
    omitted: bool,
    frames: Option<[Frame; 2]>,
}

impl Render for Probe {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.omitted {
            self.motions.frame([], window, cx);

            return div();
        }

        let order = if self.reverse_order { [1, 0] } else { [0, 1] };

        let declaration = Motion::new(Target::default().x(self.target).width(40. + self.target))
            .initial(Target::default().width(40.))
            .exit(Target::default().opacity(0.))
            .present(self.present);

        let frames = self
            .motions
            .frame(order.map(|key| (key, declaration)), window, cx);

        let mut root = div();

        self.frames = Some(if self.reverse_order {
            [frames[1], frames[0]]
        } else {
            frames
        });

        for (key, frame) in order.into_iter().zip(frames) {
            if !frame.is_mounted() {
                self.motions.remove(&key);
            }

            root = root.child(frame.render(|frame| {
                frame.apply(
                    div()
                        .id(("part", usize::from(key)))
                        .debug_selector(move || format!("part-{key}"))
                        .h(px(30.)),
                )
            }));
        }

        root
    }
}

fn open(cx: &mut TestAppContext) -> WindowHandle<Probe> {
    cx.update(|cx| cx.set_reduce_motion(false));
    let window = cx.open_window(size(px(400.), px(300.)), |_, _| Probe {
        motions: Motions::new(),
        target: 0.,
        present: true,
        reverse_order: false,
        omitted: false,
        frames: None,
    });

    cx.update_window(window.into(), |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    draw(cx, window);

    window
}

fn draw(cx: &mut TestAppContext, window: WindowHandle<Probe>) {
    cx.update_window(window.into(), |_, window, cx| {
        window.simulate_next_frame(cx);
        window.draw(cx).clear(cx);
    })
    .unwrap();
}

fn queued_frames(cx: &mut TestAppContext, window: WindowHandle<Probe>) -> usize {
    cx.update_window(window.into(), |_, window, cx| {
        window.simulate_next_frame(cx)
    })
    .unwrap()
}

pub fn batched_clock_interruption_and_scheduling(cx: &mut TestAppContext) {
    let window = open(cx);

    assert_eq!(queued_frames(cx, window), 0);
    window
        .update(cx, |probe, _, cx| {
            probe.target = 100.;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    assert_eq!(queued_frames(cx, window), 1);
    cx.executor().advance_clock(Duration::from_millis(80));
    draw(cx, window);
    let before = window
        .update(cx, |probe, _, _| probe.frames.unwrap())
        .unwrap();

    assert_eq!(before[0].target(), before[1].target());
    assert_eq!(before[0].velocity(), before[1].velocity());
    assert!(before[0].target().x_px() > 0.);
    let bounds = ui::VisualTestContext::from_window(window.into(), cx)
        .debug_bounds("part-0")
        .unwrap();

    // Native layout rounds to physical pixels; the sampler keeps fractional values.
    let pixel = cx
        .update_window(window.into(), |_, window, _| 1. / window.scale_factor())
        .unwrap();

    assert!((f32::from(bounds.origin.x) - before[0].target().x_px()).abs() <= pixel);
    assert!((f32::from(bounds.size.width) - before[0].target().width_px().unwrap()).abs() <= pixel);
    window
        .update(cx, |probe, _, cx| {
            probe.target = -20.;
            probe.reverse_order = true;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    let after = window
        .update(cx, |probe, _, _| probe.frames.unwrap())
        .unwrap();

    for (before, after) in before.into_iter().zip(after) {
        assert_eq!(before.target(), after.target());
        assert_eq!(before.velocity(), after.velocity());
        assert_eq!(before.layout_velocity(), after.layout_velocity());
    }

    cx.executor().advance_clock(Duration::from_secs(5));
    draw(cx, window);
    assert_eq!(queued_frames(cx, window), 0);
    window
        .update(cx, |probe, _, _| {
            assert_eq!(probe.frames.unwrap()[0].target().x_px(), -20.);
            assert_eq!(probe.motions.len(), 2);
        })
        .unwrap();
}

pub fn reduced_motion_exit_cleanup_and_omission(cx: &mut TestAppContext) {
    let window = open(cx);

    window
        .update(cx, |probe, _, cx| {
            probe.target = 100.;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    window
        .update(cx, |probe, _, cx| {
            probe.omitted = true;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    assert_eq!(queued_frames(cx, window), 0);
    window
        .update(cx, |probe, _, cx| {
            assert_eq!(probe.motions.len(), 2);
            probe.omitted = false;
            cx.set_reduce_motion(true);
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    window
        .update(cx, |probe, _, cx| {
            let frames = probe.frames.unwrap();

            assert!(frames.iter().all(|frame| frame.target().x_px() == 100.));
            assert!(frames.iter().all(|frame| !frame.is_animating()));
            probe.present = false;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    assert_eq!(queued_frames(cx, window), 0);
    window
        .update(cx, |probe, _, _| {
            assert!(probe.motions.is_empty());
            assert!(
                probe
                    .frames
                    .unwrap()
                    .iter()
                    .all(|frame| !frame.is_mounted())
            );
        })
        .unwrap();
    assert!(
        ui::VisualTestContext::from_window(window.into(), cx)
            .debug_bounds("part-0")
            .is_none()
    );
}

pub fn single_entry_and_explicit_samples_return_native_frames(cx: &mut TestAppContext) {
    struct Single(Motions<u8>);

    impl Render for Single {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            self.0
                .frame_one(0, Motion::new(Target::default().width(90.)), window, cx)
                .apply(
                    div()
                        .id("single")
                        .debug_selector(|| "single".into())
                        .h(px(20.)),
                )
        }
    }

    let mut motions = Motions::default();

    let [frame] = motions.sample(
        [(0, Motion::new(Target::default().width(70.)))],
        Duration::ZERO,
        false,
    );

    let mut element = frame.apply(div());

    assert_eq!(element.style().size.width, Some(px(70.).into()));
    let frame = motions.sample_one(
        1,
        Motion::new(Target::default().width(80.)),
        Duration::ZERO,
        false,
    );

    let mut element = frame.apply(div());

    assert_eq!(element.style().size.width, Some(px(80.).into()));
    motions.retain(|key| *key == 1);
    motions.clear();
    motions.shrink_to_fit();
    motions.reserve(4);
    assert!(motions.capacity() >= 4);
    let window = cx.open_window(size(px(400.), px(300.)), |_, _| Single(motions));

    cx.update_window(window.into(), |_, window, cx| {
        window.activate_window();
        window.draw(cx).clear(cx);
    })
    .unwrap();
    let bounds = ui::VisualTestContext::from_window(window.into(), cx)
        .debug_bounds("single")
        .unwrap();

    assert_eq!(bounds.size.width, px(90.));
}
