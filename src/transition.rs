use std::{error::Error, fmt, time::Duration};

/// Easing curves for duration-based transitions.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[non_exhaustive]
pub enum Easing {
    /// Constant speed.
    Linear,
    /// Cubic ease in.
    EaseIn,
    /// Application easing with an explicit derivative, prepared once.
    Custom(CustomEasing),
    /// Custom cubic Bezier with monotonic time control points.
    Bezier(Bezier),
    /// Circular ease in.
    CircIn,
    /// Circular ease out.
    CircOut,
    /// Circular ease in and out.
    CircInOut,
    /// Overshooting ease in.
    BackIn,
    /// Overshooting ease out.
    BackOut,
    /// Overshooting ease in and out.
    BackInOut,
    /// Anticipation followed by exponential deceleration.
    Anticipate,
    /// Cubic ease out.
    #[default]
    EaseOut,
    /// Cubic smoothstep, with zero velocity at both endpoints.
    EaseInOut,
}

impl Easing {
    pub(crate) fn sample(self, t: f64) -> (f64, f64) {
        match self {
            Self::Linear => (t, 1.0),
            Self::Custom(curve) => {
                let result = (curve.sample)(t);

                assert!(
                    result.0.is_finite() && result.1.is_finite(),
                    "custom easing must return finite value/derivative"
                );

                result
            }
            Self::EaseIn => (t * t * t, 3.0 * t * t),
            Self::Bezier(curve) => curve.sample(t),
            Self::CircIn => {
                let root = (1.0 - t * t).max(1e-12).sqrt();

                (1.0 - root, t / root)
            }
            Self::CircOut => {
                let root = (1.0 - (t - 1.0).powi(2)).max(1e-12).sqrt();

                (root, (1.0 - t) / root)
            }
            Self::CircInOut => {
                if t < 0.5 {
                    let (v, d) = Self::CircIn.sample(t * 2.);

                    (v * 0.5, d)
                } else {
                    let (v, d) = Self::CircOut.sample(t * 2. - 1.);

                    (0.5 + v * 0.5, d)
                }
            }
            Self::BackIn => {
                let (v, d) = Self::BackOut.sample(1. - t);

                (1. - v, d)
            }
            Self::BackInOut => {
                if t < 0.5 {
                    let (v, d) = Self::BackIn.sample(t * 2.);

                    (v * 0.5, d)
                } else {
                    let (v, d) = Self::BackOut.sample(t * 2. - 1.);

                    (0.5 + v * 0.5, d)
                }
            }
            Self::Anticipate => {
                if t < 0.5 {
                    let (v, d) = Self::BackIn.sample(t * 2.);

                    (v * 0.5, d)
                } else {
                    let decay = 2_f64.powf(-20. * (t - 0.5));

                    (1. - 0.5 * decay, 10. * std::f64::consts::LN_2 * decay)
                }
            }
            Self::BackOut => {
                let u = t - 1.0;

                (
                    1.0 + 2.70158 * u.powi(3) + 1.70158 * u * u,
                    8.10474 * u * u + 3.40316 * u,
                )
            }
            Self::EaseOut => (1.0 - (1.0 - t).powi(3), 3.0 * (1.0 - t).powi(2)),
            Self::EaseInOut => (t * t * (3.0 - 2.0 * t), 6.0 * t * (1.0 - t)),
        }
    }
}

/// Prepared application easing. Copies preserve identity so repeated declarations
/// do not restart. Call `new` once, not during rendering. Equality compares the
/// generated identity rather than unreliable function-pointer equality.
#[derive(Clone, Copy, Debug)]
pub struct CustomEasing {
    sample: fn(f64) -> (f64, f64),
    identity: u64,
}

impl PartialEq for CustomEasing {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
    }
}

impl CustomEasing {
    /// Validates finite endpoints at 0 and 1 and exact endpoint values 0/1.
    /// The callback must return finite `(value, derivative)` throughout `[0,1]`;
    /// violating that programmer contract panics during sampling. Overshoot is allowed.
    pub fn new(sample: fn(f64) -> (f64, f64)) -> Result<Self, InvalidEasing> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let start = sample(0.);
        let end = sample(1.);

        if start.0 != 0. || end.0 != 1. || ![start.1, end.1].iter().all(|v| v.is_finite()) {
            return Err(InvalidEasing);
        }

        Ok(Self {
            sample,
            identity: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        })
    }
}

/// A settling damped spring. Parameters use seconds and animated logical units.
///
/// Damping must be strictly positive so presence can eventually finish exiting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    stiffness: f64,
    damping: f64,
    mass: f64,
    rest: Option<[f64; 2]>,
}

impl Spring {
    /// Validates positive finite stiffness, damping, and mass.
    pub fn new(stiffness: f32, damping: f32, mass: f32) -> Result<Self, InvalidSpring> {
        if [stiffness, damping, mass]
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
        {
            return Err(InvalidSpring);
        }

        Ok(Self {
            stiffness: stiffness.into(),
            damping: damping.into(),
            mass: mass.into(),
            rest: None,
        })
    }

