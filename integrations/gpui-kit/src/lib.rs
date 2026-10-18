//! GPUI Kit bindings for the portable gpui-ce-motion animation engine.
//!
//! This adapter uses Kit's GPUI types. It does not enable the core crate's CE
//! feature, and it does not change either renderer. Retain one state per logical
//! object, declare its destination during render, and notify after input changes.
//!
//! ```
//! use gpui_ce_motion::{Motion, Target};
//! use gpui_motion_kit::MotionState;
//! use gpui_kit::{AnyElement, App, Window, div, prelude::*};
//!
//! fn panel(state: &mut MotionState, open: bool, window: &mut Window, cx: &App) -> AnyElement {
//!     let frame = state.frame(Motion::new(Target::default()).present(open), window, cx);
//!
//!     frame.render(|frame| frame.apply(div().child("Details")))
//! }
//! ```

use gpui_ce_motion::{Motion, MotionValue, Transition, ValueFrame};
use gpui_kit::{AnyElement, App, Empty, IntoElement, Styled, Window, px};

use std::{
    ops::Deref,
    time::{Duration, Instant},
};

/// The portable motion engine, available without enabling GPUI CE.
pub use gpui_ce_motion as core;

mod layout;
mod vector;

pub use vector::{VectorElement, VectorState};

pub use layout::{LayoutGroup, LayoutPositionElement};

/// View-owned motion state using GPUI Kit's monotonic executor clock.
#[derive(Debug, Default)]
pub struct MotionState {
    inner: gpui_ce_motion::MotionState,
    epoch: Option<Instant>,
}

impl MotionState {
    /// Samples during render and requests another frame only while moving.
    /// The Kit application supplies the operating-system reduced-motion policy.
    pub fn frame(&mut self, motion: Motion, window: &mut Window, cx: &App) -> Frame {
        let now = cx.background_executor().now();
        let epoch = *self.epoch.get_or_insert(now);

        let frame = self.inner.sample(
            motion,
            now.saturating_duration_since(epoch),
            cx.reduce_motion(),
        );

        if frame.is_animating() {
            window.request_animation_frame();
        }

        Frame(frame)
    }

    /// Samples an explicit monotonic epoch for simulations and deterministic tests.
    /// Do not mix explicit sampling and `frame` clocks on the same state.
    pub fn sample(&mut self, motion: Motion, now: Duration, reduced_motion: bool) -> Frame {
        Frame(self.inner.sample(motion, now, reduced_motion))
    }
}

/// A sampled frame with Kit-native element builders and portable status getters.
#[derive(Clone, Copy, Debug)]
pub struct Frame(gpui_ce_motion::Frame);

impl Deref for Frame {
    type Target = gpui_ce_motion::Frame;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Frame {
    /// Applies opacity, relative position, and declared dimensions/radius.
    /// Dimensions perform real layout. This sets relative position, left, and top;
    /// use an outer reservation when absolute placement is required.
    pub fn apply<E: Styled>(self, mut element: E) -> E {
        let target = self.target();

        if let Some(width) = target.width_px() {
            element = element.w(px(width));
        }

        if let Some(height) = target.height_px() {
            element = element.h(px(height));
        }

        if let Some(radius) = target.border_radius_px() {
            element = element.rounded(px(radius));
        }

        element
            .relative()
            .left(px(target.x_px()))
            .top(px(target.y_px()))
            .opacity(target.alpha())
    }

    /// Keeps exiting elements rendered until absent. The owner must guard input
    /// and restore focus before removing a focused outgoing element.
    pub fn render<E: IntoElement>(self, build: impl FnOnce(Self) -> E) -> AnyElement {
        if self.is_mounted() {
            build(self).into_any_element()
        } else {
            Empty.into_any_element()
        }
    }
}

/// A retained scalar presentation in caller-chosen units.
#[derive(Debug)]
pub struct ValueState {
    inner: MotionValue,
    epoch: Option<Instant>,
}

impl ValueState {
    /// Creates a settled scalar. Panics for non-finite input.
    pub fn new(value: f64) -> Self {
        Self {
            inner: MotionValue::new(value),
            epoch: None,
        }
    }

    /// Retargets and samples during render, preserving momentum on interruption.
    pub fn frame(
        &mut self,
        target: f64,
        transition: Transition,
        window: &mut Window,
        cx: &App,
    ) -> ValueFrame {
        let clock = cx.background_executor().now();
        let epoch = *self.epoch.get_or_insert(clock);
        let now = clock.saturating_duration_since(epoch);

        self.inner.animate(target, transition, now);
        let frame = self.inner.sample(now, cx.reduce_motion());

        if frame.is_animating() {
            window.request_animation_frame();
        }

        frame
    }
}

mod text {
    gpui_ce_motion::__motion_text_adapter!(gpui_kit);
}

pub use gpui_ce_motion::{
    NumberEffect, Scramble, TextClip, TextError, TextMatch, TextPreset, TextTarget, TextUnit,
    TextUnitInfo,
};

pub use text::{NumberElement, NumberMotionState, TextElement, TextMotionState};
