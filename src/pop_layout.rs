//! Managed-container exit layout without subtree transforms.
use crate::{LayoutPositionState, Transition};
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, Display, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, Point, Style, Window, point, px, size,
};
use std::{cell::Cell, rc::Rc, time::Instant};
/// Retain in the view. A container records its origin; exiting items reserve no
/// flow space but remain at their last measured parent-relative bounds. Use the
/// same container and retain outgoing content with `PresenceGroup` until exit
/// finishes. Ancestor clipping still applies. Independent containers must use
/// separate handles; this is not cross-parent projection.
#[derive(Clone, Debug, Default)]
pub struct PopLayout {
    origin: Rc<Cell<Point<Pixels>>>,
}
impl PopLayout {
    /// Wraps the container without adding a layout node.
    ///
    /// ```
    /// use gpui::{div, prelude::*};
    /// use gpui_ce_motion::PopLayout;
    /// let pop = PopLayout::default(); // persistent view field
    /// let row = div().flex().child(pop.item("item", true, div().child("Retained content")));
    /// let container = pop.container("container", row);
    /// ```
    ///
    /// Exiting children are
    /// measured as separate roots and painted in its coordinate space, so borders,
    /// padding and gaps do not introduce absolute-positioning corrections.
    pub fn container<E: IntoElement + 'static>(
        &self,
        id: impl Into<ElementId>,
        child: E,
    ) -> PopElement<E> {
        PopElement {
            id: id.into(),
            child: Some(child),
            scope: self.clone(),
            present: None,
            transition: Transition::default(),
        }
    }
    /// Wraps a retained item. False removes it from flow immediately. Its child
    /// still owns exit opacity/motion, input suppression and focus restoration.
    /// Owns positional reflow as well as exit positioning. Do not also wrap the
    /// item in `LayoutGroup::position`. For x/y exit motion, animate inner content
    /// inside a fixed-size child wrapper; the outer item owns its layout position.
    pub fn item<E: IntoElement + 'static>(
        &self,
        id: impl Into<ElementId>,
        present: bool,
        child: E,
    ) -> PopElement<E> {
        PopElement {
            id: id.into(),
            child: Some(child),
            scope: self.clone(),
            present: Some(present),
            transition: Transition::default(),
        }
    }
}
/// Native wrapper constructed by `PopLayout`.
pub struct PopElement<E> {
    id: ElementId,
    child: Option<E>,
    scope: PopLayout,
    present: Option<bool>,
    transition: Transition,
}
impl<E> PopElement<E> {
    /// Chooses the item's positional reflow transition. Containers do not animate
    /// themselves. Sizes change immediately; popped positions freeze until reentry.
    pub fn transition(mut self, transition: Transition) -> Self {
        self.transition = transition;

        self
    }
}
struct History {
    bounds: Option<Bounds<Pixels>>,
    position: LayoutPositionState,
    epoch: Instant,
}
struct OriginScope {
    origin: Rc<Cell<Point<Pixels>>>,
    previous: Point<Pixels>,
}
impl Drop for OriginScope {
    fn drop(&mut self) {
        self.origin.set(self.previous);
    }
}
/// Per-frame layout data for a popped element. Native implementations should use
/// `PopLayout::item`; this type only appears through the `Element` trait.
pub struct PopRequest {
    child: AnyElement,
    bounds: Option<Bounds<Pixels>>,
}
impl<E: IntoElement + 'static> IntoElement for PopElement<E> {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl<E: IntoElement + 'static> Element for PopElement<E> {
    type RequestLayoutState = PopRequest;
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
    ) -> (LayoutId, PopRequest) {
        let mut child = self.child.take().expect("one layout").into_any_element();
        let now = cx.background_executor().now();

        let old_bounds =
            w.with_element_state(id.expect("pop ID"), |history: Option<History>, _| {
                let h = history.unwrap_or_else(|| History {
                    bounds: None,
                    position: LayoutPositionState::default(),
                    epoch: now,
                });

                (h.bounds, h)
            });

        let layout = if self.present == Some(false) {
            // A detached child avoids flex gaps and containing-block border drift.
            let style = Style {
                display: Display::None,
                ..Default::default()
            };

            w.request_layout(style, [], cx)
        } else {
            child.request_layout(w, cx)
        };

        (
            layout,
            PopRequest {
                child,
                bounds: old_bounds,
            },
        )
    }
    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request: &mut PopRequest,
        w: &mut Window,
        cx: &mut App,
    ) {
        if self.present.is_none() {
            let _scope = OriginScope {
                previous: self.scope.origin.replace(bounds.origin),
                origin: self.scope.origin.clone(),
            };

            request.child.prepaint(w, cx);

            return;
        }

        let now = cx.background_executor().now();

        if self.present == Some(false) {
            if let Some(old) = request.bounds {
                // The detached placeholder has no meaningful origin. Freeze the
                // retained presentation instead, so reentry never follows it.
                w.with_element_state(id.expect("pop ID"), |history: Option<History>, _| {
                    let mut h = history.expect("history initialized during layout");

                    h.position.sample(
                        [f64::from(old.origin.x), f64::from(old.origin.y)],
                        Transition::Instant,
                        now.saturating_duration_since(h.epoch),
                        true,
                    );

                    ((), h)
                });
                request.child.prepaint_as_root(
                    self.scope.origin.get() + old.origin,
                    size(
                        AvailableSpace::Definite(old.size.width),
                        AvailableSpace::Definite(old.size.height),
                    ),
                    w,
                    cx,
                );
            }
        } else {
            let origin = bounds.origin - self.scope.origin.get();

            let frame = w.with_element_state(id.expect("pop ID"), |history: Option<History>, w| {
                let mut h = history.expect("history initialized during layout");

                let frame = h.position.sample(
                    [f64::from(origin.x), f64::from(origin.y)],
                    self.transition,
                    now.saturating_duration_since(h.epoch),
                    cx.reduce_motion(),
                );

                let [x, y] = frame
                    .position()
                    .map(|v| v.clamp(f32::MIN as f64, f32::MAX as f64) as f32);

                h.bounds = Some(Bounds {
                    origin: point(px(x), px(y)),
                    size: bounds.size,
                });

                if frame.is_animating() {
                    w.request_animation_frame();
                }

                (frame, h)
            });

            let [x, y] = frame
                .offset()
                .map(|v| v.clamp(f32::MIN as f64, f32::MAX as f64) as f32);

            w.with_element_offset(point(px(x), px(y)), |w| request.child.prepaint(w, cx));
        }
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        request: &mut PopRequest,
        _: &mut (),
        w: &mut Window,
        cx: &mut App,
    ) {
        if self.present != Some(false) || request.bounds.is_some() {
            request.child.paint(w, cx);
        }
    }
}
