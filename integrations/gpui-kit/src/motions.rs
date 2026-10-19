//! Kit clock and frame conversion for the shared keyed collection.
use crate::{Clock, Frame};
use gpui_ce_motion::Motion;
use gpui_kit::{App, Window};

use std::{
    hash::Hash,
    ops::{Deref, DerefMut},
    time::Duration,
};

/// Component-owned keyed transitions using the shared engine and Kit's clock.
///
/// Batch results follow declaration order; stable keys preserve interruption
/// history through reordering. Omitted entries stay retained. Cleanup and capacity
/// methods are available through [`core::Motions`](gpui_ce_motion::Motions).
/// Retain this collection in the view and notify after changing model targets.
#[derive(Debug)]
pub struct Motions<K> {
    inner: gpui_ce_motion::Motions<K>,
    clock: Clock,
}

impl<K> Default for Motions<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K> Motions<K> {
    /// Creates an empty collection without allocating.
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    /// Reserves space for at least `capacity` keys, without creating trajectories.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: gpui_ce_motion::Motions::with_capacity(capacity),
            clock: Clock::default(),
        }
    }
}

impl<K> Deref for Motions<K> {
    type Target = gpui_ce_motion::Motions<K>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<K> DerefMut for Motions<K> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<K: Eq + Hash> Motions<K> {
    /// Samples a batch during render using one executor timestamp and Kit's
    /// reduced-motion preference. Requests one frame if any returned frame is active.
    /// Omitted entries do not request frames. Do not mix with explicit sample times.
    ///
    /// # Panics
    /// Panics on duplicate keys. Earlier entries may already have been sampled.
    pub fn frame<const N: usize>(
        &mut self,
        declarations: [(K, Motion); N],
        window: &mut Window,
        cx: &App,
    ) -> [Frame; N] {
        let now = self.clock.tick(cx);
        let frames = self.sample(declarations, now, cx.reduce_motion());

        if frames.iter().any(|frame| frame.is_animating()) {
            window.request_animation_frame();
        }

        frames
    }

    /// Samples one native entry. Use [`Self::frame`] to synchronize several entries.
    pub fn frame_one(&mut self, key: K, motion: Motion, window: &mut Window, cx: &App) -> Frame {
        let [frame] = self.frame([(key, motion)], window, cx);

        frame
    }

    /// Samples explicit elapsed time, returning Kit-ready frames. Do not mix
    /// this clock with [`Self::frame`]. See the shared engine for lifecycle details.
    ///
    /// # Panics
    /// Panics on duplicate keys. Earlier entries may already have been sampled.
    pub fn sample<const N: usize>(
        &mut self,
        declarations: [(K, Motion); N],
        now: Duration,
        reduced_motion: bool,
    ) -> [Frame; N] {
        self.inner
            .sample(declarations, now, reduced_motion)
            .map(Into::into)
    }

    /// Samples one key at explicit elapsed time, returning a Kit-ready frame.
    pub fn sample_one(
        &mut self,
        key: K,
        motion: Motion,
        now: Duration,
        reduced_motion: bool,
    ) -> Frame {
        self.inner
            .sample_one(key, motion, now, reduced_motion)
            .into()
    }
}