    /// Builds a spring whose first arrival at 90% of a resting step is `duration`.
    /// Bounce in `-1..1` chooses damping ratio `1 - bounce`; negative values
    /// are overdamped. This is a native visual timing convention, not Motion's
    /// exact parameter solver. Preparation solves once; sampling stays analytic.
    pub fn visual(duration: Duration, bounce: f32) -> Result<Self, InvalidSpring> {
        if duration.is_zero() || !bounce.is_finite() || !(-1.0..1.0).contains(&bounce) {
            return Err(InvalidSpring);
        }

        let ratio = 1.0 - f64::from(bounce);

        let unit = Self {
            stiffness: 1.0,
            damping: 2.0 * ratio,
            mass: 1.0,
            rest: None,
        };

        let mut high = 1.0;

        while unit.matrix(high)[0] > 0.1 {
            high *= 2.0;
        }

        let mut low = 0.0;

        for _ in 0..64 {
            let mid = (low + high) * 0.5;

            if unit.matrix(mid)[0] > 0.1 {
                low = mid;
            } else {
                high = mid;
            }
        }

        let omega = high / duration.as_secs_f64();

        Ok(Self {
            stiffness: omega * omega,
            damping: 2.0 * ratio * omega,
            mass: 1.0,
            rest: None,
        })
    }

    /// Sets positive finite settling tolerances: distance units and units/second.
    /// The default remains 0.001 units with frequency-scaled velocity tolerance.
    pub fn rest(mut self, distance: f64, speed: f64) -> Result<Self, InvalidSpring> {
        if ![distance, speed].iter().all(|v| v.is_finite() && *v > 0.0) {
            return Err(InvalidSpring);
        }

        self.rest = Some([distance, speed]);

        Ok(self)
    }

    // Exact solution of m*x'' + c*x' + k*x = 0, evaluated once for all channels.
    pub(crate) fn matrix(self, seconds: f64) -> [f64; 4] {
        let omega = (self.stiffness / self.mass).sqrt();
        let decay = self.damping / (2.0 * self.mass);
        let discriminant = decay * decay - omega * omega;

        let (a, b) = if discriminant.abs() <= omega * omega * 1e-10 {
            let exponential = (-decay * seconds).exp();

            (exponential, exponential * seconds)
        } else if discriminant < 0.0 {
            let frequency = (-discriminant).sqrt();
            let (sine, cosine) = (frequency * seconds).sin_cos();
            let exponential = (-decay * seconds).exp();

            (exponential * cosine, exponential * sine / frequency)
        } else {
            let root = discriminant.sqrt();
            // The quotient avoids catastrophic cancellation in -decay + root.
            let slow = -omega * omega / (decay + root);
            let fast = -decay - root;
            let slow_exp = (slow * seconds).exp();
            let fast_exp = (fast * seconds).exp();

            (
                (slow_exp + fast_exp) * 0.5,
                (slow_exp - fast_exp) / (2.0 * root),
            )
        };

        [a + decay * b, b, -omega * omega * b, a - decay * b]
    }

    pub(crate) fn is_settled(
        self,
        position: f64,
        velocity: f64,
        target: f64,
        epsilon: f64,
    ) -> bool {
        let [distance, speed] = self
            .rest
            .unwrap_or([epsilon, epsilon * (self.stiffness / self.mass).sqrt()]);

        (position - target).abs() <= distance && velocity.abs() <= speed
    }
}

impl Default for Spring {
    fn default() -> Self {
        Self {
            stiffness: 240.0,
            damping: 26.0,
            mass: 1.0,
            rest: None,
        }
    }
}

/// Invalid physical parameters, visual timing, bounce, or settling tolerances.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidSpring;

impl fmt::Display for InvalidSpring {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid spring parameters, timing, bounce, or settling tolerances")
    }
}

impl Error for InvalidSpring {}

/// A transition between presentation and target values.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum Transition {
    /// Immediate assignment, also used when reduced motion is requested.
    Instant,
    /// A duration and easing curve. Retargeting preserves position, not velocity.
    Tween(Duration, Easing),
    /// An analytic spring; interruptions preserve position and velocity.
    Spring(Spring),
}

impl Default for Transition {
    fn default() -> Self {
        Self::Spring(Spring::default())
    }
}

/// A validated cubic Bezier easing curve. X controls are in `0..=1`; Y may overshoot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bezier {
    points: [f64; 4],
}

impl Bezier {
    /// Validates finite controls and monotonic time coordinates.
    pub fn new(x1: f64, y1: f64, x2: f64, y2: f64) -> Result<Self, InvalidEasing> {
        if ![x1, y1, x2, y2].iter().all(|v| v.is_finite())
            || !(0.0..=1.0).contains(&x1)
            || !(0.0..=1.0).contains(&x2)
        {
            return Err(InvalidEasing);
        }

        Ok(Self {
            points: [x1, y1, x2, y2],
        })
    }

    fn sample(self, x: f64) -> (f64, f64) {
        let [x1, y1, x2, y2] = self.points;

        let evaluate = |u: f64, a: f64, b: f64| {
            3.0 * (1.0 - u).powi(2) * u * a + 3.0 * (1.0 - u) * u * u * b + u * u * u
        };

        let derivative = |u: f64, a: f64, b: f64| {
            3.0 * (1.0 - u).powi(2) * a + 6.0 * (1.0 - u) * u * (b - a) + 3.0 * u * u * (1.0 - b)
        };

        let mut low = 0.0;
        let mut high = 1.0;
        let mut u = x;

        for _ in 0..32 {
            let actual = evaluate(u, x1, x2);

            if (actual - x).abs() < 1e-9 {
                break;
            }

            if actual < x {
                low = u;
            } else {
                high = u;
            }

            u = (low + high) * 0.5;
        }

        (
            evaluate(u, y1, y2),
            derivative(u, y1, y2) / derivative(u, x1, x2).max(1e-12),
        )
    }
}

/// Invalid cubic Bezier controls or custom easing endpoint/derivative contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidEasing;

impl fmt::Display for InvalidEasing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid Bezier controls or custom easing endpoints/derivatives")
    }
}

impl Error for InvalidEasing {}
