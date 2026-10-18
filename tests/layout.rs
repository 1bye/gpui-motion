//! Covers: measured layout positions, interruption, timing, reduced motion and reset.
//! Real: portable layout sampler and scalar engine. Doubles: none. Requires: none.
use gpui_ce_motion::{Easing, LayoutPositionState, Transition};
use std::time::Duration;
fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
#[test]
fn first_measurement_snaps_and_later_measurements_return_compensating_offsets() {
    let mut state = LayoutPositionState::default();
    let first = state.sample([40., 20.], Transition::default(), ms(0), false);

    assert_eq!(first.position(), [40., 20.]);
    assert_eq!(first.offset(), [0., 0.]);
    assert!(!first.is_animating());
    let moved = state.sample([140., 60.], Transition::default(), ms(10), false);

    assert_eq!(moved.layout_origin(), [140., 60.]);
    assert_eq!(moved.position(), first.position());
    assert_eq!(moved.offset(), [-100., -40.]);
    assert!(moved.is_animating());
    let end = state.sample([140., 60.], Transition::default(), ms(5000), false);

    assert_eq!(end.offset(), [0., 0.]);
    assert!(end.just_completed());
    assert!(!end.is_animating());
    assert!(
        !state
            .sample([140., 60.], Transition::default(), ms(5001), false)
            .just_completed()
    );
}
#[test]
fn midflight_relayout_preserves_presented_position_and_spring_velocity() {
    let mut state = LayoutPositionState::default();

    state.sample([0., 0.], Transition::default(), ms(0), false);
    state.sample([200., 80.], Transition::default(), ms(0), false);
    let before = state.sample([200., 80.], Transition::default(), ms(80), false);
    let after = state.sample([-100., -40.], Transition::default(), ms(80), false);

    assert_eq!(before.position(), after.position());
    assert_eq!(before.velocity(), after.velocity());
    assert_eq!(
        after.offset(),
        [after.position()[0] + 100., after.position()[1] + 40.]
    );
    assert!(
        state
            .sample([-100., -40.], Transition::default(), ms(81), false)
            .position()[0]
            > after.position()[0]
    );
}
#[test]
fn retargeting_between_samples_advances_the_previous_trajectory() {
    let mut coarse = LayoutPositionState::default();
    let mut fine = LayoutPositionState::default();

    for state in [&mut coarse, &mut fine] {
        state.sample([0., 0.], Transition::default(), ms(0), false);
        state.sample([100., 50.], Transition::default(), ms(0), false);
    }

    for n in 1..=137 {
        fine.sample([100., 50.], Transition::default(), ms(n), false);
    }

    let a = coarse.sample([-10., 20.], Transition::default(), ms(137), false);
    let b = fine.sample([-10., 20.], Transition::default(), ms(137), false);

    assert_eq!(a.position(), b.position());
    assert_eq!(a.velocity(), b.velocity());
}
#[test]
fn tween_retargeting_and_backward_timestamps_preserve_position() {
    let transition = Transition::Tween(ms(1000), Easing::Linear);
    let mut state = LayoutPositionState::default();

    state.sample([0., 0.], transition, ms(0), false);
    state.sample([100., 200.], transition, ms(0), false);
    let half = state.sample([100., 200.], transition, ms(500), false);

    assert_eq!(half.position(), [50., 100.]);
    assert_eq!(
        state
            .sample([100., 200.], transition, ms(100), false)
            .position(),
        half.position()
    );
    assert_eq!(
        state
            .sample([-50., 0.], transition, ms(500), false)
            .position(),
        half.position()
    );
    assert_eq!(
        state
            .sample([-50., 0.], transition, ms(1500), false)
            .offset(),
        [0., 0.]
    );
}
#[test]
fn reduced_motion_and_instant_transitions_stop_without_replaying() {
    for transition in [Transition::default(), Transition::Instant] {
        let mut state = LayoutPositionState::default();

        state.sample([0., 0.], transition, ms(0), false);
        let end = state.sample([100., 100.], transition, ms(10), true);

        assert_eq!(end.offset(), [0., 0.]);
        assert_eq!(end.velocity(), [0., 0.]);
        assert!(!end.is_animating());
        assert!(
            !state
                .sample([100., 100.], transition, ms(20), false)
                .is_animating()
        );
    }
}
#[test]
fn reset_uses_the_next_position_as_a_fresh_mount() {
    let mut state = LayoutPositionState::default();

    state.sample([0., 0.], Transition::default(), ms(0), false);
    state.sample([100., 0.], Transition::default(), ms(0), false);
    state.reset();
    let frame = state.sample([300., 400.], Transition::default(), ms(1), false);

    assert_eq!(frame.position(), [300., 400.]);
    assert_eq!(frame.offset(), [0., 0.]);
    assert!(!frame.is_animating());
}
#[test]
fn invalid_measured_positions_are_rejected() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(
            std::panic::catch_unwind(|| LayoutPositionState::default().sample(
                [0., bad],
                Transition::default(),
                ms(0),
                false
            ))
            .is_err()
        );
    }
}
