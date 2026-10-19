//! Prepared arc-length queries and bounded contour resampling.
use super::{PathCommand, PathError, PathShape, flatten, lerp};

#[derive(Clone, Copy, Debug)]
struct Segment {
    from: [f64; 2],
    to: [f64; 2],
    end: f64,
}

/// A position and forward tangent on a prepared path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathPoint {
    /// Logical-pixel position, without any native element transform.
    pub position: [f64; 2],
    /// Tangent heading in degrees, useful for SVG drawing rotation.
    pub tangent_degrees: f64,
}

/// Immutable arc-length lookup. Curves use the engine's 32-step approximation.
/// Separate subpaths do not introduce connecting segments. Setup allocates;
/// repeated queries use binary search and allocate no memory.
#[derive(Clone, Debug)]
pub struct PathMeasure {
    segments: Vec<Segment>,
    length: f64,
}

impl PathMeasure {
    /// Prepares a non-degenerate path. Empty/zero-length geometry is rejected.
    pub fn new(shape: &PathShape) -> Result<Self, PathError> {
        Self::from_commands(shape.commands())
    }

    fn from_commands(commands: &[PathCommand]) -> Result<Self, PathError> {
        let mut vertices = Vec::new();

        flatten(commands, &mut vertices);
        let mut segments = Vec::new();
        let mut length = 0.0;

        for pair in vertices.windows(2).filter(|pair| !pair[1].start) {
            let from = pair[0].point;
            let to = pair[1].point;
            let distance = (to[0] - from[0]).hypot(to[1] - from[1]);

            if distance > 0.0 {
                length += distance;
                segments.push(Segment {
                    from,
                    to,
                    end: length,
                });
            }
        }

        if segments.is_empty() {
            return Err(PathError::Geometry);
        }

        Ok(Self { segments, length })
    }

    /// Approximate total length in logical pixels.
    pub fn length(&self) -> f64 {
        self.length
    }

    /// Samples normalized length, clamped to `0..=1`. Panics for non-finite input.
    /// A subpath boundary chooses the preceding endpoint; the next positive step
    /// enters the next subpath without interpolating across the gap.
    pub fn point(&self, progress: f64) -> PathPoint {
        assert!(progress.is_finite(), "path progress must be finite");
        let distance = progress.clamp(0.0, 1.0) * self.length;

        let index = self
            .segments
            .partition_point(|segment| segment.end < distance)
            .min(self.segments.len() - 1);

        let segment = self.segments[index];

        let start = if index == 0 {
            0.0
        } else {
            self.segments[index - 1].end
        };

        let t = ((distance - start) / (segment.end - start)).clamp(0.0, 1.0);

        PathPoint {
            position: lerp(segment.from, segment.to, t),
            tangent_degrees: (segment.to[1] - segment.from[1])
                .atan2(segment.to[0] - segment.from[0])
                .to_degrees(),
        }
    }
}

impl PathShape {
    /// Prepares equally sampled line topology for two paths. Subpaths match by
    /// order and must agree on open/closed status; winding and start points are
    /// preserved. This is geometric resampling, not semantic contour matching.
    /// Closed contours use `samples` points; open ones include both endpoints.
    /// Curves retain the measurement's documented approximation. Cache the result
    /// before constructing `PathMorphState` so no conversion happens per frame.
    ///
    /// # Errors
    /// Returns `Topology` for different contour counts/closure, `Geometry` for
    /// zero-length contours, or `Complexity` outside 2..=4096 samples or above
    /// 65,536 total output points per shape.
    pub fn normalize_for_morph(
        &self,
        other: &Self,
        samples: usize,
    ) -> Result<(Self, Self), PathError> {
        if !(2..=4096).contains(&samples) {
            return Err(PathError::Complexity);
        }

        let contours = |shape: &Self| -> Result<Vec<(PathMeasure, bool)>, PathError> {
            let commands = shape.commands();
            let mut result = Vec::new();
            let mut start = 0;

            for end in 1..=commands.len() {
                if end == commands.len() || matches!(commands[end], PathCommand::Move(_)) {
                    result.push((
                        PathMeasure::from_commands(&commands[start..end])?,
                        matches!(commands[end - 1], PathCommand::Close),
                    ));
                    start = end;
                }
            }

            Ok(result)
        };

        let a = contours(self)?;
        let b = contours(other)?;

        if a.len() != b.len() || a.iter().zip(&b).any(|(a, b)| a.1 != b.1) {
            return Err(PathError::Topology);
        }

        if a.len().saturating_mul(samples) > 65_536 {
            return Err(PathError::Complexity);
        }

        let resample = |contours: Vec<(PathMeasure, bool)>| {
            let mut commands = Vec::new();

            for (measure, closed) in contours {
                for i in 0..samples {
                    let divisor = if closed { samples } else { samples - 1 };
                    let point = measure.point(i as f64 / divisor as f64).position;

                    commands.push(if i == 0 {
                        PathCommand::Move(point)
                    } else {
                        PathCommand::Line(point)
                    });
                }

                if closed {
                    commands.push(PathCommand::Close);
                }
            }

            Self::new(commands)
        };

        Ok((resample(a)?, resample(b)?))
    }
}
