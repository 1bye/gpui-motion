use crate::{motion, ui};
use gpui_ce_motion::{Scramble, StaggerFrom, TextMatch, TextPreset, TextUnit, Transition};
use motion::TextMotionState;

use std::{
    hint::black_box,
    time::{Duration, Instant},
};

use ui::{AppContext, Context, Render, TestAppContext, Window, div, prelude::*, px, size};

#[derive(Clone, Copy)]
enum Mode {
    Direct,
    Settled,
    Words,
    Graphemes,
    Scramble,
}

struct Board {
    states: Vec<TextMotionState>,
    mode: Mode,
    changed: bool,
    scramble: Scramble,
}

impl Render for Board {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut board = div().flex().flex_col().w(px(500.)).text_size(px(18.));

        for (index, state) in self.states.iter().enumerate() {
            let text = if self.changed {
                "Motion finds a new place"
            } else {
                "Words find their place"
            };

            let row = if matches!(self.mode, Mode::Direct) {
                div().child(text).into_any_element()
            } else {
                state
                    .element(("label", index), text)
                    .animate_initial(false)
                    .units(if matches!(self.mode, Mode::Words) {
                        TextUnit::Words
                    } else {
                        TextUnit::Graphemes
                    })
                    .matching(TextMatch::Content)
                    .reflow(Transition::default())
                    .preset(TextPreset::FadeUp)
                    .stagger(Duration::from_millis(4), StaggerFrom::Start)
                    .when(matches!(self.mode, Mode::Scramble), |text| {
                        text.scramble(self.scramble.clone())
                    })
                    .into_any_element()
            };

            board = board.child(row);
        }

        board
    }
}

fn run(mode: Mode, label: &str) {
    let mut batches = Vec::new();
    let mut times = Vec::new();

    for _ in 0..7 {
        let mut cx = TestAppContext::single();

        let window = cx.open_window(size(px(600.), px(1000.)), |_, _| Board {
            states: (0..40).map(|_| TextMotionState::default()).collect(),
            mode,
            changed: false,
            scramble: Scramble::new("ABC123").seed(42),
        });

        let mut total = Duration::ZERO;

        for frame in 0..264_u64 {
            cx.executor().advance_clock(Duration::from_micros(8333));
            let started = Instant::now();

            window
                .update(&mut cx, |view, _, cx| {
                    view.changed = !matches!(mode, Mode::Settled) && (frame / 24).is_multiple_of(2);
                    cx.notify();
                })
                .unwrap();
            cx.update_window(window.into(), |_, window, cx| {
                window.simulate_next_frame(cx);
                window.draw(cx).clear(cx);
            })
            .unwrap();
            let elapsed = started.elapsed();

            if frame >= 24 {
                total += elapsed;
                times.push(elapsed.as_secs_f64() * 1000.);
            }

            black_box(&window);
        }

        batches.push(total.as_secs_f64() * 1000. / 240.);
    }

    batches.sort_by(f64::total_cmp);
    times.sort_by(f64::total_cmp);
    println!(
        "{label}: median batch mean {:.3} ms/frame, aggregate P95 {:.3}; 40 labels, 7 × 240 measured frames",
        batches[3],
        times[times.len() * 95 / 100]
    );
}

pub fn benchmark() {
    run(Mode::Direct, "ordinary text with replacement");
    run(Mode::Settled, "settled animated-text adapter");
    run(Mode::Words, "interrupted word motion");
    run(Mode::Graphemes, "interrupted grapheme motion");
    run(Mode::Scramble, "grapheme motion + scramble");
}
