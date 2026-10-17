use std::time::Duration;
#[cfg(feature = "gpui")]
use std::time::Instant;

use crate::{MotionValue, Transition};

const CHANNELS: usize = 6;

/// Opacity, offsets, and optional width, height, and border radius in logical pixels.
///
/// Builders panic for non-finite inputs, negative layout values, or opacity
/// outside `0..=1`. Validate
/// external input before building a target. Presentation opacity is clamped;
/// internal spring values and velocity remain unclamped for continuous retargeting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    layout: u8,
    values: [f64; CHANNELS],
}

impl Default for Target {
    fn default() -> Self {
        Self {
            layout: 0,
            values: [0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
        }
    }
}

impl Target {
    /// Sets the horizontal logical-pixel offset. Panics for non-finite values.
    pub fn x(mut self, value: f32) -> Self {
        assert!(value.is_finite(), "x must be finite");
        self.values[0] = value.into();
        self.layout |= 1;
        self
    }

    /// Sets the vertical logical-pixel offset. Panics for non-finite values.
    pub fn y(mut self, value: f32) -> Self {
        assert!(value.is_finite(), "y must be finite");
        self.values[1] = value.into();
        self.layout |= 2;
        self
    }

    /// Sets opacity. Panics unless finite and in `0..=1`.
    pub fn opacity(mut self, value: f32) -> Self {
        assert!((0.0..=1.0).contains(&value), "opacity must be in 0..=1");
        self.values[2] = value.into();
        self.layout |= 4;
        self
    }

    /// Sets width in logical pixels. Panics unless finite and non-negative.
    /// Omitted layout properties inherit their previous destination; on first
    /// mount they leave the element's own style untouched.
    pub fn width(self, value: f32) -> Self {
        self.with_layout(3, value)
    }

    /// Sets height in logical pixels. Panics unless finite and non-negative.
    pub fn height(self, value: f32) -> Self {
        self.with_layout(4, value)
    }

    /// Sets all four corner radii in logical pixels.
    /// Panics unless finite and non-negative. This does not enable clipping.
    pub fn border_radius(self, value: f32) -> Self {
        self.with_layout(5, value)
    }

    /// Width, if declared, clamped to non-negative finite drawable values.
    pub fn width_px(self) -> Option<f32> {
        self.layout_value(3)
    }
    /// Height, if declared, clamped to non-negative finite drawable values.
    pub fn height_px(self) -> Option<f32> {
        self.layout_value(4)
    }
    /// Corner radius, if declared, clamped to non-negative finite drawable values.
    pub fn border_radius_px(self) -> Option<f32> {
        self.layout_value(5)
    }

    fn with_layout(mut self, ix: usize, value: f32) -> Self {
        assert!(
            value.is_finite() && value >= 0.0,
            "layout targets must be finite and non-negative"
        );
        self.values[ix] = value.into();
        self.layout |= 1 << ix;
        self
    }

    fn layout_value(self, ix: usize) -> Option<f32> {
        (self.layout & (1 << ix) != 0)
            .then(|| self.values[ix].clamp(0.0, f64::from(f32::MAX)) as f32)
    }

    fn with_layout_fallback(mut self, fallback: Self) -> Self {
        for ix in 3..CHANNELS {
            let bit = 1 << ix;
            if self.layout & bit == 0 && fallback.layout & bit != 0 {
                self.values[ix] = fallback.values[ix];
                self.layout |= bit;
            }
        }
        self
    }

    /// Overlays only explicitly set properties, for variants and interaction targets.
    /// Default x/y/opacity values in an otherwise empty patch do not override the base.
    pub fn overlay(mut self, patch: Self) -> Self {
        for ix in 0..CHANNELS {
            if patch.layout & (1 << ix) != 0 {
                self.values[ix] = patch.values[ix];
            }
        }
        self.layout |= patch.layout;
        self
    }
    pub(crate) fn values(self) -> [f64; CHANNELS] {
        self.values
    }
    pub(crate) fn from_values(values: [f64; CHANNELS], layout: u8) -> Self {
        Self { values, layout }
    }
    pub(crate) fn mask(self) -> u8 {
        self.layout
    }
    pub(crate) fn inherit(self, base: Self) -> Self {
        self.with_layout_fallback(base)
    }

