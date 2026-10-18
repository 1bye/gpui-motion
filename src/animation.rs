//! Prepared clips, keyframes, playback clocks, and explicit timeline orchestration.
use crate::{Easing, Frame, Spring, Target, Transition, value::sample_transition};
#[cfg(feature = "gpui")]
use std::time::Instant;
use std::{error::Error, fmt, sync::Arc, time::Duration};

/// Invalid prepared animation, timing, or orchestration input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AnimationError {
    /// At least two keyframes are required, at zero and one.
    Keyframes,
    /// Times must be finite, strictly increasing, and in zero through one.
    Times,
    /// A repeat requires positive cycle duration.
    ZeroPeriod,
    /// Speed must be finite.
    Speed,
    /// A label is duplicated or missing.
    Label,
    /// An infinite clip cannot define a finite sequence cursor.
    InfiniteSequence,
    /// A stagger index must belong to a nonempty collection.
    Index,
}

impl fmt::Display for AnimationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid animation input: {self:?}")
    }
}

impl Error for AnimationError {}

/// One timed target. A missing target holds the previous value, or current presentation at zero.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keyframe {
    at: f64,
    target: Option<Target>,
    easing: Easing,
}

impl Keyframe {
    /// A complete target at normalized progress `at`. Validated when preparing the clip.
    pub fn new(at: f64, target: Target) -> Self {
        Self {
            at,
            target: Some(target),
            easing: Easing::default(),
        }
    }

    /// A wildcard: current presentation at zero, otherwise hold the preceding target.
    pub fn current(at: f64) -> Self {
        Self {
            at,
            target: None,
            easing: Easing::default(),
        }
    }

    /// Easing for the segment ending at this keyframe.
    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;

        self
    }
}

/// Additional cycles after the first cycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Repeat {
    /// A finite number of additional cycles. Zero plays once.
    Count(u32),
    /// Repeat until stopped or reduced motion snaps the animation.
    Forever,
}

impl Default for Repeat {
    fn default() -> Self {
        Self::Count(0)
    }
}

/// How repeated clips travel between their endpoints.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RepeatMode {
    /// Restart forward from the original beginning.
    #[default]
    Loop,
    /// Play the same trajectory backwards on alternate cycles.
    Reverse,
    /// Reverse targets on alternate cycles, retaining forward easing/physics.
    Mirror,
}

#[derive(Clone, Debug, PartialEq)]
struct Clip {
    frames: Vec<Keyframe>,
    duration: Duration,
    spring: Option<Spring>,
}

/// A prepared, cheaply cloned clip. Construct once outside rendering; clones share keyframes.
#[derive(Clone, Debug)]
pub struct Animation {
    clip: Arc<Clip>,
    delay: Duration,
    repeat: Repeat,
    mode: RepeatMode,
    repeat_delay: Duration,
}

impl Animation {
    /// Prepares keyframes with normalized, strictly increasing times from zero to one.
    pub fn keyframes(
        duration: Duration,
        frames: impl IntoIterator<Item = Keyframe>,
    ) -> Result<Self, AnimationError> {
        let frames: Vec<_> = frames.into_iter().collect();

        if frames.len() < 2 || frames.first().unwrap().at != 0.0 || frames.last().unwrap().at != 1.0
        {
            return Err(AnimationError::Keyframes);
        }

        if frames
            .iter()
            .any(|f| !f.at.is_finite() || !(0.0..=1.0).contains(&f.at))
            || frames.windows(2).any(|p| p[0].at >= p[1].at)
        {
            return Err(AnimationError::Times);
        }

        Ok(Self {
            clip: Arc::new(Clip {
                frames,
                duration,
                spring: None,
            }),
            delay: Duration::ZERO,
            repeat: Repeat::default(),
            mode: RepeatMode::Loop,
            repeat_delay: Duration::ZERO,
        })
    }

    /// A tween from the current presentation, with explicit duration and easing.
    pub fn to(target: Target, duration: Duration, easing: Easing) -> Self {
        Self::keyframes(
            duration,
            [
                Keyframe::current(0.0),
                Keyframe::new(1.0, target).easing(easing),
            ],
        )
        .expect("valid two-frame clip")
    }

    /// A finite physical spring clip from current position and velocity.
    /// `duration` is an explicit deadline: the endpoint snaps exactly at that time.
    /// Use ordinary `Motion` springs for unconstrained, tolerance-based settling.
    pub fn spring(target: Target, spring: Spring, duration: Duration) -> Self {
        let mut animation = Self::to(target, duration, Easing::default());

        Arc::make_mut(&mut animation.clip).spring = Some(spring);

        animation
    }

