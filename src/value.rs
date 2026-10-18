//! Scalar presentation values, explicit updates, and bounded inertial release.
use crate::{Spring, Transition};
use std::time::Duration;

/// Deceleration and optional hard bounds for inertial motion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Inertia {
    tau: f64,
    bounds: Option<(f64, f64)>,
    spring: Spring,
}

impl Inertia {
    /// Creates exponential deceleration. Panics for a zero time constant.
    pub fn new(time_constant: Duration) -> Self {
        assert!(
            !time_constant.is_zero(),
            "inertia time constant must be positive"
        );

        Self {
            tau: time_constant.as_secs_f64(),
            bounds: None,
            spring: Spring::default(),
        }
    }

    /// Sets finite ordered bounds. Crossing a bound switches analytically to a spring.
    pub fn bounds(mut self, min: f64, max: f64) -> Self {
        assert!(
            min.is_finite() && max.is_finite() && min <= max,
            "invalid inertia bounds"
        );
        self.bounds = Some((min, max));

        self
    }

    /// Chooses the spring used at a boundary.
    pub fn spring(mut self, spring: Spring) -> Self {
        self.spring = spring;

        self
    }

    /// Projects a resting position and clamps it to configured bounds.
    pub fn project(self, position: f64, velocity: f64) -> f64 {
        assert!(
            position.is_finite() && velocity.is_finite(),
            "non-finite projection"
        );
        let value = finite(position + velocity * self.tau);

        self.bounds
            .map_or(value, |(min, max)| value.clamp(min, max))
    }
}

impl Default for Inertia {
    fn default() -> Self {
        Self::new(Duration::from_millis(350))
    }
}

/// A scalar presentation and its velocity, in caller-chosen units per second.
#[derive(Clone, Copy, Debug)]
pub struct ValueFrame {
    value: f64,
    velocity: f64,
    active: bool,
    completed: bool,
}

impl ValueFrame {
    /// Current presentation value.
    pub fn value(self) -> f64 {
        self.value
    }

    /// Current velocity in units per second.
    pub fn velocity(self) -> f64 {
        self.velocity
    }

    /// Whether another sample is required to complete motion.
    pub fn is_animating(self) -> bool {
        self.active
    }

    /// A completion edge, consumed by this sample.
    pub fn just_completed(self) -> bool {
        self.completed
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    Transition,
    Inertia(Inertia),
}

/// A retained scalar motion value, with no timers, observers, or heap allocations.
/// Feed its samples into any native style, paint parameter, text, or derived value.
/// Arithmetic at numeric extremes saturates to finite limits; indeterminate overflow
/// resolves to zero. Use ordinary UI-sized units for meaningful physical trajectories.
#[derive(Clone, Debug)]
pub struct MotionValue {
    value: f64,
    velocity: f64,
    from: f64,
    initial_velocity: f64,
    target: f64,
    transition: Transition,
    delay: Duration,
    started: Duration,
    last: Duration,
    set_time: Duration,
    active: bool,
    completed: bool,
    mode: Mode,
}

impl MotionValue {
    /// Creates a resting value. Panics for non-finite input.
    pub fn new(value: f64) -> Self {
        assert!(value.is_finite(), "motion value must be finite");

        Self {
            value,
            velocity: 0.0,
            from: value,
            initial_velocity: 0.0,
            target: value,
            transition: Transition::Instant,
            delay: Duration::ZERO,
            started: Duration::ZERO,
            last: Duration::ZERO,
            set_time: Duration::ZERO,
            active: false,
            completed: false,
            mode: Mode::Transition,
        }
    }

    /// Last sampled value; this does not advance time.
    pub fn get(&self) -> f64 {
        self.value
    }

    pub(crate) fn target(&self) -> f64 {
        self.target
    }

    /// Last sampled velocity.
    pub fn velocity(&self) -> f64 {
        self.velocity
    }

    /// Directly sets a value and estimates velocity from elapsed time. Cancels animation.
    pub fn set(&mut self, value: f64, now: Duration) {
        assert!(value.is_finite(), "motion value must be finite");
        let now = now.max(self.last);
        let dt = now.saturating_sub(self.set_time).as_secs_f64();

        self.velocity = if dt > 0.0 {
            finite((value - self.value) / dt)
        } else {
            self.velocity
        };
        self.value = value;
        self.target = value;
        self.from = value;
        self.active = false;
        self.completed = false;
        self.last = now;
        self.set_time = now;
        self.mode = Mode::Transition;
    }

    /// Assigns a value, clears momentum, and cancels animation without a completion event.
    pub fn jump(&mut self, value: f64, now: Duration) {
        self.set(value, now);
        self.velocity = 0.0;
    }

    /// Retargets from the presentation at `now`. Identical declarations do not restart.
    pub fn animate(&mut self, target: f64, transition: Transition, now: Duration) {
        self.animate_delayed(target, transition, Duration::ZERO, now);
    }

