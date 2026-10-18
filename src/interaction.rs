//! Explicit variants, gesture precedence, drag, scroll mapping, and viewport checks.
use crate::{AnimationError, Frame, Inertia, Motion, MotionValue, Target, Transition};
use std::time::Duration;

/// Named declarations. Keys can be application enums; propagation is explicit in Rust.
#[derive(Debug)]
pub struct Variants<K> {
    entries: Vec<(K, Motion)>,
}

impl<K: PartialEq> Variants<K> {
    /// Creates named declarations, rejecting duplicate names transactionally.
    pub fn new(entries: impl IntoIterator<Item = (K, Motion)>) -> Result<Self, AnimationError> {
        let entries: Vec<_> = entries.into_iter().collect();

        if entries
            .iter()
            .enumerate()
            .any(|(ix, (key, _))| entries[..ix].iter().any(|(old, _)| old == key))
        {
            return Err(AnimationError::Label);
        }

        Ok(Self { entries })
    }

    /// Looks up a declaration. Pass the same key to children for variant propagation.
    pub fn get(&self, key: &K) -> Option<Motion> {
        self.entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, motion)| *motion)
    }
}

/// Current input state supplied by normal native input/focus handlers.
#[derive(Clone, Copy, Debug, Default)]
#[non_exhaustive]
pub struct Interaction {
    /// Pointer is inside the native hit region.
    pub hovered: bool,
    /// A pointer or keyboard press is active.
    pub pressed: bool,
    /// The native control has keyboard-visible focus.
    pub focused: bool,
    /// A drag is active.
    pub dragging: bool,
    /// The control meets the configured viewport visibility threshold.
    pub in_view: bool,
}

/// Partial interaction targets. Priority is in-view, hover, focus, press, then drag.
#[derive(Clone, Copy, Debug, Default)]
pub struct GestureTargets {
    hover: Option<Target>,
    press: Option<Target>,
    focus: Option<Target>,
    drag: Option<Target>,
    view: Option<Target>,
}

impl GestureTargets {
    /// Declares the hover override.
    pub fn while_hover(mut self, target: Target) -> Self {
        self.hover = Some(target);

        self
    }

    /// Declares the pointer/keyboard press override.
    pub fn while_press(mut self, target: Target) -> Self {
        self.press = Some(target);

        self
    }

    /// Declares the keyboard-visible focus override.
    pub fn while_focus(mut self, target: Target) -> Self {
        self.focus = Some(target);

        self
    }

    /// Declares the dragging override.
    pub fn while_drag(mut self, target: Target) -> Self {
        self.drag = Some(target);

        self
    }

    /// Declares the viewport visibility override.
    pub fn while_in_view(mut self, target: Target) -> Self {
        self.view = Some(target);

        self
    }

    /// Combines active overrides without replacing unrelated base properties.
    pub fn resolve(self, mut base: Target, state: Interaction) -> Target {
        for (active, target) in [
            (state.in_view, self.view),
            (state.hovered, self.hover),
            (state.focused, self.focus),
            (state.pressed, self.press),
            (state.dragging, self.drag),
        ] {
            if active && let Some(target) = target {
                base = base.overlay(target);
            }
        }

        base
    }
}

/// Bounded two-dimensional drag with explicit ownership of pointer capture/cancellation.
#[derive(Debug)]
pub struct Drag {
    values: [MotionValue; 2],
    min: [f64; 2],
    max: [f64; 2],
    origin: [f64; 2],
    pointer: [f64; 2],
    dragging: bool,
    last_move: Duration,
    elasticity: f64,
    snap: Option<f64>,
    momentum: bool,
}

impl Drag {
    /// Validates finite ordered bounds and an initial position inside them.
    pub fn new(initial: [f64; 2], min: [f64; 2], max: [f64; 2]) -> Self {
        assert!(
            (0..2).all(|i| initial[i].is_finite()
                && min[i].is_finite()
                && max[i].is_finite()
                && min[i] <= initial[i]
                && initial[i] <= max[i]),
            "invalid drag bounds"
        );

        Self {
            values: initial.map(MotionValue::new),
            min,
            max,
            origin: initial,
            pointer: [0.0; 2],
            dragging: false,
            last_move: Duration::ZERO,
            elasticity: 0.15,
            snap: None,
            momentum: true,
        }
    }