    /// Holds the first presentation before playback starts.
    pub fn delay(mut self, duration: Duration) -> Self {
        self.delay = duration;

        self
    }

    /// Sets repeat count/mode and the hold between cycles.
    pub fn repeat(
        mut self,
        repeat: Repeat,
        mode: RepeatMode,
        delay: Duration,
    ) -> Result<Self, AnimationError> {
        if self.clip.duration.is_zero() && repeat != Repeat::Count(0) {
            return Err(AnimationError::ZeroPeriod);
        }

        self.repeat = repeat;
        self.mode = mode;
        self.repeat_delay = delay;

        Ok(self)
    }

    /// Total duration, including initial/inter-cycle holds; `None` denotes infinity.
    pub fn duration(&self) -> Option<Duration> {
        match self.repeat {
            Repeat::Forever => None,
            Repeat::Count(count) => Some(
                self.delay
                    .saturating_add(
                        self.clip
                            .duration
                            .saturating_mul(count)
                            .saturating_add(self.clip.duration),
                    )
                    .saturating_add(self.repeat_delay.saturating_mul(count)),
            ),
        }
    }

    fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.clip, &other.clip)
            && self.delay == other.delay
            && self.repeat == other.repeat
            && self.mode == other.mode
            && self.repeat_delay == other.repeat_delay
    }

    fn resolve(&self, time: Duration, origin: Frame, force_end: bool) -> (Frame, u64, bool) {
        let duration = self.clip.duration.as_secs_f64();
        let period = duration + self.repeat_delay.as_secs_f64();
        let relative = time.saturating_sub(self.delay).as_secs_f64();
        let done = force_end || self.duration().is_some_and(|end| time >= end);

        let iteration = if done {
            match self.repeat {
                Repeat::Count(n) => u64::from(n),
                Repeat::Forever => 0,
            }
        } else if period > 0.0 {
            (relative / period).floor().min(u64::MAX as f64) as u64
        } else {
            0
        };

        let local = if done {
            duration
        } else {
            (relative - iteration as f64 * period).clamp(0.0, duration)
        };

        let mut progress = if duration > 0.0 {
            local / duration
        } else {
            1.0
        };

        if time < self.delay && !done {
            progress = 0.0;
        }

        let odd = !iteration.is_multiple_of(2);
        let reverse = odd && self.mode == RepeatMode::Reverse;

        if reverse {
            progress = 1.0 - progress;
        }

        let mut from = origin.target().inherit(
            self.clip
                .frames
                .last()
                .and_then(|f| f.target)
                .unwrap_or_default(),
        );

        from = self.clip.frames[0].target.unwrap_or(from).inherit(from);
        let first = from;

        let final_target = self.clip.frames.iter().fold(from, |previous, frame| {
            frame.target.unwrap_or(previous).inherit(previous)
        });

        if let Some(spring) = self.clip.spring {
            let (start, end) = if odd && self.mode == RepeatMode::Mirror {
                (final_target, first)
            } else {
                (first, final_target)
            };

            let velocity = if iteration == 0 {
                origin_velocities(origin)
            } else {
                [0.0; 6]
            };

            let mut values = [0.0; 6];
            let mut velocities = [0.0; 6];

            for ix in 0..6 {
                let (v, speed, _) = sample_transition(
                    start.values()[ix],
                    velocity[ix],
                    end.values()[ix],
                    Transition::Spring(spring),
                    progress * duration,
                );

                values[ix] = if progress == 0.0 {
                    start.values()[ix]
                } else if progress == 1.0 {
                    end.values()[ix]
                } else {
                    v
                };
                velocities[ix] = if progress == 1.0 || done || time < self.delay {
                    0.0
                } else if reverse {
                    -speed
                } else {
                    speed
                };
            }

            return (
                Frame::animated(
                    Target::from_values(values, start.mask() | end.mask() | 7),
                    velocities,
                    !done,
                    false,
                ),
                iteration,
                done,
            );
        }

        let mirrored = odd && self.mode == RepeatMode::Mirror;
        let sample_progress = if mirrored { 1.0 - progress } else { progress };

        let upper = self
            .clip
            .frames
            .partition_point(|frame| frame.at < sample_progress)
            .clamp(1, self.clip.frames.len() - 1);

        let mut a = first;

        for frame in self.clip.frames.iter().take(upper).skip(1) {
            a = frame.target.unwrap_or(a).inherit(a);
        }

        let mut b = self.clip.frames[upper].target.unwrap_or(a).inherit(a);
        let lo = self.clip.frames[upper - 1].at;
        let hi = self.clip.frames[upper].at;
        let mut u = (sample_progress - lo) / (hi - lo);

        if mirrored {
            std::mem::swap(&mut a, &mut b);
            u = 1.0 - u;
        }

        let span = (hi - lo) * duration;
        let easing = self.clip.frames[upper].easing;
        let (weight, derivative) = easing.sample(u.clamp(0.0, 1.0));
        let av = a.values();
        let bv = b.values();

        let values = std::array::from_fn(|ix| {
            if u <= 0.0 {
                av[ix]
            } else if u >= 1.0 {
                bv[ix]
            } else {
                crate::value::finite(av[ix] + (bv[ix] - av[ix]) * weight)
            }
        });

        let velocity = std::array::from_fn(|ix| {
            if done || time < self.delay || local >= duration || span == 0.0 {
                0.0
            } else {
                crate::value::finite(
                    (bv[ix] - av[ix]) * derivative / span * if reverse { -1.0 } else { 1.0 },
                )
            }
        });

        (
            Frame::animated(
                Target::from_values(values, a.mask() | b.mask() | 7),
                velocity,
                !done,
                false,
            ),
            iteration,
            done,
        )
    }
}

