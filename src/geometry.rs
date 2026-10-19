//! Portable measured constraints, scroll intersections, and spatial reorder math.
use std::fmt;

/// Invalid finite geometry, scroll range, or edge-scroll configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeometryError;

impl fmt::Display for GeometryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid motion geometry")
    }
}

impl std::error::Error for GeometryError {}

/// Window-space logical-pixel rectangle. Dimensions are finite and non-negative.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutRect {
    origin: [f64; 2],
    size: [f64; 2],
}

impl LayoutRect {
    /// Validates a measured rectangle.
    pub fn new(origin: [f64; 2], size: [f64; 2]) -> Result<Self, GeometryError> {
        if origin.iter().any(|v| !v.is_finite() || v.abs() > 1e12)
            || size.iter().any(|v| !v.is_finite() || *v < 0.0 || *v > 1e12)
        {
            return Err(GeometryError);
        }

        Ok(Self { origin, size })
    }

    /// Top-left window coordinates.
    pub fn origin(self) -> [f64; 2] {
        self.origin
    }

    /// Width and height.
    pub fn size(self) -> [f64; 2] {
        self.size
    }

    /// Center in window coordinates.
    pub fn center(self) -> [f64; 2] {
        std::array::from_fn(|i| self.origin[i] + self.size[i] * 0.5)
    }

    /// Intersection with another rectangle in the same window coordinates.
    pub fn intersection(self, other: Self) -> Self {
        let origin = std::array::from_fn(|i| self.origin[i].max(other.origin[i]));

        Self {
            origin,
            size: std::array::from_fn(|i| {
                ((self.origin[i] + self.size[i]).min(other.origin[i] + other.size[i]) - origin[i])
                    .max(0.0)
            }),
        }
    }

    /// Insets top/right/bottom/left; negative margins expand the rectangle.
    /// Panics for non-finite margins. Over-insetting collapses at the leading edge.
    pub fn inset(self, margins: [f64; 4]) -> Self {
        assert!(
            margins.iter().all(|v| v.is_finite() && v.abs() <= 1e12),
            "invalid viewport margin"
        );

        Self {
            origin: [self.origin[0] + margins[3], self.origin[1] + margins[0]],
            size: [
                (self.size[0] - margins[1] - margins[3]).max(0.0),
                (self.size[1] - margins[0] - margins[2]).max(0.0),
            ],
        }
    }
}

/// Allowed drag offsets relative to an element's untranslated layout rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DragBounds {
    min: [f64; 2],
    max: [f64; 2],
}

impl DragBounds {
    /// Keeps an element within its measured container. An oversized element is
    /// pinned to the container's leading edge on that axis.
    pub fn within(container: LayoutRect, element: LayoutRect) -> Self {
        let min = std::array::from_fn(|i| container.origin[i] - element.origin[i]);

        Self {
            min,
            max: std::array::from_fn(|i| min[i] + (container.size[i] - element.size[i]).max(0.0)),
        }
    }

    /// Minimum x/y offsets.
    pub fn min(self) -> [f64; 2] {
        self.min
    }

    /// Maximum x/y offsets.
    pub fn max(self) -> [f64; 2] {
        self.max
    }
}

/// Fractional intersection of a target and its container, with a pixel adjustment.
/// Fractions may be outside `0..=1`, permitting anticipation and overscroll ranges.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollEdge {
    target: f64,
    container: f64,
    pixels: f64,
}

impl ScrollEdge {
    /// Creates an intersection. Zero means start, 0.5 center, and one end.
    /// Panics for non-finite values or geometry beyond ±1e12.
    pub fn new(target: f64, container: f64, pixels: f64) -> Self {
        assert!(
            [target, container, pixels]
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 1e12),
            "invalid scroll intersection"
        );

        Self {
            target,
            container,
            pixels,
        }
    }

    fn distance(self, container: LayoutRect, target: LayoutRect, axis: usize) -> f64 {
        target.origin[axis] + target.size[axis] * self.target
            - container.origin[axis]
            - container.size[axis] * self.container
            + self.pixels
    }
}

/// Two measured scroll intersections, independent of browser string parsing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollOffsets {
    start: ScrollEdge,
    end: ScrollEdge,
    clamp: bool,
}

impl Default for ScrollOffsets {
    fn default() -> Self {
        Self::new(
            ScrollEdge::new(0.0, 1.0, 0.0),
            ScrollEdge::new(1.0, 0.0, 0.0),
        )
    }
}

impl ScrollOffsets {
    /// Chooses start/end intersections; default progress is clamped.
    pub fn new(start: ScrollEdge, end: ScrollEdge) -> Self {
        Self {
            start,
            end,
            clamp: true,
        }
    }

    /// Allows progress outside zero through one for custom mappings.
    pub fn unclamped(mut self) -> Self {
        self.clamp = false;

        self
    }

    /// Calculates progress from current window-space target/container measurements.
    /// Axis zero is horizontal and one vertical. A collapsed range returns zero.
    /// Panics for another axis.
    pub fn progress(self, container: LayoutRect, target: LayoutRect, axis: usize) -> f64 {
        assert!(axis < 2, "scroll axis must be zero or one");
        let start = self.start.distance(container, target, axis);
        let end = self.end.distance(container, target, axis);

        let progress = if (start - end).abs() <= f64::EPSILON {
            0.0
        } else {
            start / (start - end)
        };

        if self.clamp {
            progress.clamp(0.0, 1.0)
        } else {
            progress
        }
    }
}

/// Edge autoscroll velocity policy. Use real elapsed time and the native scroll
/// handle's current bounds; do not integrate a fixed distance per rendered frame.
#[derive(Clone, Copy, Debug)]
pub struct AutoScroll {
    edge: f64,
    speed: f64,
}

