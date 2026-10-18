//! Covers: external API, interruption, lifecycle, timing, reduced motion, validation.
//! Real: motion state machine and analytic spring solver.
//! Doubles: injected monotonic clock only. Requires: no GPU or window server.

use gpui_ce_motion::{Easing, Motion, MotionState, Presence, Spring, Target, Transition};
use std::time::Duration;

fn at(ms: u64) -> Duration {
    Duration::from_millis(ms)
}
fn spec(transition: Transition) -> Motion {
    Motion::new(Target::default().x(100.0))
        .initial(Target::default())
        .transition(transition)
}

#[test]
fn tween_reaches_exact_endpoint_without_restarting() {
    let mut state = MotionState::default();
    let motion = spec(Transition::Tween(at(1000), Easing::Linear));

    state.sample(motion, at(0), false);
    assert_eq!(state.sample(motion, at(500), false).target().x_px(), 50.0);
    let done = state.sample(motion, at(1000), false);

    assert_eq!(done.target().x_px(), 100.0);
    assert_eq!(done.presence(), Presence::Present);
    assert!(!done.is_animating());
    assert_eq!(state.sample(motion, at(1200), false).target().x_px(), 100.0);
}

#[test]
fn interrupted_spring_preserves_current_position_and_velocity() {
    let mut state = MotionState::default();
    let motion = spec(Transition::default());

    state.sample(motion, at(0), false);
    let before = state.sample(motion, at(80), false);
    let reverse = Motion::new(Target::default().x(-100.0));
    let after = state.sample(reverse, at(80), false);

    assert_eq!(before.target(), after.target());
    assert_eq!(before.velocity(), after.velocity());
    // Momentum initially continues right, then reverses to the new destination.
    assert!(state.sample(reverse, at(81), false).target().x_px() > after.target().x_px());
    let done = state.sample(reverse, at(5000), false);

    assert_eq!(done.target().x_px(), -100.0);
    assert!(!done.is_animating());
}

#[test]
fn interrupted_tween_uses_presentation_instead_of_previous_target() {
    let transition = Transition::Tween(at(1000), Easing::Linear);
    let mut state = MotionState::default();

    state.sample(spec(transition), at(0), false);
    let reverse = Motion::new(Target::default().x(-100.0)).transition(transition);

    // Retarget without explicitly sampling the old trajectory first.
    assert_eq!(state.sample(reverse, at(500), false).target().x_px(), 50.0);
    assert_eq!(
        state.sample(reverse, at(1000), false).target().x_px(),
        -25.0
    );
}

#[test]
fn exit_stays_mounted_until_settled_and_reentry_reuses_presentation() {
    let mut state = MotionState::default();
    let motion = spec(Transition::default()).exit(Target::default().y(20.0).opacity(0.0));

    state.sample(motion, at(0), false);
    state.sample(motion, at(1000), false);
    let exit = state.sample(motion.present(false), at(1000), false);

    assert_eq!(exit.presence(), Presence::Exiting);
    assert!(exit.is_mounted());
    assert!(!exit.is_present());
    let exiting = state.sample(motion.present(false), at(1080), false);
    let reentry = state.sample(motion, at(1080), false);

    assert_eq!(reentry.presence(), Presence::Entering);
    assert_eq!(reentry.target(), exiting.target());
    assert_eq!(reentry.velocity(), exiting.velocity());
    state.sample(motion.present(false), at(1100), false);
    let absent = state.sample(motion.present(false), at(5000), false);

    assert_eq!(absent.presence(), Presence::Absent);
    assert!(!absent.is_mounted());
    assert!(!absent.is_animating());
}

#[test]
fn absent_first_mount_does_not_animate_and_can_enter_later() {
    let mut state = MotionState::default();
    let motion = spec(Transition::default());
    let frame = state.sample(motion.present(false), at(0), false);

    assert!(!frame.is_mounted());
    assert!(!frame.is_animating());
    assert_eq!(
        state.sample(motion, at(100), false).presence(),
        Presence::Entering
    );
}

#[test]
fn reduced_motion_completes_active_and_exit_transitions_immediately() {
    let mut state = MotionState::default();
    let motion = spec(Transition::default());

    state.sample(motion, at(0), false);
    let snapped = state.sample(motion, at(60), true);

    assert_eq!(snapped.target().x_px(), 100.0);
    assert!(!snapped.is_animating());
    let removed = state.sample(motion.present(false), at(80), true);

    assert!(!removed.is_mounted());
    assert!(!removed.is_animating());
}

