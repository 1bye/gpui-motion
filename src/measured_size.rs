//! Intrinsic content measurement with explicit constraints and real layout interpolation.
use crate::{LayoutPositionState, Transition};
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, ContentMask, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, Style, Window, px, size,
};
use std::time::Instant;

/// Which measured dimensions interpolate. Other dimensions follow content immediately.
#[derive(Clone, Copy, Debug, Default)]
pub enum SizeAxis {
    /// Animate width only.
    Width,
    /// Animate height only, typically with an explicit width constraint.
    #[default]
    Height,
    /// Animate both measured dimensions.
    Both,
}
/// Measures content as a separate layout root, reserves its animated size, and
/// clips painting/input to that reservation. This adds a layout node. Constraints
/// are explicit because the content is measured before parent layout is resolved.
/// This is real layout sizing, not GPU projection. Retain the same CE-scoped ID.
pub struct AutoSize<E> {
    id: ElementId,
    child: Option<E>,
    width: Option<f32>,
    height: Option<f32>,
    axis: SizeAxis,
    transition: Transition,
    initial: Option<[f32; 2]>,
}
impl<E> AutoSize<E> {
    /// Creates a height animation. First measurement snaps by default.
    ///
    /// ```
    /// use gpui::{div, prelude::*, px};
    /// use gpui_ce_motion::AutoSize;
    /// let accordion = AutoSize::new("accordion", div().w(px(280.)).child("Measured content")).width(280.);
    /// ```
    pub fn new(id: impl Into<ElementId>, child: E) -> Self {
        Self {
            id: id.into(),
            child: Some(child),
            width: None,
            height: None,
            axis: SizeAxis::Height,
            transition: Transition::default(),
            initial: None,
        }
    }
    /// Chooses interpolated dimensions.
    pub fn axis(mut self, axis: SizeAxis) -> Self {
        self.axis = axis;
        self
    }
    /// Available width for intrinsic measurement. Does not force the child's width.
    pub fn width(mut self, width: f32) -> Self {
        assert!(width.is_finite() && width >= 0.);
        self.width = Some(width);
        self
    }
    /// Available height for intrinsic measurement.
    pub fn height(mut self, height: f32) -> Self {
        assert!(height.is_finite() && height >= 0.);
        self.height = Some(height);
        self
    }
    /// Sets positional spring/tween/instant timing for measured sizes.
    pub fn transition(mut self, transition: Transition) -> Self {
        self.transition = transition;
        self
    }
    /// Opts into entrance from a finite non-negative size, once per mount.
    pub fn initial(mut self, value: [f32; 2]) -> Self {
        assert!(value.iter().all(|v| v.is_finite() && *v >= 0.));
        self.initial = Some(value);
        self
    }
}
struct SizeHistory {
    state: LayoutPositionState,
    epoch: Instant,
}
// The official pin adds fade_out; registry CE 0.2.2 has only bounds.
#[allow(clippy::needless_update)]
fn content_mask(bounds: Bounds<Pixels>) -> ContentMask<Pixels> {
    ContentMask {
        bounds,
        ..Default::default()
    }
}
impl<E: IntoElement + 'static> IntoElement for AutoSize<E> {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl<E: IntoElement + 'static> Element for AutoSize<E> {
    type RequestLayoutState = AnyElement;
    type PrepaintState = Bounds<Pixels>;
    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        let mut child = self.child.take().expect("one layout").into_any_element();
        let available = |value: Option<f32>| {
            value.map_or(AvailableSpace::MaxContent, |v| {
                AvailableSpace::Definite(px(v))
            })
        };
        let measured = child.layout_as_root(
            size(available(self.width), available(self.height)),
            window,
            cx,
        );
        let target = [f64::from(measured.width), f64::from(measured.height)];
        let now = cx.background_executor().now();
        let transitions = match self.axis {
            SizeAxis::Width => [self.transition, Transition::Instant],
            SizeAxis::Height => [Transition::Instant, self.transition],
            SizeAxis::Both => [self.transition; 2],
        };
        let frame = window.with_element_state(
            id.expect("size ID"),
            |state: Option<SizeHistory>, window| {
                let mut state = state.unwrap_or_else(|| {
                    let mut state = LayoutPositionState::default();
                    if let Some(initial) = self.initial {
                        state.sample(
                            initial.map(f64::from),
                            self.transition,
                            std::time::Duration::ZERO,
                            cx.reduce_motion(),
                        );
                    }
                    SizeHistory { state, epoch: now }
                });
                let frame = state.state.sample_axes(
                    target,
                    transitions,
                    now.saturating_duration_since(state.epoch),
                    cx.reduce_motion(),
                );
                if frame.is_animating() {
                    window.request_animation_frame();
                }
                (frame, state)
            },
        );
        let [mut width, mut height] = frame
            .position()
            .map(|v| px(v.clamp(0., f64::from(f32::MAX)) as f32));
        if matches!(self.axis, SizeAxis::Height) {
            width = measured.width;
        }
        if matches!(self.axis, SizeAxis::Width) {
            height = measured.height;
        }
        let style = Style {
            size: size(width.into(), height.into()),
            flex_shrink: 0.,
            ..Default::default()
        };
        (window.request_layout(style, [], cx), child)
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) -> Bounds<Pixels> {
        window.with_content_mask(Some(content_mask(bounds)), |w| {
            child.prepaint_at(bounds.origin, w, cx);
        });
        bounds
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        child: &mut AnyElement,
        bounds: &mut Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_content_mask(Some(content_mask(*bounds)), |w| child.paint(w, cx));
    }
}
