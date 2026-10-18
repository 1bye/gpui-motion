//! Covers: prepared playback, interaction and keyed lifecycle public contracts.
//! Real: motion engine and deterministic injected clocks. Doubles: none. Requires: none.
use gpui_ce_motion::*;
use std::time::Duration;
fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
fn x(x: f32) -> Target {
    Target::default().x(x)
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 0.0001, "{a} != {b}");
}
fn clip() -> Animation {
    Animation::to(x(100.), ms(1000), Easing::Linear)
}

#[test]
fn keyframes_nonuniform_holds_and_validation() {
    let clip = Animation::keyframes(
        ms(1000),
        [
            Keyframe::current(0.),
            Keyframe::new(0.25, x(100.)).easing(Easing::Linear),
            Keyframe::current(0.75),
            Keyframe::new(1., x(200.)).easing(Easing::Linear),
        ],
    )
    .unwrap();

    let mut state = AnimationState::new(x(0.));

    state.sample(&clip, ms(0), false);
    close(
        state.sample(&clip, ms(125), false).target().x_px() as f64,
        50.,
    );
    close(
        state.sample(&clip, ms(500), false).target().x_px() as f64,
        100.,
    );
    close(
        state.sample(&clip, ms(875), false).target().x_px() as f64,
        150.,
    );
    assert!(state.sample(&clip, ms(1000), false).just_completed());
    assert!(
        !state
            .sample(&clip.clone(), ms(1100), false)
            .just_completed()
    );
    assert!(Animation::keyframes(ms(1), [Keyframe::current(0.)]).is_err());
    assert!(
        Animation::keyframes(ms(1), [Keyframe::current(0.), Keyframe::current(f64::NAN)]).is_err()
    );
}
#[test]
fn repeat_reverse_and_mirror_have_different_easing_and_exact_endpoints() {
    for (mode, expected) in [
        (RepeatMode::Loop, 1.5625),
        (RepeatMode::Reverse, 42.1875),
        (RepeatMode::Mirror, 98.4375),
    ] {
        let a = Animation::to(x(100.), ms(1000), Easing::EaseIn)
            .delay(ms(100))
            .repeat(Repeat::Count(1), mode, ms(200))
            .unwrap();

        let mut s = AnimationState::new(x(0.));

        s.sample(&a, ms(0), false);
        close(s.sample(&a, ms(50), false).target().x_px() as f64, 0.);
        close(s.sample(&a, ms(1200), false).target().x_px() as f64, 100.);
        close(
            s.sample(&a, ms(1550), false).target().x_px() as f64,
            expected,
        );
        let end = s.sample(&a, ms(2300), false);

        close(
            end.target().x_px() as f64,
            if mode == RepeatMode::Loop { 100. } else { 0. },
        );
        assert!(end.just_completed());
    }

    assert!(
        Animation::to(x(1.), ms(0), Easing::Linear)
            .repeat(Repeat::Forever, RepeatMode::Loop, ms(0))
            .is_err()
    );
}
#[test]
fn controls_seek_pause_speed_backward_clocks_and_completion_edges() {
    let a = clip();
    let mut s = AnimationState::new(x(0.));

    s.sample(&a, ms(100), false);
    close(s.sample(&a, ms(350), false).target().x_px() as f64, 25.);
    close(s.sample(&a, ms(200), false).target().x_px() as f64, 25.);
    s.pause(ms(350));
    let held = s.sample(&a, ms(500), false);

    close(held.velocity()[0], 0.);
    assert!(!held.is_animating());
    s.seek(ms(750), ms(500));
    close(s.sample(&a, ms(500), false).target().x_px() as f64, 75.);
    s.set_speed(-2., ms(500)).unwrap();
    s.play(ms(500));
    let back = s.sample(&a, ms(625), false);

    close(back.target().x_px() as f64, 50.);
    close(back.velocity()[0], -200.);
    assert!(s.sample(&a, ms(1000), false).just_completed());
    assert!(!s.sample(&a, ms(1100), false).just_completed());
    assert!(s.set_speed(f64::NAN, ms(1100)).is_err());
}
#[test]
fn controls_before_binding_and_infinite_reduced_motion_do_not_restart() {
    let a = clip()
        .repeat(Repeat::Forever, RepeatMode::Reverse, ms(0))
        .unwrap();

    let mut s = AnimationState::new(x(0.));

    s.seek(ms(500), ms(0));
    s.pause(ms(0));
    close(s.sample(&a, ms(200), false).target().x_px() as f64, 50.);
    assert!(!s.sample(&a, ms(300), false).is_animating());
    let end = s.sample(&a, ms(400), true);

    assert!(end.just_completed());
    assert_eq!(end.target().x_px(), 100.);
    assert!(!s.sample(&a, ms(500), false).is_animating());
    s.restart(ms(500));
    s.play(ms(500));
    assert!(s.sample(&a, ms(600), false).is_animating());
}
#[test]
fn spring_clip_interruption_preserves_position_and_velocity() {
    let a = Animation::spring(x(200.), Spring::default(), ms(2000));
    let b = Animation::spring(x(-50.), Spring::default(), ms(2000));
    let mut s = AnimationState::new(x(0.));

    s.sample(&a, ms(0), false);
    let before = s.sample(&a, ms(100), false);
    let after = s.sample(&b, ms(100), false);

    assert_eq!(before.target().x_px(), after.target().x_px());
    close(before.velocity()[0], after.velocity()[0]);
    assert_eq!(s.sample(&b, ms(2100), false).target().x_px(), -50.);
}
#[test]
fn timeline_overlap_labels_stagger_seek_and_pause() {
    let mut t = Timeline::default();

    t.label("start").unwrap();
    t.then(clip()).unwrap();
    t.at_label("start", ms(500), clip()).unwrap();
    t.restart(ms(100));
    let f = t.sample(ms(850), false);

    close(f[0].target().x_px() as f64, 75.);
    close(f[1].target().x_px() as f64, 25.);
    t.pause(ms(850));
    let f = t.sample(ms(950), false);

    close(f[0].velocity()[0], 0.);
    assert!(!t.is_animating());
    t.seek(ms(100), ms(950));
    let f = t.sample(ms(950), false);

    close(f[0].target().x_px() as f64, 10.);
    close(f[1].target().x_px() as f64, 0.);
    assert_eq!(
        stagger(ms(100), 0, 4, StaggerFrom::Center).unwrap(),
        ms(150)
    );
    assert!(stagger(ms(1), 4, 4, StaggerFrom::Start).is_err());
    assert!(t.at_label("missing", ms(0), clip()).is_err());
}
#[test]
fn independent_property_delay_does_not_finish_presence_early() {
    let spec = Motion::new(x(100.).opacity(0.))
        .initial(x(0.))
        .transition(Transition::Tween(ms(100), Easing::Linear))
        .transition_for(
            Property::Opacity,
            Transition::Tween(ms(200), Easing::Linear),
            ms(100),
        );

    let mut s = MotionState::default();

    s.sample(spec, ms(0), false);
    let f = s.sample(spec, ms(100), false);

    assert_eq!(f.target().x_px(), 100.);
    assert!(f.is_animating());
    assert!(!f.just_completed());
    assert!(s.sample(spec, ms(300), false).just_completed());
}
#[test]
fn scalar_velocity_inertia_bounds_and_frame_rate_independence() {
    let mut v = MotionValue::new(0.);

    v.jump(0., ms(0));
    v.sample(ms(50), false);
    v.set(10., ms(100));
    close(v.velocity(), 100.);
    v.coast(Inertia::default().bounds(0., 20.), ms(100));
    let mut fine = v.clone();

    for i in 101..=800 {
        fine.sample(ms(i), false);
    }

    let a = v.sample(ms(800), false);
    let b = fine.sample(ms(800), false);

    close(a.value(), b.value());
    close(a.velocity(), b.velocity());
    close(v.sample(ms(10000), false).value(), 20.);
    v.animate(-10., Transition::default(), ms(10000));
    assert_eq!(v.sample(ms(10001), true).value(), -10.);
    let mut extreme = MotionValue::new(-f64::MAX);

    extreme.animate(f64::MAX, Transition::Tween(ms(1000), Easing::Linear), ms(0));
    assert!(extreme.sample(ms(500), false).value().is_finite());
}
#[test]
fn drag_bounds_momentum_snap_cancel_keyboard_and_regrab() {
    let mut d = Drag::new([0., 0.], [0., 0.], [200., 100.])
        .elasticity(0.)
        .snap_grid(25.)
        .momentum(false);

    d.begin([10., 10.], ms(0));
    d.move_to([83., 30.], ms(100));
    assert_eq!(d.sample(ms(100), false).target().x_px(), 73.);
    d.end(ms(100));
    assert_eq!(d.sample(ms(10000), false).target().x_px(), 75.);
    d.begin([0., 0.], ms(10000));
    d.move_to([500., 0.], ms(10100));
    assert_eq!(d.sample(ms(10100), false).target().x_px(), 200.);
    d.cancel(ms(10100));
    assert_eq!(d.sample(ms(20000), false).target().x_px(), 75.);
    d.move_by([-500., 0.], ms(20000));
    assert_eq!(d.sample(ms(20001), true).target().x_px(), 0.);
}
#[test]
fn variants_partial_gestures_scroll_visibility_and_reordering() {
    let variants = Variants::new([("on", Motion::new(x(80.).width(100.)))]).unwrap();
    let base = variants.get(&"on").unwrap();
    let mut s = MotionState::default();
    let target = s.sample(base, ms(0), true).target();

    let gestures = GestureTargets::default()
        .while_hover(Target::default().opacity(0.5))
        .while_press(x(40.));

    let mut i = Interaction::default();

    i.hovered = true;
    i.pressed = true;
    let out = gestures.resolve(target, i);

    assert_eq!(out.x_px(), 40.);
    assert_eq!(out.width_px(), Some(100.));
    assert_eq!(out.alpha(), 0.5);
    close(scroll_progress(50., 200., 100.), 0.5);
    close(scroll_progress(10., 50., 100.), 0.);
    close(transform(0.25, [0., 1.], [100., 0.], true), 75.);
    let mut view = InView::new(0.5, true);

    assert!(!view.update([0., 0., 100., 100.], [0., 80., 100., 100.]));
    assert!(view.update([0., 0., 100., 100.], [0., 50., 100., 100.]));
    assert!(!view.update([0., 0., 100., 100.], [0., 200., 100., 100.]));
    assert!(view.is_visible());
    assert_eq!(reorder_index(&[10., 30., 50.], 44.).unwrap(), 2);
    let mut items = ["a", "b", "c"];

    move_item(&mut items, 0, 2).unwrap();
    assert_eq!(items, ["b", "c", "a"]);
}
#[test]
fn linear_light_color_and_color_interruption() {
    let black = Color::rgb(0);
    let white = Color::rgb(0xffffff);

    close(black.mix(white, 0.5).channels()[0] as f64, 0.7353569);
    let mut s = ColorState::new(black);

    s.sample(white, Transition::default(), ms(0), false);
    let before = s.sample(white, Transition::default(), ms(100), false).0;
    let after = s.sample(black, Transition::default(), ms(100), false).0;

    assert_eq!(before.channels(), after.channels());
    assert_eq!(
        s.sample(white, Transition::default(), ms(101), true)
            .0
            .channels(),
        white.channels()
    );
}
#[test]
fn keyed_presence_wait_reentry_and_transactional_duplicate_validation() {
    let mut group = PresenceGroup::new(PresenceMode::Wait);

    group.update([1, 2]).unwrap();
    group.update([2, 3]).unwrap();
    assert_eq!(group.children().len(), 2);
    assert!(!group.children()[0].is_present());
    assert!(group.update([3, 3]).is_err());
    assert_eq!(*group.children()[0].key(), 1);
    group.update([1, 3]).unwrap();
    assert!(group.children()[0].is_present());
    assert!(!group.finish_exit(&1));
    assert!(group.finish_exit(&2));
    assert_eq!(*group.children()[1].key(), 3);
}

