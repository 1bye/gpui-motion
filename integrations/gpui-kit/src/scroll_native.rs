//! Native measured scroll-to-target mapping.
use crate::MotionState;
use crate::core::{Motion, Target, Transition};

use gpui_kit::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, ScrollHandle, Styled, Window,
};

use std::time::Instant;

/// Native scroll axis.
#[derive(Clone, Copy, Debug)]
pub enum Axis {
    /// Horizontal.
    X,
    /// Vertical.
    Y,
}

/// Reads native scroll offsets and extents; unscrollable content returns zero.
/// Handles Kit's negative scroll offsets and positive maximum extents.
pub fn native_scroll_progress(handle: &ScrollHandle, axis: Axis) -> f64 {
    let offset = handle.offset();
    let max = handle.max_offset();

    let (offset, max) = match axis {
        Axis::X => (offset.x, max.x),
        Axis::Y => (offset.y, max.y),
    };

    let max = f64::from(max).abs();

    if max == 0. {
        0.
    } else {
        (-f64::from(offset) / max).clamp(0., 1.)
    }
}

/// Binds native scroll progress to partial numeric targets. Defaults to instant
/// mapping; choose a spring for smoothing. Layout extent changes trigger a
/// follow-up frame, so the previous measurement cannot remain stale after resize.
/// Preserve its Kit-scoped ID and configure styles before wrapping.
pub struct ScrollMotion<E> {
    id: ElementId,
    child: Option<E>,
    handle: ScrollHandle,
    axis: Axis,
    from: Target,
    to: Target,
    transition: Transition,
}

impl<E> ScrollMotion<E> {
    /// Creates a measured scroll binding.
    pub fn new(
        id: impl Into<ElementId>,
        handle: ScrollHandle,
        axis: Axis,
        from: Target,
        to: Target,
        child: E,
    ) -> Self {
        Self {
            id: id.into(),
            child: Some(child),
            handle,
            axis,
            from,
            to,
            transition: Transition::Instant,
        }
    }

    /// Selects optional smoothing.
    pub fn transition(mut self, transition: Transition) -> Self {
        self.transition = transition;

        self
    }
}

struct History {
    state: MotionState,
    epoch: Instant,
    progress: f64,
}

impl<E: IntoElement + Styled + 'static> IntoElement for ScrollMotion<E> {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl<E: IntoElement + Styled + 'static> Element for ScrollMotion<E> {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

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
        w: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        let progress = native_scroll_progress(&self.handle, self.axis);
        let now = cx.background_executor().now();
        let target = interpolate(self.from, self.to, progress);

        let frame = w.with_element_state(id.expect("scroll ID"), |state: Option<History>, w| {
            let mut state = state.unwrap_or_else(|| History {
                state: MotionState::default(),
                epoch: now,
                progress,
            });

            state.progress = progress;
            let frame = state.state.sample(
                Motion::new(target).transition(self.transition),
                now.saturating_duration_since(state.epoch),
                cx.reduce_motion(),
            );

            if frame.is_animating() {
                w.request_animation_frame();
            }

            (frame, state)
        });

        let mut child = frame
            .apply(self.child.take().expect("one layout"))
            .into_any_element();

        let layout = child.request_layout(w, cx);

        (layout, child)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        child: &mut AnyElement,
        w: &mut Window,
        cx: &mut App,
    ) {
        child.prepaint(w, cx);
        let progress = native_scroll_progress(&self.handle, self.axis);

        w.with_element_state(id.expect("scroll ID"), |state: Option<History>, w| {
            let state = state.expect("layout first");

            if (state.progress - progress).abs() > 1e-9 {
                w.request_animation_frame();
            }

            ((), state)
        });
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        child: &mut AnyElement,
        _: &mut (),
        w: &mut Window,
        cx: &mut App,
    ) {
        child.paint(w, cx);
    }
}

// Map renderer-measured progress to the engine's public numeric target API.
fn interpolate(from: Target, to: Target, progress: f64) -> Target {
    let blend = |a: f32, b: f32| (f64::from(a) + (f64::from(b) - f64::from(a)) * progress) as f32;

    let mut target = Target::default()
        .x(blend(from.x_px(), to.x_px()))
        .y(blend(from.y_px(), to.y_px()))
        .opacity(blend(from.alpha(), to.alpha()));

    for (a, b, property) in [
        (from.width_px(), to.width_px(), 0),
        (from.height_px(), to.height_px(), 1),
        (from.border_radius_px(), to.border_radius_px(), 2),
    ] {
        if let Some(value) = a.or(b) {
            let value = blend(a.unwrap_or(value), b.unwrap_or(value));

            target = match property {
                0 => target.width(value),
                1 => target.height(value),
                _ => target.border_radius(value),
            };
        }
    }

    target
}
