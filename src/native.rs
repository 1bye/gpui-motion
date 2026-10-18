use gpui::{AnyElement, App, Empty, IntoElement, Styled, Window, px};

use crate::{Frame, Motion, MotionState};

impl MotionState {
    /// Samples native monotonic time and GPUI's reduced-motion flag. Call only
    /// from `Render::render` or an element drawing phase: frame requests require
    /// an active view. Mutate model state and call `cx.notify()` from handlers.
    /// No frames are scheduled after settling or after a reduced-motion snap.
    ///
    /// ```
    /// use gpui::{AnyElement, App, Window, div, prelude::*};
    /// use gpui_ce_motion::{Motion, MotionState, Target};
    ///
    /// fn panel(state: &mut MotionState, open: bool, window: &mut Window, cx: &App) -> AnyElement {
    ///     let frame = state.frame(
    ///         Motion::new(Target::default()).present(open), window, cx,
    ///     );
    ///     frame.render(|frame| frame.apply(div().id("panel").child("Panel")))
    /// }
    /// ```
    pub fn frame(&mut self, motion: Motion, window: &mut Window, cx: &App) -> Frame {
        let frame = self.sample(motion, self.epoch.elapsed(), cx.reduce_motion());

        if frame.is_animating() {
            window.request_animation_frame();
        }

        frame
    }
}

impl Frame {
    /// Applies opacity, relative left/top insets, and declared width, height,
    /// and radius to a styled element. Sizes cause real layout reflow; these
    /// are not GPU transforms or layout projection. Undeclared layout properties
    /// preserve the element's style. Radius does not enable overflow clipping.
    /// This replaces the div's position mode and left/top values. Use a wrapper
    /// if the div itself needs absolute positioning. Normal layout space remains
    /// reserved until exit completes; siblings do not move with these offsets.
    pub fn apply<E: Styled>(self, element: E) -> E {
        let target = self.target();
        let mut element = element;

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
            .left(px(self.target().x_px()))
            .top(px(self.target().y_px()))
            .opacity(self.target().alpha())
    }

    /// Builds an element only while mounted, retaining it throughout exit.
    /// Continue declaring this every render even when `present(false)`.
    /// This does not disable handlers or transfer focus during exit: the owner
    /// must guard input using `is_present()` and restore focus before removal.
    pub fn render<E: IntoElement>(self, build: impl FnOnce(Self) -> E) -> AnyElement {
        if self.is_mounted() {
            build(self).into_any_element()
        } else {
            Empty.into_any_element()
        }
    }
}

impl crate::AnimationState {
    /// Current timestamp for native playback controls. Do not mix native `frame`
    /// with another epoch; use `state.pause(state.now())`, seek, or speed controls.
    pub fn now(&self) -> std::time::Duration {
        self.epoch.elapsed()
    }

    /// Samples native time and requests frames only while this prepared clip is playing.
    pub fn frame(&mut self, animation: &crate::Animation, window: &mut Window, cx: &App) -> Frame {
        let frame = self.sample(animation, self.epoch.elapsed(), cx.reduce_motion());

        if frame.is_animating() {
            window.request_animation_frame();
        }

        frame
    }
}

impl crate::Color {
    /// Converts the sampled color to GPUI's sRGB representation.
    pub fn to_rgba(self) -> gpui::Rgba {
        let [r, g, b, a] = self.channels();

        gpui::Rgba::new(r, g, b, a)
    }
}

impl crate::SvgMotionState {
    /// Samples native time and the app's reduced-motion flag. Call only during
    /// render/element drawing; notify the owner after target changes in input handlers.
    pub fn frame(
        &mut self,
        motion: crate::SvgMotion,
        window: &mut Window,
        cx: &App,
    ) -> crate::SvgFrame {
        let frame = self.sample(motion, self.epoch.elapsed(), cx.reduce_motion());

        if frame.is_animating() {
            window.request_animation_frame();
        }

        frame
    }
}

impl crate::SvgFrame {
    /// Converts to CE's center-origin SVG transform: signed scale, then rotation.
    /// Only the drawing angle is reduced modulo one turn, preserving the engine's
    /// unwrapped presentation and velocity. This replaces any existing SVG transform.
    pub fn transformation(self) -> gpui::Transformation {
        let [x, y] = self.target().scale_factors();

        let angle = self
            .target()
            .rotation_degrees()
            .rem_euclid(360.0)
            .to_radians() as f32;

        gpui::Transformation::scale(gpui::size(x, y))
            .with_rotation(gpui::radians(angle))
            .with_translation(gpui::point(
                px(self.target().translation()[0]),
                px(self.target().translation()[1]),
            ))
    }
    /// Converts with a custom pivot expressed as logical pixels from the SVG's
    /// top-left. Supply its untransformed bounds size. Translation compensates
    /// for CE's fixed center origin. Native layout/hitbox remain unchanged.
    pub fn transformation_about(
        self,
        bounds: gpui::Size<gpui::Pixels>,
        pivot: gpui::Point<gpui::Pixels>,
    ) -> gpui::Transformation {
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

        self.transformation().with_translation(gpui::point(
            finite(tx + dx - (cos * sx * dx - sin * sy * dy)),
            finite(ty + dy - (sin * sx * dx + cos * sy * dy)),
        ))
    }
    /// Applies GPU scale/rotation to one CE SVG drawing around its bounds center.
    /// Native layout and hitbox stay unchanged, and clipping follows CE's existing
    /// axis-aligned masks. This does not transform surrounding divs, text, or children.
    /// Use a fixed, sufficiently padded parent as the interactive hit target.
    ///
    /// ```
    /// use gpui::{svg, prelude::*, px};
    /// use gpui_ce_motion::{SvgMotion, SvgMotionState, SvgTarget};
    /// use std::time::Duration;
    /// let mut state = SvgMotionState::default();
    /// let frame = state.sample(SvgMotion::new(SvgTarget::default().scale(1.2).rotate_degrees(30.0)), Duration::ZERO, false);
    /// let element = frame.apply(svg().path("icons/arrow.svg").size(px(24.0)));
    /// ```
    pub fn apply(self, element: gpui::Svg) -> gpui::Svg {
        element.with_transformation(self.transformation())
    }
}
