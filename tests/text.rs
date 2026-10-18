//! Covers Unicode segmentation, identity, interruption, stagger, playback, and reduced motion.
//! Real: portable text and spring engines. Doubles: prepared font geometry. Requires: text.
#![cfg(feature = "text")]
use gpui_ce_motion::{
    Animation, Easing, NumberEffect, PresenceMode, Repeat, RepeatMode, Scramble, StaggerFrom,
    Target, TextLayoutUnit, TextMatch, TextOptions, TextPlaybackState, TextPreset, TextTarget,
    TextUnit, Transition, text_ranges,
};
use std::time::Duration;

fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}

fn layout(text: &str) -> Vec<TextLayoutUnit> {
    text_ranges(text, TextUnit::Graphemes)
        .into_iter()
        .enumerate()
        .map(|(index, range)| {
            TextLayoutUnit::new(
                &text[range],
                index,
                [index as f64 * 12., 0.],
                [12., 20.],
                20.,
            )
        })
        .collect()
}

fn options() -> TextOptions {
    TextOptions {
        transition: Transition::Tween(ms(200), Easing::Linear),
        matching: TextMatch::Content,
        ..TextOptions::default()
    }
}

#[test]
fn graphemes_preserve_combining_marks_flags_and_joined_emoji() {
    let text = "e\u{301}👩🏽‍💻🇺🇦";

    let parts: Vec<_> = text_ranges(text, TextUnit::Graphemes)
        .into_iter()
        .map(|range| &text[range])
        .collect();

    assert_eq!(parts, ["e\u{301}", "👩🏽‍💻", "🇺🇦"]);
    let ranges = text_ranges("a\nb\n", TextUnit::Lines);

    assert_eq!(ranges, [0..1, 2..3]);
    assert!(text_ranges("", TextUnit::Whole).is_empty());
}

#[test]
fn stagger_holds_then_finishes_and_stops_frames() {
    let mut state = TextPlaybackState::default();

    let options = TextOptions {
        stagger: ms(100),
        stagger_from: StaggerFrom::Start,
        ..options()
    };

    state.reconcile("ab", &layout("ab"), &options, ms(0), false);
    assert!(state.sample(ms(100), false, None));
    let frames: Vec<_> = state.presentation().copied().collect();

    assert!((frames[0].opacity - 0.5).abs() < 0.001);
    assert_eq!(frames[1].opacity, 0.);
    assert!(!state.sample(ms(301), false, None));
    assert!(state.presentation().all(|frame| frame.opacity == 1.));
}

#[test]
fn repeated_content_has_unique_stable_identities_and_reclaims_exits() {
    let mut state = TextPlaybackState::default();
    let first = state.reconcile("aba", &layout("aba"), &options(), ms(0), true);
    let second = state.reconcile("baa", &layout("baa"), &options(), ms(0), false);

    assert_eq!(second, [first[1], first[0], first[2]]);
    state.reconcile("b", &layout("b"), &options(), ms(30), false);
    let restored = state.reconcile("baa", &layout("baa"), &options(), ms(50), false);

    assert_eq!(restored, second);
    assert_eq!(state.presentation().count(), 3);
}

#[test]
fn spring_reflow_reverses_without_a_position_jump() {
    let mut state = TextPlaybackState::default();

    let options = TextOptions {
        reflow: Some(Transition::default()),
        animate_initial: false,
        ..options()
    };

    let mut units = layout("a");

    state.reconcile("a", &units, &options, ms(0), false);
    units[0].origin[0] = 100.;
    state.reconcile("a", &units, &options, ms(0), false);
    state.sample(ms(80), false, None);
    let before = state.presentation().next().unwrap().origin[0];

    assert!(before > 0. && before < 100.);
    units[0].origin[0] = -40.;
    state.reconcile("a", &units, &options, ms(80), false);
    assert!((state.presentation().next().unwrap().origin[0] - before).abs() < 1e-6);
    state.sample(ms(5000), false, None);
    assert!((state.presentation().next().unwrap().origin[0] + 40.).abs() < 0.01);
}

#[test]
fn exit_and_reentry_preserve_the_displayed_pose() {
    let mut state = TextPlaybackState::default();

    let options = TextOptions {
        enter: TextTarget::default().y_em(1.).opacity(0.),
        exit: TextTarget::default().y_em(-1.).opacity(0.),
        ..options()
    };

    let id = state.reconcile("a", &layout("a"), &options, ms(0), true)[0];

    state.reconcile("", &[], &options, ms(100), false);
    state.sample(ms(170), false, None);
    let before = *state.presentation().next().unwrap();

    state.reconcile("a", &layout("a"), &options, ms(170), false);
    let after = state.presentation().next().unwrap();

    assert_eq!(after.id, id);
    assert_eq!(after.origin, before.origin);
    assert_eq!(after.opacity, before.opacity);
    assert!(!after.exiting);
}