    /// Sets drag resistance outside bounds. Panics unless in zero through one.
    pub fn elasticity(mut self, value: f64) -> Self {
        assert!((0.0..=1.0).contains(&value), "invalid elasticity");
        self.elasticity = value;

        self
    }

    /// Snaps release destinations to this positive finite grid spacing.
    pub fn snap_grid(mut self, spacing: f64) -> Self {
        assert!(spacing.is_finite() && spacing > 0.0, "invalid snap grid");
        self.snap = Some(spacing);

        self
    }

    /// Enables/disables velocity projection on release.
    pub fn momentum(mut self, enabled: bool) -> Self {
        self.momentum = enabled;

        self
    }

    /// Grabs the currently presented position, interrupting any previous release motion.
    pub fn begin(&mut self, pointer: [f64; 2], now: Duration) {
        assert!(pointer.iter().all(|v| v.is_finite()), "invalid pointer");

        for ix in 0..2 {
            let value = self.values[ix].sample(now, false).value();

            self.values[ix].jump(value, now);
            self.origin[ix] = value;
        }

        self.pointer = pointer;
        self.last_move = now;
        self.dragging = true;
    }

    /// Follows the pointer one-to-one inside bounds, with configured resistance outside.
    pub fn move_to(&mut self, pointer: [f64; 2], now: Duration) {
        assert!(pointer.iter().all(|v| v.is_finite()), "invalid pointer");

        if !self.dragging {
            return;
        }

        self.last_move = self.last_move.max(now);

        for (ix, point) in pointer.into_iter().enumerate() {
            let raw = crate::value::finite(
                self.origin[ix] + crate::value::finite(point - self.pointer[ix]),
            );

            let clamped = raw.clamp(self.min[ix], self.max[ix]);

            self.values[ix].set(clamped + (raw - clamped) * self.elasticity, now);
        }
    }

    /// Releases into bounded inertia, or a velocity-preserving spring to a snap point.
    /// Holding still for over 100 ms clears stale pointer momentum before release.
    pub fn end(&mut self, now: Duration) {
        if !self.dragging {
            return;
        }

        self.dragging = false;

        if now.saturating_sub(self.last_move) > Duration::from_millis(100) {
            for value in &mut self.values {
                value.jump(value.get(), now);
            }
        }

        for ix in 0..2 {
            let inertia = Inertia::default().bounds(self.min[ix], self.max[ix]);

            if let Some(grid) = self.snap {
                let projected = if self.momentum {
                    inertia.project(self.values[ix].get(), self.values[ix].velocity())
                } else {
                    self.values[ix].get()
                };

                let target = (projected / grid)
                    .round()
                    .mul_add(grid, 0.0)
                    .clamp(self.min[ix], self.max[ix]);

                self.values[ix].animate(target, Transition::default(), now);
            } else if self.momentum {
                self.values[ix].coast(inertia, now);
            } else {
                let target = self.values[ix].get().clamp(self.min[ix], self.max[ix]);

                self.values[ix].animate(target, Transition::default(), now);
            }
        }
    }

    /// Cancels an active drag and returns to the grabbed position. Use on Escape or lost capture.
    /// Does nothing after release, so later focus loss cannot undo a completed gesture.
    pub fn cancel(&mut self, now: Duration) {
        if !self.dragging {
            return;
        }

        self.dragging = false;

        for ix in 0..2 {
            self.values[ix].animate(
                self.origin[ix].clamp(self.min[ix], self.max[ix]),
                Transition::default(),
                now,
            );
        }
    }

    /// A keyboard equivalent of dragging: retargets a bounded position using a spring.
    pub fn move_by(&mut self, delta: [f64; 2], now: Duration) {
        assert!(delta.iter().all(|v| v.is_finite()), "invalid drag delta");
        self.dragging = false;

        for (ix, delta) in delta.into_iter().enumerate() {
            let target = (self.values[ix].target() + delta).clamp(self.min[ix], self.max[ix]);

            self.values[ix].animate(target, Transition::default(), now);
        }
    }

    /// Samples a drawable x/y presentation. The owner requests frames while it is active.
    pub fn sample(&mut self, now: Duration, reduced_motion: bool) -> Frame {
        let values = self
            .values
            .each_mut()
            .map(|value| value.sample(now, reduced_motion));

        Frame::animated(
            Target::default()
                .x(values[0].value().clamp(-(f32::MAX as f64), f32::MAX as f64) as f32)
                .y(values[1].value().clamp(-(f32::MAX as f64), f32::MAX as f64) as f32),
            [
                values[0].velocity(),
                values[1].velocity(),
                0.0,
                0.0,
                0.0,
                0.0,
            ],
            values.iter().any(|v| v.is_animating()),
            values.iter().any(|v| v.just_completed()) && values.iter().all(|v| !v.is_animating()),
        )
    }

