//! Real Kit CPU layout/prepaint/paint benchmark on TestPlatform; no display/GPU needed.
//! Run `cargo bench --features native-tests --bench kit_layout`.
use gpui_kit::{
    AppContext, Context, Render, TestAppContext, Window, div, prelude::*, px, rgb, size,
};

use gpui_motion_kit::LayoutGroup;

use std::{
    hint::black_box,
    time::{Duration, Instant},
};

#[derive(Clone, Copy)]
enum Mode {
    Direct,
    Disabled,
    Animated,
}

struct Board {
    group: LayoutGroup,
    compact: bool,
    reverse: bool,
    mode: Mode,
}

impl Render for Board {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut board = div()
            .w(px(if self.compact { 480. } else { 600. }))
            .flex()
            .flex_wrap()
            .gap(px(4.));

        for index in 0..400usize {
            let id = if self.reverse { 399 - index } else { index };

            let card = div()
                .id(("card", id))
                .w(px(20.))
                .h(px(16.))
                .flex_shrink_0()
                .bg(rgb(0x526a97));

            board = board.child(match self.mode {
                Mode::Direct => card.into_any_element(),
                Mode::Disabled | Mode::Animated => self
                    .group
                    .position(("position", id), card)
                    .enabled(matches!(self.mode, Mode::Animated))
                    .into_any_element(),
            });
        }

        div().size_full().child(board)
    }
}

fn run(mode: Mode, label: &str) {
    let mut batches = Vec::with_capacity(7);
    let mut frames = Vec::with_capacity(7 * 240);

    for _ in 0..7 {
        let mut cx = TestAppContext::single();

        let window = cx.open_window(size(px(800.), px(600.)), |_, _| Board {
            group: LayoutGroup::default(),
            compact: false,
            reverse: false,
            mode,
        });

        let mut total = Duration::ZERO;

        for frame in 0..264usize {
            cx.executor().advance_clock(Duration::from_micros(8333));
            let start = Instant::now();

            window
                .update(&mut cx, |view, _, cx| {
                    view.compact = (frame / 24).is_multiple_of(2);
                    view.reverse = (frame / 48).is_multiple_of(2);
                    cx.notify();
                })
                .unwrap();
            cx.update_window(window.into(), |_, window, cx| {
                window.simulate_next_frame(cx);
                window.draw(cx).clear(cx);
            })
            .unwrap();
            let duration = start.elapsed();

            if frame >= 24 {
                total += duration;
                frames.push(duration.as_secs_f64() * 1000.);
            }

            black_box(&window);
        }

        batches.push(total.as_secs_f64() * 1000. / 240.);
    }

    batches.sort_by(f64::total_cmp);
    frames.sort_by(f64::total_cmp);
    println!(
        "{label}: median batch mean {:.3} ms/frame, all-frame P95 {:.3} ms; 400 elements, 7 x 240 frames",
        batches[3],
        frames[(frames.len() as f64 * 0.95) as usize]
    );
}

fn main() {
    run(Mode::Direct, "direct layout");
    run(Mode::Disabled, "position tracking, interpolation disabled");
    run(
        Mode::Animated,
        "automatic measured positions + interruption",
    );
}