fn origin_velocities(frame: Frame) -> [f64; 6] {
    let basic = frame.velocity();
    let layout = frame.layout_velocity();

    [
        basic[0],
        basic[1],
        basic[2],
        layout[0].unwrap_or(0.0),
        layout[1].unwrap_or(0.0),
        layout[2].unwrap_or(0.0),
    ]
}

/// An explicit monotonic playback clock. It performs no scheduling.
#[derive(Clone, Debug)]
pub struct Playback {
    anchor: Duration,
    offset: f64,
    speed: f64,
    paused: bool,
    last: Duration,
}

impl Default for Playback {
    fn default() -> Self {
        Self {
            anchor: Duration::ZERO,
            offset: 0.0,
            speed: 1.0,
            paused: false,
            last: Duration::ZERO,
        }
    }
}

impl Playback {
    /// Virtual playhead at a monotonic real timestamp. Backward real timestamps are held.
    pub fn time(&mut self, now: Duration) -> Duration {
        let now = now.max(self.last);

        self.last = now;
        let seconds = self.offset
            + if self.paused {
                0.0
            } else {
                now.saturating_sub(self.anchor).as_secs_f64() * self.speed
            };

        Duration::try_from_secs_f64(seconds.max(0.0)).unwrap_or(Duration::MAX)
    }

    /// Freezes at the current playhead.
    pub fn pause(&mut self, now: Duration) {
        self.offset = self.time(now).as_secs_f64();
        self.anchor = now.max(self.last).max(self.anchor);
        self.paused = true;
    }

    /// Resumes from the current playhead.
    pub fn play(&mut self, now: Duration) {
        if self.paused {
            self.anchor = now.max(self.last).max(self.anchor);
            self.paused = false;
        }
    }

    /// Sets the playhead without changing speed or paused state.
    pub fn seek(&mut self, time: Duration, now: Duration) {
        self.last = self.last.max(now);
        self.offset = time.as_secs_f64();
        self.anchor = now.max(self.last).max(self.anchor);
    }

    /// Sets finite playback speed. Zero holds; negative speed plays toward zero.
    pub fn set_speed(&mut self, speed: f64, now: Duration) -> Result<(), AnimationError> {
        if !speed.is_finite() {
            return Err(AnimationError::Speed);
        }

        self.offset = self.time(now).as_secs_f64();
        self.anchor = now.max(self.last).max(self.anchor);
        self.speed = speed;

        Ok(())
    }

    /// Whether this clock advances.
    pub fn is_playing(&self) -> bool {
        !self.paused && self.speed != 0.0
    }
}

/// Retained playback for a prepared clip. New clips interrupt from current presentation.
#[derive(Debug)]
pub struct AnimationState {
    clip: Option<Animation>,
    clock: Playback,
    origin: Frame,
    current: Frame,
    complete: bool,
    force_end: bool,
    iteration: u64,
    #[cfg(feature = "gpui")]
    pub(crate) epoch: Instant,
}

impl Default for AnimationState {
    fn default() -> Self {
        Self::new(Target::default())
    }
}

