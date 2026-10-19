//! Keyed batches versus direct states: identical springs, retargets, and key order.
//! Also reports requested heap bytes (not allocator/RSS overhead) for retained keys.
use gpui_ce_motion::{Motion, MotionState, Motions, Target};

use std::{
    alloc::{GlobalAlloc, Layout, System},
    hint::black_box,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

static LIVE: AtomicUsize = AtomicUsize::new(0);

struct Measuring;

// SAFETY: All allocations retain System's exact pointer/layout contract.
// The counter observes requested byte sizes without accessing allocated memory.
unsafe impl GlobalAlloc for Measuring {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: The caller's valid layout is forwarded unchanged.
        let pointer = unsafe { System.alloc(layout) };

        if !pointer.is_null() {
            LIVE.fetch_add(layout.size(), Ordering::Relaxed);
        }

        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);

        // SAFETY: The pointer and layout come from the corresponding allocation.
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Measuring = Measuring;

fn declaration(destination: f32) -> Motion {
    Motion::new(
        Target::default()
            .x(destination)
            .y(destination / 2.)
            .opacity(if destination == 0. { 0.5 } else { 1. })
            .width(200. + destination)
            .height(100. + destination / 2.)
            .border_radius(8. + destination / 10.),
    )
    .initial(Target::default().width(200.).height(100.).border_radius(8.))
}

fn run<const N: usize>(reverse: bool) {
    let before = LIVE.load(Ordering::Relaxed);
    let mut motions = Motions::with_capacity(N);
    let initial = std::array::from_fn::<_, N, _>(|key| (key, declaration(0.)));

    motions.sample(initial, Duration::ZERO, false);
    let heap = LIVE.load(Ordering::Relaxed) - before;

    println!(
        "{N} keys: capacity {}, requested collection heap {heap} bytes; direct state bytes {}",
        motions.capacity(),
        N * std::mem::size_of::<MotionState>()
    );
    let mut direct: Vec<_> = (0..N).map(|_| MotionState::default()).collect();

    for state in &mut direct {
        state.sample(declaration(0.), Duration::ZERO, false);
    }

    // Give small collections enough measured work to avoid sub-millisecond batches.
    let frames_per_batch = (240_000 / N).max(240);

    for keyed in if reverse {
        [true, false]
    } else {
        [false, true]
    } {
        let mut times = [0.; 25];

        for (batch, elapsed) in times.iter_mut().enumerate() {
            let started = Instant::now();

            for frame in 0..frames_per_batch {
                let tick = batch * frames_per_batch + frame + 1;
                let now = Duration::from_micros(tick as u64 * 8333);

                let destination = if (tick / 24).is_multiple_of(2) {
                    100.
                } else {
                    0.
                };

                let motion = black_box(declaration(destination));

                let declarations = std::array::from_fn::<_, N, _>(|index| {
                    let key = if tick.is_multiple_of(2) {
                        index
                    } else {
                        N - 1 - index
                    };

                    (key, motion)
                });

                let frames = if keyed {
                    motions.sample(black_box(declarations), now, false)
                } else {
                    black_box(declarations)
                        .map(|(key, motion)| direct[key].sample(motion, now, false))
                };

                black_box(frames);
            }

            *elapsed = started.elapsed().as_secs_f64() * 1e9 / (frames_per_batch * N) as f64;
        }

        times.sort_by(f64::total_cmp);
        println!(
            "{} / {N}: median {:.1} ns/object, batch P95 {:.1} ns/object; {:.3} ms/frame",
            if keyed { "Motions" } else { "direct" },
            times[12],
            times[23],
            times[12] * N as f64 / 1e6
        );
    }

    motions.clear();
    motions.shrink_to_fit();
    assert_eq!(motions.capacity(), 0);
}

fn main() {
    println!(
        "portable stack bytes: MotionState {}, Motions<usize> {} (collection heap reported separately)",
        std::mem::size_of::<MotionState>(),
        std::mem::size_of::<Motions<usize>>()
    );
    let reverse = std::env::args().any(|argument| argument == "--reverse");

    run::<8>(reverse);
    run::<64>(reverse);
    run::<1000>(reverse);
}