    /// Retargets with a delay, preserving incoming spring velocity across the hold.
    pub fn animate_delayed(
        &mut self,
        target: f64,
        transition: Transition,
        delay: Duration,
        now: Duration,
    ) {
        assert!(target.is_finite(), "motion target must be finite");
        let now = now.max(self.last);

        if self.mode == Mode::Transition
            && self.target == target
            && self.transition == transition
            && self.delay == delay
        {
            return;
        }

        self.advance(now);
        self.from = self.value;
        self.initial_velocity = self.velocity;
        self.target = target;
        self.transition = transition;
        self.delay = delay;
        self.started = now;
        self.active = self.from != target || self.velocity != 0.0;
        self.mode = Mode::Transition;
        self.completed = false;
    }

    /// Starts exponential inertia from the last presentation and velocity.
    /// Bounds switch to a spring at the exact crossing time, independent of frame rate.
    pub fn coast(&mut self, inertia: Inertia, now: Duration) {
        let now = now.max(self.last);

        self.advance(now);
        self.from = self.value;
        self.initial_velocity = self.velocity;
        self.target = inertia.project(self.value, self.velocity);
        self.started = now;
        self.delay = Duration::ZERO;
        self.mode = Mode::Inertia(inertia);
        self.active = self.value != self.target || self.velocity != 0.0;
        self.completed = false;
    }

    /// Samples a monotonic clock. Reduced motion finishes immediately, including inertia.
    pub fn sample(&mut self, now: Duration, reduced_motion: bool) -> ValueFrame {
        if reduced_motion && self.active {
            self.finish();
        } else {
            self.advance(now.max(self.last));
        }

        self.last = self.last.max(now);
        let completed = std::mem::take(&mut self.completed);

        ValueFrame {
            value: self.value,
            velocity: self.velocity,
            active: self.active,
            completed,
        }
    }

    fn finish(&mut self) {
        self.value = self.target;
        self.velocity = 0.0;
        self.completed |= self.active;
        self.active = false;
    }

    fn advance(&mut self, now: Duration) {
        self.last = now;

        if !self.active {
            return;
        }

        let elapsed = now.saturating_sub(self.started);

        if elapsed < self.delay {
            self.velocity = 0.0;

            return;
        }

        let t = elapsed.saturating_sub(self.delay).as_secs_f64();

        let (value, velocity, done) = match self.mode {
            Mode::Transition => sample_transition(
                self.from,
                self.initial_velocity,
                self.target,
                self.transition,
                t,
            ),
            Mode::Inertia(config) => {
                let amplitude = finite(self.initial_velocity * config.tau);
                let end = finite(self.from + amplitude);

                let boundary = config.bounds.and_then(|(min, max)| {
                    if self.from < min {
                        Some((min, 0.0))
                    } else if self.from > max {
                        Some((max, 0.0))
                    } else if end < min || end > max {
                        let bound = if end < min { min } else { max };
                        let crossing = -config.tau * (1.0 - (bound - self.from) / amplitude).ln();

                        Some((bound, crossing.max(0.0)))
                    } else {
                        None
                    }
                });

                if let Some((bound, crossing)) = boundary
                    && t >= crossing
                {
                    let from = if crossing == 0.0 { self.from } else { bound };
                    let velocity = self.initial_velocity * (-crossing / config.tau).exp();

                    sample_transition(
                        from,
                        velocity,
                        bound,
                        Transition::Spring(config.spring),
                        t - crossing,
                    )
                } else {
                    let decay = (-t / config.tau).exp();
                    let value = finite(self.from + amplitude * (1.0 - decay));
                    let velocity = finite(self.initial_velocity * decay);

                    (
                        value,
                        velocity,
                        (end - value).abs() <= 0.001 && velocity.abs() <= 0.001,
                    )
                }
            }
        };

        self.value = value;
        self.velocity = velocity;
        self.set_time = now;

        if done {
            self.finish();
        }
    }
}

pub(crate) fn sample_transition(
    from: f64,
    velocity: f64,
    target: f64,
    transition: Transition,
    t: f64,
) -> (f64, f64, bool) {
    match transition {
        Transition::Instant => (target, 0.0, true),
        Transition::Tween(duration, easing) => {
            let seconds = duration.as_secs_f64();

            if t >= seconds {
                return (target, 0.0, true);
            }

            let (progress, derivative) = easing.sample(t / seconds);

            (
                if t == 0.0 {
                    from
                } else {
                    finite(from * (1.0 - progress) + target * progress)
                },
                finite(finite(target - from) * derivative / seconds),
                false,
            )
        }
        Transition::Spring(spring) => {
            if t == 0.0 {
                return (from, velocity, false);
            }

            let m = spring.matrix(t);
            let displacement = finite(from - target);
            let value = finite(target + m[0] * displacement + m[1] * velocity);
            let velocity = finite(m[2] * displacement + m[3] * velocity);

            (
                value,
                velocity,
                spring.is_settled(value, velocity, target, 0.001),
            )
        }
    }
}

pub(crate) fn finite(value: f64) -> f64 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(-f64::MAX, f64::MAX)
    }
}