    /// Whether a pointer currently owns this gesture.
    pub fn is_dragging(&self) -> bool {
        self.dragging
    }
}

/// Maps a finite scalar between ranges, optionally clamping progress.
/// Supports reversed input/output ranges. Panics for non-finite or zero input span.
pub fn transform(value: f64, input: [f64; 2], output: [f64; 2], clamp: bool) -> f64 {
    assert!(
        [value, input[0], input[1], output[0], output[1]]
            .iter()
            .all(|v| v.is_finite())
            && input[0] != input[1],
        "invalid transform range"
    );
    let progress = (value - input[0]) / (input[1] - input[0]);

    let progress = if clamp {
        progress.clamp(0.0, 1.0)
    } else {
        progress
    };

    output[0] + (output[1] - output[0]) * progress
}

/// Normalizes a positive scroll offset using measured content and viewport extents.
/// An unscrollable viewport reports zero. Panics for invalid metrics.
pub fn scroll_progress(offset: f64, content: f64, viewport: f64) -> f64 {
    assert!(
        [offset, content, viewport].iter().all(|v| v.is_finite())
            && content >= 0.0
            && viewport >= 0.0,
        "invalid scroll metrics"
    );

    if content <= viewport || offset <= 0.0 {
        0.0
    } else {
        (offset / (content - viewport)).clamp(0.0, 1.0)
    }
}

/// Retained visibility threshold with optional once-only triggering.
#[derive(Clone, Copy, Debug)]
pub struct InView {
    amount: f64,
    once: bool,
    entered: bool,
    visible: bool,
}

impl InView {
    /// Creates a threshold. Zero means any positive intersection; one means fully visible.
    pub fn new(amount: f64, once: bool) -> Self {
        assert!(
            (0.0..=1.0).contains(&amount),
            "invalid visibility threshold"
        );

        Self {
            amount,
            once,
            entered: false,
            visible: false,
        }
    }

    /// Updates from viewport/element `[x,y,width,height]` bounds in the same coordinate space.
    /// Returns whether visibility changed. Zero-area elements are invisible.
    pub fn update(&mut self, viewport: [f64; 4], element: [f64; 4]) -> bool {
        assert!(
            viewport.iter().chain(element.iter()).all(|v| v.is_finite())
                && viewport[2..]
                    .iter()
                    .chain(element[2..].iter())
                    .all(|v| *v >= 0.0),
            "invalid viewport bounds"
        );
        let width =
            (viewport[0] + viewport[2]).min(element[0] + element[2]) - viewport[0].max(element[0]);

        let height =
            (viewport[1] + viewport[3]).min(element[1] + element[3]) - viewport[1].max(element[1]);

        let area = element[2] * element[3];

        let fraction = if area > 0.0 {
            width.max(0.0) * height.max(0.0) / area
        } else {
            0.0
        };

        let visible = (self.once && self.entered) || (fraction > 0.0 && fraction >= self.amount);
        let changed = self.visible != visible;

        self.visible = visible;
        self.entered |= visible;

        changed
    }

    /// Last resolved visibility, including a once-only latch.
    pub fn is_visible(&self) -> bool {
        self.visible
    }
}

/// Finds the nearest destination in a sorted list of item centers, for native reorder UIs.
pub fn reorder_index(centers: &[f64], position: f64) -> Result<usize, AnimationError> {
    if centers.is_empty()
        || !position.is_finite()
        || centers.iter().any(|v| !v.is_finite())
        || centers.windows(2).any(|p| p[0] >= p[1])
    {
        return Err(AnimationError::Index);
    }

    Ok(centers
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| (*a - position).abs().total_cmp(&(*b - position).abs()))
        .unwrap()
        .0)
}

/// Moves an item without reallocating or changing its identity. Invalid indices return an error.
pub fn move_item<T>(items: &mut [T], from: usize, to: usize) -> Result<(), AnimationError> {
    if from >= items.len() || to >= items.len() {
        return Err(AnimationError::Index);
    }

    if from < to {
        items[from..=to].rotate_left(1);
    } else {
        items[to..=from].rotate_right(1);
    }

    Ok(())
}
