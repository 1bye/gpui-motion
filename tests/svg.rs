//! Covers: SVG transform public API, interruption, clocks and native conversion.
//! Real: scalar solver and optional CE adapter. Doubles: none. Requires: no GPU.
use gpui_ce_motion::{Easing, SvgMotion, SvgMotionState, SvgTarget, Transition};
use std::time::Duration;
fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
fn destination() -> SvgTarget {
    SvgTarget::default()
        .scale_xy(1.5, 0.75)
        .rotate_degrees(180.)
}
fn motion() -> SvgMotion {
    SvgMotion::new(destination()).initial(SvgTarget::default())
}

#[test]
fn first_mount_without_initial_is_immediate_and_initial_is_used_once() {
    let mut state = SvgMotionState::default();
    let frame = state.sample(SvgMotion::new(destination()), ms(0), false);

    assert_eq!(frame.target(), destination());
    assert!(!frame.is_animating());
    assert!(!frame.just_completed());
    let next = state.sample(
        SvgMotion::new(SvgTarget::default()).initial(SvgTarget::default().scale(0.)),
        ms(100),
        false,
    );

    assert_eq!(frame.target(), next.target());
    assert!(next.is_animating());
}
#[test]
fn interrupted_springs_preserve_all_presentations_and_velocities_between_samples() {
    let mut state = SvgMotionState::default();

    state.sample(motion(), ms(0), false);
    let mut reference = SvgMotionState::default();

    reference.sample(motion(), ms(0), false);
    let before = reference.sample(motion(), ms(80), false);
    let reverse = SvgMotion::new(SvgTarget::default().scale_xy(-1., 1.).rotate_degrees(-90.));
    let after = state.sample(reverse, ms(80), false);

    assert_eq!(before.target(), after.target());
    assert_eq!(before.velocity(), after.velocity());
    assert!(
        state
            .sample(reverse, ms(81), false)
            .target()
            .rotation_degrees()
            > after.target().rotation_degrees()
    );
    let done = state.sample(reverse, ms(5000), false);

    assert_eq!(
        done.target(),
        SvgTarget::default().scale_xy(-1., 1.).rotate_degrees(-90.)
    );
    assert!(!done.is_animating());
    assert!(done.just_completed());
    assert!(!state.sample(reverse, ms(5001), false).just_completed());
}
#[test]
fn tween_interruption_delay_and_backward_time_preserve_position() {
    let spec = motion()
        .transition(Transition::Tween(ms(1000), Easing::Linear))
        .delay(ms(100));

    let mut state = SvgMotionState::default();

    state.sample(spec, ms(0), false);
    assert_eq!(
        state.sample(spec, ms(50), false).target(),
        SvgTarget::default()
    );
    let before = state.sample(spec, ms(600), false);

    assert_eq!(before.target().scale_factors(), [1.25, 0.875]);
    assert_eq!(before.target().rotation_degrees(), 90.);
    assert_eq!(state.sample(spec, ms(200), false).target(), before.target());
    let reverse =
        SvgMotion::new(SvgTarget::default()).transition(Transition::Tween(ms(200), Easing::Linear));

    assert_eq!(
        state.sample(reverse, ms(600), false).target(),
        before.target()
    );
    assert_eq!(
        state.sample(reverse, ms(800), false).target(),
        SvgTarget::default()
    );
}
#[test]
fn absolute_rotation_preserves_full_turns_and_negative_scales() {
    let spec = SvgMotion::new(SvgTarget::default().scale_xy(-1., 0.).rotate_degrees(720.))
        .initial(SvgTarget::default())
        .transition(Transition::Tween(ms(1000), Easing::Linear));

    let mut state = SvgMotionState::default();

    state.sample(spec, ms(0), false);
    let half = state.sample(spec, ms(500), false);

    assert_eq!(half.target().scale_factors(), [0., 0.5]);
    assert_eq!(half.target().rotation_degrees(), 360.);
    assert_eq!(half.velocity(), [-2., -1., 720.]);
    assert_eq!(
        state
            .sample(spec, ms(1000), false)
            .target()
            .rotation_degrees(),
        720.
    );
}
#[test]
fn reduced_motion_finishes_delays_and_does_not_replay() {
    let spec = motion().delay(ms(1000));
    let mut state = SvgMotionState::default();

    state.sample(spec, ms(0), false);
    let done = state.sample(spec, ms(1), true);

    assert_eq!(done.target(), destination());
    assert_eq!(done.velocity(), [0.; 3]);
    assert!(done.just_completed());
    assert!(!done.is_animating());
    let idle = state.sample(spec, ms(2), false);

    assert_eq!(idle.target(), destination());
    assert!(!idle.is_animating());
    assert!(!idle.just_completed());
}
#[test]
fn analytic_svg_sampling_is_independent_of_frame_rate() {
    let mut fine = SvgMotionState::default();
    let mut coarse = SvgMotionState::default();

    fine.sample(motion(), ms(0), false);
    coarse.sample(motion(), ms(0), false);

    for n in 1..=137 {
        fine.sample(motion(), ms(n), false);
    }

    let a = fine.sample(motion(), ms(137), false);
    let b = coarse.sample(motion(), ms(137), false);

    assert_eq!(a.target(), b.target());
    assert_eq!(a.velocity(), b.velocity());
}
#[test]
fn transform_builders_reject_nonfinite_values() {
    for v in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(std::panic::catch_unwind(|| SvgTarget::default().scale(v)).is_err());
        assert!(std::panic::catch_unwind(|| SvgTarget::default().scale_xy(1., v)).is_err());
        assert!(
            std::panic::catch_unwind(|| SvgTarget::default().rotate_degrees(f64::from(v))).is_err()
        );
    }
}
#[cfg(feature = "gpui")]
#[test]
fn native_transform_uses_signed_scales_and_reduces_only_the_paint_angle() {
    use gpui::{Styled, px, svg};
    let mut state = SvgMotionState::default();

    let frame = state.sample(
        SvgMotion::new(SvgTarget::default().scale_xy(-2., 0.5).rotate_degrees(810.)),
        ms(0),
        false,
    );

    let expected = gpui::Transformation::scale(gpui::size(-2., 0.5))
        .with_rotation(gpui::radians(std::f32::consts::FRAC_PI_2));

    assert_eq!(frame.transformation(), expected);
    assert_eq!(frame.target().rotation_degrees(), 810.);
    let mut svg = frame.apply(svg().size(px(24.)));

    assert_eq!(svg.style().size.width, Some(px(24.).into()));
    assert_eq!(svg.style().size.height, Some(px(24.).into()));
}
#[cfg(feature = "gpui")]
#[test]
fn huge_finite_angles_stay_finite_at_the_native_boundary() {
    let target = SvgTarget::default().rotate_degrees(f64::MAX);
    let mut state = SvgMotionState::default();
    let frame = state.sample(SvgMotion::new(target), ms(0), false);

    let expected = gpui::Transformation::default()
        .with_rotation(gpui::radians(f64::MAX.rem_euclid(360.).to_radians() as f32));

    assert_eq!(frame.transformation(), expected);
}