#[test]
fn waiting_replacements_keep_only_the_latest_pending_text() {
    let mut state = TextPlaybackState::default();

    let options = TextOptions {
        matching: TextMatch::Replace,
        presence_mode: PresenceMode::Wait,
        ..options()
    };

    state.reconcile("a", &layout("a"), &options, ms(0), true);
    state.reconcile("b", &layout("b"), &options, ms(10), false);
    assert!(state.presentation().any(|frame| !frame.visible));
    state.reconcile("c", &layout("c"), &options, ms(50), false);
    state.sample(ms(211), false, None);
    assert_eq!(state.text(), "c");
    assert_eq!(state.presentation().count(), 1);
    assert!(
        state
            .presentation()
            .all(|frame| frame.visible && !frame.exiting)
    );
    assert!(!state.sample(ms(500), false, None));
}

#[test]
fn reduced_motion_removes_exits_and_finishes_even_repeating_clips() {
    let mut state = TextPlaybackState::default();

    let clip = Animation::to(Target::default().y(5.), ms(400), Easing::Linear)
        .repeat(Repeat::Forever, RepeatMode::Mirror, Duration::ZERO)
        .unwrap();

    let options = TextOptions {
        animation: Some(clip),
        scramble: Some(Scramble::new("ABC")),
        ..options()
    };

    state.reconcile("a", &layout("a"), &options, ms(0), false);
    assert!(state.sample(ms(20), false, None));
    assert!(!state.sample(ms(30), true, None));
    assert!(
        state
            .presentation()
            .all(|frame| frame.opacity == 1. && frame.substitution.is_none())
    );
    state.reconcile("", &[], &options, ms(40), true);
    assert_eq!(state.presentation().count(), 0);
}

#[test]
fn pause_seek_resume_and_restart_have_observable_playheads() {
    let mut state = TextPlaybackState::default();

    state.reconcile("a", &layout("a"), &options(), ms(0), false);
    state.pause(ms(80));
    let held = state.presentation().next().unwrap().opacity;

    assert!(!state.sample(ms(1000), false, None));
    assert_eq!(state.presentation().next().unwrap().opacity, held);
    assert!(state.seek(0.75, ms(1000)));
    assert!((state.presentation().next().unwrap().opacity - 0.75).abs() < 0.001);
    state.play(ms(1000));
    state.sample(ms(1500), false, None);
    assert_eq!(state.presentation().next().unwrap().opacity, 1.);
    state.restart(ms(1500));
    state.sample(ms(1500), false, None);
    assert_eq!(state.presentation().next().unwrap().opacity, 0.);
}

#[test]
fn scrubbing_accounts_for_stagger_and_can_move_backwards() {
    let mut state = TextPlaybackState::default();

    let options = TextOptions {
        stagger: ms(100),
        ..options()
    };

    state.reconcile("ab", &layout("ab"), &options, ms(0), false);
    assert!(!state.sample(ms(0), false, Some(0.5)));
    let values: Vec<_> = state.presentation().map(|frame| frame.opacity).collect();

    assert!((values[0] - 0.75).abs() < 0.001);
    assert!((values[1] - 0.25).abs() < 0.001);
    state.sample(ms(30), false, Some(0.));
    assert!(state.presentation().all(|frame| frame.opacity == 0.));
    state.sample(ms(40), false, Some(1.));
    assert!(state.presentation().all(|frame| frame.opacity == 1.));
}

#[test]
fn scramble_is_deterministic_finite_and_never_changes_semantic_text() {
    let mut a = TextPlaybackState::default();
    let mut b = TextPlaybackState::default();

    let options = TextOptions {
        scramble: Some(Scramble::new("XYZ").seed(42).duration(ms(300))),
        ..options()
    };

    for state in [&mut a, &mut b] {
        state.reconcile("👩🏽‍💻", &layout("👩🏽‍💻"), &options, ms(0), false);
        state.sample(ms(100), false, None);
    }

    assert_eq!(
        a.presentation().next().unwrap().substitution,
        b.presentation().next().unwrap().substitution
    );
    assert!(a.presentation().next().unwrap().substitution.is_some());
    assert_eq!(a.text(), "👩🏽‍💻");
    assert!(!a.sample(ms(400), false, None));
    assert!(a.presentation().next().unwrap().substitution.is_none());
}

