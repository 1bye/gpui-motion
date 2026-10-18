//! Kit styles, prepared playback, SVG conversion and native scheduling contracts.
//! TestPlatform substitutes fonts/display/GPU; clocks and UI phases are real.
use gpui_kit::{AppContext, Context, Render, TestAppContext, Window, div, prelude::*, px, size};

use gpui_motion_kit::core::{
    Animation, Color, ColorProperty, Easing, Repeat, RepeatMode, StyleMotion, StyleProperty,
    StyleTarget, SvgMotion, SvgTarget, Target, Transition,
};

use gpui_motion_kit::{AnimationState, Frame, StyleMotionState, SvgMotionState, Timeline};
use std::time::Duration;

fn clip() -> Animation {
    Animation::to(
        Target::default().x(100.),
        Duration::from_secs(1),
        Easing::Linear,
    )
}

struct Probe {
    player: AnimationState,
    clip: Animation,
    timeline: Timeline,
    timeline_mode: bool,
    shown: Vec<Frame>,
    style: StyleMotionState,
    svg: SvgMotionState,
    changed: bool,
    last_svg: Option<gpui_motion_kit::SvgFrame>,
    last_style: Option<gpui_motion_kit::StyleFrame>,
}

impl Render for Probe {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.shown.clear();

        if self.timeline_mode {
            self.shown
                .extend_from_slice(self.timeline.frame(window, cx));
        } else {
            self.shown.push(self.player.frame(&self.clip, window, cx));
        }

        let style = self.style.frame(
            StyleMotion::new(
                StyleTarget::default().padding([if self.changed { 20. } else { 0. }; 4]),
            )
            .transition(Transition::Tween(Duration::from_secs(1), Easing::Linear)),
            window,
            cx,
        );

        let svg = self.svg.frame(
            SvgMotion::new(
                SvgTarget::default()
                    .scale(if self.changed { 2. } else { 1. })
                    .rotate_degrees(if self.changed { 450. } else { 0. }),
            ),
            window,
            cx,
        );

        self.last_svg = Some(svg);
        self.last_style = Some(style);

        // Force the same public drawing conversion used by the live gallery.
        let _drawing = svg.apply(gpui_kit::svg().size(px(24.)));

        div().flex().flex_col().children(
            self.shown
                .iter()
                .copied()
                .map(|frame| style.apply(frame.apply(div().w(px(20.)).h(px(20.))))),
        )
    }
}

fn open(cx: &mut TestAppContext, timeline_mode: bool) -> gpui_kit::WindowHandle<Probe> {
    let window = cx.open_window(size(px(500.), px(300.)), |_, _| {
        let mut timeline = Timeline::default();

        timeline.at(Duration::from_millis(500), clip());

        Probe {
            player: AnimationState::default(),
            clip: clip(),
            timeline,
            timeline_mode,
            shown: Vec::with_capacity(1),
            style: StyleMotionState::default(),
            svg: SvgMotionState::default(),
            changed: false,
            last_svg: None,
            last_style: None,
        }
    });

    draw(cx, window);

    window
}

fn draw(cx: &mut TestAppContext, window: gpui_kit::WindowHandle<Probe>) {
    cx.update_window(window.into(), |_, window, cx| {
        window.simulate_next_frame(cx);
        window.draw(cx).clear(cx);
    })
    .unwrap();
}

fn advance(cx: &mut TestAppContext, window: gpui_kit::WindowHandle<Probe>, millis: u64) {
    cx.executor().advance_clock(Duration::from_millis(millis));
    draw(cx, window);
}

