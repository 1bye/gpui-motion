//! Measures allocation-free sampling against a settled-state baseline.
//! Run with `cargo bench -p gpui-ce-motion --bench sampling`.

use gpui_ce_motion::{
    Animation, AnimationState, Color, ColorProperty, ColorState, Drag, Easing, LayoutPositionState,
    Motion, MotionState, PathCommand, PathMorphState, PathShape, Repeat, RepeatMode, StrokeReveal,
    StyleMotion, StyleMotionState, StyleTarget, SvgMotion, SvgMotionState, SvgTarget, Target,
    Transition,
};
use std::{
    hint::black_box,
    time::{Duration, Instant},
};

fn run(label: &str, transition: Transition, active: bool, layout: bool) {
    let count = 1_000;
    let frames: u64 = 240;
    let mut states: Vec<_> = (0..count).map(|_| MotionState::default()).collect();
    let mut batches = Vec::with_capacity(25);
    for _ in 0..25 {
        for state in &mut states {
            *state = MotionState::default();
        }
        let started = Instant::now();
        for frame in 0..frames {
            let x = if active && (frame / 24).is_multiple_of(2) {
                200.0
            } else {
                0.0
            };
            let mut target = Target::default().x(x).y(x / 2.0);
            let mut initial = Target::default();
            if layout {
                target = target
                    .width(240.0 + x)
                    .height(160.0 + x / 2.0)
                    .border_radius(8.0 + x / 10.0);
                initial = initial.width(240.0).height(160.0).border_radius(8.0);
            }
            let spec = Motion::new(target).initial(initial).transition(transition);
            for state in &mut states {
                black_box(state.sample(
                    black_box(spec),
                    Duration::from_micros(frame * 8333),
                    false,
                ));
            }
        }
        batches.push(started.elapsed().as_secs_f64() * 1e9 / (count * frames as usize) as f64);
    }
    batches.sort_by(f64::total_cmp);
    println!(
        "{label}: median {:.1} ns/object, p95 {:.1} ns/object; 1000 objects median {:.3} ms/frame",
        batches[12],
        batches[23],
        batches[12] / 1000.0
    );
}

