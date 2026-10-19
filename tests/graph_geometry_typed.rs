//! Covers: batched dependencies, measured spatial math and mixed typed playback.
//! Real: portable shared engine. Doubles: explicit monotonic clocks. Requires: none.
use gpui_ce_motion::*;
use std::{cell::RefCell, rc::Rc, time::Duration};

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

#[test]
fn graph_batches_dependencies_events_and_owns_subscription_lifetime() {
    let mut graph = MotionGraph::default();
    let x = graph.source(0.0);
    let twice = graph.derive(&[x], |v| v[0] * 2.0).unwrap();
    let sum = graph.derive(&[x, twice], |v| v[0] + v[1]).unwrap();
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();

    let subscription = graph
        .subscribe(sum, move |event| output.borrow_mut().push(event))
        .unwrap();

    graph.set(x, 1.0, ms(10)).unwrap();
    graph.set(x, 2.0, ms(20)).unwrap();
    graph.sample(ms(20), false);
    assert_eq!(graph.get(sum), Ok(6.0));
    assert_eq!(events.borrow().len(), 1);
    assert_eq!(events.borrow()[0].kind, MotionEventKind::Change);
    assert_eq!(graph.set(sum, 2.0, ms(30)), Err(GraphError::Derived));
    drop(subscription);
    graph.set(x, 3.0, ms(30)).unwrap();
    graph.sample(ms(30), false);
    assert_eq!(events.borrow().len(), 1);
    let other = MotionGraph::default();

    assert_eq!(other.get(x), Err(GraphError::Identity));
}

#[test]
fn graph_springs_interrupt_continuously_and_reduced_motion_stops_dependants() {
    let mut graph = MotionGraph::default();
    let x = graph.source(0.0);
    let alpha = graph.derive(&[x], |v| 1.0 - v[0] / 100.0).unwrap();

    graph
        .animate(x, 100.0, Transition::default(), ms(0))
        .unwrap();
    graph.sample(ms(80), false);
    let before = (graph.get(x).unwrap(), graph.velocity(x).unwrap());

    graph
        .animate(x, 0.0, Transition::default(), ms(80))
        .unwrap();
    graph.sample(ms(80), false);
    assert_eq!((graph.get(x).unwrap(), graph.velocity(x).unwrap()), before);
    graph.sample(ms(100), true);
    assert_eq!(graph.get(alpha), Ok(1.0));
    assert!(!graph.is_animating());
    let target = graph
        .target(Target::default(), &[(Property::Opacity, alpha)])
        .unwrap();

    assert_eq!(target.alpha(), 1.0);
}

#[test]
fn intersections_constraints_and_edge_velocity_handle_resize_and_reverse_ranges() {
    let viewport = LayoutRect::new([0.0, 0.0], [100.0, 100.0]).unwrap();
    let target = LayoutRect::new([0.0, 100.0], [100.0, 100.0]).unwrap();

    assert_eq!(ScrollOffsets::default().progress(viewport, target, 1), 0.0);
    let target = LayoutRect::new([0.0, 0.0], [100.0, 100.0]).unwrap();

    assert_eq!(ScrollOffsets::default().progress(viewport, target, 1), 0.5);
    let bounds = DragBounds::within(
        viewport,
        LayoutRect::new([10.0, 20.0], [30.0, 200.0]).unwrap(),
    );

    assert_eq!(bounds.min(), [-10.0, -20.0]);
    assert_eq!(bounds.max(), [60.0, -20.0]);
    let scroll = AutoScroll::default();

    assert!(scroll.velocity(0.0, 0.0, 100.0) < 0.0);
    assert_eq!(scroll.velocity(50.0, 0.0, 100.0), 0.0);
    assert!(scroll.velocity(100.0, 0.0, 100.0) > 0.0);
    assert!(LayoutRect::new([f64::NAN, 0.0], [1.0, 1.0]).is_err());
}

