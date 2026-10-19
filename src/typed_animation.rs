//! Typed prepared tracks over the existing per-channel playback and spring solver.
use crate::{
    Animation, AnimationError, AnimationState, Easing, Keyframe, Playback, Repeat, RepeatMode,
    Spring, Target,
};

use std::{
    any::Any,
    fmt::Debug,
    marker::PhantomData,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

/// Fixed numeric representation of a typed animation value. Implementations must
/// return finite channels and reconstruct drawable values without allocating.
/// `compatible` identifies an unchanged property mask; keyframes cannot silently
/// introduce/remove properties. Implement this for application-owned value types.
pub trait MotionData<const N: usize>: Copy + Debug + PartialEq + 'static {
    /// Numeric channels, including unclamped spring presentation where available.
    fn channels(self) -> [f64; N];

    /// Reconstructs a value using this value's property mask and sampled channels.
    fn with_channels(self, values: [f64; N]) -> Self;

    /// Whether both values declare the same properties.
    fn compatible(self, other: Self) -> bool;
}

impl MotionData<1> for f64 {
    fn channels(self) -> [f64; 1] {
        [self]
    }

    fn with_channels(self, values: [f64; 1]) -> Self {
        values[0]
    }

    fn compatible(self, _: Self) -> bool {
        true
    }
}

impl MotionData<6> for Target {
    fn channels(self) -> [f64; 6] {
        self.values()
    }

    fn with_channels(self, values: [f64; 6]) -> Self {
        Self::from_values(values, self.mask())
    }

    fn compatible(self, other: Self) -> bool {
        self.mask() == other.mask()
    }
}

/// A normalized typed keyframe; `current` captures each channel's presentation.
#[derive(Clone, Copy, Debug)]
pub struct TypedKeyframe<T> {
    at: f64,
    value: Option<T>,
    easing: Easing,
}

impl<T> TypedKeyframe<T> {
    /// Declares a value at normalized time `at`.
    pub fn new(at: f64, value: T) -> Self {
        Self {
            at,
            value: Some(value),
            easing: Easing::Linear,
        }
    }

    /// Captures current presentation at the first frame; later occurrences hold.
    pub fn current(at: f64) -> Self {
        Self {
            at,
            value: None,
            easing: Easing::Linear,
        }
    }

    /// Easing from the preceding frame to this frame.
    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;

        self
    }
}

fn scalar(value: f64) -> Target {
    Target::from_values([value, 0.0, 1.0, 0.0, 0.0, 0.0], 1)
}

/// Immutable typed clip. Channels use the same solver as ordinary `Animation`;
/// interruption preserves each spring channel's own velocity. Prepare once.
#[derive(Clone, Debug)]
pub struct TypedAnimation<T, const N: usize> {
    channels: [Animation; N],
    template: T,
}

impl<T: MotionData<N>, const N: usize> TypedAnimation<T, N> {
    /// Validates timing and consistent masks before preparing numeric channels.
    /// Uses `AnimationError::Keyframes` for non-finite/mismatched value channels.
    pub fn keyframes(
        duration: Duration,
        frames: impl IntoIterator<Item = TypedKeyframe<T>>,
    ) -> Result<Self, AnimationError> {
        let frames: Vec<_> = frames.into_iter().collect();

        let template = frames
            .iter()
            .find_map(|frame| frame.value)
            .ok_or(AnimationError::Keyframes)?;

        if N == 0
            || frames.iter().filter_map(|frame| frame.value).any(|value| {
                !template.compatible(value) || value.channels().iter().any(|v| !v.is_finite())
            })
        {
            return Err(AnimationError::Keyframes);
        }

        let channels = (0..N)
            .map(|i| {
                Animation::keyframes(
                    duration,
                    frames.iter().map(|frame| {
                        let key = frame.value.map_or_else(
                            || Keyframe::current(frame.at),
                            |value| Keyframe::new(frame.at, scalar(value.channels()[i])),
                        );

                        key.easing(frame.easing)
                    }),
                )
            })
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .unwrap_or_else(|_| unreachable!());

        Ok(Self { channels, template })
    }

    /// Captures the current value and tweens to a typed destination.
    pub fn to(value: T, duration: Duration, easing: Easing) -> Result<Self, AnimationError> {
        let values = value.channels();

        if N == 0 || values.iter().any(|value| !value.is_finite()) {
            return Err(AnimationError::Keyframes);
        }

        let channels = std::array::from_fn(|i| Animation::to(scalar(values[i]), duration, easing));

        Ok(Self {
            channels,
            template: value,
        })
    }

    /// Creates per-channel physical springs with a finite deadline.
    pub fn spring(value: T, spring: Spring, deadline: Duration) -> Result<Self, AnimationError> {
        if N == 0 || value.channels().iter().any(|v| !v.is_finite()) {
            return Err(AnimationError::Keyframes);
        }

        let channels = std::array::from_fn(|i| {
            Animation::spring(scalar(value.channels()[i]), spring, deadline)
        });

        Ok(Self {
            channels,
            template: value,
        })
    }

