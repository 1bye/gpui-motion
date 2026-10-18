//! Portable style tracks applied to stock Kit's native styles.
use crate::core::{Color, ColorProperty, StyleMotion, StyleProperty};
use gpui_kit::{App, Rgba, Window};
use std::{ops::Deref, time::Duration};

/// Converts the engine's sRGB color to Kit's native RGBA value.
pub fn color_to_rgba(color: Color) -> Rgba {
    let [r, g, b, a] = color.channels();

    Rgba { r, g, b, a }
}

/// A requested style track has no public renderer hook in Kit 0.7.3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnsupportedStyle(StyleProperty);

impl UnsupportedStyle {
    /// The unsupported scalar property (element blur or backdrop blur).
    pub fn property(self) -> StyleProperty {
        self.0
    }
}

impl std::fmt::Display for UnsupportedStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "stock GPUI Kit does not support the {:?} style track",
            self.0
        )
    }
}

impl std::error::Error for UnsupportedStyle {}

/// Retain one style sampler per logical element. Uses Kit's executor clock.
#[derive(Debug, Default)]
pub struct StyleMotionState {
    inner: crate::core::StyleMotionState,
    clock: crate::Clock,
}

impl StyleMotionState {
    /// Samples during rendering and schedules only while a sampled track moves.
    /// Use `StyleFrame::try_apply` to detect unsupported filter tracks.
    pub fn frame(&mut self, motion: StyleMotion, window: &mut Window, cx: &App) -> StyleFrame {
        let now = self.clock.tick(cx);

        let frame = self.sample(motion, now, cx.reduce_motion());

        if frame.is_animating() {
            window.request_animation_frame();
        }

        frame
    }

    /// Samples one explicit epoch for deterministic tests. Do not mix with `frame`.
    pub fn sample(
        &mut self,
        motion: StyleMotion,
        now: Duration,
        reduced_motion: bool,
    ) -> StyleFrame {
        StyleFrame(self.inner.sample(motion, now, reduced_motion))
    }
}

/// Presented style tracks with Kit-native conversion and engine status getters.
#[derive(Clone, Copy, Debug)]
pub struct StyleFrame(crate::core::StyleFrame);

impl From<crate::core::StyleFrame> for StyleFrame {
    fn from(frame: crate::core::StyleFrame) -> Self {
        Self(frame)
    }
}

impl Deref for StyleFrame {
    type Target = crate::core::StyleFrame;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl StyleFrame {
    /// Applies Kit-supported tracks, panicking if a filter track is declared.
    /// Only introduced properties are replaced; layout properties reflow.
    ///
    /// # Panics
    /// Panics when element or backdrop blur is declared. Use `try_apply` for
    /// portable declarations that may contain unsupported filter tracks.
    pub fn apply<E: gpui_kit::Styled>(self, element: E) -> E {
        self.try_apply(element).expect("unsupported Kit style")
    }

    /// Applies introduced colors, spacing, borders, corners and one outer shadow.
    /// Returns `UnsupportedStyle` before applying anything if element/backdrop blur
    /// is declared, including a zero radius. Kit has no public filter hook.
    ///
    /// # Errors
    /// Returns `UnsupportedStyle` for element/backdrop blur, before mutating the element.
    pub fn try_apply<E: gpui_kit::Styled>(self, mut element: E) -> Result<E, UnsupportedStyle> {
        for property in [StyleProperty::Blur, StyleProperty::BackdropBlur] {
            if self.target().get(property).is_some() {
                return Err(UnsupportedStyle(property));
            }
        }

        use StyleProperty::*;
        use gpui_kit::px;

        if let Some(v) = self.target().get(RadiusTopLeft) {
            element.style().corner_radii.top_left = Some(px(v).into());
        }

        if let Some(v) = self.target().get(RadiusTopRight) {
            element.style().corner_radii.top_right = Some(px(v).into());
        }

        if let Some(v) = self.target().get(RadiusBottomRight) {
            element.style().corner_radii.bottom_right = Some(px(v).into());
        }

        if let Some(v) = self.target().get(RadiusBottomLeft) {
            element.style().corner_radii.bottom_left = Some(px(v).into());
        }

        if let Some(v) = self.target().get(BorderTop) {
            element.style().border_widths.top = Some(px(v).into());
        }

        if let Some(v) = self.target().get(BorderRight) {
            element.style().border_widths.right = Some(px(v).into());
        }

        if let Some(v) = self.target().get(BorderBottom) {
            element.style().border_widths.bottom = Some(px(v).into());
        }

        if let Some(v) = self.target().get(BorderLeft) {
            element.style().border_widths.left = Some(px(v).into());
        }

        if let Some(v) = self.target().get(PaddingTop) {
            element.style().padding.top = Some(px(v).into());
        }

        if let Some(v) = self.target().get(PaddingRight) {
            element.style().padding.right = Some(px(v).into());
        }

        if let Some(v) = self.target().get(PaddingBottom) {
            element.style().padding.bottom = Some(px(v).into());
        }

        if let Some(v) = self.target().get(PaddingLeft) {
            element.style().padding.left = Some(px(v).into());
        }

        if let Some(v) = self.target().get(MarginTop) {
            element.style().margin.top = Some(px(v).into());
        }

        if let Some(v) = self.target().get(MarginRight) {
            element.style().margin.right = Some(px(v).into());
        }

        if let Some(v) = self.target().get(MarginBottom) {
            element.style().margin.bottom = Some(px(v).into());
        }

        if let Some(v) = self.target().get(MarginLeft) {
            element.style().margin.left = Some(px(v).into());
        }

        if let Some(v) = self.target().get(GapX) {
            element.style().gap.width = Some(px(v).into());
        }

        if let Some(v) = self.target().get(GapY) {
            element.style().gap.height = Some(px(v).into());
        }

        if let Some(color) = self.target().get_color(ColorProperty::Background) {
            element = element.bg(color_to_rgba(color));
        }

        if let Some(color) = self.target().get_color(ColorProperty::Text) {
            element = element.text_color(color_to_rgba(color));
        }

        if let Some(color) = self.target().get_color(ColorProperty::Border) {
            element = element.border_color(color_to_rgba(color));
        }

        if [ShadowX, ShadowY, ShadowBlur, ShadowSpread]
            .iter()
            .any(|property| self.target().get(*property).is_some())
            || self.target().get_color(ColorProperty::Shadow).is_some()
        {
            let color = self
                .target()
                .get_color(ColorProperty::Shadow)
                .unwrap_or(Color::rgba(0., 0., 0., 0.25));

            let color = color_to_rgba(color);

            element.style().box_shadow = Some(vec![
                gpui_kit::BoxShadow::new(
                    px(self.target().get(ShadowX).unwrap_or(0.)),
                    px(self.target().get(ShadowY).unwrap_or(0.)),
                    color.into(),
                )
                .blur_radius(px(self.target().get(ShadowBlur).unwrap_or(0.)))
                .spread_radius(px(self.target().get(ShadowSpread).unwrap_or(0.))),
            ]);
        }

        Ok(element)
    }
}
