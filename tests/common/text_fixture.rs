use crate::{motion, ui};
use gpui_ce_motion::{Easing, Scramble, TextMatch, TextPreset, TextUnit, Transition};
use motion::{NumberEffect, NumberMotionState, TextMotionState};
use std::time::Duration;
use ui::{AppContext, Context, Render, TestAppContext, Window, div, prelude::*, px, size};

struct Fixture {
    text: String,
    state: TextMotionState,
    number: NumberMotionState,
    count: Option<i64>,
    effect: NumberEffect,
    preset: TextPreset,
    units: TextUnit,
    width: f32,
    font_size: f32,
    scramble: bool,
    progress: Option<f64>,
    centered: bool,
    line_height: Option<f32>,
    opacity_transition: Option<Transition>,
}

impl Render for Fixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let transition = Transition::Tween(Duration::from_millis(200), Easing::Linear);

        let text = if let Some(count) = self.count {
            self.number
                .element("count", count)
                .effect(self.effect)
                .min_digits(3)
                .transition(transition)
                .when_some(self.opacity_transition, |count, timing| {
                    count.opacity_transition(timing)
                })
                .text_size(px(self.font_size))
                .into_any_element()
        } else {
            self.state
                .element("text", self.text.clone())
                .units(self.units)
                .preset(self.preset)
                .transition(transition)
                .when_some(self.opacity_transition, |text, timing| {
                    text.opacity_transition(timing)
                })
                .matching(TextMatch::Content)
                .reflow(transition)
                .text_size(px(self.font_size))
                .when(self.scramble, |text| {
                    text.scramble(Scramble::new("AB👩🏽‍💻").seed(7))
                })
                .when_some(self.progress, |text, progress| text.progress(progress))
                .into_any_element()
        };

        div()
            .w(px(self.width))
            .when_some(self.line_height, |view, height| {
                view.line_height(px(height))
            })
            .when(self.centered, |view| view.flex().justify_center())
            .child(text)
    }
}