fn advanced(label: &str, mut sample: impl FnMut(Duration)) {
    let mut batches = Vec::with_capacity(25);
    for batch in 0..25 {
        let started = Instant::now();
        for frame in 0..240 {
            sample(Duration::from_micros((batch * 240 + frame) * 8333));
        }
        batches.push(started.elapsed().as_secs_f64() * 1e9 / 240_000.0);
    }
    batches.sort_by(f64::total_cmp);
    println!(
        "{label}: median {:.1} ns/object, p95 {:.1} ns/object; 1000 objects median {:.3} ms/frame",
        batches[12],
        batches[23],
        batches[12] / 1000.0
    );
}
fn main() {
    println!(
        "retained bytes/state: MotionState {}, AnimationState {}, ColorState {}, Drag {}",
        std::mem::size_of::<MotionState>(),
        std::mem::size_of::<AnimationState>(),
        std::mem::size_of::<ColorState>(),
        std::mem::size_of::<Drag>()
    );
    println!(
        "retained bytes/SVG state: {}",
        std::mem::size_of::<SvgMotionState>()
    );
    let mut styles: Vec<_> = (0..1000).map(|_| StyleMotionState::default()).collect();
    advanced("style padding/radii/color springs + interruption", |now| {
        let expanded = (now.as_millis() / 200).is_multiple_of(2);
        let target = StyleTarget::default()
            .padding([if expanded { 20. } else { 0. }; 4])
            .radii([if expanded { 12. } else { 2. }; 4])
            .color(
                ColorProperty::Background,
                Color::rgb(if expanded { 0xeeddcc } else { 0x223344 }),
            );
        for state in &mut styles {
            black_box(state.sample(black_box(StyleMotion::new(target)), now, false));
        }
    });
    let path_a = PathShape::new([
        PathCommand::Move([0., 0.]),
        PathCommand::Cubic([20., 0.], [80., 0.], [100., 0.]),
    ])
    .unwrap();
    let path_b = PathShape::new([
        PathCommand::Move([0., 0.]),
        PathCommand::Cubic([20., 80.], [80., -80.], [100., 0.]),
    ])
    .unwrap();
    let mut paths: Vec<_> = (0..1000)
        .map(|_| PathMorphState::new(path_a.clone()))
        .collect();
    advanced("prepared cubic morph + measured stroke reveal", |now| {
        let target = if (now.as_millis() / 200).is_multiple_of(2) {
            &path_a
        } else {
            &path_b
        };
        for state in &mut paths {
            let frame = state
                .sample(black_box(target), Transition::default(), now, false)
                .unwrap();
            frame.stroke_segments(StrokeReveal::new(0.5, 0.25, 1.).unwrap(), |a, b| {
                black_box((a, b));
            });
        }
    });
    let mut svgs: Vec<_> = (0..1000).map(|_| SvgMotionState::default()).collect();
    advanced("SVG scale/rotation springs + interruption", |now| {
        let expanded = (now.as_millis() / 200).is_multiple_of(2);
        let motion = SvgMotion::new(
            SvgTarget::default()
                .scale(if expanded { 1.1 } else { 0.7 })
                .rotate_degrees(if expanded { 30. } else { -30. }),
        )
        .initial(SvgTarget::default());
        for state in &mut svgs {
            black_box(state.sample(black_box(motion), now, false));
        }
    });
    let mut positions: Vec<_> = (0..1000).map(|_| LayoutPositionState::default()).collect();
    advanced("measured layout position + interruption", |now| {
        let origin = if (now.as_millis() / 200).is_multiple_of(2) {
            [150., 75.]
        } else {
            [0., 0.]
        };
        for state in &mut positions {
            black_box(state.sample(black_box(origin), Transition::default(), now, false));
        }
    });
    let clip = Animation::to(
        Target::default()
            .x(200.)
            .width(300.)
            .height(160.)
            .border_radius(24.),
        Duration::from_millis(500),
        Easing::EaseInOut,
    )
    .repeat(Repeat::Forever, RepeatMode::Mirror, Duration::ZERO)
    .unwrap();
    let mut clips: Vec<_> = (0..1000).map(|_| AnimationState::default()).collect();
    advanced("repeating prepared keyframes", |now| {
        for state in &mut clips {
            black_box(state.sample(black_box(&clip), now, false));
        }
    });
    let mut colors: Vec<_> = (0..1000).map(|_| ColorState::new(Color::rgb(0))).collect();
    advanced("color spring + interruption", |now| {
        let color = Color::rgb(if (now.as_millis() / 200).is_multiple_of(2) {
            0xffeedd
        } else {
            0x112244
        });
        for state in &mut colors {
            black_box(
                state
                    .sample(black_box(color), Transition::default(), now, false)
                    .0
                    .channels(),
            );
        }
    });
    let mut drags: Vec<_> = (0..1000)
        .map(|_| Drag::new([0., 0.], [0., 0.], [300., 100.]))
        .collect();
    advanced("drag + bounded inertial release", |now| {
        for state in &mut drags {
            if (now.as_millis() / 200).is_multiple_of(2) {
                if !state.is_dragging() {
                    state.begin([0., 0.], now);
                }
                state.move_to([100., 40.], now);
            } else {
                state.end(now);
            }
            black_box(state.sample(now, false));
        }
    });
    run("settled", Transition::default(), false, false);
    run(
        "tween + interruption",
        Transition::Tween(Duration::from_millis(300), Easing::EaseInOut),
        true,
        false,
    );
    run("spring + interruption", Transition::default(), true, false);
    run(
        "six-channel layout spring + interruption",
        Transition::default(),
        true,
        true,
    );
}
