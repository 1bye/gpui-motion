//! Position-only layout following, independent of the native measurement adapter.
use crate::{MotionValue, Transition};
use std::time::Duration;

/// A measured position and the offset needed to display its animated presentation.
/// Units are logical pixels in a consistent caller-chosen coordinate space.
#[derive(Clone, Copy, Debug)]
pub struct LayoutPositionFrame {
    origin: [f64; 2],
    position: [f64; 2],
    velocity: [f64; 2],
    active: bool,
    completed: bool,
}
impl LayoutPositionFrame {
    /// The current layout destination, without the animation offset.
    pub fn layout_origin(self) -> [f64; 2] {
        self.origin
    }
    /// Presented position in the same coordinate space as the measured origin.
    pub fn position(self) -> [f64; 2] {
        self.position
    }
    /// Translation to apply after layout; size is unaffected.
    pub fn offset(self) -> [f64; 2] {
        std::array::from_fn(|i| crate::value::finite(self.position[i] - self.origin[i]))
    }
    /// Current velocity in logical pixels per second.
    pub fn velocity(self) -> [f64; 2] {
        self.velocity
    }
    /// Whether another sample is required.
    pub fn is_animating(self) -> bool {
        self.active
    }
    /// Completion edge when the last active axis finishes.
    pub fn just_completed(self) -> bool {
        self.completed
    }
}

/// Retained position-only layout sampler. First measurement snaps, later measurements
/// interrupt from the current presentation and preserve spring velocity. Sampling allocates
/// no memory. The optional native `LayoutGroup` supplies measurements automatically.
#[derive(Clone, Debug)]
pub struct LayoutPositionState {
    axes: [MotionValue; 2],
    initialized: bool,
}
impl Default for LayoutPositionState {
    fn default() -> Self {
        Self {
            axes: [MotionValue::new(0.), MotionValue::new(0.)],
            initialized: false,
        }
    }
}
impl LayoutPositionState {
    /// Follows a finite layout origin in logical pixels. Panics for non-finite input.
    /// Use one monotonic epoch; backward timestamps hold. Reduced motion snaps and
    /// completes once. Offsets/velocity are never used as the next layout measurement.
    ///
    /// ```
    /// use gpui_ce_motion::{LayoutPositionState, Transition};
    /// use std::time::Duration;
    /// let mut state = LayoutPositionState::default();
    /// state.sample([0., 0.], Transition::default(), Duration::ZERO, false);
    /// let moved = state.sample([100., 40.], Transition::default(), Duration::ZERO, false);
    /// assert_eq!(moved.position(), [0., 0.]);
    /// assert_eq!(moved.offset(), [-100., -40.]);
    /// let done = state.sample([100., 40.], Transition::default(), Duration::from_secs(5), false);
    /// assert_eq!(done.offset(), [0., 0.]);
    /// ```
    pub fn sample(
        &mut self,
        origin: [f64; 2],
        transition: Transition,
        now: Duration,
        reduced_motion: bool,
    ) -> LayoutPositionFrame {
        self.sample_axes(origin, [transition; 2], now, reduced_motion)
    }

    /// Samples independently timed axes. Instant axes never schedule interpolation;
    /// useful for constrained sizing and one-dimensional layout followers.
    pub fn sample_axes(
        &mut self,
        origin: [f64; 2],
        transitions: [Transition; 2],
        now: Duration,
        reduced_motion: bool,
    ) -> LayoutPositionFrame {
        assert!(
            origin.iter().all(|v| v.is_finite()),
            "layout origin must be finite"
        );

        if !self.initialized {
            for (axis, value) in self.axes.iter_mut().zip(origin) {
                axis.jump(value, now);
            }

            self.initialized = true;
        }

        let mut active = false;
        let mut completed = false;
        let mut velocity = [0.; 2];

        let position = std::array::from_fn(|i| {
            self.axes[i].animate(origin[i], transitions[i], now);
            let frame = self.axes[i].sample(now, reduced_motion);

            active |= frame.is_animating();
            completed |= frame.just_completed();
            velocity[i] = frame.velocity();

            frame.value()
        });

        LayoutPositionFrame {
            origin,
            position,
            velocity,
            active,
            completed: completed && !active,
        }
    }
    /// Forgets measurement history. The next measurement mounts immediately.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
