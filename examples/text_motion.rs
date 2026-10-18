//! Stock-CE text playground. E: effect, Space: text, R: replay, P: pause/resume,
//! M: reduced motion, Up/Down: count. Run with `--features demo --example text_motion`.
use gpui::{
    App, Bounds, Context, FocusHandle, KeyBinding, MouseButton, Window, WindowBounds,
    WindowOptions, actions, div, prelude::*, px, rgb, size,
};
use gpui_ce_motion::{
    Animation, Easing, Keyframe, NumberEffect, NumberMotionState, Repeat, RepeatMode, Scramble,
    StaggerFrom, Target, TextMatch, TextMotionState, TextPreset, TextTarget, TextUnit, Transition,
};
use std::time::Duration;

actions!(
    ce_text,
    [
        /// Select the next effect.
        NextEffect,
        /// Change the sample text.
        ChangeText,
        /// Replay the current effect.
        Replay,
        /// Pause or resume text motion.
        TogglePause,
        /// Toggle reduced motion.
        ToggleReduced,
        /// Increase the sample count.
        Increase,
        /// Decrease the sample count.
        Decrease,
        /// Quit the playground.
        Quit
    ]
);
const MODES: [&str; 12] = [
    "Fade",
    "Words",
    "Graphemes",
    "Match",
    "Lines",
    "Wipe",
    "Typewriter",
    "Scramble",
    "Alternate",
    "Wave",
    "Scrub 50%",
    "Numbers",
];
const TEXT: [&str; 4] = [
    "Made to move.",
    "Words find their place.",
    "A little more room to breathe.",
    "Café 👩🏽‍💻 — مرحباً بالعالم",
];
struct Palette;
impl Palette {
    fn background() -> gpui::Rgba {
        rgb(0x18181b)
    }
    fn foreground() -> gpui::Rgba {
        rgb(0xf4f4f5)
    }
    fn surface() -> gpui::Rgba {
        rgb(0x27272a)
    }
}
struct Demo {
    motion: TextMotionState,
    number: NumberMotionState,
    sample: usize,
    mode: usize,
    paused: bool,
    count: i64,
    focus: FocusHandle,
    wave: Animation,
    scramble: Scramble,
}
impl Demo {
    fn switch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.mode = (self.mode + 1) % MODES.len();
        self.motion.restart(window, cx);
        cx.notify();
    }
    fn replay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.motion.restart(window, cx);
        self.number.text_state().restart(window, cx);
        cx.notify();
    }
}
impl Render for Demo {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let text = self
            .motion
            .element("ce-text", TEXT[self.sample])
            // Avoid stock macOS system-font glyph clipping at display sizes.
            .when(cfg!(target_os = "macos"), |text| {
                text.font_family("Helvetica Neue")
            })
            .text_size(px(48.));