#[test]
fn analytic_springs_are_independent_of_frame_rate_in_all_damping_regimes() {
    for damping in [12.0, 20.0, 50.0] {
        let motion = spec(Transition::Spring(
            Spring::new(100.0, damping, 1.0).unwrap(),
        ));

        let mut direct = MotionState::default();
        let mut stepped = MotionState::default();

        direct.sample(motion, at(0), false);
        stepped.sample(motion, at(0), false);
        let end = direct.sample(motion, at(200), false);

        for time in (10..200).step_by(10) {
            stepped.sample(motion, at(time), false);
        }

        let divided = stepped.sample(motion, at(200), false);

        assert!((end.target().x_px() - divided.target().x_px()).abs() < 0.0001);
        assert!((end.velocity()[0] - divided.velocity()[0]).abs() < 1e-8);
    }
}

#[test]
fn backward_clock_does_not_rewind_and_zero_duration_is_instant() {
    let mut state = MotionState::default();
    let motion = spec(Transition::Tween(at(1000), Easing::Linear));

    state.sample(motion, at(0), false);
    let before = state.sample(motion, at(300), false);

    assert_eq!(
        before.target(),
        state.sample(motion, at(200), false).target()
    );
    let instant = state.sample(
        motion.transition(Transition::Tween(Duration::ZERO, Easing::Linear)),
        at(400),
        false,
    );

    assert_eq!(instant.target().x_px(), 100.0);
    assert!(!instant.is_animating());
}

#[test]
fn invalid_springs_are_recoverable_errors() {
    for value in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(Spring::new(value, 26.0, 1.0).is_err());
        assert!(Spring::new(240.0, value, 1.0).is_err());
        assert!(Spring::new(240.0, 26.0, value).is_err());
    }
}

#[test]
fn overshoot_never_produces_invalid_opacity() {
    let motion = Motion::new(Target::default())
        .initial(Target::default().opacity(0.0))
        .transition(Transition::Spring(Spring::new(200.0, 1.0, 1.0).unwrap()));

    let mut state = MotionState::default();

    for time in (0..5000).step_by(8) {
        assert!((0.0..=1.0).contains(&state.sample(motion, at(time), false).target().alpha()));
    }
}

#[test]
fn critical_spring_matches_closed_form_solution() {
    let mut state = MotionState::default();
    let motion = spec(Transition::Spring(Spring::new(100.0, 20.0, 1.0).unwrap()));

    state.sample(motion, at(0), false);
    let frame = state.sample(motion, at(100), false);
    let expected = 100.0 * (1.0 - 2.0 * (-1.0_f64).exp());

    assert!((f64::from(frame.target().x_px()) - expected).abs() < 0.00001);
    assert!((frame.velocity()[0] - 1000.0 * (-1.0_f64).exp()).abs() < 1e-8);
}

#[test]
fn changing_transition_preserves_position_then_uses_the_new_transition() {
    let mut state = MotionState::default();
    let spring = spec(Transition::default());

    state.sample(spring, at(0), false);
    let current = state.sample(spring, at(80), false);
    let tween = spring.transition(Transition::Tween(at(500), Easing::Linear));

    assert_eq!(
        state.sample(tween, at(80), false).target(),
        current.target()
    );
    assert_eq!(state.sample(tween, at(580), false).target().x_px(), 100.0);
}

#[cfg(feature = "gpui")]
#[test]
fn native_style_adapter_accepts_stateful_elements() {
    use gpui::{div, prelude::*};
    let mut state = MotionState::default();
    let frame = state.sample(spec(Transition::Instant), at(0), false);
    // Builds style only: this consumer test does not exercise a window or GPU.
    let _element = frame.apply(div().id("stable-panel").child("Panel"));
}

#[test]
fn extreme_finite_targets_stay_finite_when_springs_overshoot() {
    let mut state = MotionState::default();

    let motion = Motion::new(Target::default().x(f32::MAX).y(f32::MIN))
        .initial(Target::default())
        .transition(Transition::Spring(Spring::new(100.0, 1.0, 1.0).unwrap()));

    state.sample(motion, at(0), false);
    let overshoot = state.sample(motion, at(300), false);

    assert!(overshoot.target().x_px().is_finite());
    assert!(overshoot.target().y_px().is_finite());
    assert_eq!(overshoot.target().x_px(), f32::MAX);
    assert_eq!(overshoot.target().y_px(), f32::MIN);
}

