//! Kit-native clocks and rendering for the shared prepared playback engine.
use crate::{Clock, Frame, core};
use core::{Animation, AnimationError, Target};
use gpui_kit::{App, Window};

use std::{
    ops::{Deref, DerefMut},
    time::Duration,
};

/// Retained prepared-clip playback using Kit's controllable executor clock.
/// Controls are inherited from `core::AnimationState`; use `now(cx)` for them.
#[derive(Debug, Default)]
pub struct AnimationState {
    inner: core::AnimationState,
    clock: Clock,
}

impl AnimationState {
    /// Creates a player at a static initial target. Retain it between renders.
    pub fn new(initial: Target) -> Self {
        Self {
            inner: core::AnimationState::new(initial),
            clock: Clock::default(),
        }
    }

    /// Current native timestamp for pause/play/seek/speed/restart controls.
    /// Returns zero before the first native frame; no system clock is mixed in.
    pub fn now(&self, cx: &App) -> Duration {
        self.clock.now(cx)
    }

    /// Samples a prepared clip and schedules only while playing or delayed.
    pub fn frame(&mut self, animation: &Animation, window: &mut Window, cx: &App) -> Frame {
        let now = self.clock.tick(cx);

        let frame = self.sample(animation, now, cx.reduce_motion());

        if frame.is_animating() {
            window.request_animation_frame();
        }

        frame
    }

    /// Samples an explicit epoch and returns a Kit-ready frame.
    /// Do not mix explicit timestamps with native `frame`/`now` on one player.
    pub fn sample(&mut self, animation: &Animation, now: Duration, reduced_motion: bool) -> Frame {
        self.inner.sample(animation, now, reduced_motion).into()
    }
}

impl Deref for AnimationState {
    type Target = core::AnimationState;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for AnimationState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// Prepared tracks for separate Kit elements, using the shared timeline engine.
/// Track insertion allocates; sampling reuses a Kit frame buffer. Retain the
/// timeline, add tracks once, and use `now(cx)` for playback controls.
#[derive(Debug, Default)]
pub struct Timeline {
    inner: core::Timeline,
    frames: Vec<Frame>,
    clock: Clock,
}

impl Timeline {
    fn prepare_frame(&mut self, index: usize) -> usize {
        debug_assert_eq!(self.frames.len(), index);
        self.frames.push(core::Frame::at(Target::default()).into());

        index
    }

    /// Adds an absolute track and returns its stable frame index.
    pub fn at(&mut self, at: Duration, animation: Animation) -> usize {
        let index = self.inner.at(at, animation);

        self.prepare_frame(index)
    }

    /// Appends after the last finite track. Infinite predecessors return an error.
    pub fn then(&mut self, animation: Animation) -> Result<usize, AnimationError> {
        let index = self.inner.then(animation)?;

        Ok(self.prepare_frame(index))
    }

    /// Records a unique label at the current finite sequence cursor.
    pub fn label(&mut self, name: impl Into<String>) -> Result<(), AnimationError> {
        self.inner.label(name)
    }

    /// Adds a track relative to an existing label, returning its frame index.
    pub fn at_label(
        &mut self,
        name: &str,
        offset: Duration,
        animation: Animation,
    ) -> Result<usize, AnimationError> {
        let index = self.inner.at_label(name, offset, animation)?;

        Ok(self.prepare_frame(index))
    }

    /// Current native timestamp. Returns zero before the first `frame` call.
    pub fn now(&self, cx: &App) -> Duration {
        self.clock.now(cx)
    }

    /// Starts all prepared tracks over using this owner's clock.
    pub fn restart(&mut self, now: Duration) {
        self.inner.restart(now);
    }

    /// Pauses all tracks, including delays.
    pub fn pause(&mut self, now: Duration) {
        self.inner.pause(now);
    }

    /// Resumes all tracks.
    pub fn play(&mut self, now: Duration) {
        self.inner.play(now);
    }

    /// Holds every track at an absolute playhead, including backward seeks.
    pub fn seek(&mut self, playhead: Duration, now: Duration) {
        self.inner.seek(playhead, now);
    }

    /// Sets a finite rate; negative values play backward.
    pub fn set_speed(&mut self, speed: f64, now: Duration) -> Result<(), AnimationError> {
        self.inner.set_speed(speed, now)
    }

    /// Samples one explicit epoch without allocating. Do not mix with native time.
    pub fn sample(&mut self, now: Duration, reduced_motion: bool) -> &[Frame] {
        for (output, input) in self
            .frames
            .iter_mut()
            .zip(self.inner.sample(now, reduced_motion))
        {
            *output = (*input).into();
        }

        &self.frames
    }

    /// Samples native time and schedules even when only a future track is pending.
    pub fn frame(&mut self, window: &mut Window, cx: &App) -> &[Frame] {
        let now = self.clock.tick(cx);

        self.sample(now, cx.reduce_motion());

        if self.inner.is_animating() {
            window.request_animation_frame();
        }

        &self.frames
    }

    /// Whether the last sample needs another frame (including future tracks).
    pub fn is_animating(&self) -> bool {
        self.inner.is_animating()
    }
}