    /// Horizontal offset in logical pixels, saturated to finite `f32` limits.
    pub fn x_px(self) -> f32 {
        self.values[0].clamp(f64::from(f32::MIN), f64::from(f32::MAX)) as f32
    }
    /// Vertical offset in logical pixels, saturated to finite `f32` limits.
    pub fn y_px(self) -> f32 {
        self.values[1].clamp(f64::from(f32::MIN), f64::from(f32::MAX)) as f32
    }
    /// Opacity, clamped to the drawable range.
    pub fn alpha(self) -> f32 {
        self.values[2].clamp(0.0, 1.0) as f32
    }
}

/// A disposable declaration of desired state. Keep its [`MotionState`] alive.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    target: Target,
    initial: Option<Target>,
    exit: Target,
    transition: Transition,
    present: bool,
    delay: Duration,
    overrides: [Option<(Transition, Duration)>; CHANNELS],
}

impl Motion {
    /// Creates a declaration. Without `initial`, the first mount is immediate.
    /// Default exit fades out at the original position using the same transition.
    pub fn new(target: Target) -> Self {
        Self {
            target,
            initial: None,
            exit: target.opacity(0.0),
            transition: Transition::default(),
            present: true,
            delay: Duration::ZERO,
            overrides: [None; CHANNELS],
        }
    }

    /// Reads the declared destination, before interaction overrides.
    pub fn target(self) -> Target {
        self.target
    }
    /// Replaces the destination while retaining transition, entrance, and exit policy.
    pub fn with_target(mut self, target: Target) -> Self {
        self.target = target;
        self
    }
    /// Sets the first mount's presentation, used only once per retained state.
    pub fn initial(mut self, value: Target) -> Self {
        self.initial = Some(value);
        self
    }
    /// Sets the exit target; re-entry starts from the current exit presentation.
    pub fn exit(mut self, value: Target) -> Self {
        self.exit = value;
        self
    }
    /// Sets the transition. A change interrupts the current transition.
    pub fn transition(mut self, value: Transition) -> Self {
        self.transition = value;
        self
    }
    /// Delays each new target. An exit uses the same delay unless overridden.
    pub fn delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }
    /// Sets a property's independent transition and delay.
    pub fn transition_for(
        mut self,
        property: Property,
        transition: Transition,
        delay: Duration,
    ) -> Self {
        self.overrides[property as usize] = Some((transition, delay));
        self
    }
    /// Requests mount or removal. Continue sampling while false to finish exit.
    pub fn present(mut self, value: bool) -> Self {
        self.present = value;
        self
    }
}

/// A typed property for per-property transition configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Property {
    /// Horizontal logical-pixel offset.
    X,
    /// Vertical logical-pixel offset.
    Y,
    /// Opacity.
    Opacity,
    /// Width in logical pixels.
    Width,
    /// Height in logical pixels.
    Height,
    /// Corner radius in logical pixels.
    BorderRadius,
}

/// Lifecycle of a retained element. Exit completes only after all channels settle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Presence {
    /// First mount or re-entry is moving toward the visible target.
    Entering,
    /// Mounted; may still be transitioning between visible targets.
    Present,
    /// Removal requested; the element must remain rendered until settled.
    Exiting,
    /// Exit completed; omit the element from layout, painting, and input.
    Absent,
}

/// A read-only presentation snapshot. It owns no tasks or GPUI resources.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    target: Target,
    velocity: [f64; CHANNELS],
    presence: Presence,
    animating: bool,
    completed: bool,
}

impl Frame {
    /// Creates a static presentation, useful for scroll, drag, and custom value bindings.
    pub fn at(target: Target) -> Self {
        Self {
            target,
            velocity: [0.0; CHANNELS],
            presence: Presence::Present,
            animating: false,
            completed: false,
        }
    }
    pub(crate) fn animated(
        target: Target,
        velocity: [f64; CHANNELS],
        active: bool,
        completed: bool,
    ) -> Self {
        Self {
            target,
            velocity,
            presence: Presence::Present,
            animating: active,
            completed,
        }
    }
    /// Completion edge. Read once per view render to dispatch an owner-level event.
    pub fn just_completed(self) -> bool {
        self.completed
    }
    /// Current presentation values, rather than the declaration's destination.
    pub fn target(self) -> Target {
        self.target
    }
    /// Current x, y, and opacity velocities per second, before drawing clamps.
    pub fn velocity(self) -> [f64; 3] {
        [self.velocity[0], self.velocity[1], self.velocity[2]]
    }
    /// Width, height, and radius velocities in logical pixels per second.
    /// Undeclared channels return `None`; velocities are before drawing clamps.
    pub fn layout_velocity(self) -> [Option<f64>; 3] {
        std::array::from_fn(|ix| {
            (self.target.layout & (1 << (ix + 3)) != 0).then_some(self.velocity[ix + 3])
        })
    }
    /// Current lifecycle phase.
    pub fn presence(self) -> Presence {
        self.presence
    }
    /// Whether another animation frame is needed.
    pub fn is_animating(self) -> bool {
        self.animating
    }
    /// Whether the element must remain in the rendered tree, including its exit.
    pub fn is_mounted(self) -> bool {
        self.presence != Presence::Absent
    }
    /// Whether the model requests presence. Disable exit-time input using this.
    pub fn is_present(self) -> bool {
        matches!(self.presence, Presence::Entering | Presence::Present)
    }
}