#[test]
fn layout_properties_are_optional_and_interpolate_numeric_endpoints() {
    let base = Target::default();

    assert_eq!(base.width_px(), None);
    assert_eq!(base.height_px(), None);
    assert_eq!(base.border_radius_px(), None);
    let motion = Motion::new(base.width(300.0).height(160.0).border_radius(24.0))
        .initial(base.width(0.0).height(0.0).border_radius(0.0))
        .transition(Transition::Tween(at(1000), Easing::Linear));

    let mut state = MotionState::default();

    state.sample(motion, at(0), false);
    let halfway = state.sample(motion, at(500), false);

    assert_eq!(halfway.target().width_px(), Some(150.0));
    assert_eq!(halfway.target().height_px(), Some(80.0));
    assert_eq!(halfway.target().border_radius_px(), Some(12.0));
    let done = state.sample(motion, at(1000), false);

    assert_eq!(done.target().width_px(), Some(300.0));
    assert_eq!(done.target().height_px(), Some(160.0));
    assert_eq!(done.target().border_radius_px(), Some(24.0));
    assert!(!done.is_animating());
}

#[test]
fn unspecified_initial_size_inherits_destination_instead_of_animating_from_zero() {
    let motion = Motion::new(Target::default().width(240.0).height(160.0))
        .initial(Target::default().opacity(0.0));

    let mut state = MotionState::default();
    let first = state.sample(motion, at(0), false);

    assert_eq!(first.target().width_px(), Some(240.0));
    assert_eq!(first.target().height_px(), Some(160.0));
    assert_eq!(first.layout_velocity(), [Some(0.0), Some(0.0), None]);
    assert_eq!(first.target().alpha(), 0.0);
}

#[test]
fn newly_introduced_layout_channel_snaps_without_resetting_spring_momentum() {
    let mut state = MotionState::default();
    let motion = spec(Transition::default());

    state.sample(motion, at(0), false);
    let before = state.sample(motion, at(80), false);
    let sized = Motion::new(Target::default().x(100.0).width(240.0));
    let after = state.sample(sized, at(80), false);

    assert_eq!(after.target().width_px(), Some(240.0));
    assert_eq!(after.target().x_px(), before.target().x_px());
    assert_eq!(after.velocity(), before.velocity());
    assert_eq!(after.layout_velocity()[0], Some(0.0));
    assert_eq!(
        state.sample(sized, at(160), false).target().width_px(),
        Some(240.0)
    );
}

#[test]
fn layout_springs_retarget_from_current_position_and_velocity() {
    let motion = Motion::new(
        Target::default()
            .width(300.0)
            .height(200.0)
            .border_radius(40.0),
    )
    .initial(
        Target::default()
            .width(100.0)
            .height(80.0)
            .border_radius(4.0),
    );

    let reverse = Motion::new(
        Target::default()
            .width(80.0)
            .height(40.0)
            .border_radius(0.0),
    );

    let mut state = MotionState::default();

    state.sample(motion, at(0), false);
    let before = state.sample(motion, at(80), false);
    let after = state.sample(reverse, at(80), false);

    assert_eq!(before.target(), after.target());
    assert_eq!(before.layout_velocity(), after.layout_velocity());
    assert!(
        state
            .sample(reverse, at(81), false)
            .target()
            .width_px()
            .unwrap()
            > after.target().width_px().unwrap()
    );
    let done = state.sample(reverse, at(5000), false);

    assert_eq!(done.target().width_px(), Some(80.0));
    assert_eq!(done.layout_velocity(), [Some(0.0); 3]);
    assert!(!done.is_animating());
}

#[test]
fn omitted_layout_targets_and_partial_exits_retain_previous_destination() {
    let mut state = MotionState::default();

    let sized = Motion::new(
        Target::default()
            .width(300.0)
            .height(200.0)
            .border_radius(16.0),
    )
    .initial(Target::default().width(0.0).height(0.0).border_radius(0.0));

    state.sample(sized, at(0), false);
    let before = state.sample(sized, at(80), false);
    let partial = Motion::new(Target::default()).exit(Target::default().opacity(0.0));
    let after = state.sample(partial, at(80), false);

    assert_eq!(before.target(), after.target());
    assert_eq!(before.layout_velocity(), after.layout_velocity());
    let exiting = state.sample(partial.present(false), at(100), false);

    assert_eq!(exiting.presence(), Presence::Exiting);
    assert!(exiting.target().width_px().unwrap() > after.target().width_px().unwrap());
    let absent = state.sample(partial.present(false), at(5000), false);

    assert_eq!(absent.target().width_px(), Some(300.0));
    assert_eq!(absent.target().height_px(), Some(200.0));
    assert_eq!(absent.target().border_radius_px(), Some(16.0));
    assert!(!absent.is_mounted());
}

