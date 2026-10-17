//! Compatible path morphing and approximate arc-length stroke drawing.
use crate::{MotionValue, Transition};
use std::{fmt, sync::Arc, time::Duration};
/// Absolute path commands in logical pixels. Subpaths start with Move.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathCommand {
    /// Starts a subpath.
    Move([f64; 2]),
    /// Straight segment.
    Line([f64; 2]),
    /// Quadratic Bezier: control and endpoint.
    Quadratic([f64; 2], [f64; 2]),
    /// Cubic Bezier: two controls and endpoint.
    Cubic([f64; 2], [f64; 2], [f64; 2]),
    /// Closes the current subpath.
    Close,
}
impl PathCommand {
    fn kind(self) -> u8 {
        match self {
            Self::Move(_) => 0,
            Self::Line(_) => 1,
            Self::Quadratic(..) => 2,
            Self::Cubic(..) => 3,
            Self::Close => 4,
        }
    }
    fn coordinates(self) -> ([f64; 6], usize) {
        match self {
            Self::Move([x, y]) | Self::Line([x, y]) => ([x, y, 0., 0., 0., 0.], 2),
            Self::Quadratic([x, y], [a, b]) => ([x, y, a, b, 0., 0.], 4),
            Self::Cubic([x, y], [a, b], [c, d]) => ([x, y, a, b, c, d], 6),
            Self::Close => ([0.; 6], 0),
        }
    }
    fn with(self, v: [f64; 6]) -> Self {
        match self {
            Self::Move(_) => Self::Move([v[0], v[1]]),
            Self::Line(_) => Self::Line([v[0], v[1]]),
            Self::Quadratic(..) => Self::Quadratic([v[0], v[1]], [v[2], v[3]]),
            Self::Cubic(..) => Self::Cubic([v[0], v[1]], [v[2], v[3]], [v[4], v[5]]),
            Self::Close => Self::Close,
        }
    }
}
/// Recoverable geometry, topology, tessellation or dash-budget failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathError {
    /// Empty path, drawing before Move, or non-finite/out-of-range coordinates.
    Geometry,
    /// Command kinds/counts do not match; arbitrary topology conversion is absent.
    Topology,
    /// Dash pattern exceeds 4096 repeated periods per path.
    Complexity,
    /// Native tessellation failed.
    Tessellation,
}
impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid motion path: {self:?}")
    }
}
impl std::error::Error for PathError {}
/// Immutable shared absolute path. Curves retain their controls for exact native
/// fill; stroke length/reveal uses 32 equal-parameter segments per curve. Prepare
/// paths once. Coordinates are bounded to ±1e9 logical pixels to keep native
/// tessellation and length arithmetic in a useful finite range.
#[derive(Clone, Debug)]
pub struct PathShape(Arc<[PathCommand]>);
impl PathShape {
    /// Validates commands before allocating retained morph state.
    pub fn new(commands: impl IntoIterator<Item = PathCommand>) -> Result<Self, PathError> {
        let commands: Vec<_> = commands.into_iter().collect();
        let mut started = false;
        if commands.is_empty() {
            return Err(PathError::Geometry);
        }
        for &command in &commands {
            if matches!(command, PathCommand::Move(_)) {
                started = true;
            } else if !started {
                return Err(PathError::Geometry);
            }
            let (values, count) = command.coordinates();
            if values[..count]
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 1e9)
            {
                return Err(PathError::Geometry);
            }
        }
        Ok(Self(commands.into()))
    }
    /// Prepared absolute commands.
    pub fn commands(&self) -> &[PathCommand] {
        &self.0
    }
    /// Whether native coordinate interpolation can preserve command topology.
    pub fn compatible(&self, other: &Self) -> bool {
        self.0.len() == other.0.len()
            && self
                .0
                .iter()
                .zip(other.0.iter())
                .all(|(a, b)| a.kind() == b.kind())
    }
}
/// Normalized line drawing. Length and gap are fractions of total path length;
/// offset shifts the pattern along the path. Zero length paints nothing. A zero
/// gap paints continuously. At most 4096 periods are allowed to bound paint cost.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrokeReveal {
    values: [f64; 3],
}
impl Default for StrokeReveal {
    fn default() -> Self {
        Self {
            values: [1., 0., 1.],
        }
    }
}
impl StrokeReveal {
    /// Validates finite non-negative length/gap and finite offset. Length/gap must
    /// be at most one; offset may be any finite number of path lengths.
    pub fn new(length: f64, offset: f64, spacing: f64) -> Result<Self, PathError> {
        if !(0.0..=1.).contains(&length) || !(0.0..=1.).contains(&spacing) || !offset.is_finite() {
            return Err(PathError::Geometry);
        }
        if length > 0. && length + spacing < 1. / 4096. {
            return Err(PathError::Complexity);
        }
        Ok(Self {
            values: [length, offset, spacing],
        })
    }
    /// Length, offset and gap fractions.
    pub fn values(self) -> [f64; 3] {
        self.values
    }
}
#[derive(Clone, Copy, Debug)]
struct Vertex {
    point: [f64; 2],
    start: bool,
}
fn lerp(a: [f64; 2], b: [f64; 2], t: f64) -> [f64; 2] {
    std::array::from_fn(|i| a[i] * (1. - t) + b[i] * t)
}
fn flatten(commands: &[PathCommand], vertices: &mut Vec<Vertex>) -> f64 {
    vertices.clear();
    let mut current = [0.; 2];
    let mut start = current;
    for &command in commands {
        match command {
            PathCommand::Move(p) => {
                current = p;
                start = p;
                vertices.push(Vertex {
                    point: p,
                    start: true,
                });
            }
            PathCommand::Line(p) => {
                vertices.push(Vertex {
                    point: p,
                    start: false,
                });
                current = p;
            }
            PathCommand::Quadratic(control, to) => {
                let from = current;
                for i in 1..=32 {
                    let t = f64::from(i) / 32.;
                    let p = lerp(lerp(from, control, t), lerp(control, to, t), t);
                    vertices.push(Vertex {
                        point: p,
                        start: false,
                    });
                }
                current = to;
            }
            PathCommand::Cubic(a, b, to) => {
                let from = current;
                for i in 1..=32 {
                    let t = f64::from(i) / 32.;
                    let p = lerp(
                        lerp(lerp(from, a, t), lerp(a, b, t), t),
                        lerp(lerp(a, b, t), lerp(b, to, t), t),
                        t,
                    );
                    vertices.push(Vertex {
                        point: p,
                        start: false,
                    });
                }
                current = to;
            }
            PathCommand::Close => {
                vertices.push(Vertex {
                    point: start,
                    start: false,
                });
                current = start;
            }
        }
    }
    vertices
        .windows(2)
        .filter(|p| !p[1].start)
        .map(|p| (p[1].point[0] - p[0].point[0]).hypot(p[1].point[1] - p[0].point[1]))
        .sum()
}
/// Retained compatible topology morph. Construction allocates its reusable scalar,
/// command and length buffers. Sampling/reveal iteration do not allocate. Springs
/// retain velocity for every control coordinate at interruption.
#[derive(Debug)]
pub struct PathMorphState {
    shape: PathShape,
    values: Vec<MotionValue>,
    commands: Vec<PathCommand>,
    vertices: Vec<Vertex>,
    length: f64,
}
impl PathMorphState {
    /// Creates a resting path and prepares curve-length approximation buffers.
    ///
    /// ```
    /// use gpui_ce_motion::{PathShape, PathCommand, PathMorphState, Transition};
    /// use std::time::Duration;
    /// let path = PathShape::new([PathCommand::Move([0.,0.]), PathCommand::Line([100.,0.])])?;
    /// let mut state = PathMorphState::new(path.clone());
    /// let frame = state.sample(&path, Transition::Instant, Duration::ZERO, false)?;
    /// assert_eq!(frame.length(), 100.);
    /// # Ok::<(), gpui_ce_motion::PathError>(())
    /// ```
    pub fn new(shape: PathShape) -> Self {
        let mut values = Vec::new();
        for &command in shape.commands() {
            let (v, n) = command.coordinates();
            values.extend(v[..n].iter().map(|v| MotionValue::new(*v)));
        }
        let commands = shape.commands().to_vec();
        let mut vertices = Vec::with_capacity(commands.len() * 32 + 1);
        let length = flatten(&commands, &mut vertices);
        Self {
            shape,
            values,
            commands,
            vertices,
            length,
        }
    }
    /// Checks a destination before changing any state.
    pub fn validate(&self, target: &PathShape) -> Result<(), PathError> {
        if self.shape.compatible(target) {
            Ok(())
        } else {
            Err(PathError::Topology)
        }
    }
    /// Samples coordinate motion transactionally. Incompatible topology leaves the
    /// state unchanged. Reduced motion snaps; backward time holds.
    pub fn sample(
        &mut self,
        target: &PathShape,
        transition: Transition,
        now: Duration,
        reduced: bool,
    ) -> Result<PathFrame<'_>, PathError> {
        self.validate(target)?;
        let mut index = 0;
        let mut active = false;
        let mut completed = false;
        let mut changed = false;
        for (command, &target) in self.commands.iter_mut().zip(target.commands()) {
            let (mut coordinates, count) = target.coordinates();
            for value in &mut coordinates[..count] {
                self.values[index].animate(*value, transition, now);
                let frame = self.values[index].sample(now, reduced);
                index += 1;
                *value = frame.value().clamp(-1e9, 1e9);
                active |= frame.is_animating();
                completed |= frame.just_completed();
            }
            let next = target.with(coordinates);
            changed |= *command != next;
            *command = next;
        }
        if changed {
            self.length = flatten(&self.commands, &mut self.vertices);
        }
        Ok(PathFrame {
            commands: &self.commands,
            vertices: &self.vertices,
            length: self.length,
            active,
            completed: completed && !active,
        })
    }
}
/// Borrowed sampled geometry. Stroke length is approximate; fills retain curves.
pub struct PathFrame<'a> {
    commands: &'a [PathCommand],
    vertices: &'a [Vertex],
    length: f64,
    active: bool,
    completed: bool,
}
impl PathFrame<'_> {
    /// Presented absolute commands.
    pub fn commands(&self) -> &[PathCommand] {
        self.commands
    }
    /// Approximate total arc length, excluding jumps between subpaths.
    pub fn length(&self) -> f64 {
        self.length
    }
    /// Whether coordinate motion needs another sample.
    pub fn is_animating(&self) -> bool {
        self.active
    }
    /// Completion edge when the last moving coordinate finishes.
    pub fn just_completed(&self) -> bool {
        self.completed
    }
    /// Visits visible stroke segments without allocation. Subpath moves do not
    /// consume length or draw connectors. The pattern's length/offset/gap are in
    /// normalized total arc length; the finite period budget bounds traversal.
    pub fn stroke_segments(&self, reveal: StrokeReveal, mut emit: impl FnMut([f64; 2], [f64; 2])) {
        let [length, offset, gap] = reveal.values;
        if self.length <= 0. || length == 0. {
            return;
        }
        let period = (length + gap) * self.length;
        let visible = length * self.length;
        let shift = offset.rem_euclid(length + gap) * self.length;
        let mut distance = 0.;
        for points in self.vertices.windows(2) {
            if points[1].start {
                continue;
            }
            let a = points[0].point;
            let b = points[1].point;
            let span = (b[0] - a[0]).hypot(b[1] - a[1]);
            if span == 0. {
                continue;
            }
            let end = distance + span;
            let mut cursor = distance;
            while cursor < end {
                let phase = (cursor - shift).rem_euclid(period);
                let drawing = phase < visible;
                let remaining = if drawing {
                    visible - phase
                } else {
                    period - phase
                };
                let next = (cursor + remaining).min(end);
                if next <= cursor {
                    break;
                }
                if drawing {
                    emit(
                        lerp(a, b, (cursor - distance) / span),
                        lerp(a, b, (next - distance) / span),
                    );
                }
                cursor = next;
            }
            distance = end;
        }
    }
}
