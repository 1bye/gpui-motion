//! Linear-light RGBA interpolation and interruptible color motion.
use crate::{MotionValue, Transition};
use std::time::Duration;
/// A linear-light RGBA color, constructed and read as sRGB channels in zero through one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    linear: [f64; 4],
}
impl Color {
    /// Validates sRGB channels and alpha. Panics unless all are in zero through one.
    pub fn rgba(r: f64, g: f64, b: f64, a: f64) -> Self {
        assert!(
            [r, g, b, a].iter().all(|v| (0.0..=1.0).contains(v)),
            "invalid color"
        );
        let linear = |v: f64| {
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };

        Self {
            linear: [linear(r), linear(g), linear(b), a],
        }
    }
    /// Creates an opaque color from `0xRRGGBB`.
    pub fn rgb(hex: u32) -> Self {
        Self::rgba(
            f64::from((hex >> 16) & 255) / 255.0,
            f64::from((hex >> 8) & 255) / 255.0,
            f64::from(hex & 255) / 255.0,
            1.0,
        )
    }
    /// Reads clamped sRGB channels and alpha.
    pub fn channels(self) -> [f32; 4] {
        let srgb = |v: f64| {
            let v = v.clamp(0.0, 1.0);

            if v <= 0.0031308 {
                12.92 * v
            } else {
                1.055 * v.powf(1.0 / 2.4) - 0.055
            }
        };

        [
            srgb(self.linear[0]) as f32,
            srgb(self.linear[1]) as f32,
            srgb(self.linear[2]) as f32,
            self.linear[3].clamp(0.0, 1.0) as f32,
        ]
    }
    /// Mixes in linear light; finite progress is clamped to zero through one.
    pub fn mix(self, other: Self, progress: f64) -> Self {
        assert!(progress.is_finite(), "invalid color progress");
        let t = progress.clamp(0.0, 1.0);

        Self {
            linear: std::array::from_fn(|ix| {
                self.linear[ix] + (other.linear[ix] - self.linear[ix]) * t
            }),
        }
    }
}
/// Retained, interruptible color motion. Each linear-light channel retains spring velocity.
#[derive(Debug)]
pub struct ColorState {
    channels: [MotionValue; 4],
}
impl ColorState {
    /// Creates a resting color.
    pub fn new(color: Color) -> Self {
        Self {
            channels: color.linear.map(MotionValue::new),
        }
    }
    /// Samples a target color with no heap allocation. Returns color and whether motion is active.
    pub fn sample(
        &mut self,
        color: Color,
        transition: Transition,
        now: Duration,
        reduced_motion: bool,
    ) -> (Color, bool) {
        let mut active = false;

        let linear = std::array::from_fn(|ix| {
            self.channels[ix].animate(color.linear[ix], transition, now);
            let frame = self.channels[ix].sample(now, reduced_motion);

            active |= frame.is_animating();

            frame.value()
        });

        (Color { linear }, active)
    }
}