#[test]
fn typewriter_reveals_complete_units_at_discrete_times() {
    let mut state = TextPlaybackState::default();
    let mut options = options();

    options.preset(TextPreset::Typewriter);
    options.stagger = ms(50);
    state.reconcile("abc", &layout("abc"), &options, ms(0), false);
    state.sample(ms(51), false, None);
    let opacities: Vec<_> = state.presentation().map(|frame| frame.opacity).collect();

    assert_eq!(opacities, [1., 1., 0.]);
    assert!(!state.sample(ms(101), false, None));
    assert_eq!(NumberEffect::default(), NumberEffect::Roll);
}

#[test]
#[should_panic]
fn non_finite_geometry_is_rejected() {
    TextLayoutUnit::new("a", 0, [f64::NAN, 0.], [1., 1.], 16.);
}

#[test]
fn changing_coordinate_reference_preserves_the_motion_trajectory() {
    let mut reference = TextPlaybackState::default();
    let mut translated = TextPlaybackState::default();

    let options = TextOptions {
        reflow: Some(Transition::default()),
        animate_initial: false,
        ..options()
    };

    let mut units = layout("a");

    for state in [&mut reference, &mut translated] {
        state.reconcile("a", &units, &options, ms(0), false);
    }

    units[0].origin[0] = 100.;

    for state in [&mut reference, &mut translated] {
        state.reconcile("a", &units, &options, ms(0), false);
        state.sample(ms(80), false, None);
    }

    translated.translate([50., -20.]);

    for time in [80, 81, 100, 200, 1000] {
        reference.sample(ms(time), false, None);
        translated.sample(ms(time), false, None);
        let a = reference.presentation().next().unwrap();
        let b = translated.presentation().next().unwrap();

        assert!((b.origin[0] - a.origin[0] - 50.).abs() < 1e-6);
        assert!((b.origin[1] - a.origin[1] + 20.).abs() < 1e-6);
    }
}

#[test]
fn resuming_a_scrubbed_prepared_clip_does_not_apply_its_offset_or_alpha_twice() {
    let mut state = TextPlaybackState::default();

    let animation = Animation::to(
        Target::default().x(100.).opacity(0.5),
        ms(200),
        Easing::Linear,
    );

    let options = TextOptions {
        enter: TextTarget::default(),
        animation: Some(animation),
        ..options()
    };

    state.reconcile("a", &layout("a"), &options, ms(0), false);
    assert!(state.seek(0.5, ms(0)));
    let before = *state.presentation().next().unwrap();

    state.play(ms(0));
    state.sample(ms(0), false, None);
    let after = state.presentation().next().unwrap();

    assert_eq!(before.origin, after.origin);
    assert_eq!(before.opacity, after.opacity);
}

#[test]
fn scrubbed_staggered_clips_resume_from_each_held_cursor() {
    let mut state = TextPlaybackState::default();
    let animation = Animation::to(Target::default().x(100.), ms(200), Easing::Linear);

    let options = TextOptions {
        enter: TextTarget::default(),
        animation: Some(animation),
        stagger: ms(100),
        ..options()
    };

    state.reconcile("ab", &layout("ab"), &options, ms(0), false);
    state.seek(0.5, ms(0));
    let before: Vec<_> = state.presentation().map(|frame| frame.origin[0]).collect();

    state.play(ms(0));
    state.sample(ms(1), false, None);
    let after: Vec<_> = state.presentation().map(|frame| frame.origin[0]).collect();

    assert!(after[0] > before[0] + 0.1);
    assert!(
        after[1] > before[1] + 0.1,
        "a clip already inside its active range must not replay its original stagger hold"
    );
}

#[test]
fn replay_uses_the_latest_declared_stagger() {
    let mut state = TextPlaybackState::default();

    let mut options = TextOptions {
        stagger: ms(100),
        ..options()
    };

    state.reconcile("ab", &layout("ab"), &options, ms(0), false);
    state.sample(ms(500), false, None);
    options.stagger = ms(200);
    state.reconcile("ab", &layout("ab"), &options, ms(500), false);
    state.restart(ms(500));
    state.sample(ms(700), false, None);

    let frames: Vec<_> = state.presentation().copied().collect();

    assert_eq!(frames[0].opacity, 1.);
    assert_eq!(frames[1].opacity, 0.);
}