    /// Adds a hold before playback.
    pub fn delay(mut self, duration: Duration) -> Self {
        self.channels = self.channels.map(|channel| channel.delay(duration));

        self
    }

    /// Configures finite/forever repeats identically for all channels.
    pub fn repeat(
        mut self,
        repeat: Repeat,
        mode: RepeatMode,
        delay: Duration,
    ) -> Result<Self, AnimationError> {
        self.channels = self
            .channels
            .into_iter()
            .map(|channel| channel.repeat(repeat, mode, delay))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .unwrap_or_else(|_| unreachable!());

        Ok(self)
    }

    /// Total duration including repeats/delays; `None` denotes forever.
    pub fn duration(&self) -> Option<Duration> {
        self.channels[0].duration()
    }
}

/// Sampled typed presentation and scheduling state.
#[derive(Clone, Copy, Debug)]
pub struct TypedFrame<T> {
    value: T,
    active: bool,
    completed: bool,
}

impl<T: Copy> TypedFrame<T> {
    /// Current drawable value.
    pub fn value(self) -> T {
        self.value
    }

    /// Whether another frame is required.
    pub fn is_animating(self) -> bool {
        self.active
    }

    /// Completion edge consumed by this sample.
    pub fn just_completed(self) -> bool {
        self.completed
    }
}

/// Retained independent channel trajectories. No allocation occurs in sampling.
#[derive(Debug)]
pub struct TypedState<T, const N: usize> {
    states: [AnimationState; N],
    frame: TypedFrame<T>,
    #[cfg(feature = "gpui")]
    epoch: Option<std::time::Instant>,
}

impl<T: MotionData<N>, const N: usize> TypedState<T, N> {
    /// Seeds all channels. Panics for a non-finite seed or an empty representation.
    pub fn new(value: T) -> Self {
        assert!(
            N > 0 && value.channels().iter().all(|v| v.is_finite()),
            "invalid typed seed"
        );

        Self {
            states: std::array::from_fn(|i| AnimationState::new(scalar(value.channels()[i]))),
            frame: TypedFrame {
                value,
                active: false,
                completed: false,
            },
            #[cfg(feature = "gpui")]
            epoch: None,
        }
    }

    /// Samples at one explicit monotonic epoch. Panics if the clip's mask differs
    /// from the seed's mask; create a new retained state for a different shape.
    pub fn sample(
        &mut self,
        clip: &TypedAnimation<T, N>,
        now: Duration,
        reduced: bool,
    ) -> TypedFrame<T> {
        assert!(
            self.frame.value.compatible(clip.template),
            "typed property masks must match"
        );
        let samples = std::array::from_fn::<_, N, _>(|i| {
            self.states[i].sample(&clip.channels[i], now, reduced)
        });

        let active = samples.iter().any(|frame| frame.is_animating());

        self.frame = TypedFrame {
            value: clip
                .template
                .with_channels(samples.map(|frame| frame.target().values()[0])),
            active,
            completed: !active && samples.iter().any(|frame| frame.just_completed()),
        };

        self.frame
    }

    /// Last sampled value.
    pub fn value(&self) -> T {
        self.frame.value
    }

    /// Per-channel physical velocity.
    pub fn velocities(&self) -> [f64; N] {
        std::array::from_fn(|i| self.states[i].current_velocity())
    }

    /// Pauses every channel.
    pub fn pause(&mut self, now: Duration) {
        for state in &mut self.states {
            state.pause(now);
        }
    }

    /// Resumes every channel.
    pub fn play(&mut self, now: Duration) {
        for state in &mut self.states {
            state.play(now);
        }
    }

    /// Seeks every channel.
    pub fn seek(&mut self, time: Duration, now: Duration) {
        for state in &mut self.states {
            state.seek(time, now);
        }
    }

    /// Restarts every channel; cancels previous completion observers.
    pub fn restart(&mut self, now: Duration) {
        for state in &mut self.states {
            state.restart(now);
        }
    }

    /// Validates and changes speed for all channels transactionally.
    pub fn set_speed(&mut self, speed: f64, now: Duration) -> Result<(), AnimationError> {
        if !speed.is_finite() {
            return Err(AnimationError::Speed);
        }

        for state in &mut self.states {
            state.set_speed(speed, now)?;
        }

        Ok(())
    }

    /// Completes all channels at the next sample, including forever clips.
    pub fn finish(&mut self) {
        for state in &mut self.states {
            state.finish();
        }
    }

    /// Observes completion/cancellation. All channels share the same clip timing.
    pub fn completion(&mut self) -> crate::Completion {
        self.states[0].completion()
    }

