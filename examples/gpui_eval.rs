//! Native redraw probe. Run with `cargo run --release --features demo --example gpui_eval -- 400 8`.
//! Modes: baseline/motion, layout-baseline/layout, keyframes-baseline/keyframes,
//! svg-baseline/svg, position-baseline/position. Position modes include automatic
//! prepaint sampling, which is excluded from the view-construction timer.
//! Reports view construction time and render intervals, not GPU time or screen scanout.

mod common;

use std::time::{Duration, Instant};

use gpui::{
    App, Bounds, Context, Render, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb,
    size,
};

use gpui_ce_motion::{
    Animation, AnimationState, Easing, LayoutGroup, Motion, MotionState, Repeat, RepeatMode,
    SvgMotion, SvgMotionState, SvgTarget, Target,
};

#[derive(Clone, Copy)]
enum Mode {
    Baseline,
    Motion,
    LayoutBaseline,
    Layout,
    KeyframesBaseline,
    Keyframes,
    SvgBaseline,
    Svg,
    PositionBaseline,
    Position,
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Motion => "motion",
            Self::LayoutBaseline => "layout-baseline",
            Self::Layout => "layout",
            Self::KeyframesBaseline => "keyframes-baseline",
            Self::Keyframes => "keyframes",
            Self::SvgBaseline => "svg-baseline",
            Self::Svg => "svg",
            Self::PositionBaseline => "position-baseline",
            Self::Position => "position",
        }
    }
    fn has_layout(self) -> bool {
        matches!(
            self,
            Self::LayoutBaseline | Self::Layout | Self::KeyframesBaseline | Self::Keyframes
        )
    }
}

struct Probe {
    mode: Mode,
    positions: LayoutGroup,
    shared: MotionState,
    shared_svg: SvgMotionState,
    svgs: Vec<SvgMotionState>,
    clip: Animation,
    shared_clip: AnimationState,
    clips: Vec<AnimationState>,
    states: Vec<MotionState>,
    started: Instant,
    last_frame: Option<Instant>,
    count: usize,
    seconds: u64,
    intervals_ms: Vec<f64>,
    construction_ms: Vec<f64>,
    inactive_frames: usize,
    was_active: bool,
    active_intervals_ms: Vec<f64>,
}

fn percentile(values: &mut [f64], fraction: f64) -> f64 {
    values.sort_by(f64::total_cmp);
    values[((values.len() - 1) as f64 * fraction).round() as usize]
}

impl Render for Probe {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        let elapsed = now.duration_since(self.started);
        // SVG first use may initialize font fallback and raster caches.
        let warmup = if matches!(
            self.mode,
            Mode::SvgBaseline | Mode::Svg | Mode::PositionBaseline | Mode::Position
        ) {
            5
        } else {
            1
        };
        let measuring = elapsed >= Duration::from_secs(warmup);

        let active = window.is_window_active();
        if measuring && !active {
            self.inactive_frames += 1;
        }

        if let Some(last) = self.last_frame.replace(now)
            && measuring
        {
            self.intervals_ms.push((now - last).as_secs_f64() * 1000.0);
            if active && self.was_active {
                self.active_intervals_ms
                    .push((now - last).as_secs_f64() * 1000.0);
            }
        }
        self.was_active = active;

        if elapsed >= Duration::from_secs(self.seconds + warmup) {
            assert!(
                !self.intervals_ms.is_empty(),
                "probe received no measured frames"
            );
            assert!(
                !self.construction_ms.is_empty(),
                "probe constructed no measured views"
            );
            let frames = self.intervals_ms.len();
            let frame_p50 = percentile(&mut self.intervals_ms, 0.5);
            let frame_p95 = percentile(&mut self.intervals_ms, 0.95);
            let frame_p99 = percentile(&mut self.intervals_ms, 0.99);
            let frame_max = percentile(&mut self.intervals_ms, 1.0);
            let build_p50 = percentile(&mut self.construction_ms, 0.5);
            let build_p95 = percentile(&mut self.construction_ms, 0.95);
            let active_samples = self.active_intervals_ms.len();
            let active_p95 = if active_samples > 0 {
                format!("{:.3}", percentile(&mut self.active_intervals_ms, 0.95))
            } else {
                "null".into()
            };
            eprintln!("GPUI_EVAL_MODE {}", self.mode.name());
            eprintln!(
                "GPUI_EVAL {{\"cards\":{},\"seconds\":{},\"frames\":{},\"inactive_frames\":{},\"active_samples\":{},\"active_interval_p95_ms\":{},\"interval_p50_ms\":{:.3},\"interval_p95_ms\":{:.3},\"interval_p99_ms\":{:.3},\"interval_max_ms\":{:.3},\"construction_p50_ms\":{:.3},\"construction_p95_ms\":{:.3}}}",
                self.count,
                self.seconds,
                frames,
                self.inactive_frames,
                active_samples,
                active_p95,
                frame_p50,
                frame_p95,
                frame_p99,
                frame_max,
                build_p50,
                build_p95,
            );
            cx.quit();
        } else {
            window.request_animation_frame();
        }