impl Default for AutoScroll {
    fn default() -> Self {
        Self {
            edge: 48.0,
            speed: 600.0,
        }
    }
}

impl AutoScroll {
    /// Validates edge size in pixels and maximum speed in pixels/second.
    pub fn new(edge: f64, speed: f64) -> Result<Self, GeometryError> {
        if !edge.is_finite() || edge <= 0.0 || !speed.is_finite() || speed < 0.0 {
            return Err(GeometryError);
        }

        Ok(Self { edge, speed })
    }

    /// Signed content-scroll velocity. Opposite edges cancel in a tiny viewport;
    /// a pointer outside the viewport keeps the nearest edge at maximum speed.
    pub fn velocity(self, pointer: f64, start: f64, length: f64) -> f64 {
        assert!(
            [pointer, start, length].iter().all(|v| v.is_finite()) && length >= 0.0,
            "invalid scroll metrics"
        );

        if length == 0.0 {
            return 0.0;
        }

        let edge = self.edge.min(length * 0.5);
        let leading = ((start + edge - pointer) / edge).clamp(0.0, 1.0);
        let trailing = ((pointer - (start + length - edge)) / edge).clamp(0.0, 1.0);

        (trailing * trailing - leading * leading) * self.speed
    }
}

/// Reorder detection mode, independent of native scroll axes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReorderAxis {
    /// Infer a row, column, or wrapped/grid layout from measured centers.
    #[default]
    Auto,
    /// Horizontal list.
    X,
    /// Vertical list.
    Y,
    /// Two-dimensional nearest-center detection.
    XY,
}

impl ReorderAxis {
    /// Detects a row/column using overlapping cross-axis extents; a wrapped layout
    /// uses both axes. Empty and single-item collections default to vertical.
    pub fn resolve(self, rects: &[LayoutRect]) -> Self {
        if self != Self::Auto {
            return self;
        }

        if rects.len() < 2 {
            return Self::Y;
        }

        let overlap = |axis: usize| {
            let leading = rects
                .iter()
                .map(|r| r.origin[axis])
                .fold(f64::NEG_INFINITY, f64::max);

            let trailing = rects
                .iter()
                .map(|r| r.origin[axis] + r.size[axis])
                .fold(f64::INFINITY, f64::min);

            leading < trailing
        };

        if overlap(1) {
            Self::X
        } else if overlap(0) {
            Self::Y
        } else {
            Self::XY
        }
    }

    /// Finds the closest measured destination. Ties preserve the first index.
    pub fn destination(self, position: [f64; 2], rects: &[LayoutRect]) -> Option<usize> {
        assert!(
            position.iter().all(|v| v.is_finite()),
            "invalid reorder position"
        );
        let axis = self.resolve(rects);

        rects
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                let distance = |rect: LayoutRect| {
                    let c = rect.center();

                    let x = if axis == Self::Y {
                        0.0
                    } else {
                        c[0] - position[0]
                    };

                    let y = if axis == Self::X {
                        0.0
                    } else {
                        c[1] - position[1]
                    };

                    x.hypot(y)
                };

                distance(**a).total_cmp(&distance(**b))
            })
            .map(|(i, _)| i)
    }
}

/// Native scroll sample, also suitable for feeding a portable graph or typed track.
#[derive(Clone, Copy, Debug)]
pub struct ScrollFrame {
    /// Intersection progress, optionally smoothed/unclamped.
    pub progress: f64,
    /// Progress units per second.
    pub velocity: f64,
    /// Positive native content offsets in pixels.
    pub offset: [f64; 2],
    /// Both target and container have measured geometry.
    pub ready: bool,
    /// A smoothing trajectory remains active.
    pub active: bool,
}

impl ScrollFrame {
    /// Maps progress to partial numeric targets; drawable opacity/dimensions clamp
    /// at native application time. Omitted dimensions inherit the other endpoint.
    pub fn target(self, from: crate::Target, to: crate::Target) -> crate::Target {
        let from = from.inherit(to);
        let to = to.inherit(from);
        let a = from.values();
        let b = to.values();

        crate::Target::from_values(
            std::array::from_fn(|i| crate::value::finite(a[i] + (b[i] - a[i]) * self.progress)),
            from.mask() | to.mask(),
        )
    }
}

/// Native pointer/pinch lifecycle phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GesturePhase {
    /// Gesture has acquired ownership.
    Started,
    /// Pointer/scale changed.
    Moved,
    /// Gesture committed.
    Ended,
    /// Escape, lost ownership, blur or platform cancellation.
    Cancelled,
}

/// Structured pointer information in logical window pixels.
#[derive(Clone, Copy, Debug)]
pub struct DragEvent {
    /// Gesture phase.
    pub phase: GesturePhase,
    /// Current pointer location.
    pub point: [f64; 2],
    /// Movement since the preceding event.
    pub delta: [f64; 2],
    /// Movement since acquisition.
    pub offset: [f64; 2],
    /// Pointer units/second, independent of elastic element displacement.
    pub velocity: [f64; 2],
    /// Automatically locked axis, if configured: zero horizontal, one vertical.
    pub locked_axis: Option<usize>,
}

/// Native pinch event. Scale is relative to the beginning of this gesture;
/// translating a normal subtree or scaling an SVG remains an application mapping.
#[derive(Clone, Copy, Debug)]
pub struct PinchGesture {
    /// Gesture phase.
    pub phase: GesturePhase,
    /// Native pinch center in window coordinates.
    pub center: [f64; 2],
    /// Multiplicative scale accumulated within this gesture.
    pub scale: f64,
    /// Native percentage delta for this event.
    pub delta: f64,
}
