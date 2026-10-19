//! Shared-engine CPU sampling. Setup is excluded; each workload retains its state.
use gpui_ce_motion::*;

use std::{
    hint::black_box,
    time::{Duration, Instant},
};

fn run(label: &str, count: usize, mut sample: impl FnMut(Duration, u64)) {
    let mut batches = Vec::with_capacity(15);

    for batch in 0..15 {
        let started = Instant::now();

        for frame in 0..240 {
            let index = batch * 240 + frame;

            sample(Duration::from_micros(index * 8333), index);
        }

        batches.push(started.elapsed().as_secs_f64() * 1e9 / (240 * count) as f64);
    }

    batches.sort_by(f64::total_cmp);
    println!(
        "{label}: median {:.1} ns/item, batch P95 {:.1} ns/item; {count} items",
        batches[7], batches[14]
    );
}

fn main() {
    let mut graph = MotionGraph::default();
    let sources: Vec<_> = (0..1000).map(|_| graph.source(0.)).collect();

    for id in &sources {
        graph.derive(&[*id], |values| values[0] * 2.).unwrap();
    }

    let subscriptions: Vec<_> = sources
        .iter()
        .map(|id| {
            graph
                .subscribe(*id, |event| {
                    black_box(event);
                })
                .unwrap()
        })
        .collect();

    run("source + derived + subscriber", 1000, |now, index| {
        if index.is_multiple_of(24) {
            for id in &sources {
                graph
                    .animate(
                        *id,
                        if index.is_multiple_of(48) { 100. } else { 0. },
                        Transition::default(),
                        now,
                    )
                    .unwrap();
            }
        }

        graph.sample(now, false);
        black_box(graph.get(sources[0]).unwrap());
    });
    black_box(subscriptions);
    let mut tree = MotionTree::default();

    for key in 0..1000 {
        let variants = Variants::new([
            (true, Motion::new(Target::default().x(100.))),
            (false, Motion::new(Target::default())),
        ])
        .unwrap();

        let node = MotionNode::new(key, Target::default(), variants);

        tree.add(if key % 5 == 0 {
            node.when(ChildrenOrder::BeforeChildren)
        } else {
            node.parent(key - key % 5)
        })
        .unwrap();
    }

    run("variant tree + spring retargeting", 1000, |now, index| {
        if index.is_multiple_of(24) {
            tree.set_variant(&index.is_multiple_of(48), now, false)
                .unwrap();
        }

        tree.sample(now, false);
        black_box(tree.get(&0));
    });
    let clip = TypedAnimation::to(
        Color::rgb(0xffffff),
        Duration::from_millis(800),
        Easing::EaseInOut,
    )
    .unwrap()
    .repeat(Repeat::Forever, RepeatMode::Mirror, Duration::ZERO)
    .unwrap();

    let mut timeline = TypedTimeline::default();

    let tracks: Vec<_> = (0..1000)
        .map(|index| {
            timeline.at(
                Duration::from_millis(index % 20),
                Color::rgb(0),
                clip.clone(),
            )
        })
        .collect();

    run("mixed-capable timeline / RGBA track", 1000, |now, _| {
        timeline.sample(now, false);
        black_box(timeline.value(tracks[0]));
    });
    let shape = PathShape::new([
        PathCommand::Move([0., 0.]),
        PathCommand::Cubic([100., 0.], [0., 100.], [100., 100.]),
    ])
    .unwrap();

    let measure = PathMeasure::new(&shape).unwrap();

    run("prepared path-follow query", 1000, |_, index| {
        for n in 0..1000 {
            black_box(measure.point(((index + n) % 1000) as f64 / 1000.));
        }
    });
}