impl AnimationState {
    /// Seeds the presentation before the first clip. No work or timers are started.
    pub fn new(initial: Target) -> Self {
        Self {
            clip: None,
            clock: Playback::default(),
            origin: Frame::at(initial),
            current: Frame::at(initial),
            complete: false,
            force_end: false,
            iteration: 0,
            #[cfg(feature = "gpui")]
            epoch: Instant::now(),
        }
    }

    /// Samples a retained prepared clip; cloning an `Animation` preserves its identity.
    /// Rebuilding a clip intentionally restarts it. Reduced motion finishes even infinite clips.
    pub fn sample(&mut self, animation: &Animation, now: Duration, reduced_motion: bool) -> Frame {
        if !self.clip.as_ref().is_some_and(|old| old.same(animation)) {
            if let Some(old) = &self.clip {
                let frame = old
                    .resolve(self.clock.time(now), self.origin, self.force_end)
                    .0;

                let factor = if self.clock.is_playing() {
                    self.clock.speed
                } else {
                    0.0
                };

                self.current = Frame::animated(
                    frame.target(),
                    origin_velocities(frame).map(|v| v * factor),
                    frame.is_animating(),
                    false,
                );
            }

            self.origin = self.current;
            let first = self.clip.is_none();

            self.clip = Some(animation.clone());

            if first {
                self.clock.anchor = now.max(self.clock.last);
                self.clock.last = self.clock.anchor;
            } else {
                self.clock.seek(Duration::ZERO, now);
            }

            self.complete = false;

            if !first {
                self.force_end = false;
            }
        }

        self.force_end |= reduced_motion;
        let time = self.clock.time(now);
        let (frame, iteration, finished) = animation.resolve(time, self.origin, self.force_end);
        let backward_end = self.clock.speed < 0.0 && time.is_zero();
        let complete = finished || backward_end;

        self.current = Frame::animated(
            frame.target(),
            origin_velocities(frame).map(|v| {
                v * if self.clock.is_playing() {
                    self.clock.speed
                } else {
                    0.0
                }
            }),
            !complete && self.clock.is_playing(),
            complete && !self.complete,
        );
        self.complete = complete;
        self.iteration = iteration;

        self.current
    }

    /// Pauses playback; call the host's notify method after controls change.
    pub fn pause(&mut self, now: Duration) {
        self.clock.pause(now);
    }

    /// Resumes playback.
    pub fn play(&mut self, now: Duration) {
        self.clock.play(now);
    }

    /// Stops at the current presentation. Use play to resume, or restart to reset.
    pub fn stop(&mut self, now: Duration) {
        self.pause(now);
    }

    /// Seeks to a virtual playhead, including initial delay and repeats.
    pub fn seek(&mut self, time: Duration, now: Duration) {
        self.clock.seek(time, now);
    }

    /// Changes playback speed; negative speed is supported after seeking forward.
    pub fn set_speed(&mut self, speed: f64, now: Duration) -> Result<(), AnimationError> {
        self.clock.set_speed(speed, now)
    }

    /// Restarts the same clip from its captured origin.
    pub fn restart(&mut self, now: Duration) {
        self.clock.seek(Duration::ZERO, now);
        self.clock.play(now);
        self.force_end = false;
        self.complete = false;
    }

    /// Finishes immediately on the next sample, including an infinite clip.
    pub fn finish(&mut self) {
        self.force_end = true;
    }

    /// Current repeat iteration, zero based.
    pub fn iteration(&self) -> u64 {
        self.iteration
    }

    /// Whether controls permit time to advance.
    pub fn is_playing(&self) -> bool {
        self.clock.is_playing()
    }
}

/// Origin of a staggered group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaggerFrom {
    /// First child starts first.
    Start,
    /// Last child starts first.
    End,
    /// Center children start first.
    Center,
}

/// Computes a child's delay without allocating. Center origins support half-step offsets.
pub fn stagger(
    step: Duration,
    ix: usize,
    count: usize,
    from: StaggerFrom,
) -> Result<Duration, AnimationError> {
    if count == 0 || ix >= count {
        return Err(AnimationError::Index);
    }

    let distance = match from {
        StaggerFrom::Start => ix as f64,
        StaggerFrom::End => (count - 1 - ix) as f64,
        StaggerFrom::Center => (ix as f64 - (count - 1) as f64 / 2.0).abs(),
    };

    Ok(Duration::try_from_secs_f64(step.as_secs_f64() * distance).unwrap_or(Duration::MAX))
}

