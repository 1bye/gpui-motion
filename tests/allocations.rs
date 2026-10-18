//! Covers: allocation-free steady sampling of prepared native-independent engines.
//! Real: system allocator and public motion APIs. Doubles: none. Requires: none.
use gpui_ce_motion::*;

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    time::Duration,
};

thread_local! { static COUNT: Cell<Option<usize>> = const { Cell::new(None) }; }

struct Counting;

// SAFETY: Every operation forwards the exact allocation contract to System;
// the thread-local counter only observes allocations and never owns their memory.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        COUNT.with(|count| {
            if let Some(n) = count.get() {
                count.set(Some(n + 1));
            }
        });

        // SAFETY: Caller supplies a valid allocation layout, forwarded unchanged.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: Pointer and layout have the allocator's original provenance.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        COUNT.with(|count| {
            if let Some(n) = count.get() {
                count.set(Some(n + 1));
            }
        });

        // SAFETY: The caller's valid reallocation request is forwarded unchanged.
        unsafe { System.realloc(ptr, layout, size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

#[test]
fn prepared_samples_and_controls_do_not_allocate() {
    let clip = Animation::to(
        Target::default().x(100.).width(200.),
        Duration::from_secs(1),
        Easing::EaseInOut,
    )
    .repeat(Repeat::Forever, RepeatMode::Mirror, Duration::ZERO)
    .unwrap();

    let mut a = AnimationState::default();
    let mut m = MotionState::default();
    let mut layout = LayoutPositionState::default();
    let mut svg = SvgMotionState::default();
    let mut styles = StyleMotionState::default();

    let path_a = PathShape::new([
        PathCommand::Move([0., 0.]),
        PathCommand::Cubic([10., 20.], [30., 40.], [50., 0.]),
    ])
    .unwrap();

    let path_b = PathShape::new([
        PathCommand::Move([0., 0.]),
        PathCommand::Cubic([20., 40.], [40., 20.], [100., 0.]),
    ])
    .unwrap();

    let mut path = PathMorphState::new(path_a.clone());
    let mut color = ColorState::new(Color::rgb(0));
    let mut drag = Drag::new([0., 0.], [0., 0.], [100., 100.]);
    let mut timeline = Timeline::default();

    timeline.at(Duration::ZERO, clip.clone());
    COUNT.with(|c| c.set(Some(0)));

    for i in 0..240 {
        let now = Duration::from_millis(i * 8);

        std::hint::black_box(a.sample(&clip, now, false));
        std::hint::black_box(styles.sample(
            StyleMotion::new(StyleTarget::default().padding([i as f32; 4]).color(
                ColorProperty::Background,
                Color::rgb(if i % 2 == 0 { 0xffeecc } else { 0x223344 }),
            )),
            now,
            false,
        ));
        let frame = path
            .sample(
                if i % 2 == 0 { &path_a } else { &path_b },
                Transition::default(),
                now,
                false,
            )
            .unwrap();

        frame.stroke_segments(StrokeReveal::new(0.5, 0.2, 1.).unwrap(), |a, b| {
            std::hint::black_box((a, b));
        });
        std::hint::black_box(layout.sample(
            [i as f64 * 2., i as f64],
            Transition::default(),
            now,
            false,
        ));
        std::hint::black_box(
            svg.sample(
                SvgMotion::new(SvgTarget::default().scale(1.5).rotate_degrees(i as f64))
                    .initial(SvgTarget::default()),
                now,
                false,
            ),
        );
        std::hint::black_box(m.sample(Motion::new(Target::default().x(i as f32)), now, false));
        std::hint::black_box(color.sample(Color::rgb(0xffffff), Transition::default(), now, false));
        std::hint::black_box(timeline.sample(now, false));

        if i == 0 {
            drag.begin([0., 0.], now);
        }

        if i < 20 {
            drag.move_to([i as f64, i as f64], now);
        } else {
            drag.end(now);
        }

        std::hint::black_box(drag.sample(now, false));
    }

    let count = COUNT.with(|c| c.replace(None)).unwrap();

    assert_eq!(count, 0);
}