/// Retained motion for one logical object, owned by a view or keyed collection.
///
/// Dropping this value cancels motion. Retain it through exit and reuse it for
/// re-entry; replacing it with `Default` explicitly resets mount history. Sampling
/// is O(1), uses no heap, and does not advance settled trajectories.
#[derive(Debug)]
pub struct MotionState {
    layout: u8,
    channels: [MotionValue; CHANNELS],
    presence: Presence,
    initialized: bool,
    #[cfg(feature = "gpui")]
    pub(crate) epoch: Instant,
}
impl Default for MotionState {
    fn default() -> Self {
        Self {
            layout: 0,
            channels: std::array::from_fn(|ix| MotionValue::new(if ix == 2 { 1.0 } else { 0.0 })),
            presence: Presence::Absent,
            initialized: false,
            #[cfg(feature = "gpui")]
            epoch: Instant::now(),
        }
    }
}
impl MotionState {
    /// Samples absolute monotonic time. Identical targets do not restart. Each
    /// property retargets independently, preserving spring velocity. Omitted layout
    /// targets retain their previous destination. Reduced motion snaps and exits.
    /// Do not mix this clock with the native `frame` clock on the same state.
    pub fn sample(&mut self, motion: Motion, now: Duration, reduced_motion: bool) -> Frame {
        if !self.initialized {
            self.initialized = true;
            let target = motion
                .target
                .with_layout_fallback(motion.initial.unwrap_or_default());
            let initial = if motion.present {
                motion
                    .initial
                    .unwrap_or(target)
                    .with_layout_fallback(target)
            } else {
                motion.exit.with_layout_fallback(target)
            };
            self.layout = initial.layout;
            for ix in 0..CHANNELS {
                self.channels[ix].jump(initial.values[ix], now);
            }
            self.presence = if motion.present {
                Presence::Entering
            } else {
                Presence::Absent
            };
        }
        let previous_presence = self.presence;
        let was_present = matches!(self.presence, Presence::Present | Presence::Entering);
        let requested = if motion.present {
            motion.target
        } else {
            motion.exit
        }
        .with_layout_fallback(Target {
            values: std::array::from_fn(|ix| self.channels[ix].target()),
            layout: self.layout,
        });
        let added = requested.layout & !self.layout;
        for ix in 3..CHANNELS {
            if added & (1 << ix) != 0 {
                self.channels[ix].jump(requested.values[ix], now);
            }
        }
        self.layout = requested.layout;
        if was_present != motion.present {
            self.presence = if motion.present {
                Presence::Entering
            } else {
                Presence::Exiting
            };
        }
        let mut active = false;
        let mut completed = false;
        let mut position = [0.0; CHANNELS];
        let mut velocity = [0.0; CHANNELS];
        for ix in 0..CHANNELS {
            let (transition, delay) =
                motion.overrides[ix].unwrap_or((motion.transition, motion.delay));
            self.channels[ix].animate_delayed(requested.values[ix], transition, delay, now);
            let value = self.channels[ix].sample(now, reduced_motion);
            position[ix] = value.value();
            velocity[ix] = value.velocity();
            active |= value.is_animating();
            completed |= value.just_completed();
        }
        if !active {
            self.presence = if motion.present {
                Presence::Present
            } else {
                Presence::Absent
            };
        }
        completed &= !active;
        completed |= self.presence == Presence::Absent && previous_presence != Presence::Absent;
        Frame {
            target: Target {
                values: position,
                layout: self.layout | 7,
            },
            velocity,
            presence: self.presence,
            animating: active,
            completed,
        }
    }
}