#[derive(Debug)]
struct Track {
    at: Duration,
    animation: Animation,
    state: AnimationState,
}

/// A retained, allocation-free-at-sample timeline. Tracks target separate caller-owned elements.
#[derive(Debug, Default)]
pub struct Timeline {
    tracks: Vec<Track>,
    frames: Vec<Frame>,
    clock: Playback,
    cursor: Option<Duration>,
    labels: Vec<(String, Duration)>,
    active: bool,
}

impl Timeline {
    /// Adds an absolute-positioned track. Returns its stable index.
    pub fn at(&mut self, at: Duration, animation: Animation) -> usize {
        let end = animation
            .duration()
            .map(|duration| at.saturating_add(duration));

        self.cursor = end;
        let mut state = AnimationState::default();

        state.sample(&animation, Duration::ZERO, false);
        self.frames.push(Frame::at(Target::default()));
        self.tracks.push(Track {
            at,
            animation,
            state,
        });

        self.tracks.len() - 1
    }

    /// Appends after the previously inserted track; an infinite predecessor is rejected.
    pub fn then(&mut self, animation: Animation) -> Result<usize, AnimationError> {
        let at = if self.tracks.is_empty() {
            Duration::ZERO
        } else {
            self.cursor.ok_or(AnimationError::InfiniteSequence)?
        };

        Ok(self.at(at, animation))
    }

    /// Records a label at the current finite sequence cursor.
    pub fn label(&mut self, name: impl Into<String>) -> Result<(), AnimationError> {
        let name = name.into();

        if self.labels.iter().any(|(n, _)| *n == name) {
            return Err(AnimationError::Label);
        }

        let at = if self.tracks.is_empty() {
            Duration::ZERO
        } else {
            self.cursor.ok_or(AnimationError::InfiniteSequence)?
        };

        self.labels.push((name, at));

        Ok(())
    }

    /// Adds an overlapping track relative to a previously recorded label.
    pub fn at_label(
        &mut self,
        name: &str,
        offset: Duration,
        animation: Animation,
    ) -> Result<usize, AnimationError> {
        let at = self
            .labels
            .iter()
            .find(|(n, _)| n == name)
            .ok_or(AnimationError::Label)?
            .1;

        Ok(self.at(at.saturating_add(offset), animation))
    }

    /// Starts this timeline's clock at zero at an application timestamp.
    pub fn restart(&mut self, now: Duration) {
        self.clock.seek(Duration::ZERO, now);
        self.clock.play(now);

        for track in &mut self.tracks {
            track.state.restart(now);
        }
    }

    /// Pauses the whole timeline.
    pub fn pause(&mut self, now: Duration) {
        self.clock.pause(now);
    }

    /// Resumes the whole timeline.
    pub fn play(&mut self, now: Duration) {
        self.clock.play(now);
    }

    /// Scrubs all tracks, including backward seeks.
    pub fn seek(&mut self, time: Duration, now: Duration) {
        self.clock.seek(time, now);
    }

    /// Changes the whole timeline playback rate, including negative playback.
    pub fn set_speed(&mut self, speed: f64, now: Duration) -> Result<(), AnimationError> {
        self.clock.set_speed(speed, now)
    }

    /// Samples all tracks into a retained buffer. No allocation occurs here.
    pub fn sample(&mut self, now: Duration, reduced_motion: bool) -> &[Frame] {
        let playhead = self.clock.time(now);
        let playing = self.clock.is_playing() && !(self.clock.speed < 0.0 && playhead.is_zero());

        self.active = false;

        for (track, output) in self.tracks.iter_mut().zip(&mut self.frames) {
            let local = playhead.saturating_sub(track.at);

            track.state.seek(local, now);
            let frame = track.state.sample(&track.animation, now, reduced_motion);
            let pending = playhead < track.at && !reduced_motion;

            self.active |= (pending || frame.is_animating()) && playing;
            *output = Frame::animated(
                frame.target(),
                origin_velocities(frame).map(|v| {
                    if pending || !playing {
                        0.0
                    } else {
                        v * self.clock.speed
                    }
                }),
                !pending && frame.is_animating() && playing,
                !pending && frame.just_completed(),
            );
        }

        &self.frames
    }

    /// Whether sampled tracks or future tracks require another frame.
    pub fn is_animating(&self) -> bool {
        self.active
    }
}

impl PartialEq for Animation {
    fn eq(&self, other: &Self) -> bool {
        self.same(other)
    }
}
