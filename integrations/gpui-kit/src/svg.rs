//! Native SVG drawing transforms, without transforming a subtree or its hitbox.
use crate::core::SvgMotion;
use gpui_kit::{App, Window, px};
use std::{ops::Deref, time::Duration};

/// Retained SVG transform sampler using Kit's monotonic executor clock.
#[derive(Debug, Default)]
pub struct SvgMotionState {
    inner: crate::core::SvgMotionState,
    clock: crate::Clock,
}

impl SvgMotionState {
    /// Samples while rendering and requests frames only during active motion.
    pub fn frame(&mut self, motion: SvgMotion, window: &mut Window, cx: &App) -> SvgFrame {
        let now = self.clock.tick(cx);

        let frame = self.sample(motion, now, cx.reduce_motion());

        if frame.is_animating() {
            window.request_animation_frame();
        }

        frame
    }

    /// Samples an explicit epoch; do not mix this clock with native `frame` calls.
    pub fn sample(&mut self, motion: SvgMotion, now: Duration, reduced_motion: bool) -> SvgFrame {
        SvgFrame(self.inner.sample(motion, now, reduced_motion))
    }
}

/// A portable SVG presentation with Kit-native transform conversion.
#[derive(Clone, Copy, Debug)]
pub struct SvgFrame(crate::core::SvgFrame);

impl From<crate::core::SvgFrame> for SvgFrame {
    fn from(frame: crate::core::SvgFrame) -> Self {
        Self(frame)
    }
}

impl Deref for SvgFrame {
    type Target = crate::core::SvgFrame;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl SvgFrame {
    /// Converts to Kit's center-origin SVG transform: signed scale, then rotation.
    /// Only the drawing angle is reduced modulo one turn, preserving the engine's
    /// unwrapped presentation and velocity. This replaces any existing SVG transform.
    pub fn transformation(self) -> gpui_kit::Transformation {
        let [x, y] = self.target().scale_factors();

        let angle = self
            .target()
            .rotation_degrees()
            .rem_euclid(360.0)
            .to_radians() as f32;

        gpui_kit::Transformation::scale(gpui_kit::size(x, y))
            .with_rotation(gpui_kit::radians(angle))
            .with_translation(gpui_kit::point(
                px(self.target().translation()[0]),
                px(self.target().translation()[1]),
            ))
    }

    /// Converts with a custom pivot expressed as logical pixels from the SVG's
    /// top-left. Supply its untransformed bounds size. Translation compensates
    /// for Kit's fixed center origin. Native layout/hitbox remain unchanged.
    pub fn transformation_about(
        self,
        bounds: gpui_kit::Size<gpui_kit::Pixels>,
        pivot: gpui_kit::Point<gpui_kit::Pixels>,
    ) -> gpui_kit::Transformation {
        assert!(
            [
                f32::from(bounds.width),
                f32::from(bounds.height),
                f32::from(pivot.x),
                f32::from(pivot.y)
            ]
            .iter()
            .all(|v| v.is_finite())
                && bounds.width >= px(0.)
                && bounds.height >= px(0.),
            "SVG bounds and pivot must be finite with non-negative size"
        );
        let [sx, sy] = self.target().scale_factors().map(f64::from);

        let angle = self
            .target()
            .rotation_degrees()
            .rem_euclid(360.)
            .to_radians();

        let (sin, cos) = angle.sin_cos();
        let dx = f64::from(pivot.x) - f64::from(bounds.width) * 0.5;
        let dy = f64::from(pivot.y) - f64::from(bounds.height) * 0.5;
        let [tx, ty] = self.target().translation().map(f64::from);
        let finite = |v: f64| px(v.clamp(f64::from(f32::MIN), f64::from(f32::MAX)) as f32);

        self.transformation().with_translation(gpui_kit::point(
            finite(tx + dx - (cos * sx * dx - sin * sy * dy)),
            finite(ty + dy - (sin * sx * dx + cos * sy * dy)),
        ))
    }

    /// Applies GPU scale/rotation to one Kit SVG drawing around its bounds center.
    /// Native layout and hitbox stay unchanged, and clipping follows Kit's existing
    /// axis-aligned masks. This does not transform surrounding divs, text, or children.
    /// Use a fixed, sufficiently padded parent as the interactive hit target.
    ///
    /// ```
    /// use gpui_kit::{svg, prelude::*, px};
    /// use gpui_motion_kit::{SvgMotionState, core::{SvgMotion, SvgTarget}};
    /// use std::time::Duration;
    /// let mut state = SvgMotionState::default();
    /// let frame = state.sample(SvgMotion::new(SvgTarget::default().scale(1.2).rotate_degrees(30.0)), Duration::ZERO, false);
    /// let element = frame.apply(svg().path("icons/arrow.svg").size(px(24.0)));
    /// ```
    pub fn apply(self, element: gpui_kit::Svg) -> gpui_kit::Svg {
        element.with_transformation(self.transformation())
    }
}