#[test]
fn measured_grid_detection_and_direction_lock_do_not_move_the_other_axis() {
    let rectangles = [[0.0, 0.0], [40.0, 0.0], [0.0, 40.0], [40.0, 40.0]]
        .map(|p| LayoutRect::new(p, [30.0, 30.0]).unwrap());

    assert_eq!(ReorderAxis::Auto.resolve(&rectangles), ReorderAxis::XY);
    assert_eq!(
        ReorderAxis::Auto.destination([55.0, 55.0], &rectangles),
        Some(3)
    );
    let mut drag = Drag::new([0.0, 0.0], [-100.0; 2], [100.0; 2]).direction_lock(5.0);

    drag.begin([0.0; 2], ms(0));
    drag.move_to([2.0, 1.0], ms(1));
    assert_eq!(drag.presentation().0, [0.0; 2]);
    drag.move_to([20.0, 10.0], ms(10));
    assert_eq!(drag.locked_axis(), Some(0));
    assert_eq!(drag.presentation().0, [20.0, 0.0]);
    drag.set_bounds([-10.0; 2], [10.0; 2], ms(10));
    assert_eq!(drag.presentation().0, [20.0, 0.0]);
    drag.end(ms(10));
    assert!(drag.sample(ms(20), true).target().x_px() <= 10.0);
}

#[test]
fn typed_tracks_preserve_f64_and_per_channel_spring_velocity_on_replacement() {
    let seed = SvgTarget::default().rotate_degrees(1e12);
    let destination = seed.rotate_degrees(1e12 + 360.0).scale_xy(2.0, 0.5);
    let clip = TypedAnimation::spring(destination, Spring::default(), ms(1000)).unwrap();
    let mut state = TypedState::new(seed);

    state.sample(&clip, ms(0), false);
    state.sample(&clip, ms(80), false);
    let before = (state.value(), state.velocities());
    let reverse = TypedAnimation::spring(seed, Spring::default(), ms(1000)).unwrap();
    let frame = state.sample(&reverse, ms(80), false);

    assert_eq!(frame.value(), before.0);
    assert_eq!(state.velocities(), before.1);
    assert_eq!(state.sample(&reverse, ms(100), true).value(), seed);
}

#[test]
fn mixed_timeline_seeks_delayed_tracks_and_holds_without_false_completion() {
    let mut timeline = TypedTimeline::default();

    let x = timeline.at(
        ms(0),
        0.0,
        TypedAnimation::to(100.0, ms(1000), Easing::Linear).unwrap(),
    );

    timeline.label("middle", ms(500)).unwrap();
    let color = timeline
        .at_label(
            "middle",
            -0.25,
            Color::rgb(0),
            TypedAnimation::to(Color::rgb(0xffffff), ms(1000), Easing::Linear).unwrap(),
        )
        .unwrap();

    timeline.sample(ms(500), false);
    assert_eq!(timeline.value(x), Some(50.0));
    assert!(timeline.value(color).unwrap().channels()[0] > 0.0);
    timeline.pause(ms(500));
    timeline.sample(ms(900), false);
    assert_eq!(timeline.value(x), Some(50.0));
    assert!(!timeline.is_animating());
    timeline.seek(ms(0), ms(900));
    timeline.sample(ms(900), false);
    assert_eq!(timeline.value(x), Some(0.0));
    assert_eq!(timeline.value(color), Some(Color::rgb(0)));
    timeline.play(ms(900));
    timeline.sample(ms(2000), true);
    assert_eq!(timeline.value(x), Some(100.0));
    assert!(!timeline.is_animating());
    let foreign = TypedTimeline::default();

    assert_eq!(foreign.value(x), None);
}