#[test]
fn reduced_motion_and_reentry_apply_all_layout_channels() {
    let motion = Motion::new(
        Target::default()
            .width(300.0)
            .height(200.0)
            .border_radius(40.0),
    )
    .initial(Target::default().width(0.0).height(0.0).border_radius(0.0))
    .exit(Target::default().width(0.0).height(0.0).border_radius(0.0));

    let mut state = MotionState::default();

    state.sample(motion, at(0), false);
    state.sample(motion, at(5000), false);
    state.sample(motion.present(false), at(5000), false);
    let exit = state.sample(motion.present(false), at(5080), false);
    let entry = state.sample(motion, at(5080), false);

    assert_eq!(exit.target(), entry.target());
    assert_eq!(exit.layout_velocity(), entry.layout_velocity());
    let snapped = state.sample(motion, at(5090), true);

    assert_eq!(snapped.target().width_px(), Some(300.0));
    assert!(!snapped.is_animating());
    let removed = state.sample(motion.present(false), at(5100), true);

    assert_eq!(removed.target().height_px(), Some(0.0));
    assert!(!removed.is_mounted());
}

#[test]
fn negative_spring_overshoot_is_clamped_for_layout_without_losing_velocity() {
    let spring = Transition::Spring(Spring::new(100.0, 1.0, 1.0).unwrap());

    let motion = Motion::new(Target::default().width(0.0).height(0.0).border_radius(0.0))
        .initial(
            Target::default()
                .width(100.0)
                .height(100.0)
                .border_radius(10.0),
        )
        .transition(spring);

    let mut state = MotionState::default();

    state.sample(motion, at(0), false);
    let undershoot = state.sample(motion, at(300), false);

    assert_eq!(undershoot.target().width_px(), Some(0.0));
    assert_eq!(undershoot.target().height_px(), Some(0.0));
    assert_eq!(undershoot.target().border_radius_px(), Some(0.0));
    assert!(undershoot.layout_velocity()[0].unwrap() < 0.0);
    let reverse = Motion::new(
        Target::default()
            .width(100.0)
            .height(100.0)
            .border_radius(10.0),
    )
    .transition(spring);

    assert_eq!(
        state.sample(reverse, at(300), false).layout_velocity(),
        undershoot.layout_velocity()
    );
}

#[test]
fn layout_builders_reject_invalid_values_and_accept_zero() {
    for value in [-1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(std::panic::catch_unwind(|| Target::default().width(value)).is_err());
        assert!(std::panic::catch_unwind(|| Target::default().height(value)).is_err());
        assert!(std::panic::catch_unwind(|| Target::default().border_radius(value)).is_err());
    }

    assert_eq!(Target::default().width(0.0).width_px(), Some(0.0));
}

#[cfg(feature = "gpui")]
#[test]
fn native_adapter_preserves_omitted_styles_and_applies_declared_layout_values() {
    use gpui::{div, prelude::*, px};
    let mut state = MotionState::default();
    let unchanged = state.sample(Motion::new(Target::default()), at(0), false);
    let mut element = unchanged.apply(div().w(px(123.0)).h(px(45.0)).rounded(px(7.0)));

    assert_eq!(element.style().size.width, Some(px(123.0).into()));
    assert_eq!(element.style().size.height, Some(px(45.0).into()));
    assert_eq!(element.style().corner_radii.top_left, Some(px(7.0).into()));
    let sized = state.sample(
        Motion::new(
            Target::default()
                .width(222.0)
                .height(111.0)
                .border_radius(12.0),
        )
        .transition(Transition::Instant),
        at(1),
        false,
    );

    let mut element = sized.apply(element);

    assert_eq!(element.style().size.width, Some(px(222.0).into()));
    assert_eq!(element.style().size.height, Some(px(111.0).into()));
    assert_eq!(element.style().corner_radii.top_left, Some(px(12.0).into()));
    assert_eq!(
        element.style().corner_radii.bottom_right,
        Some(px(12.0).into())
    );
}
