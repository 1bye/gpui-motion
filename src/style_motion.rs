//! Declarative native style tracks, separate from positional/presence targets.
use crate::{Color, ColorState, MotionValue, Transition};
use std::time::Duration;
/// Scalar style properties in logical pixels. Margins and shadow offsets/spread
/// may be negative; the other properties require non-negative targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum StyleProperty {
    /// RadiusTopLeft in logical pixels.
    RadiusTopLeft,
    /// RadiusTopRight in logical pixels.
    RadiusTopRight,
    /// RadiusBottomRight in logical pixels.
    RadiusBottomRight,
    /// RadiusBottomLeft in logical pixels.
    RadiusBottomLeft,
    /// BorderTop in logical pixels.
    BorderTop,
    /// BorderRight in logical pixels.
    BorderRight,
    /// BorderBottom in logical pixels.
    BorderBottom,
    /// BorderLeft in logical pixels.
    BorderLeft,
    /// PaddingTop in logical pixels.
    PaddingTop,
    /// PaddingRight in logical pixels.
    PaddingRight,
    /// PaddingBottom in logical pixels.
    PaddingBottom,
    /// PaddingLeft in logical pixels.
    PaddingLeft,
    /// MarginTop in logical pixels.
    MarginTop,
    /// MarginRight in logical pixels.
    MarginRight,
    /// MarginBottom in logical pixels.
    MarginBottom,
    /// MarginLeft in logical pixels.
    MarginLeft,
    /// GapX in logical pixels.
    GapX,
    /// GapY in logical pixels.
    GapY,
    /// ShadowX in logical pixels.
    ShadowX,
    /// ShadowY in logical pixels.
    ShadowY,
    /// ShadowBlur in logical pixels.
    ShadowBlur,
    /// ShadowSpread in logical pixels.
    ShadowSpread,
    /// Blur in logical pixels.
    Blur,
    /// BackdropBlur in logical pixels.
    BackdropBlur,
}
const COUNT: usize = 24;
/// Native color tracks, interpolated in linear light.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum ColorProperty {
    /// Element background.
    Background,
    /// Inherited text color.
    Text,
    /// Border paint.
    Border,
    /// A single outer shadow's paint.
    Shadow,
}
fn signed(index: usize) -> bool {
    (12..16).contains(&index) || [18, 19, 21].contains(&index)
}
/// Partial style destination. Omitted properties preserve the element's styles
/// until introduced; subsequently they retain their last declared destination.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StyleTarget {
    values: [Option<f64>; COUNT],
    colors: [Option<Color>; 4],
}
impl StyleTarget {
    /// Sets a scalar. Panics for non-finite or disallowed negative values.
    pub fn value(mut self, property: StyleProperty, value: f32) -> Self {
        let index = property as usize;
        assert!(
            value.is_finite() && (signed(index) || value >= 0.),
            "invalid style value"
        );
        self.values[index] = Some(value.into());
        self
    }
    /// Sets a color track.
    pub fn color(mut self, property: ColorProperty, color: Color) -> Self {
        self.colors[property as usize] = Some(color);
        self
    }
    /// Reads a scalar presentation, with non-negative properties clamped for drawing.
    pub fn get(self, property: StyleProperty) -> Option<f32> {
        let index = property as usize;
        self.values[index].map(|v| {
            v.clamp(
                if signed(index) {
                    f64::from(f32::MIN)
                } else {
                    0.
                },
                f64::from(f32::MAX),
            ) as f32
        })
    }
    /// Reads a presented color.
    pub fn get_color(self, property: ColorProperty) -> Option<Color> {
        self.colors[property as usize]
    }
    /// Sets corners clockwise from top-left.
    pub fn radii(mut self, values: [f32; 4]) -> Self {
        for (property, value) in [
            StyleProperty::RadiusTopLeft,
            StyleProperty::RadiusTopRight,
            StyleProperty::RadiusBottomRight,
            StyleProperty::RadiusBottomLeft,
        ]
        .into_iter()
        .zip(values)
        {
            self = self.value(property, value);
        }
        self
    }
    /// Sets border widths in top/right/bottom/left order.
    pub fn border_widths(mut self, values: [f32; 4]) -> Self {
        for (property, value) in [
            StyleProperty::BorderTop,
            StyleProperty::BorderRight,
            StyleProperty::BorderBottom,
            StyleProperty::BorderLeft,
        ]
        .into_iter()
        .zip(values)
        {
            self = self.value(property, value);
        }
        self
    }
    /// Sets padding in top/right/bottom/left order.
    pub fn padding(mut self, values: [f32; 4]) -> Self {
        for (property, value) in [
            StyleProperty::PaddingTop,
            StyleProperty::PaddingRight,
            StyleProperty::PaddingBottom,
            StyleProperty::PaddingLeft,
        ]
        .into_iter()
        .zip(values)
        {
            self = self.value(property, value);
        }
        self
    }
    /// Sets margins in top/right/bottom/left order.
    pub fn margin(mut self, values: [f32; 4]) -> Self {
        for (property, value) in [
            StyleProperty::MarginTop,
            StyleProperty::MarginRight,
            StyleProperty::MarginBottom,
            StyleProperty::MarginLeft,
        ]
        .into_iter()
        .zip(values)
        {
            self = self.value(property, value);
        }
        self
    }
    /// Sets horizontal and vertical gap.
    pub fn gap(self, x: f32, y: f32) -> Self {
        self.value(StyleProperty::GapX, x)
            .value(StyleProperty::GapY, y)
    }
    /// Sets a single outer shadow. No shadow-list topology interpolation is implied.
    pub fn shadow(self, offset: [f32; 2], blur: f32, spread: f32, color: Color) -> Self {
        self.value(StyleProperty::ShadowX, offset[0])
            .value(StyleProperty::ShadowY, offset[1])
            .value(StyleProperty::ShadowBlur, blur)
            .value(StyleProperty::ShadowSpread, spread)
            .color(ColorProperty::Shadow, color)
    }
}
/// Disposable declaration of style destinations and timing.
#[derive(Clone, Copy, Debug)]
pub struct StyleMotion {
    target: StyleTarget,
    initial: Option<StyleTarget>,
    transition: Transition,
    overrides: [Option<Transition>; COUNT],
    color_overrides: [Option<Transition>; 4],
}
impl StyleMotion {
    /// Creates a destination. First mount snaps unless `initial` is supplied.
    pub fn new(target: StyleTarget) -> Self {
        Self {
            target,
            initial: None,
            transition: Transition::default(),
            overrides: [None; COUNT],
            color_overrides: [None; 4],
        }
    }
    /// Chooses the first presentation; omitted channels inherit the destination.
    pub fn initial(mut self, target: StyleTarget) -> Self {
        self.initial = Some(target);
        self
    }
    /// Sets default timing for all style tracks.
    pub fn transition(mut self, transition: Transition) -> Self {
        self.transition = transition;
        self
    }
    /// Overrides timing for one scalar track.
    pub fn transition_for(mut self, property: StyleProperty, transition: Transition) -> Self {
        self.overrides[property as usize] = Some(transition);
        self
    }
    /// Overrides timing for one color track.
    pub fn color_transition(mut self, property: ColorProperty, transition: Transition) -> Self {
        self.color_overrides[property as usize] = Some(transition);
        self
    }
}
/// Sampled style properties, ready to apply with the native feature.
#[derive(Clone, Copy, Debug)]
pub struct StyleFrame {
    target: StyleTarget,
    active: bool,
    completed: bool,
}
impl StyleFrame {
    /// Presented style destination, with clamped readers.
    pub fn target(self) -> StyleTarget {
        self.target
    }
    /// Whether another sample is needed.
    pub fn is_animating(self) -> bool {
        self.active
    }
    /// Completion edge when every active scalar/color finishes.
    pub fn just_completed(self) -> bool {
        self.completed
    }
}
/// Retained allocation-free style sampler. Springs preserve each channel's
/// momentum; fields omitted after introduction retain their destination.
#[derive(Debug, Default)]
pub struct StyleMotionState {
    values: [Option<MotionValue>; COUNT],
    colors: [Option<ColorState>; 4],
    destination: StyleTarget,
    initialized: bool,
    active: bool,
    #[cfg(feature = "gpui")]
    epoch: Option<std::time::Instant>,
}
impl StyleMotionState {
    /// Samples using one monotonic application epoch. Reduced motion snaps.
    ///
    /// ```
    /// use gpui_ce_motion::{StyleMotionState, StyleMotion, StyleTarget, StyleProperty};
    /// use std::time::Duration;
    /// let mut state = StyleMotionState::default();
    /// let frame = state.sample(StyleMotion::new(StyleTarget::default().gap(12., 8.)), Duration::ZERO, false);
    /// assert_eq!(frame.target().get(StyleProperty::GapX), Some(12.));
    /// assert!(!frame.is_animating());
    /// ```
    pub fn sample(&mut self, motion: StyleMotion, now: Duration, reduced: bool) -> StyleFrame {
        let mut result = StyleTarget::default();
        let mut active = false;
        let mut completed = false;
        for i in 0..COUNT {
            if let Some(value) = motion.target.values[i] {
                self.destination.values[i] = Some(value);
            }
            if let Some(target) = self.destination.values[i] {
                let initial = if self.initialized {
                    target
                } else {
                    motion.initial.and_then(|v| v.values[i]).unwrap_or(target)
                };
                let state = self.values[i].get_or_insert_with(|| MotionValue::new(initial));
                state.animate(
                    target,
                    motion.overrides[i].unwrap_or(motion.transition),
                    now,
                );
                let frame = state.sample(now, reduced);
                completed |= frame.just_completed();
                result.values[i] = Some(frame.value());
                active |= frame.is_animating();
            }
        }
        for i in 0..4 {
            if let Some(color) = motion.target.colors[i] {
                completed |= self.colors[i].is_some() && self.destination.colors[i] != Some(color);
                self.destination.colors[i] = Some(color);
            }
            if let Some(target) = self.destination.colors[i] {
                let initial = if self.initialized {
                    target
                } else {
                    motion.initial.and_then(|v| v.colors[i]).unwrap_or(target)
                };
                completed |= !self.initialized && initial != target;
                let state = self.colors[i].get_or_insert_with(|| ColorState::new(initial));
                let (color, running) = state.sample(
                    target,
                    motion.color_overrides[i].unwrap_or(motion.transition),
                    now,
                    reduced,
                );
                result.colors[i] = Some(color);
                active |= running;
            }
        }
        let completed = (self.active || completed) && !active;
        self.active = active;
        self.initialized = true;
        StyleFrame {
            target: result,
            active,
            completed,
        }
    }
    /// Samples and schedules using native time; invoke during rendering.
    #[cfg(feature = "gpui")]
    pub fn frame(
        &mut self,
        motion: StyleMotion,
        window: &mut gpui::Window,
        cx: &gpui::App,
    ) -> StyleFrame {
        let now = *self.epoch.get_or_insert_with(std::time::Instant::now);
        let frame = self.sample(motion, now.elapsed(), cx.reduce_motion());
        if frame.is_animating() {
            window.request_animation_frame();
        }
        frame
    }
}
#[cfg(feature = "gpui")]
impl StyleFrame {
    /// Applies only introduced style tracks. Blur/filter/shadow lists are replaced
    /// when declared. Layout-affecting tracks reflow; blur requires offscreen paint.
    pub fn apply<E: gpui::Styled>(self, mut element: E) -> E {
        use StyleProperty::*;
        use gpui::px;
        if let Some(v) = self.target.get(RadiusTopLeft) {
            element.style().corner_radii.top_left = Some(px(v).into());
        }
        if let Some(v) = self.target.get(RadiusTopRight) {
            element.style().corner_radii.top_right = Some(px(v).into());
        }
        if let Some(v) = self.target.get(RadiusBottomRight) {
            element.style().corner_radii.bottom_right = Some(px(v).into());
        }
        if let Some(v) = self.target.get(RadiusBottomLeft) {
            element.style().corner_radii.bottom_left = Some(px(v).into());
        }
        if let Some(v) = self.target.get(BorderTop) {
            element.style().border_widths.top = Some(px(v).into());
        }
        if let Some(v) = self.target.get(BorderRight) {
            element.style().border_widths.right = Some(px(v).into());
        }
        if let Some(v) = self.target.get(BorderBottom) {
            element.style().border_widths.bottom = Some(px(v).into());
        }
        if let Some(v) = self.target.get(BorderLeft) {
            element.style().border_widths.left = Some(px(v).into());
        }
        if let Some(v) = self.target.get(PaddingTop) {
            element.style().padding.top = Some(px(v).into());
        }
        if let Some(v) = self.target.get(PaddingRight) {
            element.style().padding.right = Some(px(v).into());
        }
        if let Some(v) = self.target.get(PaddingBottom) {
            element.style().padding.bottom = Some(px(v).into());
        }
        if let Some(v) = self.target.get(PaddingLeft) {
            element.style().padding.left = Some(px(v).into());
        }
        if let Some(v) = self.target.get(MarginTop) {
            element.style().margin.top = Some(px(v).into());
        }
        if let Some(v) = self.target.get(MarginRight) {
            element.style().margin.right = Some(px(v).into());
        }
        if let Some(v) = self.target.get(MarginBottom) {
            element.style().margin.bottom = Some(px(v).into());
        }
        if let Some(v) = self.target.get(MarginLeft) {
            element.style().margin.left = Some(px(v).into());
        }
        if let Some(v) = self.target.get(GapX) {
            element.style().gap.width = Some(px(v).into());
        }
        if let Some(v) = self.target.get(GapY) {
            element.style().gap.height = Some(px(v).into());
        }
        if let Some(color) = self.target.get_color(ColorProperty::Background) {
            element = element.bg(color.to_rgba());
        }
        if let Some(color) = self.target.get_color(ColorProperty::Text) {
            element = element.text_color(color.to_rgba());
        }
        if let Some(color) = self.target.get_color(ColorProperty::Border) {
            element = element.border_color(color.to_rgba());
        }
        if self.target.values[18..22].iter().any(Option::is_some) || self.target.colors[3].is_some()
        {
            let color = self
                .target
                .get_color(ColorProperty::Shadow)
                .unwrap_or(Color::rgba(0., 0., 0., 0.25))
                .to_rgba();
            element.style().box_shadow = Some(vec![
                gpui::BoxShadow::new(
                    px(self.target.get(ShadowX).unwrap_or(0.)),
                    px(self.target.get(ShadowY).unwrap_or(0.)),
                    gpui::rgb_to_hsla(color),
                )
                .blur_radius(px(self.target.get(ShadowBlur).unwrap_or(0.)))
                .spread_radius(px(self.target.get(ShadowSpread).unwrap_or(0.))),
            ]);
        }
        if let Some(radius) = self.target.get(Blur) {
            element = element.filter(vec![gpui::Filter::Blur(px(radius))]);
        }
        if let Some(radius) = self.target.get(BackdropBlur) {
            element = element.backdrop_filter(vec![gpui::Filter::Blur(px(radius))]);
        }
        element
    }
}