#[test]
fn nested_variants_wait_for_parent_and_exits_retain_the_subtree() {
    let variants = || {
        Variants::new([
            (
                true,
                Motion::new(Target::default().x(100.0))
                    .transition(Transition::Tween(ms(100), Easing::Linear)),
            ),
            (
                false,
                Motion::new(Target::default())
                    .present(false)
                    .exit(Target::default().opacity(0.0))
                    .transition(Transition::Tween(ms(100), Easing::Linear)),
            ),
        ])
        .unwrap()
    };

    let mut tree = MotionTree::default();

    tree.add(
        MotionNode::new("parent", Target::default(), variants())
            .when(ChildrenOrder::BeforeChildren),
    )
    .unwrap();
    tree.add(MotionNode::new("child", Target::default(), variants()).parent("parent"))
        .unwrap();
    tree.set_variant(&true, ms(0), false).unwrap();
    tree.sample(ms(0), false);
    tree.sample(ms(50), false);
    assert_eq!(tree.get(&"parent").unwrap().target().x_px(), 50.0);
    assert_eq!(tree.get(&"child").unwrap().target().x_px(), 0.0);
    tree.sample(ms(100), false);
    tree.sample(ms(150), false);
    assert_eq!(tree.get(&"child").unwrap().target().x_px(), 50.0);
    tree.set_variant(&false, ms(150), false).unwrap();
    tree.sample(ms(150), false);
    tree.sample(ms(250), false);
    assert!(tree.is_mounted(&"parent"));
    tree.sample(ms(350), true);
    assert!(!tree.is_mounted(&"parent"));
    assert!(!tree.is_animating());
}

#[test]
fn completion_observers_finish_cancel_restart_and_owner_drop_without_an_executor() {
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Poll, Waker},
    };
    let mut cx = Context::from_waker(Waker::noop());
    let clip = Animation::to(Target::default().x(1.0), ms(100), Easing::Linear);
    let mut state = AnimationState::default();

    state.sample(&clip, ms(0), false);
    let mut finished = state.completion();

    assert_eq!(Pin::new(&mut finished).poll(&mut cx), Poll::Pending);
    state.sample(&clip, ms(100), false);
    assert_eq!(
        Pin::new(&mut finished).poll(&mut cx),
        Poll::Ready(CompletionStatus::Completed)
    );
    state.restart(ms(100));
    let mut cancelled = state.completion();

    state.restart(ms(100));
    assert_eq!(
        Pin::new(&mut cancelled).poll(&mut cx),
        Poll::Ready(CompletionStatus::Cancelled)
    );
    let mut dropped = state.completion();

    drop(state);
    assert_eq!(
        Pin::new(&mut dropped).poll(&mut cx),
        Poll::Ready(CompletionStatus::Cancelled)
    );
}

#[test]
fn derived_lifecycle_and_typed_bindings_preserve_transactional_ownership() {
    let mut graph = MotionGraph::default();
    let ids = graph.typed_source(Color::rgb(0));
    let total = graph.derive(&ids, |values| values.iter().sum()).unwrap();
    let events = Rc::new(RefCell::new(Vec::new()));
    let sink = events.clone();

    let _subscription = graph
        .subscribe(total, move |event| sink.borrow_mut().push(event.kind))
        .unwrap();

    graph
        .animate_typed(ids, Color::rgb(0xffffff), Transition::default(), ms(0))
        .unwrap();
    graph.sample(ms(50), false);
    assert!(events.borrow().contains(&MotionEventKind::Start));
    let before = graph.typed_value(Color::rgb(0), ids).unwrap();
    let invalid = [ids[0], total, ids[2], ids[3]];

    assert_eq!(
        graph.animate_typed(invalid, Color::rgb(0), Transition::Instant, ms(50)),
        Err(GraphError::Derived)
    );
    assert_eq!(graph.typed_value(Color::rgb(0), ids).unwrap(), before);
    graph
        .animate(
            ids[0],
            1.,
            Transition::Tween(ms(100), Easing::Linear),
            ms(50),
        )
        .unwrap();
    graph.sample(ms(50), false);
    assert!(events.borrow().contains(&MotionEventKind::Cancel));
    graph.sample(ms(1000), true);
    assert!(events.borrow().contains(&MotionEventKind::Complete));
}