#[gpui_kit::test]
fn prepared_clip_uses_executor_clock_for_pause_seek_speed_and_restart(cx: &mut TestAppContext) {
    let window = open(cx, false);

    advance(cx, window, 400);
    window
        .update(cx, |view, _, cx| {
            assert_eq!(view.shown[0].target().x_px(), 40.);
            let now = view.player.now(cx);

            view.player.pause(now);
            cx.notify();
        })
        .unwrap();
    advance(cx, window, 200);
    window
        .update(cx, |view, _, cx| {
            assert_eq!(view.shown[0].target().x_px(), 40.);
            assert!(!view.shown[0].is_animating());
            let now = view.player.now(cx);

            view.player.seek(Duration::from_millis(800), now);
            view.player.set_speed(-1., now).unwrap();
            view.player.play(now);
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    advance(cx, window, 200);
    window
        .update(cx, |view, _, cx| {
            assert_eq!(view.shown[0].target().x_px(), 60.);
            let now = view.player.now(cx);

            view.player.set_speed(1., now).unwrap();
            view.player.restart(now);
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    advance(cx, window, 1000);
    window
        .update(cx, |view, _, _| {
            assert_eq!(view.shown[0].target().x_px(), 100.);
            assert!(view.shown[0].just_completed());
        })
        .unwrap();
    draw(cx, window);
    window
        .update(cx, |view, _, _| assert!(!view.shown[0].just_completed()))
        .unwrap();
    assert_eq!(
        cx.update_window(window.into(), |_, w, cx| w.simulate_next_frame(cx))
            .unwrap(),
        0
    );
}

#[gpui_kit::test]
fn timeline_schedules_future_tracks_and_stops_when_paused_or_reduced(cx: &mut TestAppContext) {
    let window = open(cx, true);

    window
        .update(cx, |view, _, _| {
            assert!(!view.shown[0].is_animating());
            assert!(
                view.timeline.is_animating(),
                "a future track still needs frames"
            );
        })
        .unwrap();
    assert!(
        cx.update_window(window.into(), |_, w, cx| w.simulate_next_frame(cx))
            .unwrap()
            > 0
    );
    advance(cx, window, 1000);
    window
        .update(cx, |view, _, cx| {
            assert_eq!(view.shown[0].target().x_px(), 50.);
            let now = view.timeline.now(cx);

            view.timeline.pause(now);
            cx.notify();
        })
        .unwrap();
    advance(cx, window, 500);
    window
        .update(cx, |view, _, cx| {
            assert_eq!(view.shown[0].target().x_px(), 50.);
            assert!(!view.timeline.is_animating());
            let now = view.timeline.now(cx);

            view.timeline.seek(Duration::from_millis(600), now);
            view.timeline.set_speed(-1., now).unwrap();
            view.timeline.play(now);
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    advance(cx, window, 100);
    window
        .update(cx, |view, _, cx| {
            assert_eq!(view.shown[0].target().x_px(), 0.);
            let now = view.timeline.now(cx);

            view.timeline.set_speed(1., now).unwrap();
            view.timeline.restart(now);
            cx.set_reduce_motion(true);
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    window
        .update(cx, |view, _, _| {
            assert_eq!(view.shown[0].target().x_px(), 100.);
            assert!(!view.timeline.is_animating());
        })
        .unwrap();
    assert_eq!(
        cx.update_window(window.into(), |_, w, cx| w.simulate_next_frame(cx))
            .unwrap(),
        0
    );
}

#[gpui_kit::test]
fn interrupted_style_and_svg_frames_use_native_time_and_settle_in_reduced_motion(
    cx: &mut TestAppContext,
) {
    let window = open(cx, false);

    window
        .update(cx, |view, _, cx| {
            view.player.finish();
            view.changed = true;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    advance(cx, window, 300);
    let before = window
        .update(cx, |view, _, cx| {
            let pose = view.last_svg.unwrap();

            let padding = view
                .last_style
                .unwrap()
                .target()
                .get(StyleProperty::PaddingTop);

            view.changed = false;
            cx.notify();

            (pose.target(), pose.velocity(), padding)
        })
        .unwrap();

    draw(cx, window);
    window
        .update(cx, |view, _, cx| {
            let pose = view.last_svg.unwrap();

            assert_eq!(pose.target(), before.0);
            assert_eq!(pose.velocity(), before.1);
            assert_eq!(
                view.last_style
                    .unwrap()
                    .target()
                    .get(StyleProperty::PaddingTop),
                before.2
            );
            cx.set_reduce_motion(true);
            view.changed = true;
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

#[gpui_kit::test]
fn reduced_motion_settles_forever_clips_without_rescheduling(cx: &mut TestAppContext) {
    let window = open(cx, false);

    window
        .update(cx, |view, _, cx| {
            view.clip = clip()
                .repeat(Repeat::Forever, RepeatMode::Mirror, Duration::ZERO)
                .unwrap();
            cx.set_reduce_motion(true);
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    window
        .update(cx, |view, _, _| {
            assert_eq!(view.shown[0].target().x_px(), 100.);
            assert!(!view.shown[0].is_animating());
        })
        .unwrap();
    assert_eq!(
        cx.update_window(window.into(), |_, w, cx| w.simulate_next_frame(cx))
            .unwrap(),
        0
    );
}

#[test]
fn style_conversion_preserves_undeclared_values_and_applies_all_supported_tracks() {
    let target = StyleTarget::default()
        .radii([1., 2., 3., 4.])
        .border_widths([1., 2., 3., 4.])
        .padding([5., 6., 7., 8.])
        .margin([-1., 2., 3., 4.])
        .gap(9., 10.)
        .color(ColorProperty::Background, Color::rgb(0x123456))
        .color(ColorProperty::Text, Color::rgb(0xffffff))
        .color(ColorProperty::Border, Color::rgb(0x334455))
        .shadow([1., 2.], 3., 4., Color::rgba(0., 0., 0., 0.5));

    let frame = StyleMotionState::default().sample(StyleMotion::new(target), Duration::ZERO, false);

    let mut element = frame.apply(div().w(px(80.)));
    let style = element.style();

    assert_eq!(style.size.width, Some(px(80.).into()));
    assert_eq!(style.padding.top, Some(px(5.).into()));
    assert_eq!(style.padding.left, Some(px(8.).into()));
    assert_eq!(style.margin.top, Some(px(-1.).into()));
    assert_eq!(style.corner_radii.bottom_left, Some(px(4.).into()));
    assert_eq!(style.border_widths.right, Some(px(2.).into()));
    assert_eq!(style.gap.height, Some(px(10.).into()));
    let shadow = &style.box_shadow.as_ref().unwrap()[0];

    assert_eq!(shadow.blur_radius, px(3.));
    assert_eq!(shadow.spread_radius, px(4.));
    assert_eq!(shadow.offset, gpui_kit::point(px(1.), px(2.)));
    assert!(style.background.is_some());
    assert!(style.text.color.is_some());
    assert!(style.border_color.is_some());
}

#[test]
fn unsupported_filters_are_reported_instead_of_silently_ignored() {
    for property in [StyleProperty::Blur, StyleProperty::BackdropBlur] {
        let target = StyleTarget::default().value(property, 0.);

        let frame =
            StyleMotionState::default().sample(StyleMotion::new(target), Duration::ZERO, false);

        let error = frame
            .try_apply(div())
            .err()
            .expect("Kit has no filter hook");

        assert_eq!(error.property(), property);
        assert!(error.to_string().contains("stock GPUI Kit"));
    }
}

#[test]
fn timeline_tracks_labels_errors_and_frame_storage_follow_the_shared_engine() {
    let mut timeline = Timeline::default();

    assert_eq!(timeline.at(Duration::ZERO, clip()), 0);
    timeline.label("next").unwrap();
    assert_eq!(timeline.then(clip()).unwrap(), 1);
    assert_eq!(
        timeline
            .at_label("next", Duration::from_millis(500), clip())
            .unwrap(),
        2
    );
    assert!(
        timeline
            .at_label("missing", Duration::ZERO, clip())
            .is_err()
    );
    assert!(timeline.label("next").is_err());
    let frames = timeline.sample(Duration::ZERO, false);
    let storage = frames.as_ptr();

    assert_eq!(frames.len(), 3);

    for millis in [100, 500, 1000, 1600, 3000] {
        let frames = timeline.sample(Duration::from_millis(millis), false);

        assert_eq!(frames.len(), 3);
        assert_eq!(frames.as_ptr(), storage);
    }

    let mut infinite = Timeline::default();

    infinite.at(
        Duration::ZERO,
        clip()
            .repeat(Repeat::Forever, RepeatMode::Loop, Duration::ZERO)
            .unwrap(),
    );
    assert!(infinite.then(clip()).is_err());
    assert_eq!(infinite.sample(Duration::ZERO, true).len(), 1);
}

#[test]
fn svg_native_transform_preserves_unwrapped_rotation_signed_scale_and_custom_pivot() {
    let target = SvgTarget::default()
        .scale_xy(-2., 3.)
        .rotate_degrees(450.)
        .translate(5., 6.);

    let frame = SvgMotionState::default().sample(SvgMotion::new(target), Duration::ZERO, false);

    let expected = gpui_kit::Transformation::scale(size(-2., 3.))
        .with_rotation(gpui_kit::radians(std::f32::consts::FRAC_PI_2))
        .with_translation(gpui_kit::point(px(5.), px(6.)));

    assert_eq!(frame.transformation(), expected);
    assert_eq!(frame.target().rotation_degrees(), 450.);
    let around_top_left =
        frame.transformation_about(size(px(100.), px(80.)), gpui_kit::point(px(0.), px(0.)));

    // p - center is (-50,-40); rotation(90) * scale(-2,3) maps it to (120,100).
    assert_eq!(
        around_top_left,
        expected.with_translation(gpui_kit::point(px(-165.), px(-134.)))
    );
    let _drawing = frame.apply(gpui_kit::svg().size(px(24.)));
}