fn open(cx: &mut TestAppContext) -> ui::WindowHandle<Fixture> {
    let window = cx.open_window(size(px(600.), px(400.)), |_, _| Fixture {
        text: "aba".into(),
        state: TextMotionState::default(),
        number: NumberMotionState::default(),
        count: None,
        effect: NumberEffect::Roll,
        preset: TextPreset::Fade,
        units: TextUnit::Graphemes,
        width: 500.,
        font_size: 24.,
        scramble: false,
        progress: None,
        centered: false,
        line_height: None,
        opacity_transition: None,
    });

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

fn advance(cx: &mut TestAppContext, window: ui::WindowHandle<Fixture>, milliseconds: u64) {
    cx.executor()
        .advance_clock(Duration::from_millis(milliseconds));
    draw(cx, window);
}

pub fn clock_and_interruption(cx: &mut TestAppContext) {
    let window = open(cx);

    advance(cx, window, 250);
    let first = window
        .update(cx, |view, _, _| view.state.presentation())
        .unwrap();

    assert!(first.iter().all(|frame| frame.opacity == 1.));
    window
        .update(cx, |view, _, cx| {
            view.text = "baa".into();
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    advance(cx, window, 70);
    let before = window
        .update(cx, |view, _, _| view.state.presentation())
        .unwrap();

    window
        .update(cx, |view, _, cx| {
            view.text = "aba".into();
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    let reversed = window
        .update(cx, |view, _, _| view.state.presentation())
        .unwrap();

    for frame in before {
        let after = reversed
            .iter()
            .find(|after| after.id == frame.id)
            .expect("retained character");

        assert!((frame.origin[0] - after.origin[0]).abs() < 0.01);
    }

    advance(cx, window, 300);
    window
        .update(cx, |view, _, _| {
            assert!(!view.state.is_animating());
            assert!(view.state.error().is_none());
        })
        .unwrap();
}

pub fn unicode_wrap_and_mask(cx: &mut TestAppContext) {
    let window = open(cx);

    window
        .update(cx, |view, _, cx| {
            view.text = "e\u{301} 👩🏽‍💻 🇺🇦 مرحباً بالعالم\nwords wrap across the panel".into();
            view.width = 130.;
            view.units = TextUnit::Lines;
            view.preset = TextPreset::WipeLeft;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    advance(cx, window, 250);
    window
        .update(cx, |view, _, _| {
            let frames = view.state.presentation();

            assert!(frames.len() > 1, "text must wrap into visual lines");
            assert!(
                frames
                    .iter()
                    .all(|frame| frame.origin.iter().all(|value| value.is_finite()))
            );
            assert!(view.state.error().is_none());
        })
        .unwrap();
    window
        .update(cx, |view, _, cx| {
            view.font_size = 32.;
            view.width = 210.;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    advance(cx, window, 300);
    window
        .update(cx, |view, _, _| {
            assert!(!view.state.is_animating());
            assert!(view.state.error().is_none());
        })
        .unwrap();
}

pub fn native_controls(cx: &mut TestAppContext) {
    let window = open(cx);

    advance(cx, window, 80);
    window
        .update(cx, |view, window, cx| view.state.pause(window, cx))
        .unwrap();
    let held = window
        .update(cx, |view, _, _| view.state.presentation())
        .unwrap();

    advance(cx, window, 1000);
    window
        .update(cx, |view, window, cx| {
            assert_eq!(view.state.presentation()[0].opacity, held[0].opacity);
            assert!(view.state.seek(0.75, window, cx));
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    window
        .update(cx, |view, window, cx| {
            assert!((view.state.presentation()[0].opacity - 0.75).abs() < 0.01);
            view.state.play(window, cx);
            cx.notify();
        })
        .unwrap();
    advance(cx, window, 300);
    window
        .update(cx, |view, window, cx| {
            assert!(!view.state.is_animating());
            view.state.restart(window, cx);
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    window
        .update(cx, |view, _, _| {
            assert!(
                view.state
                    .presentation()
                    .iter()
                    .any(|frame| frame.opacity < 1.)
            )
        })
        .unwrap();
}

pub fn reduced_and_scrubbed(cx: &mut TestAppContext) {
    let window = open(cx);

    window
        .update(cx, |view, _, cx| {
            view.progress = Some(0.5);
            view.scramble = true;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    window
        .update(cx, |view, _, _| {
            assert!(
                view.state
                    .presentation()
                    .iter()
                    .any(|frame| frame.substitution.is_some())
            );
            assert!(!view.state.is_animating());
        })
        .unwrap();
    cx.update(|cx| cx.set_reduce_motion(true));
    draw(cx, window);
    window
        .update(cx, |view, _, _| {
            assert!(
                view.state
                    .presentation()
                    .iter()
                    .all(|frame| frame.opacity == 1. && frame.substitution.is_none())
            );
            assert!(!view.state.is_animating());
            assert!(view.state.error().is_none());
        })
        .unwrap();
}

pub fn digit_place_value(cx: &mut TestAppContext) {
    let window = open(cx);

    window
        .update(cx, |view, _, cx| {
            view.count = Some(199);
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    advance(cx, window, 250);
    let before = window
        .update(cx, |view, _, _| view.number.text_state().presentation())
        .unwrap();

    window
        .update(cx, |view, _, cx| {
            view.count = Some(299);
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    let during = window
        .update(cx, |view, _, _| view.number.text_state().presentation())
        .unwrap();

    let kept = before
        .iter()
        .filter(|old| during.iter().any(|new| new.id == old.id && !new.exiting))
        .count();

    assert_eq!(kept, 2, "both unchanged nines keep their decimal places");
    advance(cx, window, 300);
    window
        .update(cx, |view, _, _| {
            assert!(!view.number.text_state().is_animating());
            assert_eq!(view.number.text_state().presentation().len(), 3);
            assert!(view.number.text_state().error().is_none());
        })
        .unwrap();

    for effect in [
        NumberEffect::Crossfade,
        NumberEffect::Slide,
        NumberEffect::Interpolate,
    ] {
        window
            .update(cx, |view, _, cx| {
                view.effect = effect;
                view.count = Some(-1024);
                cx.notify();
            })
            .unwrap();
        draw(cx, window);
        advance(cx, window, 300);
        window
            .update(cx, |view, _, _| {
                assert!(view.number.text_state().error().is_none())
            })
            .unwrap();
    }
}

pub fn centered_text_does_not_jump_when_its_width_changes(cx: &mut TestAppContext) {
    let window = open(cx);

    window
        .update(cx, |view, _, cx| {
            view.centered = true;
            view.text = "a".into();
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    advance(cx, window, 300);
    let before = window
        .update(cx, |view, _, _| view.state.presentation()[0])
        .unwrap();

    assert!(before.origin[0] > 100.);
    window
        .update(cx, |view, _, cx| {
            view.text = "a longer title".into();
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    window
        .update(cx, |view, _, _| {
            let after = view
                .state
                .presentation()
                .into_iter()
                .find(|frame| frame.id == before.id)
                .unwrap();

            assert!(
                (after.origin[0] - before.origin[0]).abs() < 0.01,
                "centered text must preserve its visible position before reflow"
            );
        })
        .unwrap();
    advance(cx, window, 300);
    window
        .update(cx, |view, _, _| {
            assert!(
                view.state
                    .presentation()
                    .iter()
                    .find(|frame| frame.id == before.id)
                    .unwrap()
                    .origin[0]
                    < before.origin[0]
            )
        })
        .unwrap();
}

pub fn semantic_text_has_one_accessible_label() {
    use ui::Element as _;

    let state = TextMotionState::default();
    let element = state.element("label", "Café 👩🏽‍💻").into_element();

    assert_eq!(element.a11y_role(), Some(ui::Role::Label));

    let mut node = ui::accesskit::Node::new(ui::Role::Label);

    element.write_a11y_info(&mut node);

    assert_eq!(node.value(), Some("Café 👩🏽‍💻"));
    assert_eq!(node.label(), None);

    let element = state
        .element("label", "Latest requested text")
        .into_element();

    let mut node = ui::accesskit::Node::new(ui::Role::Label);

    element.write_a11y_info(&mut node);

    assert_eq!(node.value(), Some("Latest requested text"));

    let count = NumberMotionState::default();
    let element = count.element("count", 129).min_digits(6).into_element();
    let mut node = ui::accesskit::Node::new(ui::Role::Label);

    element.write_a11y_info(&mut node);

    assert_eq!(element.a11y_role(), Some(ui::Role::Label));
    assert_eq!(node.value(), Some("129"));
}

pub fn rolling_and_sliding_digits_clear_tall_line_boxes(cx: &mut TestAppContext) {
    for effect in [NumberEffect::Roll, NumberEffect::Slide] {
        let window = open(cx);

        window
            .update(cx, |view, _, cx| {
                view.count = Some(199);
                view.effect = effect;
                view.line_height = Some(72.);
                cx.notify();
            })
            .unwrap();
        draw(cx, window);

        let frames = window
            .update(cx, |view, _, _| view.number.text_state().presentation())
            .unwrap();

        assert_eq!(frames.len(), 3);

        for frame in frames {
            assert!((frame.size[1] - 72.).abs() < 0.01);
            assert!(
                (frame.origin[1] - frame.clip_origin[1] - frame.size[1]).abs() < 0.01,
                "{effect:?} must clear the whole measured slot: {frame:?}"
            );
        }

        advance(cx, window, 250);

        for _ in 0..4 {
            window
                .update(cx, |view, _, cx| {
                    view.count = Some(view.count.unwrap() + 101);
                    cx.notify();
                })
                .unwrap();
            draw(cx, window);
            advance(cx, window, 30);
        }

        advance(cx, window, 100);

        window
            .update(cx, |view, _, _| {
                let frames = view.number.text_state().presentation();
                let retiring: Vec<_> = frames.iter().filter(|frame| frame.exiting).collect();

                assert!(!retiring.is_empty());
                assert!(
                    retiring.iter().all(|frame| frame.opacity == 0.),
                    "{effect:?} left opaque retired digits: {frames:?}"
                );
                assert!(view.number.text_state().error().is_none());
            })
            .unwrap();

        advance(cx, window, 300);

        window
            .update(cx, |view, _, _| {
                assert!(!view.number.text_state().is_animating());
                assert_eq!(view.number.text_state().presentation().len(), 3);
            })
            .unwrap();

        window
            .update(cx, |view, _, cx| {
                view.opacity_transition =
                    Some(Transition::Tween(Duration::from_millis(40), Easing::Linear));
                view.count = Some(111);
                cx.notify();
            })
            .unwrap();
        draw(cx, window);
        advance(cx, window, 40);

        window
            .update(cx, |view, _, _| {
                let frames = view.number.text_state().presentation();
                let retiring: Vec<_> = frames.iter().filter(|frame| frame.exiting).collect();

                assert!(!retiring.is_empty());
                assert!(
                    retiring.iter().all(|frame| frame.opacity == 0.),
                    "explicit opacity timing must override the preset: {frames:?}"
                );
            })
            .unwrap();
    }
}