        let offset = (elapsed.as_secs_f32() * 3.0).sin() * 3.0;
        let mut scene = div()
            .size_full()
            .relative()
            .bg(rgb(0x18181b))
            .text_color(rgb(0xf4f4f5));
        let expanded = (elapsed.as_millis() / 200).is_multiple_of(2);
        let layout = Motion::new(
            Target::default()
                .width(if expanded { 42.0 } else { 30.0 })
                .height(if expanded { 26.0 } else { 18.0 })
                .border_radius(if expanded { 8.0 } else { 2.0 }),
        )
        .initial(
            Target::default()
                .width(42.0)
                .height(26.0)
                .border_radius(8.0),
        );
        let shared = match self.mode {
            Mode::LayoutBaseline => Some(self.shared.sample(layout, elapsed, false)),
            Mode::KeyframesBaseline => Some(self.shared_clip.sample(&self.clip, elapsed, false)),
            _ => None,
        };

        let svg_motion = SvgMotion::new(
            SvgTarget::default()
                .scale(if expanded { 1.1 } else { 0.7 })
                .rotate_degrees(if expanded { 30. } else { -30. }),
        )
        .initial(SvgTarget::default());
        let shared_svg = matches!(self.mode, Mode::SvgBaseline)
            .then(|| self.shared_svg.sample(svg_motion, elapsed, false));
        let position_mode = matches!(self.mode, Mode::PositionBaseline | Mode::Position);
        if position_mode {
            let mut board = div()
                .m(px(16.))
                .w(px(if expanded { 900. } else { 720. }))
                .flex()
                .flex_wrap()
                .gap(px(3.));
            for ix in 0..self.count {
                let id = if expanded { self.count - 1 - ix } else { ix };
                let card = div()
                    .id(("card", id))
                    .w(px(30.))
                    .h(px(18.))
                    .flex_shrink_0()
                    .bg(rgb(0x303a50))
                    .text_xs()
                    .child(id.to_string());
                board = board.child(if matches!(self.mode, Mode::Position) {
                    self.positions
                        .position(("position", id), card)
                        .into_any_element()
                } else {
                    card.into_any_element()
                });
            }
            scene = scene.child(board);
        }
        for ix in 0..if position_mode { 0 } else { self.count } {
            if matches!(self.mode, Mode::SvgBaseline | Mode::Svg) {
                let frame =
                    shared_svg.unwrap_or_else(|| self.svgs[ix].sample(svg_motion, elapsed, false));
                scene = scene.child(
                    div()
                        .absolute()
                        .left(px(24. + (ix % 20) as f32 * 45.))
                        .top(px(12. + (ix / 20) as f32 * 29.))
                        .w(px(45.))
                        .h(px(29.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            frame.apply(
                                gpui::svg()
                                    .path("motion.svg")
                                    .size(px(18.))
                                    .text_color(rgb(0xb8c8ff)),
                            ),
                        ),
                );
                continue;
            }
            let offset = if matches!(self.mode, Mode::Motion) {
                let destination = if (elapsed.as_millis() / 200).is_multiple_of(2) {
                    3.0
                } else {
                    -3.0
                };
                self.states[ix]
                    .sample(
                        Motion::new(Target::default().x(destination)).initial(Target::default()),
                        elapsed,
                        false,
                    )
                    .target()
                    .x_px()
            } else {
                offset
            };
            if self.mode.has_layout() {
                let frame = shared.unwrap_or_else(|| {
                    if matches!(self.mode, Mode::Keyframes) {
                        self.clips[ix].sample(&self.clip, elapsed, false)
                    } else {
                        self.states[ix].sample(layout, elapsed, false)
                    }
                });
                scene = scene.child(
                    div()
                        .absolute()
                        .left(px(24.0 + (ix % 20) as f32 * 45.0))
                        .top(px(12.0 + (ix / 20) as f32 * 29.0))
                        .w(px(45.0))
                        .h(px(29.0))
                        .child(
                            frame.apply(div().bg(rgb(0x303a50)).text_xs().child(ix.to_string())),
                        ),
                );
                continue;
            }
            scene = scene.child(
                div()
                    .absolute()
                    .left(px(24.0 + (ix % 20) as f32 * 45.0 + offset))
                    .top(px(12.0 + (ix / 20) as f32 * 29.0))
                    .w(px(42.0))
                    .h(px(26.0))
                    .rounded_md()
                    .bg(rgb(0x303a50))
                    .text_xs()
                    .child(ix.to_string()),
            );
        }

        if measuring {
            self.construction_ms
                .push(now.elapsed().as_secs_f64() * 1000.0);
        }

        scene
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let count: usize = args.next().unwrap_or_else(|| "400".into()).parse()?;
    let seconds: u64 = args.next().unwrap_or_else(|| "8".into()).parse()?;
    let mode = match args.next().as_deref().unwrap_or("baseline") {
        "baseline" => Mode::Baseline,
        "motion" => Mode::Motion,
        "layout-baseline" => Mode::LayoutBaseline,
        "layout" => Mode::Layout,
        "keyframes-baseline" => Mode::KeyframesBaseline,
        "keyframes" => Mode::Keyframes,
        "svg-baseline" => Mode::SvgBaseline,
        "svg" => Mode::Svg,
        "position-baseline" => Mode::PositionBaseline,
        "position" => Mode::Position,
        _ => return Err("mode must be baseline, motion, layout-baseline, layout, keyframes-baseline, keyframes, svg-baseline, svg, position-baseline, or position".into()),
    };
    if count > 400 || !(1..=60).contains(&seconds) || args.next().is_some() {
        return Err("usage: gpui_eval [cards: 0..400] [seconds: 1..60] [baseline|motion|layout-baseline|layout|keyframes-baseline|keyframes|svg-baseline|svg|position-baseline|position]".into());
    }

    let assets = common::SvgAsset(include_bytes!("assets/motion.svg"));
    gpui_platform::application()
        .with_assets(assets)
        .run(move |cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(960.0), px(640.0)), cx);
            let window = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        ..Default::default()
                    },
                    move |_, cx| {
                        cx.new(|_| Probe {
                            mode,
                            positions: LayoutGroup::default(),
                            shared: MotionState::default(),
                            shared_svg: SvgMotionState::default(),
                            svgs: (0..if matches!(mode, Mode::Svg) { count } else { 0 })
                                .map(|_| SvgMotionState::default())
                                .collect(),
                            clip: Animation::keyframes(
                                Duration::from_millis(400),
                                [
                                    gpui_ce_motion::Keyframe::new(
                                        0.,
                                        Target::default().width(42.).height(26.).border_radius(8.),
                                    ),
                                    gpui_ce_motion::Keyframe::new(
                                        1.,
                                        Target::default().width(30.).height(18.).border_radius(2.),
                                    )
                                    .easing(Easing::EaseInOut),
                                ],
                            )
                            .expect("valid probe clip")
                            .repeat(Repeat::Forever, RepeatMode::Mirror, Duration::ZERO)
                            .expect("positive period"),
                            shared_clip: AnimationState::default(),
                            clips: (0..count).map(|_| AnimationState::default()).collect(),
                            states: (0..count).map(|_| MotionState::default()).collect(),
                            started: Instant::now(),
                            last_frame: None,
                            count,
                            seconds,
                            intervals_ms: Vec::with_capacity(seconds as usize * 240),
                            construction_ms: Vec::with_capacity(seconds as usize * 240),
                            inactive_frames: 0,
                            was_active: false,
                            active_intervals_ms: Vec::with_capacity(seconds as usize * 240),
                        })
                    },
                )
                .expect("failed to open evaluation window");
            window
                .update(cx, |_, window, _| window.activate_window())
                .expect("evaluation window remains open");
            cx.on_window_closed(|cx, _| cx.quit()).detach();
            cx.activate(true);
        });

    Ok(())
}
