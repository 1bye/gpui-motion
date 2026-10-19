//! Covers: public keyed batches, interruption, lifecycle, cleanup, and clocks.
//! Real: shared sampler and hash table. Doubles: explicit time. Requires: none.
use gpui_ce_motion::{Easing, Motion, MotionState, Motions, Target, Transition};
use std::time::Duration;

fn at(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

fn moving(x: f32) -> Motion {
    Motion::new(Target::default().x(x).width(200. + x))
        .initial(Target::default().width(200.))
        .exit(Target::default().y(12.).opacity(0.))
}

#[test]
fn batches_match_individual_states_after_reordering_and_interruption() {
    let mut motions = Motions::new();
    let mut individual: [MotionState; 3] = std::array::from_fn(|_| MotionState::default());

    for (time, order, destinations) in [
        (0, [0, 1, 2], [100., 80., 60.]),
        (80, [2, 0, 1], [100., 80., 60.]),
        (80, [1, 2, 0], [-100., 40., 20.]),
        (81, [0, 2, 1], [-100., 40., 20.]),
        (5000, [2, 1, 0], [-100., 40., 20.]),
    ] {
        let frames = motions.sample(
            order.map(|key| (key, moving(destinations[key]))),
            at(time),
            false,
        );

        for (key, frame) in order.into_iter().zip(frames) {
            let expected = individual[key].sample(moving(destinations[key]), at(time), false);

            assert_eq!(frame.target(), expected.target());
            assert_eq!(frame.velocity(), expected.velocity());
            assert_eq!(frame.layout_velocity(), expected.layout_velocity());
            assert_eq!(frame.presence(), expected.presence());
            assert_eq!(frame.is_animating(), expected.is_animating());
            assert_eq!(frame.just_completed(), expected.just_completed());
        }
    }

    assert_eq!(motions.len(), 3);
}

#[test]
fn a_batch_shares_time_but_each_entry_keeps_its_own_timing() {
    let mut motions = Motions::new();
    let quick = moving(100.).transition(Transition::Tween(at(200), Easing::Linear));

    let delayed = moving(100.)
        .transition(Transition::Tween(at(400), Easing::Linear))
        .delay(at(100));

    motions.sample([(0, quick), (1, delayed)], at(0), false);
    let [quick, delayed] = motions.sample([(0, quick), (1, delayed)], at(200), false);

    assert_eq!(quick.target().x_px(), 100.);
    assert!(!quick.is_animating());
    assert_eq!(delayed.target().x_px(), 25.);
    assert!(delayed.is_animating());
}

#[test]
fn omitted_entries_keep_history_without_pausing_time() {
    let mut motions = Motions::new();
    let motion = moving(100.).transition(Transition::Tween(at(1000), Easing::Linear));

    motions.sample_one(0, motion, at(0), false);
    let empty = motions.sample([], at(200), false);

    assert!(empty.is_empty());
    assert_eq!(motions.len(), 1);
    motions.sample_one(1, motion, at(500), false);
    let [first, second] = motions.sample([(0, motion), (1, motion)], at(750), false);

    assert_eq!(first.target().x_px(), 75.);
    assert_eq!(second.target().x_px(), 25.);
}

#[test]
fn backward_time_is_clamped_for_existing_and_new_keys() {
    let mut motions = Motions::new();
    let motion = moving(100.).transition(Transition::Tween(at(1000), Easing::Linear));

    motions.sample_one(0, motion, at(100), false);
    motions.sample_one(1, motion, at(0), false);
    let [first, second] = motions.sample([(0, motion), (1, motion)], at(600), false);

    assert_eq!(first.target(), second.target());
    assert_eq!(first.target().x_px(), 50.);
}

#[test]
fn exit_and_reentry_preserve_momentum_until_explicit_removal() {
    let mut motions = Motions::new();
    let motion = moving(100.);

    motions.sample_one("notice", motion, at(0), false);
    motions.sample_one("notice", motion, at(3000), false);
    motions.sample_one("notice", motion.present(false), at(3000), false);
    let exiting = motions.sample_one("notice", motion.present(false), at(3080), false);
    let reentry = motions.sample_one("notice", motion, at(3080), false);

    assert!(exiting.is_mounted());
    assert_eq!(exiting.target(), reentry.target());
    assert_eq!(exiting.velocity(), reentry.velocity());
    motions.sample_one("notice", motion.present(false), at(3200), false);
    let absent = motions.sample_one("notice", motion.present(false), at(8000), false);

    assert!(!absent.is_mounted());
    assert!(motions.contains_key(&"notice"));
    assert!(motions.remove(&"notice"));
    assert!(!motions.remove(&"notice"));
    assert!(motions.is_empty());
    let fresh = motions.sample_one("notice", motion, at(8000), false);

    assert_eq!(fresh.target().x_px(), 0.);
    assert_eq!(fresh.velocity(), [0.; 3]);
}

#[test]
fn cleanup_cancels_selected_entries_without_resetting_survivors() {
    let mut motions = Motions::with_capacity(100);
    let motion = moving(100.).transition(Transition::Tween(at(1000), Easing::Linear));

    motions.sample([(0, motion), (1, motion), (2, motion)], at(0), false);
    let capacity = motions.capacity();

    motions.remove(&1);
    motions.retain(|key| *key == 2);
    assert_eq!(motions.len(), 1);
    assert_eq!(motions.capacity(), capacity);
    motions.shrink_to_fit();
    assert!(motions.capacity() < capacity);
    let survivor = motions.sample_one(2, motion, at(500), false);

    assert_eq!(survivor.target().x_px(), 50.);
    motions.reserve(50);
    assert!(motions.capacity() >= motions.len() + 50);
    let capacity = motions.capacity();

    motions.clear();
    assert!(motions.is_empty());
    assert_eq!(motions.capacity(), capacity);
    motions.shrink_to_fit();
    assert_eq!(motions.capacity(), 0);
    let fresh = motions.sample_one(2, motion, at(600), false);

    assert_eq!(fresh.target().x_px(), 0.);
}

#[test]
fn reduced_motion_snaps_every_submitted_entry_and_completes_exits() {
    let mut motions = Motions::new();
    let motion = moving(100.);

    motions.sample([(0, motion), (1, motion)], at(0), false);
    let [present, absent] = motions.sample([(0, motion), (1, motion.present(false))], at(80), true);

    assert_eq!(present.target().x_px(), 100.);
    assert!(!present.is_animating());
    assert!(!absent.is_mounted());
    assert!(!absent.is_animating());
    assert_eq!(motions.len(), 2);
}

#[test]
fn duplicate_keys_are_a_programmer_error_and_do_not_poison_later_batches() {
    let mut motions = Motions::new();
    let motion = moving(100.);

    let duplicate = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        motions.sample([(0, motion), (0, motion)], at(0), false);
    }));

    assert!(duplicate.is_err());
    let [first, second] = motions.sample([(0, motion), (1, motion)], at(80), false);

    assert!(first.target().x_px() > 0.);
    assert_eq!(second.target().x_px(), 0.);
    assert_eq!(motions.len(), 2);
}

#[test]
fn keys_need_neither_clone_nor_default_and_collisions_keep_identity() {
    #[derive(PartialEq, Eq)]
    struct Key(u8);

    impl std::hash::Hash for Key {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            state.write_u8(0);
        }
    }

    let mut motions = Motions::<Key>::default();
    let instant = |x| moving(x).transition(Transition::Instant);

    let [first, second] = motions.sample(
        [(Key(0), instant(10.)), (Key(1), instant(20.))],
        at(0),
        false,
    );

    assert_eq!(first.target().x_px(), 10.);
    assert_eq!(second.target().x_px(), 20.);
    assert!(motions.remove(&Key(0)));
    assert!(motions.contains_key(&Key(1)));
}