    /// Whether the last sample requires another frame.
    pub fn is_animating(&self) -> bool {
        self.frame.active
    }

    /// Native executor time for controls; zero before the first native frame.
    #[cfg(feature = "gpui")]
    pub fn now(&self, cx: &gpui::App) -> Duration {
        self.epoch.map_or(Duration::ZERO, |epoch| {
            cx.background_executor()
                .now()
                .saturating_duration_since(epoch)
        })
    }

    /// Samples on stock CE and schedules only while active. Do not mix clocks.
    #[cfg(feature = "gpui")]
    pub fn frame(
        &mut self,
        clip: &TypedAnimation<T, N>,
        window: &mut gpui::Window,
        cx: &gpui::App,
    ) -> TypedFrame<T> {
        let instant = cx.background_executor().now();
        let epoch = *self.epoch.get_or_insert(instant);

        let frame = self.sample(
            clip,
            instant.saturating_duration_since(epoch),
            cx.reduce_motion(),
        );

        if frame.active {
            window.request_animation_frame();
        }

        frame
    }
}

trait ErasedTrack: Debug {
    fn sample(&mut self, time: Duration, now: Duration, speed: f64, reduced: bool) -> bool;

    fn restart(&mut self);

    fn as_any(&self) -> &dyn Any;
}

#[derive(Debug)]
struct Track<T, const N: usize> {
    clip: TypedAnimation<T, N>,
    state: TypedState<T, N>,
    start: Duration,
}

impl<T: MotionData<N>, const N: usize> ErasedTrack for Track<T, N> {
    fn sample(&mut self, time: Duration, now: Duration, speed: f64, reduced: bool) -> bool {
        self.state
            .set_speed(speed, now)
            .expect("validated timeline speed");
        self.state.seek(time.saturating_sub(self.start), now);
        self.state.sample(&self.clip, now, reduced);

        (time < self.start && !reduced)
            || !self.state.states.iter().all(|state| state.is_complete())
    }

