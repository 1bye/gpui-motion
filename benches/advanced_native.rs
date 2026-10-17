//! Full CE CPU preparation on TestPlatform. Excludes GPU/OS/display execution.
//! 100 elements, 5 batches x 240 measured frames after 24 warmup frames.
#[path = "../tests/common/activation.rs"]
mod activation;
use gpui::{AppContext, Context, Render, TestAppContext, Window, div, prelude::*, px, rgb, size};
use gpui_ce_motion::*;
use std::time::{Duration, Instant};
#[derive(Clone, Copy)]
enum Mode {
    Direct,
    Styles,
    Blur,
    Gestures,
    Size,
    Pop,
    Reorder,
    Shared,
    Vector,
}
fn shape(curved: bool) -> PathShape {
    PathShape::new([
        PathCommand::Move([0., 8.]),
        PathCommand::Cubic(
            [5., if curved { 16. } else { 8. }],
            [15., if curved { 0. } else { 8. }],
            [20., 8.],
        ),
    ])
    .unwrap()
}
struct Board {
    mode: Mode,
    expanded: bool,
    epoch: Option<Instant>,
    styles: Vec<StyleMotionState>,
    gestures: Vec<GestureState>,
    pop: PopLayout,
    reorder: ReorderGroup<usize>,
    shared: SharedLayout<usize>,
    vectors: Vec<VectorState>,
    paths: [PathShape; 2],
    motions: Vec<MotionState>,
}
impl Render for Board {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let epoch = *self
            .epoch
            .get_or_insert_with(|| cx.background_executor().now());
        let now = cx
            .background_executor()
            .now()
            .saturating_duration_since(epoch);
        let mut board = div()
            .id("board")
            .relative()
            .flex()
            .flex_wrap()
            .gap(px(4.))
            .w(px(480.));
        let order: Vec<_> = if matches!(self.mode, Mode::Reorder) {
            self.reorder.order().to_vec()
        } else {
            (0..100).collect()
        };
        for id in order {
            let card = div()
                .id(("card", id))
                .w(px(20.))
                .h(px(if self.expanded { 24. } else { 16. }))
                .flex_shrink_0()
                .bg(rgb(0x526a97));
            let element = match self.mode {
                Mode::Direct => card.into_any_element(),
                Mode::Styles | Mode::Blur => {
                    let mut target = StyleTarget::default()
                        .radii([if self.expanded { 4. } else { 1. }; 4])
                        .border_widths([1.; 4])
                        .color(
                            ColorProperty::Background,
                            Color::rgb(if self.expanded { 0x6688bb } else { 0x334466 }),
                        );
                    if matches!(self.mode, Mode::Blur) {
                        target =
                            target.value(StyleProperty::Blur, if self.expanded { 2. } else { 0. });
                    }
                    let frame = self.styles[id].sample(StyleMotion::new(target), now, false);
                    if frame.is_animating() {
                        w.request_animation_frame();
                    }
                    frame.apply(card).into_any_element()
                }
                Mode::Gestures => self.gestures[id]
                    .bind(
                        ("gesture", id),
                        card,
                        Motion::new(Target::default().opacity(if self.expanded {
                            1.
                        } else {
                            0.5
                        })),
                        GestureTargets::default().while_hover(Target::default().opacity(0.8)),
                    )
                    .into_any_element(),
                Mode::Size => AutoSize::new(("size", id), card)
                    .width(20.)
                    .into_any_element(),
                Mode::Pop => {
                    let present = id >= 25 || self.expanded;
                    let frame = self.motions[id].sample(
                        Motion::new(Target::default()).present(present),
                        now,
                        false,
                    );
                    if frame.is_animating() {
                        w.request_animation_frame();
                    }
                    self.pop
                        .item(("pop", id), present, frame.apply(card))
                        .into_any_element()
                }
                Mode::Reorder => self
                    .reorder
                    .item(("reorder", id), id, card)
                    .into_any_element(),
                Mode::Shared => self
                    .shared
                    .member(
                        id,
                        ("shared", id),
                        true,
                        card.left(px(if self.expanded { 12. } else { 0. })),
                    )
                    .into_any_element(),
                Mode::Vector => self.vectors[id]
                    .element(
                        ("vector", id),
                        self.paths[usize::from(self.expanded)].clone(),
                        [20., 16.],
                        Color::rgb(0x99bbff),
                    )
                    .unwrap()
                    .reveal(
                        StrokeReveal::new(if self.expanded { 0.5 } else { 1. }, 0., 1.).unwrap(),
                    )
                    .into_any_element(),
            };
            board = board.child(element);
        }
        div()
            .size_full()
            .child(self.pop.container("pop-container", board))
    }
}
fn run(mode: Mode, label: &str) {
    let mut batches = Vec::new();
    let mut frames = Vec::new();
    for _ in 0..5 {
        let mut cx = TestAppContext::single();
        let window = cx.open_window(size(px(800.), px(600.)), |_, _| Board {
            mode,
            expanded: false,
            epoch: None,
            styles: (0..100).map(|_| StyleMotionState::default()).collect(),
            gestures: (0..100).map(|_| GestureState::default()).collect(),
            pop: PopLayout::default(),
            reorder: ReorderGroup::new((0..100).collect(), Axis::X).unwrap(),
            shared: SharedLayout::default(),
            vectors: (0..100).map(|_| VectorState::new(shape(false))).collect(),
            paths: [shape(false), shape(true)],
            motions: (0..100).map(|_| MotionState::default()).collect(),
        });
        cx.update_window(window.into(), |_, w, _| activation::activate(w))
            .unwrap();
        cx.run_until_parked();
        let mut total = Duration::ZERO;
        for frame in 0..264usize {
            cx.executor().advance_clock(Duration::from_micros(8333));
            let start = Instant::now();
            window
                .update(&mut cx, |board, _, cx| {
                    let expanded = (frame / 24).is_multiple_of(2);
                    if expanded != board.expanded && matches!(mode, Mode::Reorder) {
                        board
                            .reorder
                            .set_order(if expanded {
                                (0..100).collect()
                            } else {
                                (0..100).rev().collect()
                            })
                            .unwrap();
                    }
                    board.expanded = expanded;
                    cx.notify();
                })
                .unwrap();
            cx.update_window(window.into(), |_, w, cx| {
                w.simulate_next_frame(cx);
                w.draw(cx).clear(cx);
            })
            .unwrap();
            if frame >= 24 {
                let elapsed = start.elapsed();
                total += elapsed;
                frames.push(elapsed.as_secs_f64() * 1000.);
            }
        }
        batches.push(total.as_secs_f64() * 1000. / 240.);
    }
    batches.sort_by(f64::total_cmp);
    frames.sort_by(f64::total_cmp);
    println!(
        "{label}: median batch mean {:.3} ms/frame; frame P95 {:.3} ms; 100 elements, 5 x 240 frames",
        batches[2],
        frames[(frames.len() as f64 * 0.95) as usize]
    );
}
fn main() {
    for (mode, label) in [
        (Mode::Direct, "direct reflow"),
        (Mode::Styles, "native style tracks"),
        (Mode::Blur, "blur CPU preparation only"),
        (Mode::Gestures, "native gesture bindings + opacity"),
        (Mode::Size, "intrinsic height measurement"),
        (Mode::Pop, "popped retained exits"),
        (Mode::Reorder, "measured reorder + handlers"),
        (Mode::Shared, "shared position bindings"),
        (Mode::Vector, "curve morph/reveal + tessellation"),
    ] {
        run(mode, label);
    }
}