#[test]
fn child_first_sequencing_inheritance_and_dynamic_targets_are_idempotent() {
    let variants = || {
        Variants::new([
            (0, Motion::new(Target::default())),
            (
                1,
                Motion::new(Target::default().x(100.))
                    .transition(Transition::Tween(ms(100), Easing::Linear)),
            ),
        ])
        .unwrap()
    };

    let mut tree = MotionTree::default();

    tree.add(MotionNode::new(0, Target::default(), variants()).when(ChildrenOrder::AfterChildren))
        .unwrap();
    tree.add(MotionNode::new(1, Target::default(), variants()).parent(0))
        .unwrap();
    tree.add(
        MotionNode::new(2, Target::default(), variants())
            .parent(0)
            .inherit(false),
    )
    .unwrap();
    tree.set_variant(&1, ms(0), false).unwrap();
    tree.sample(ms(0), false);
    tree.set_variant(&1, ms(50), false).unwrap();
    tree.sample(ms(50), false);
    assert_eq!(tree.get(&1).unwrap().target().x_px(), 50.);
    assert_eq!(tree.get(&0).unwrap().target().x_px(), 0.);
    assert_eq!(tree.get(&2).unwrap().target().x_px(), 0.);
    tree.sample(ms(100), false);
    tree.sample(ms(150), false);
    assert_eq!(tree.get(&0).unwrap().target().x_px(), 50.);
    tree.set_motion(
        &0,
        Motion::new(Target::default().x(-50.))
            .transition(Transition::Tween(ms(100), Easing::Linear)),
        ms(150),
        false,
    )
    .unwrap();
    tree.sample(ms(150), false);
    assert_eq!(tree.get(&0).unwrap().target().x_px(), 50.);
    assert_eq!(
        tree.set_node_variant(&99, &0, ms(150), false),
        Err(TreeError::Identity)
    );
    tree.sample(ms(250), false);
    assert_eq!(tree.get(&0).unwrap().target().x_px(), -50.);
}

#[test]
fn mixed_timeline_reverse_repeats_future_tracks_and_reduced_restart() {
    let mut timeline = TypedTimeline::default();

    let x = timeline.at(
        ms(500),
        0.,
        TypedAnimation::to(100., ms(100), Easing::Linear)
            .unwrap()
            .repeat(Repeat::Count(1), RepeatMode::Mirror, ms(100))
            .unwrap(),
    );

    timeline.sample(ms(0), false);
    assert!(timeline.is_animating());
    timeline.sample(ms(600), false);
    assert_eq!(timeline.value(x), Some(100.));
    timeline.pause(ms(600));
    timeline.seek(ms(550), ms(600));
    timeline.sample(ms(600), false);
    assert_eq!(timeline.value(x), Some(50.));
    timeline.play(ms(600));
    timeline.set_speed(-1., ms(600)).unwrap();
    timeline.sample(ms(650), false);
    assert_eq!(timeline.value(x), Some(0.));
    timeline.sample(ms(1200), false);
    assert!(!timeline.is_animating());
    timeline.restart(ms(1300));
    timeline.sample(ms(1300), true);
    assert!(!timeline.is_animating());
    timeline.restart(ms(1400));
    timeline.set_speed(1., ms(1400)).unwrap();
    timeline.sample(ms(1400), false);
    assert!(timeline.is_animating());
    let forever = TypedAnimation::to(1., ms(100), Easing::Linear)
        .unwrap()
        .repeat(Repeat::Forever, RepeatMode::Loop, ms(0))
        .unwrap();

    timeline.at(ms(0), 0., forever);
    assert!(
        timeline
            .after(
                0.,
                0.,
                TypedAnimation::to(1., ms(100), Easing::Linear).unwrap()
            )
            .is_err()
    );
}

#[test]
fn completion_wakes_once_and_replacing_a_clip_cancels_the_previous_run() {
    use std::{
        future::Future,
        pin::Pin,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        task::{Context, Poll, Wake, Waker},
    };

    struct Counter(AtomicUsize);

    impl Wake for Counter {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }
    let counter = Arc::new(Counter(AtomicUsize::new(0)));
    let waker = Waker::from(counter.clone());
    let mut cx = Context::from_waker(&waker);
    let mut state = AnimationState::default();
    let first = Animation::to(Target::default().x(100.), ms(100), Easing::Linear);

    state.sample(&first, ms(0), false);
    let mut complete = state.completion();

    assert_eq!(Pin::new(&mut complete).poll(&mut cx), Poll::Pending);
    let next = Animation::to(Target::default().x(50.), ms(100), Easing::Linear);

    state.sample(&next, ms(50), false);
    assert_eq!(counter.0.load(Ordering::Relaxed), 1);
    assert_eq!(
        Pin::new(&mut complete).poll(&mut cx),
        Poll::Ready(CompletionStatus::Cancelled)
    );
    state.sample(&next, ms(500), true);
    assert_eq!(counter.0.load(Ordering::Relaxed), 1);
}