    fn restart(&mut self) {
        self.state.restart(Duration::ZERO);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

static NEXT_TIMELINE: AtomicU64 = AtomicU64::new(1);

/// Typed track identity, scoped to the timeline that created it.
#[derive(Debug)]
pub struct TrackId<T, const N: usize> {
    owner: u64,
    index: usize,
    marker: PhantomData<T>,
}

impl<T, const N: usize> Copy for TrackId<T, N> {}

impl<T, const N: usize> Clone for TrackId<T, N> {
    fn clone(&self) -> Self {
        *self
    }
}

/// A heterogeneous timeline of scalar, color, style, SVG and target tracks.
/// Track setup allocates; samples reuse each track's state. Deferred tracks keep
/// scheduling even before their first visible sample. Ownership is explicit.
#[derive(Debug)]
pub struct TypedTimeline {
    owner: u64,
    tracks: Vec<Box<dyn ErasedTrack>>,
    clock: Playback,
    cursor: Option<Duration>,
    labels: Vec<(String, Duration)>,
    active: bool,
    completed: bool,
    observers: crate::completion::CompletionSource,
    #[cfg(feature = "gpui")]
    epoch: Option<std::time::Instant>,
}

impl Default for TypedTimeline {
    fn default() -> Self {
        Self {
            owner: NEXT_TIMELINE.fetch_add(1, Ordering::Relaxed),
            tracks: Vec::new(),
            clock: Playback::default(),
            cursor: Some(Duration::ZERO),
            labels: Vec::new(),
            active: false,
            completed: false,
            observers: Default::default(),
            #[cfg(feature = "gpui")]
            epoch: None,
        }
    }
}

impl TypedTimeline {
    /// Adds a typed track at an absolute playhead position, with its initial value.
    pub fn at<T: MotionData<N>, const N: usize>(
        &mut self,
        start: Duration,
        initial: T,
        clip: TypedAnimation<T, N>,
    ) -> TrackId<T, N> {
        assert!(
            initial.compatible(clip.template),
            "typed property masks must match"
        );
        self.observers.resolve(crate::CompletionStatus::Cancelled);
        self.cursor = clip
            .duration()
            .map(|duration| start.saturating_add(duration));
        let id = TrackId {
            owner: self.owner,
            index: self.tracks.len(),
            marker: PhantomData,
        };

        let mut state = TypedState::new(initial);

        state.sample(&clip, Duration::ZERO, false);
        self.tracks.push(Box::new(Track { state, clip, start }));
        self.completed = false;

        id
    }

    fn offset(base: Option<Duration>, seconds: f64) -> Result<Duration, AnimationError> {
        let base = base.ok_or(AnimationError::Times)?;
        let value = base.as_secs_f64() + seconds;

        Duration::try_from_secs_f64(value).map_err(|_| AnimationError::Times)
    }

    /// Adds after the preceding track's end, with positive delay or negative overlap.
    /// Fails without mutation after a forever track or for a negative/invalid time.
    pub fn after<T: MotionData<N>, const N: usize>(
        &mut self,
        seconds: f64,
        initial: T,
        clip: TypedAnimation<T, N>,
    ) -> Result<TrackId<T, N>, AnimationError> {
        let at = Self::offset(self.cursor, seconds)?;

        Ok(self.at(at, initial, clip))
    }

    /// Records a unique label at an absolute position.
    pub fn label(&mut self, name: impl Into<String>, at: Duration) -> Result<(), AnimationError> {
        let name = name.into();

        if self.labels.iter().any(|(old, _)| *old == name) {
            return Err(AnimationError::Label);
        }

        self.labels.push((name, at));

        Ok(())
    }

    /// Adds at a label with a signed seconds offset; missing labels fail transactionally.
    pub fn at_label<T: MotionData<N>, const N: usize>(
        &mut self,
        name: &str,
        seconds: f64,
        initial: T,
        clip: TypedAnimation<T, N>,
    ) -> Result<TrackId<T, N>, AnimationError> {
        let at = self
            .labels
            .iter()
            .find(|(label, _)| label == name)
            .map(|(_, at)| *at)
            .ok_or(AnimationError::Label)?;

        Ok(self.at(Self::offset(Some(at), seconds)?, initial, clip))
    }

    /// Samples all tracks and dispatches completion observers at the end.
    pub fn sample(&mut self, now: Duration, reduced: bool) {
        let time = self.clock.time(now);
        let mut unfinished = false;

        let speed = if self.clock.is_playing() {
            self.clock.speed()
        } else {
            0.0
        };

        for track in &mut self.tracks {
            unfinished |= track.sample(time, now, speed, reduced);
        }

        if speed < 0.0 && time.is_zero() {
            unfinished = false;
        }

        self.active = unfinished && self.clock.is_playing();

        if !unfinished && !self.completed {
            self.observers.resolve(crate::CompletionStatus::Completed);
        }

        self.completed = !unfinished;
    }

    /// Reads a sampled typed value; foreign track identities return `None`.
    pub fn value<T: MotionData<N>, const N: usize>(&self, id: TrackId<T, N>) -> Option<T> {
        if id.owner != self.owner {
            return None;
        }

        self.tracks
            .get(id.index)?
            .as_any()
            .downcast_ref::<Track<T, N>>()
            .map(|track| track.state.value())
    }

    /// Pauses the shared clock.
    pub fn pause(&mut self, now: Duration) {
        self.clock.pause(now);
    }

    /// Resumes the shared clock.
    pub fn play(&mut self, now: Duration) {
        self.clock.play(now);
    }

    /// Seeks without replacing prepared tracks.
    pub fn seek(&mut self, time: Duration, now: Duration) {
        self.clock.seek(time, now);
    }

    /// Changes speed, including zero and reverse playback.
    pub fn set_speed(&mut self, speed: f64, now: Duration) -> Result<(), AnimationError> {
        self.clock.set_speed(speed, now)
    }

    /// Restarts and rearms reduced-motion completion.
    pub fn restart(&mut self, now: Duration) {
        self.observers.resolve(crate::CompletionStatus::Cancelled);
        self.clock.seek(Duration::ZERO, now);
        self.clock.play(now);

        for track in &mut self.tracks {
            track.restart();
        }

        self.completed = false;
    }

    /// Completion observer for this run. Dropping this timeline cancels observers.
    pub fn completion(&mut self) -> crate::Completion {
        self.observers.subscribe(self.completed)
    }

    /// Whether another sample is required, including upcoming delayed tracks.
    pub fn is_animating(&self) -> bool {
        self.active
    }

    /// Native executor time for controls.
    #[cfg(feature = "gpui")]
    pub fn now(&self, cx: &gpui::App) -> Duration {
        self.epoch.map_or(Duration::ZERO, |epoch| {
            cx.background_executor()
                .now()
                .saturating_duration_since(epoch)
        })
    }

    /// Samples a stock CE window using its executor; do not mix with explicit time.
    #[cfg(feature = "gpui")]
    pub fn frame(&mut self, window: &mut gpui::Window, cx: &gpui::App) {
        let instant = cx.background_executor().now();
        let epoch = *self.epoch.get_or_insert(instant);

        self.sample(instant.saturating_duration_since(epoch), cx.reduce_motion());

        if self.active {
            window.request_animation_frame();
        }
    }
}

impl<const N: usize> MotionData<N> for [f64; N] {
    fn channels(self) -> [f64; N] {
        self
    }

    fn with_channels(self, values: [f64; N]) -> Self {
        values
    }

    fn compatible(self, _: Self) -> bool {
        true
    }
}
