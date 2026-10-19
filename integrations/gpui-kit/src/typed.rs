//! Native clocks for the shared typed clip and timeline engines.
use crate::core::{AnimationError, MotionData, TypedAnimation, TypedFrame};
use gpui_kit::{App, Window};

use std::{
    ops::{Deref, DerefMut},
    time::Duration,
};

/// Kit executor clock around a shared, independently sampled typed state.
#[derive(Debug)]
pub struct TypedState<T, const N: usize> {
    inner: crate::core::TypedState<T, N>,
    clock: crate::Clock,
}

impl<T, const N: usize> Deref for TypedState<T, N> {
    type Target = crate::core::TypedState<T, N>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T, const N: usize> DerefMut for TypedState<T, N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T: MotionData<N>, const N: usize> TypedState<T, N> {
    /// Seeds the native presentation; use matching property masks in its clips.
    pub fn new(value: T) -> Self {
        Self {
            inner: crate::core::TypedState::new(value),
            clock: Default::default(),
        }
    }

    /// Native control timestamp. Do not mix this clock with manual samples.
    pub fn now(&self, cx: &App) -> Duration {
        self.clock.now(cx)
    }

    /// Samples the shared clip and schedules only while active.
    pub fn frame(
        &mut self,
        clip: &TypedAnimation<T, N>,
        window: &mut Window,
        cx: &App,
    ) -> TypedFrame<T> {
        let now = self.clock.tick(cx);
        let frame = self.inner.sample(clip, now, cx.reduce_motion());

        if frame.is_animating() {
            window.request_animation_frame();
        }

        frame
    }
}

/// Kit executor clock around a shared heterogeneous timeline.
#[derive(Debug, Default)]
pub struct TypedTimeline {
    inner: crate::core::TypedTimeline,
    clock: crate::Clock,
}

impl Deref for TypedTimeline {
    type Target = crate::core::TypedTimeline;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for TypedTimeline {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl TypedTimeline {
    /// Native timestamp for playback controls.
    pub fn now(&self, cx: &App) -> Duration {
        self.clock.now(cx)
    }

    /// Samples all typed tracks using Kit's reduced-motion preference.
    pub fn frame(&mut self, window: &mut Window, cx: &App) {
        self.inner.sample(self.clock.tick(cx), cx.reduce_motion());

        if self.inner.is_animating() {
            window.request_animation_frame();
        }
    }

    /// Changes playback speed using this adapter's clock.
    pub fn speed(&mut self, speed: f64, cx: &App) -> Result<(), AnimationError> {
        self.inner.set_speed(speed, self.now(cx))
    }
}