#[test]
fn mirror_with_nonuniform_segments_and_huge_repeat_skips() {
    let clip = Animation::keyframes(
        ms(1000),
        [
            Keyframe::new(0., x(0.)),
            Keyframe::new(0.25, x(100.)).easing(Easing::Linear),
            Keyframe::new(1., x(200.)).easing(Easing::Linear),
        ],
    )
    .unwrap()
    .repeat(Repeat::Forever, RepeatMode::Mirror, ms(0))
    .unwrap();

    let mut s = AnimationState::default();

    s.sample(&clip, ms(0), false);
    close(
        s.sample(&clip, ms(1375), false).target().x_px() as f64,
        150.,
    );
    let frame = s.sample(&clip, Duration::from_secs(1_000_000_001), false);

    assert_eq!(s.iteration(), 1_000_000_001);
    assert_eq!(frame.target().x_px(), 200.);
}
#[test]
fn timeline_backward_end_is_idle_and_speed_scales_velocity() {
    let mut t = Timeline::default();

    t.at(ms(0), clip());
    t.restart(ms(0));
    t.seek(ms(500), ms(0));
    t.set_speed(-2., ms(0)).unwrap();
    close(t.sample(ms(100), false)[0].velocity()[0], -200.);
    t.sample(ms(500), false);
    assert!(!t.is_animating());
}
#[test]
fn bezier_validation_and_linear_curve_are_observable_through_playback() {
    assert!(Bezier::new(-1., 0., 1., 1.).is_err());
    assert!(Bezier::new(0., f64::NAN, 1., 1.).is_err());
    let clip = Animation::to(
        x(100.),
        ms(1000),
        Easing::Bezier(Bezier::new(0.25, 0.25, 0.75, 0.75).unwrap()),
    );

    let mut s = AnimationState::default();

    s.sample(&clip, ms(0), false);
    close(s.sample(&clip, ms(250), false).target().x_px() as f64, 25.);
}

#[test]
fn loss_of_capture_after_release_does_not_undo_finished_drag() {
    let mut d = Drag::new([0., 0.], [0., 0.], [200., 100.]).momentum(false);

    d.begin([0., 0.], ms(0));
    d.move_to([100., 20.], ms(100));
    d.end(ms(100));
    let before = d.sample(ms(10000), false);

    d.cancel(ms(10000));
    let after = d.sample(ms(20000), false);

    assert_eq!(before.target(), after.target());
}

#[test]
fn holding_before_release_does_not_launch_stale_momentum() {
    let mut d = Drag::new([0., 0.], [0., 0.], [200., 100.]);

    d.begin([0., 0.], ms(0));
    d.move_to([50., 20.], ms(100));
    d.end(ms(500));
    let frame = d.sample(ms(5000), false);

    assert_eq!(frame.target().x_px(), 50.);
    assert_eq!(frame.target().y_px(), 20.);
}

#[test]
fn finish_before_first_bind_is_honored() {
    let clip = clip();
    let mut state = AnimationState::default();

    state.finish();
    let frame = state.sample(&clip, ms(100), false);

    assert_eq!(frame.target().x_px(), 100.);
    assert!(frame.just_completed());
    assert!(!frame.is_animating());
}
