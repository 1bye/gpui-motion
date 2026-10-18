//! Portable prepared text sampling. Excludes segmentation, shaping, matching, and native paint.
use gpui_ce_motion::{
    Scramble, StaggerFrom, TextLayoutUnit, TextMatch, TextOptions, TextPlaybackState, TextTarget,
    Transition,
};

use std::{
    hint::black_box,
    time::{Duration, Instant},
};

fn main() {
    for (label, scramble) in [
        ("spring text units", false),
        ("spring + deterministic scramble", true),
    ] {
        let mut batches = Vec::new();

        for _ in 0..15 {
            let units: Vec<_> = (0..1000)
                .map(|index| {
                    TextLayoutUnit::new(
                        "a",
                        index,
                        [(index % 50) as f64 * 16., (index / 50) as f64 * 24.],
                        [16., 24.],
                        20.,
                    )
                })
                .collect();

            let options = TextOptions {
                enter: TextTarget::default().y_em(0.5).opacity(0.),
                transition: Transition::default(),
                matching: TextMatch::Content,
                stagger: Duration::from_micros(100),
                stagger_from: StaggerFrom::Center,
                scramble: scramble
                    .then(|| Scramble::new("ABC123").duration(Duration::from_secs(3))),
                ..TextOptions::default()
            };

            let mut state = TextPlaybackState::default();

            state.reconcile("prepared", &units, &options, Duration::ZERO, false);
            let start = Instant::now();

            for frame in 0..240 {
                black_box(state.sample(Duration::from_micros(frame * 8333), false, None));
            }

            batches.push(start.elapsed().as_secs_f64() * 1e9 / (1000. * 240.));
        }

        batches.sort_by(f64::total_cmp);
        println!(
            "{label}: median {:.1} ns/unit/sample, batch P95 {:.1}; 1000 units × 240 frames × 15 batches",
            batches[7], batches[14]
        );
    }
}