#[test]
fn wait_release_preserves_the_staggered_scrub_cursor() {
    let mut state = TextPlaybackState::default();

    let options = TextOptions {
        stagger: ms(200),
        presence_mode: PresenceMode::Wait,
        ..options()
    };

    state.reconcile("a", &layout("a"), &options, ms(0), false);
    state.sample(ms(500), false, None);
    state.reconcile("bc", &layout("bc"), &options, ms(500), false);
    state.sample(ms(700), false, Some(0.5));

    let frames: Vec<_> = state.presentation().copied().collect();

    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0].opacity, 1.);
    assert_eq!(frames[1].opacity, 0.);
}

#[test]
fn masked_slides_travel_the_complete_measured_unit_height() {
    let mut state = TextPlaybackState::default();
    let mut options = options();

    options.preset(TextPreset::SlideUp);
    let units = [TextLayoutUnit::new("a", 0, [0., 0.], [12., 36.], 20.)];

    state.reconcile("a", &units, &options, ms(0), false);

    let frame = state.presentation().next().unwrap();

    assert_eq!(frame.origin[1] - frame.clip_origin[1], 36.);
}

#[test]
fn slide_and_wipe_exits_fade_before_positional_playback_finishes() {
    for preset in [
        TextPreset::SlideUp,
        TextPreset::WipeRight,
        TextPreset::WipeLeft,
    ] {
        let mut state = TextPlaybackState::default();
        let mut options = options();

        options.preset(preset);
        state.reconcile("a", &layout("a"), &options, ms(0), false);
        state.sample(ms(250), false, None);
        state.reconcile("b", &layout("b"), &options, ms(250), false);
        state.sample(ms(300), false, None);

        let outgoing = state.presentation().find(|frame| frame.exiting).unwrap();

        assert!(
            outgoing.opacity > 0. && outgoing.opacity < 0.5,
            "{preset:?} needs a short exit fade: {outgoing:?}"
        );

        state.sample(ms(350), false, None);

        let outgoing = state.presentation().find(|frame| frame.exiting).unwrap();

        assert_eq!(
            outgoing.opacity, 0.,
            "{preset:?} must stop painting old fragments"
        );
        assert!(state.is_animating(), "movement keeps its own timing");
    }
}

#[test]
fn relative_targets_and_independent_opacity_scrub_and_resume_together() {
    let units = [TextLayoutUnit::new("a", 0, [0., 0.], [12., 40.], 20.)];
    let mut state = TextPlaybackState::default();
    let mut options = options();

    options.enter = TextTarget::default().y_em(0.5).y_relative(1.).opacity(0.);
    options.opacity_transition = Some(Transition::Tween(ms(100), Easing::Linear));
    state.reconcile("a", &units, &options, ms(0), false);
    state.seek(0.25, ms(0));

    let held = *state.presentation().next().unwrap();

    assert_eq!(held.origin[1], 30.);
    assert_eq!(held.opacity, 0.5);

    state.play(ms(20));
    state.sample(ms(20), false, None);

    let resumed = state.presentation().next().unwrap();

    assert_eq!(resumed.origin[1], held.origin[1]);
    assert_eq!(resumed.opacity, held.opacity);

    state.sample(ms(120), false, None);

    let frame = state.presentation().next().unwrap();

    assert_eq!(frame.origin[1], 15.);
    assert_eq!(frame.opacity, 1.);

    options.enter = options.enter.y_em(0.5);
    let mut other = TextPlaybackState::default();

    other.reconcile("a", &units, &options, ms(0), false);

    assert_eq!(other.presentation().next().unwrap().origin[1], 10.);
}

#[test]
fn reclaiming_a_fading_slide_preserves_its_displayed_pose() {
    let mut state = TextPlaybackState::default();
    let mut options = options();

    options.transition = Transition::default();
    options.preset(TextPreset::SlideUp);
    state.reconcile("a", &layout("a"), &options, ms(0), true);
    state.reconcile("b", &layout("b"), &options, ms(20), false);
    state.sample(ms(60), false, None);

    let held = *state.presentation().find(|frame| frame.exiting).unwrap();

    assert!(held.opacity > 0. && held.opacity < 1.);

    state.reconcile("a", &layout("a"), &options, ms(60), false);

    let reclaimed = state
        .presentation()
        .find(|frame| frame.id == held.id)
        .unwrap();

    assert!(!reclaimed.exiting);
    assert_eq!(reclaimed.origin, held.origin);
    assert_eq!(reclaimed.opacity, held.opacity);

    state.sample(ms(160), false, None);

    assert_eq!(
        state
            .presentation()
            .find(|frame| frame.id == held.id)
            .unwrap()
            .opacity,
        1.
    );
    state.sample(ms(2000), false, None);

    assert!(!state.is_animating());
    assert_eq!(state.presentation().count(), 1);
}
