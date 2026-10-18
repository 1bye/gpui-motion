//! Interactive text gallery. Run with `--features demo --example text_motion`.
use gpui_ce_motion::{
    Animation, Easing, Keyframe, PresenceMode, Repeat, RepeatMode, Spring, StaggerFrom, Target,
    Transition,
};

use gpui_kit::TestSupportExt as _;
use gpui_kit::base::StyledExt as _;

use gpui_kit::{
    AnyElement, AppContext, Bounds, Context, Entity, IntoElement, KeyBinding, Render, SharedString,
    Subscription, Window, WindowBounds, WindowOptions, actions,
    component::{
        ActiveTheme, Disableable,
        button::{Button, ButtonVariants},
        checkbox::Checkbox,
        input::{Input, InputEvent, InputState},
        slider::{Slider, SliderEvent, SliderState},
        tab::{Tab, TabBar},
    },
    div, point,
    prelude::*,
    px, rems, size,
};

use gpui_motion_kit::{
    NumberEffect, NumberMotionState, Scramble, TextMatch, TextMotionState, TextPreset, TextTarget,
    TextUnit,
};

use std::time::Duration;

actions!(
    text_gallery,
    [
        /// Quit the text gallery.
        Quit
    ]
);

const MODES: [&str; 12] = [
    "Fade",
    "Words",
    "Letters",
    "Match",
    "Lines",
    "Wipe",
    "Type",
    "Scramble",
    "Alternate",
    "Wave",
    "Scrub",
    "Numbers",
];

const SAMPLES: [&str; 5] = [
    "Made to move.",
    "Words find their place.",
    "A little more room to breathe.",
    "Café 👩🏽‍💻 — مرحباً بالعالم",
    "Ready when you are.",
];

struct Gallery {
    motion: TextMotionState,
    number: NumberMotionState,
    input: Entity<InputState>,
    slider: Entity<SliderState>,
    subscriptions: Vec<Subscription>,
    text: SharedString,
    mode: usize,
    sample: usize,
    progress: f64,
    count: i64,
    number_effect: usize,
    wave: Animation,
    scramble: Scramble,
}

