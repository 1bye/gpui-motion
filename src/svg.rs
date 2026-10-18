//! Retained scale/rotation values for the stock GPUI CE SVG drawing path.
use crate::{MotionValue, Transition};
use std::time::Duration;
#[cfg(feature = "gpui")]
use std::time::Instant;

/// SVG-only scale, rotation and translation destination. Defaults to unit scale and zero rotation.
/// Negative scale mirrors an axis; zero collapses it. Rotation is unwrapped degrees:
/// `0 -> 720` makes two turns; use `350 -> 370` to cross zero by the short route.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SvgTarget {
    values: [f64; 5],
}
impl Default for SvgTarget {
    fn default() -> Self {
        Self {
            values: [1.0, 1.0, 0.0, 0.0, 0.0],
        }
    }
}
impl SvgTarget {
    /// Sets equal scale on both axes. Panics for non-finite input.
    pub fn scale(self, factor: f32) -> Self {
        self.scale_xy(factor, factor)
    }
    /// Sets independent signed scale factors. Panics for non-finite input.
    pub fn scale_xy(mut self, x: f32, y: f32) -> Self {
        assert!(x.is_finite() && y.is_finite(), "SVG scale must be finite");
        self.values[0] = f64::from(x);
        self.values[1] = f64::from(y);

        self
    }
    /// Sets an absolute unwrapped angle in degrees. Panics for non-finite input.
    pub fn rotate_degrees(mut self, degrees: f64) -> Self {
        assert!(degrees.is_finite(), "SVG rotation must be finite");
        self.values[2] = degrees;

        self
    }
    /// Sets a drawing-only logical-pixel translation. Layout and hitboxes stay fixed.
    pub fn translate(mut self, x: f32, y: f32) -> Self {
        assert!(
            x.is_finite() && y.is_finite(),
            "SVG translation must be finite"
        );
        self.values[3] = x.into();
        self.values[4] = y.into();

        self
    }
    /// Presented drawing translation in logical pixels.
    pub fn translation(self) -> [f32; 2] {
        [self.values[3], self.values[4]]
            .map(|v| v.clamp(f64::from(f32::MIN), f64::from(f32::MAX)) as f32)
    }
    /// Drawable signed scale factors, saturated to finite `f32` limits.
    pub fn scale_factors(self) -> [f32; 2] {
        [self.values[0], self.values[1]]
            .map(|v| v.clamp(f64::from(f32::MIN), f64::from(f32::MAX)) as f32)
    }
    /// Unwrapped rotation in degrees, including whole turns.
    pub fn rotation_degrees(self) -> f64 {
        self.values[2]
    }
}

/// Disposable SVG transform declaration; retain one [`SvgMotionState`] per logical SVG.
/// This does not transform a GPUI subtree or add native children to an SVG.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SvgMotion {
    target: SvgTarget,
    initial: Option<SvgTarget>,
    transition: Transition,
    delay: Duration,
}
impl SvgMotion {
    /// Declares a destination. First mount is immediate unless `initial` is supplied.
    pub fn new(target: SvgTarget) -> Self {
        Self {
            target,
            initial: None,
            transition: Transition::default(),
            delay: Duration::ZERO,
        }
    }
    /// Sets the first presentation, used only once by the retained state.
    pub fn initial(mut self, initial: SvgTarget) -> Self {
        self.initial = Some(initial);

        self
    }
    /// Chooses spring, tween, or instant motion for the five transform channels.
    pub fn transition(mut self, transition: Transition) -> Self {
        self.transition = transition;

        self
    }
    /// Holds the presentation before each new destination. Identical declarations do not restart.
    pub fn delay(mut self, delay: Duration) -> Self {
        self.delay = delay;

        self
    }
}

/// A sampled SVG drawing transform. No layout, clipping, or hitbox changes are implied.
#[derive(Clone, Copy, Debug)]
pub struct SvgFrame {
    target: SvgTarget,
    velocity: [f64; 3],
    translation_velocity: [f64; 2],
    active: bool,
    completed: bool,
}
impl SvgFrame {
    /// Presented signed scales, unwrapped rotation and drawing translation.
    pub fn target(self) -> SvgTarget {
        self.target
    }
    /// X scale/second, Y scale/second, and degrees/second.
    pub fn velocity(self) -> [f64; 3] {
        self.velocity
    }
    /// Drawing translation velocity, logical pixels/second.
    pub fn translation_velocity(self) -> [f64; 2] {
        self.translation_velocity
    }
    /// Whether another frame is needed. False after settling or a reduced-motion snap.
    pub fn is_animating(self) -> bool {
        self.active
    }
    /// Completion edge when the final active channel finishes; consumed by this sample.
    pub fn just_completed(self) -> bool {
        self.completed
    }
}

/// Allocation-free SVG transform state using the same analytic scalar engine as native motion.
/// Springs preserve position and velocity at interruption; tweens preserve position.
#[derive(Debug)]
pub struct SvgMotionState {
    channels: [MotionValue; 5],
    initialized: bool,
    #[cfg(feature = "gpui")]
    pub(crate) epoch: Instant,
}
impl Default for SvgMotionState {
    fn default() -> Self {
        Self {
            channels: SvgTarget::default().values.map(MotionValue::new),
            initialized: false,
            #[cfg(feature = "gpui")]
            epoch: Instant::now(),
        }
    }
}
impl SvgMotionState {
    /// Samples a monotonic application timestamp. Backward time holds; identical targets
    /// do not restart. Reduced motion snaps to the destination and completes once.
    /// Do not mix this epoch with the optional native `frame` clock on the same state.
    ///
    /// ```
    /// use gpui_ce_motion::{SvgMotion, SvgMotionState, SvgTarget};
    /// use std::time::Duration;
    /// let mut state = SvgMotionState::default();
    /// let motion = SvgMotion::new(SvgTarget::default().scale(1.5).rotate_degrees(90.0))
    ///     .initial(SvgTarget::default());
    /// assert!(state.sample(motion, Duration::ZERO, false).is_animating());
    /// let settled = state.sample(motion, Duration::from_secs(5), false);
    /// assert_eq!(settled.target().scale_factors(), [1.5, 1.5]);
    /// assert_eq!(settled.target().rotation_degrees(), 90.0);
    /// assert!(!settled.is_animating());
    /// ```
    pub fn sample(&mut self, motion: SvgMotion, now: Duration, reduced_motion: bool) -> SvgFrame {
        if !self.initialized {
            let initial = motion.initial.unwrap_or(motion.target);

            for (channel, value) in self.channels.iter_mut().zip(initial.values) {
                channel.jump(value, now);
            }

            self.initialized = true;
        }

        let mut active = false;
        let mut completed = false;
        let mut velocity = [0.0; 5];

        let values = std::array::from_fn(|ix| {
            self.channels[ix].animate_delayed(
                motion.target.values[ix],
                motion.transition,
                motion.delay,
                now,
            );
            let frame = self.channels[ix].sample(now, reduced_motion);

            active |= frame.is_animating();
            completed |= frame.just_completed();
            velocity[ix] = frame.velocity();

            frame.value()
        });

        SvgFrame {
            target: SvgTarget { values },
            velocity: [velocity[0], velocity[1], velocity[2]],
            translation_velocity: [velocity[3], velocity[4]],
            active,
            completed: completed && !active,
        }
    }
}