#[test]
fn instantaneous_graph_completion_reaches_dependants_but_idle_redeclarations_do_not_start() {
    let mut graph = MotionGraph::default();
    let x = graph.source(0.);
    let derived = graph.derive(&[x], |v| v[0] * 2.).unwrap();
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();

    let _listener = graph
        .subscribe(derived, move |event| output.borrow_mut().push(event.kind))
        .unwrap();

    graph.animate(x, 0., Transition::default(), ms(0)).unwrap();
    graph.sample(ms(0), false);
    assert!(events.borrow().is_empty());
    graph.animate(x, 1., Transition::Instant, ms(10)).unwrap();
    graph.sample(ms(10), false);
    assert_eq!(
        &*events.borrow(),
        &[
            MotionEventKind::Start,
            MotionEventKind::Change,
            MotionEventKind::Complete
        ]
    );
}

#[test]
fn separate_exit_order_keeps_the_parent_visible_until_children_have_left() {
    let variants = || {
        Variants::new([
            (
                true,
                Motion::new(Target::default().x(100.))
                    .transition(Transition::Tween(ms(100), Easing::Linear)),
            ),
            (
                false,
                Motion::new(Target::default())
                    .present(false)
                    .exit(Target::default().opacity(0.))
                    .transition(Transition::Tween(ms(100), Easing::Linear)),
            ),
        ])
        .unwrap()
    };

    let mut tree = MotionTree::default();

    tree.add(
        MotionNode::new(0, Target::default(), variants())
            .when(ChildrenOrder::BeforeChildren)
            .exit_when(ChildrenOrder::AfterChildren),
    )
    .unwrap();
    tree.add(MotionNode::new(1, Target::default(), variants()).parent(0))
        .unwrap();
    tree.set_variant(&true, ms(0), false).unwrap();

    for time in [0, 100, 200] {
        tree.sample(ms(time), false);
    }

    tree.set_variant(&false, ms(200), false).unwrap();
    tree.sample(ms(200), false);
    tree.sample(ms(250), false);
    assert_eq!(tree.get(&0).unwrap().target().alpha(), 1.);
    assert_eq!(tree.get(&1).unwrap().target().alpha(), 0.5);
    tree.sample(ms(300), false);
    tree.sample(ms(350), false);
    assert_eq!(tree.get(&0).unwrap().target().alpha(), 0.5);
    tree.sample(ms(400), false);
    assert!(!tree.is_mounted(&0));
}

#[test]
fn jumping_resets_source_and_derived_momentum_even_at_the_same_timestamp() {
    let mut graph = MotionGraph::default();
    let source = graph.source(0.);
    let derived = graph.derive(&[source], |values| values[0] * 2.).unwrap();

    graph
        .animate(source, 100., Transition::default(), ms(0))
        .unwrap();
    graph.sample(ms(80), false);
    assert!(graph.velocity(derived).unwrap() > 0.);

    graph.jump(source, 50., ms(80)).unwrap();
    graph.sample(ms(80), false);

    assert_eq!(graph.get(derived), Ok(100.));
    assert_eq!(graph.velocity(source), Ok(0.));
    assert_eq!(graph.velocity(derived), Ok(0.));
}

#[test]
fn dropping_a_subscription_inside_its_callback_stops_the_rest_of_the_batch() {
    let mut graph = MotionGraph::default();
    let source = graph.source(0.);
    let holder = Rc::new(RefCell::new(None));
    let owner = holder.clone();
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();

    let subscription = graph
        .subscribe(source, move |event| {
            output.borrow_mut().push(event.kind);
            owner.borrow_mut().take();
        })
        .unwrap();

    *holder.borrow_mut() = Some(subscription);

    graph
        .animate(source, 1., Transition::Instant, ms(10))
        .unwrap();
    graph.sample(ms(10), false);

    assert_eq!(&*events.borrow(), &[MotionEventKind::Start]);
}