impl Gallery {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).default_value(SAMPLES[0]));

        let slider = cx.new(|_| {
            SliderState::new()
                .min(0.)
                .max(1.)
                .step(0.01)
                .default_value(0.5)
        });

        let subscriptions = vec![
            cx.subscribe(&input, |this, input, event, cx| {
                if matches!(event, InputEvent::Change) {
                    this.text = input.read(cx).value();
                    cx.notify();
                }
            }),
            cx.subscribe(&slider, |this, _, event: &SliderEvent, cx| {
                if let SliderEvent::Change(value) = event {
                    this.progress = value.start().into();
                    cx.notify();
                }
            }),
        ];

        let wave = Animation::keyframes(
            Duration::from_millis(750),
            [
                Keyframe::new(0., Target::default()),
                Keyframe::new(0.5, Target::default().y(-8.)),
                Keyframe::new(1., Target::default()),
            ],
        )
        .expect("valid wave frames")
        .repeat(Repeat::Forever, RepeatMode::Loop, Duration::ZERO)
        .expect("positive period");

        Self {
            motion: TextMotionState::default(),
            number: NumberMotionState::default(),
            input,
            slider,
            subscriptions,
            text: SAMPLES[0].into(),
            mode: 0,
            sample: 0,
            progress: 0.5,
            count: 199,
            number_effect: 0,
            wave,
            scramble: Scramble::new("ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789").seed(42),
        }
    }

    fn spring() -> Transition {
        Transition::Spring(Spring::visual(Duration::from_millis(260), 0.08).expect("valid spring"))
    }

    fn animated(&self) -> AnyElement {
        let text = self
            .motion
            .element("animated-text", self.text.clone())
            // Avoid stock macOS system-font glyph clipping at display sizes.
            .when(cfg!(target_os = "macos"), |text| {
                text.font_family("Helvetica Neue")
            })
            .text_size(rems(3.))
            .transition(Self::spring());

        match self.mode {
            0 => text.preset(TextPreset::Fade).into_any_element(),
            1 => text
                .units(TextUnit::Words)
                .preset(TextPreset::FadeUp)
                .stagger(Duration::from_millis(35), StaggerFrom::Start)
                .into_any_element(),
            2 => text
                .units(TextUnit::Graphemes)
                .preset(TextPreset::FadeUp)
                .stagger(Duration::from_millis(14), StaggerFrom::Center)
                .into_any_element(),
            3 => text
                .units(TextUnit::Graphemes)
                .matching(TextMatch::Content)
                .reflow(Self::spring())
                .preset(TextPreset::FadeUp)
                .into_any_element(),
            4 => text
                .w(rems(24.))
                .units(TextUnit::Lines)
                .preset(TextPreset::SlideUp)
                .stagger(Duration::from_millis(65), StaggerFrom::Start)
                .into_any_element(),
            5 => text
                .units(TextUnit::Words)
                .preset(TextPreset::WipeRight)
                .transition(Transition::Tween(
                    Duration::from_millis(330),
                    Easing::EaseOut,
                ))
                .stagger(Duration::from_millis(40), StaggerFrom::Start)
                .into_any_element(),
            6 => text
                .units(TextUnit::Graphemes)
                .preset(TextPreset::Typewriter)
                .stagger(Duration::from_millis(30), StaggerFrom::Start)
                .into_any_element(),
            7 => text
                .units(TextUnit::Graphemes)
                .scramble(self.scramble.clone())
                .stagger(Duration::from_millis(10), StaggerFrom::Start)
                .into_any_element(),
            8 => text
                .units(TextUnit::Words)
                .enter_with(|unit| {
                    let direction = if unit.index % 2 == 0 { -1. } else { 1. };

                    TextTarget::default().y_em(direction * 0.7).opacity(0.)
                })
                .exit(TextTarget::default().opacity(0.))
                .stagger(Duration::from_millis(35), StaggerFrom::Center)
                .presence_mode(PresenceMode::Sync)
                .into_any_element(),
            9 => text
                .units(TextUnit::Graphemes)
                .animation(&self.wave)
                .stagger(Duration::from_millis(35), StaggerFrom::Start)
                .into_any_element(),
            10 => text
                .units(TextUnit::Words)
                .preset(TextPreset::FadeUp)
                .transition(Transition::Tween(
                    Duration::from_millis(300),
                    Easing::EaseOut,
                ))
                .stagger(Duration::from_millis(70), StaggerFrom::Start)
                .progress(self.progress)
                .into_any_element(),
            _ => self
                .number
                .element("animated-count", self.count)
                .min_digits(3)
                .effect(
                    [
                        NumberEffect::Roll,
                        NumberEffect::Crossfade,
                        NumberEffect::Slide,
                        NumberEffect::Interpolate,
                    ][self.number_effect],
                )
                .transition(Self::spring())
                // Avoid stock macOS system-font glyph clipping at display sizes.
                .when(cfg!(target_os = "macos"), |number| {
                    number.font_family("Helvetica Neue")
                })
                .text_size(rems(5.))
                .into_any_element(),
        }
    }
}