        let content = match self.mode {
            0 => text.preset(TextPreset::Fade).into_any_element(),
            1 => text
                .units(TextUnit::Words)
                .preset(TextPreset::FadeUp)
                .stagger(Duration::from_millis(40), StaggerFrom::Start)
                .into_any_element(),
            2 => text
                .units(TextUnit::Graphemes)
                .preset(TextPreset::FadeUp)
                .stagger(Duration::from_millis(15), StaggerFrom::Center)
                .into_any_element(),
            3 => text
                .units(TextUnit::Graphemes)
                .matching(TextMatch::Content)
                .reflow(Transition::default())
                .into_any_element(),
            4 => text
                .w(px(380.))
                .units(TextUnit::Lines)
                .preset(TextPreset::SlideUp)
                .into_any_element(),
            5 => text
                .units(TextUnit::Words)
                .preset(TextPreset::WipeLeft)
                .transition(Transition::Tween(
                    Duration::from_millis(300),
                    Easing::EaseOut,
                ))
                .into_any_element(),
            6 => text
                .units(TextUnit::Graphemes)
                .preset(TextPreset::Typewriter)
                .stagger(Duration::from_millis(30), StaggerFrom::Start)
                .into_any_element(),
            7 => text
                .units(TextUnit::Graphemes)
                .scramble(self.scramble.clone())
                .into_any_element(),
            8 => text
                .units(TextUnit::Words)
                .enter_with(|unit| {
                    TextTarget::default()
                        .y_em(if unit.index % 2 == 0 { -0.7 } else { 0.7 })
                        .opacity(0.)
                })
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
                    Easing::Linear,
                ))
                .progress(0.5)
                .into_any_element(),
            _ => self
                .number
                .element("ce-count", self.count)
                .effect(NumberEffect::Roll)
                .min_digits(3)
                // Avoid stock macOS system-font glyph clipping at display sizes.
                .when(cfg!(target_os = "macos"), |number| {
                    number.font_family("Helvetica Neue")
                })
                .text_size(px(72.))
                .into_any_element(),
        };

        div()
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .gap_6()
            .p_8()
            .bg(Palette::background())
            .text_color(Palette::foreground())
            .on_action(cx.listener(|this, _: &NextEffect, window, cx| this.switch(window, cx)))
            .on_action(cx.listener(|this, _: &ChangeText, _, cx| {
                this.sample = (this.sample + 1) % TEXT.len();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Replay, window, cx| this.replay(window, cx)))
            .on_action(cx.listener(|this, _: &TogglePause, window, cx| {
                this.paused = !this.paused;

                if this.paused {
                    this.motion.pause(window, cx);
                } else {
                    this.motion.play(window, cx);
                }

                cx.notify();
            }))
            .on_action(cx.listener(|_, _: &ToggleReduced, _, cx| {
                let reduced = !cx.reduce_motion();

                cx.set_reduce_motion(reduced);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Increase, _, cx| {
                this.count = this.count.saturating_add(101);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Decrease, _, cx| {
                this.count = this.count.saturating_sub(101);
                cx.notify();
            }))
            .child(div().text_2xl().child("Native text motion"))
            .child(format!(
                "{:02} / 12 · {} · Reduced motion {}",
                self.mode + 1,
                MODES[self.mode],
                cx.reduce_motion()
            ))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_lg()
                    .bg(Palette::surface())
                    .overflow_hidden()
                    .p_6()
                    .child(content),
            )
            .child(
                div()
                    .flex()
                    .gap_4()
                    .child(
                        div()
                            .id("next-effect")
                            .aria_label("Next text effect")
                            .px_4()
                            .py_2()
                            .rounded_md()
                            .bg(Palette::surface())
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| this.switch(window, cx)),
                            )
                            .child("Next effect · E"),
                    )
                    .child(
                        div()
                            .id("change-text")
                            .aria_label("Change text")
                            .px_4()
                            .py_2()
                            .rounded_md()
                            .bg(Palette::surface())
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.sample = (this.sample + 1) % TEXT.len();
                                    cx.notify();
                                }),
                            )
                            .child("Change text · Space"),
                    )
                    .child(
                        div()
                            .id("replay")
                            .aria_label("Replay text motion")
                            .px_4()
                            .py_2()
                            .rounded_md()
                            .bg(Palette::surface())
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| this.replay(window, cx)),
                            )
                            .child("Replay · R"),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .child("P pause/resume · M reduced motion · ↑ ↓ count · Command+Q quit"),
            )
    }
}
fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        cx.bind_keys([
            KeyBinding::new("e", NextEffect, None),
            KeyBinding::new("space", ChangeText, None),
            KeyBinding::new("r", Replay, None),
            KeyBinding::new("p", TogglePause, None),
            KeyBinding::new("m", ToggleReduced, None),
            KeyBinding::new("up", Increase, None),
            KeyBinding::new("down", Decrease, None),
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("ctrl-q", Quit, None),
        ]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1080.), px(680.)),
                    cx,
                ))),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("CE Text Motion".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                cx.new(|cx| {
                    let focus = cx.focus_handle();

                    focus.focus(window, cx);
                    let wave = Animation::keyframes(
                        Duration::from_millis(700),
                        [
                            Keyframe::new(0., Target::default()),
                            Keyframe::new(0.5, Target::default().y(-8.)),
                            Keyframe::new(1., Target::default()),
                        ],
                    )
                    .expect("valid keyframes")
                    .repeat(Repeat::Forever, RepeatMode::Loop, Duration::ZERO)
                    .expect("positive wave period");

                    Demo {
                        motion: TextMotionState::default(),
                        number: NumberMotionState::default(),
                        sample: 0,
                        mode: 0,
                        paused: false,
                        count: 199,
                        focus,
                        wave,
                        scramble: Scramble::new("ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789").seed(42),
                    }
                })
            },
        )
        .expect("open CE text playground");
    });
}