impl Render for Gallery {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _keep_subscriptions = &self.subscriptions;

        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_6()
            .p_8()
            .bg(cx.theme().muted)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_2xl().font_semibold().child("Words, in motion."))
                    .child(
                        Checkbox::new("reduced-motion")
                            .label("Reduced motion")
                            .checked(cx.reduce_motion())
                            .on_change(cx.listener(|_, checked, _, cx| {
                                cx.set_reduce_motion(*checked);
                                cx.notify();
                            })),
                    ),
            )
            .child(
                TabBar::new("text-modes")
                    .pill()
                    .selected_index(self.mode)
                    .children(MODES.map(|mode| Tab::new().label(mode)))
                    .on_click(cx.listener(|this, index: &usize, window, cx| {
                        this.mode = *index;
                        this.motion.restart(window, cx);
                        cx.notify();
                    })),
            )
            .child(Input::new(&self.input).id("text-input"))
            .child(
                div()
                    .id("text-stage")
                    .test_support()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(cx.theme().background)
                    .rounded_lg()
                    .p_8()
                    .overflow_hidden()
                    .child(self.animated()),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        Button::new("next-text")
                            .label("Change text")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.sample = (this.sample + 1) % SAMPLES.len();
                                this.input.update(cx, |input, cx| {
                                    input.set_value(SAMPLES[this.sample], window, cx)
                                });
                                this.text = SAMPLES[this.sample].into();
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("replay-text")
                            .disabled(
                                self.mode == 10 || (self.mode == 11 && self.number_effect == 3),
                            )
                            .ghost()
                            .label("Replay")
                            .on_click(cx.listener(|this, _, window, cx| {
                                if this.mode == 11 {
                                    this.number.text_state().restart(window, cx);
                                } else {
                                    this.motion.restart(window, cx);
                                }

                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("pause-text")
                            .disabled(self.mode == 10 || self.mode == 11)
                            .ghost()
                            .label("Pause")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.motion.pause(window, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("resume-text")
                            .disabled(self.mode == 10 || self.mode == 11)
                            .ghost()
                            .label("Resume")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.motion.play(window, cx);
                                cx.notify();
                            })),
                    )
                    .when(self.mode == 11, |row| {
                        row.child(Button::new("decrease-count").label("− 101").on_click(
                            cx.listener(|this, _, _, cx| {
                                this.count = this.count.saturating_sub(101);
                                cx.notify();
                            }),
                        ))
                        .child(
                            Button::new("increase-count")
                                .label("+ 101")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.count = this.count.saturating_add(101);
                                    cx.notify();
                                })),
                        )
                        .child(
                            TabBar::new("number-effects")
                                .pill()
                                .selected_index(self.number_effect)
                                .children(
                                    ["Roll", "Fade", "Slide", "Count"]
                                        .map(|label| Tab::new().label(label)),
                                )
                                .on_click(cx.listener(|this, index: &usize, _, cx| {
                                    this.number_effect = *index;
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .when(self.mode == 10, |view| {
                view.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_4()
                        .child(div().text_sm().child("Reveal progress"))
                        .child(
                            div()
                                .id("scrub-control")
                                .test_support()
                                .flex_1()
                                .child(Slider::new(&self.slider)),
                        )
                        .child(format!("{:03}%", (self.progress * 100.).round() as u32)),
                )
            })
    }
}

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            cx.bind_keys([
                KeyBinding::new("cmd-q", Quit, None),
                KeyBinding::new("ctrl-q", Quit, None),
            ]);
            cx.on_action(|_: &Quit, cx| cx.quit());
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(80.), px(80.)),
                        size(px(1180.), px(780.)),
                    ))),
                    window_min_size: Some(size(px(1000.), px(640.))),
                    titlebar: Some(gpui_kit::TitlebarOptions {
                        title: Some("Text / Motion".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| Gallery::new(window, cx)),
            )
            .expect("open text gallery");
            cx.activate(true);
        });
}

#[cfg(all(test, feature = "native-tests"))]
mod tests {
    //! Production gallery and Kit input dispatch; TestPlatform supplies fonts/display/GPU.
    use super::*;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{TestAppContext, WindowHandle, base::Root};

    fn setup(cx: &mut TestAppContext, mode: usize) -> (WindowHandle<Root>, Entity<Gallery>) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_reduce_motion(false);
            let (window, view) = gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(0.), px(0.)),
                        size(px(1180.), px(780.)),
                    ))),
                    ..Default::default()
                },
                cx,
                |window, cx| {
                    cx.new(|cx| {
                        let mut view = Gallery::new(window, cx);

                        view.mode = mode;

                        view
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

    fn advance(cx: &mut TestAppContext, window: WindowHandle<Root>, milliseconds: u64) {
        cx.executor()
            .advance_clock(Duration::from_millis(milliseconds));
        cx.run_until_parked();
        cx.update_window(window.into(), |_, window, cx| window.render_frame(cx))
            .unwrap();
    }

    #[gpui_kit::test]
    fn editing_input_retargets_real_text(cx: &mut TestAppContext) {
        let (window, view) = setup(cx, 3);

        cx.update_window(window.into(), |_, window, cx| {
            window.click("text-input", cx);
            window.press("cmd-a", cx);
            window.input("Café 👩🏽‍💻 مرحباً", cx);
        })
        .unwrap();
        cx.run_until_parked();
        advance(cx, window, 1500);
        cx.update(|cx| {
            assert_eq!(view.read(cx).text.as_ref(), "Café 👩🏽‍💻 مرحباً");
            assert!(view.read(cx).motion.error().is_none());
            assert!(!view.read(cx).motion.is_animating());
        });
    }

    #[gpui_kit::test]
    fn pause_resume_and_replay_buttons_control_the_presentation(cx: &mut TestAppContext) {
        let (window, view) = setup(cx, 0);

        advance(cx, window, 80);
        cx.update_window(window.into(), |_, window, cx| {
            window.click("pause-text", cx)
        })
        .unwrap();
        let held = cx.update(|cx| view.read(cx).motion.presentation()[0].opacity);

        advance(cx, window, 1000);
        cx.update(|cx| assert_eq!(view.read(cx).motion.presentation()[0].opacity, held));
        cx.update_window(window.into(), |_, window, cx| {
            window.click("resume-text", cx)
        })
        .unwrap();
        advance(cx, window, 1000);
        cx.update(|cx| assert!(!view.read(cx).motion.is_animating()));
        cx.update_window(window.into(), |_, window, cx| {
            window.click("replay-text", cx)
        })
        .unwrap();
        cx.update(|cx| assert!(view.read(cx).motion.presentation()[0].opacity < 1.));
    }

    #[gpui_kit::test]
    fn reduced_motion_checkbox_finishes_the_repeating_wave(cx: &mut TestAppContext) {
        let (window, view) = setup(cx, 9);

        advance(cx, window, 80);
        cx.update(|cx| assert!(view.read(cx).motion.is_animating()));
        cx.update_window(window.into(), |_, window, cx| {
            window.click("reduced-motion", cx)
        })
        .unwrap();
        advance(cx, window, 100);
        cx.update(|cx| {
            assert!(cx.reduce_motion());
            assert!(!view.read(cx).motion.is_animating());
            assert!(view.read(cx).motion.error().is_none());
        });
    }

    #[gpui_kit::test]
    fn reveal_slider_updates_progress_through_pointer_input(cx: &mut TestAppContext) {
        let (window, view) = setup(cx, 10);

        cx.update_window(window.into(), |_, window, cx| {
            let bounds = window.find("scrub-control").bounds();
            let from = bounds.origin + point(bounds.size.width * 0.5, bounds.size.height / 2.);
            let to = bounds.origin + point(bounds.size.width * 0.85, bounds.size.height / 2.);

            window.drag(from, to, cx);
        })
        .unwrap();
        cx.run_until_parked();
        advance(cx, window, 100);
        cx.update(|cx| {
            assert!(view.read(cx).progress > 0.7);
            assert!(!view.read(cx).motion.is_animating());
        });
    }

    #[gpui_kit::test]
    fn count_buttons_change_the_rolling_value(cx: &mut TestAppContext) {
        let (window, view) = setup(cx, 11);

        cx.update_window(window.into(), |_, window, cx| {
            window.click("increase-count", cx)
        })
        .unwrap();
        advance(cx, window, 1000);
        cx.update(|cx| {
            assert_eq!(view.read(cx).count, 300);
            assert!(view.read(cx).number.text_state().error().is_none());
        });
        cx.update_window(window.into(), |_, window, cx| {
            window.click("decrease-count", cx)
        })
        .unwrap();
        advance(cx, window, 1000);
        cx.update(|cx| assert_eq!(view.read(cx).count, 199));
    }

    #[gpui_kit::test]
    fn rapidly_replaced_lines_clear_their_masks_and_fade_out(cx: &mut TestAppContext) {
        let (window, view) = setup(cx, 4);

        cx.update(|cx| {
            let frames = view.read(cx).motion.presentation();

            assert!(!frames.is_empty());
            assert!(frames.iter().all(|frame| {
                (frame.origin[1] - frame.clip_origin[1] - frame.size[1]).abs() < 0.01
            }));
        });
        advance(cx, window, 1500);

        for _ in 0..4 {
            cx.update_window(window.into(), |_, window, cx| window.click("next-text", cx))
                .unwrap();
            advance(cx, window, 30);
        }

        advance(cx, window, 120);
        cx.update(|cx| {
            let frames = view.read(cx).motion.presentation();
            let outgoing: Vec<_> = frames.iter().filter(|frame| frame.exiting).collect();

            assert!(!outgoing.is_empty());
            assert!(
                outgoing.iter().all(|frame| frame.opacity == 0.),
                "old lines must not linger as opaque fragments: {frames:?}"
            );
            assert!(view.read(cx).motion.error().is_none());
        });
        advance(cx, window, 2000);
        cx.update(|cx| assert!(!view.read(cx).motion.is_animating()));
    }
}
