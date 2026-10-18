//! Kit-native position-only layout animation using the portable motion engine.
use gpui_ce_motion::{LayoutPositionState, Transition};

use gpui_kit::{
    AnyElement, App, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window, point, px,
};

use std::{cell::Cell, rc::Rc, time::Instant};

/// A view-owned coordinate scope for automatic position-only layout animation.
/// Clone shares the ancestor scope; retain one group across renders and use that
/// same group for nested animated elements. Independent groups should not nest.
/// Element history is owned by Kit's keyed element state and lasts while mounted.
#[derive(Clone, Debug, Default)]
pub struct LayoutGroup {
    ancestor: Rc<Cell<Option<[f64; 2]>>>,
}

impl LayoutGroup {
    /// Wraps an element without adding a layout node or changing its styles.
    /// Keep the ID stable per logical object, unique under the enclosing Kit ID.
    /// Build styles/handlers on `element` before wrapping it. New mounts snap.
    ///
    /// Measurements exclude scrolling/manual prepaint offsets. Nested members use
    /// positions relative to their closest animated ancestor, preventing double
    /// animation. Sizes change immediately. Normal ancestor clipping still applies.
    ///
    /// ```
    /// use gpui_kit::{div, prelude::*, px};
    /// use gpui_motion_kit::LayoutGroup;
    /// // A persistent view field, initialized once:
    /// let group = LayoutGroup::default();
    /// let item = group.position("card-position", div().id("card").w(px(140.0)).child("Card"));
    /// ```
    pub fn position<E: IntoElement + 'static>(
        &self,
        id: impl Into<ElementId>,
        element: E,
    ) -> LayoutPositionElement<E> {
        LayoutPositionElement {
            id: id.into(),
            element: Some(element),
            group: self.clone(),
            transition: Transition::default(),
            enabled: true,
        }
    }
}

/// Transparent native wrapper produced by [`LayoutGroup::position`].
/// Its child's paint, hitboxes and accessibility bounds receive the same prepaint
/// translation. No scale/rotation, size projection, or cross-parent shared IDs.
pub struct LayoutPositionElement<E> {
    id: ElementId,
    element: Option<E>,
    group: LayoutGroup,
    transition: Transition,
    enabled: bool,
}

impl<E> LayoutPositionElement<E> {
    /// Chooses the positional spring/tween/instant transition.
    pub fn transition(mut self, transition: Transition) -> Self {
        self.transition = transition;

        self
    }

    /// Disables interpolation without changing IDs or coordinate scopes. Disabled
    /// elements follow layout immediately and do not schedule animation frames.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;

        self
    }
}

struct RetainedPosition {
    state: LayoutPositionState,
    epoch: Instant,
}

struct AncestorScope {
    ancestor: Rc<Cell<Option<[f64; 2]>>>,
    previous: Option<[f64; 2]>,
}

impl Drop for AncestorScope {
    fn drop(&mut self) {
        self.ancestor.set(self.previous);
    }
}

impl<E: IntoElement + 'static> IntoElement for LayoutPositionElement<E> {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl<E: IntoElement + 'static> Element for LayoutPositionElement<E> {
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
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        let mut child = self
            .element
            .take()
            .expect("layout requested once")
            .into_any_element();

        let layout = child.request_layout(window, cx);

        (layout, child)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: gpui_kit::Bounds<Pixels>,
        child: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Kit has already added the snapped ancestor/scroll offset to these bounds.
        let raw = bounds.origin - window.pixel_snap_point(window.element_offset());
        let origin = [f64::from(raw.x), f64::from(raw.y)];
        let parent = self.group.ancestor.get().unwrap_or([0.; 2]);
        let relative = std::array::from_fn(|i| origin[i] - parent[i]);
        let now = cx.background_executor().now();
        let reduced = cx.reduce_motion() || !self.enabled;

        let frame = window.with_element_state(
            id.expect("position wrapper has an ID"),
            |state: Option<RetainedPosition>, window| {
                let mut retained = state.unwrap_or_else(|| RetainedPosition {
                    state: LayoutPositionState::default(),
                    epoch: now,
                });

                let frame = retained.state.sample(
                    relative,
                    self.transition,
                    now.saturating_duration_since(retained.epoch),
                    reduced,
                );

                if frame.is_animating() {
                    window.request_animation_frame();
                }

                (frame, retained)
            },
        );

        let ancestor = self.group.ancestor.clone();

        let _scope = AncestorScope {
            previous: ancestor.replace(Some(origin)),
            ancestor,
        };

        let [x, y] = frame
            .offset()
            .map(|v| v.clamp(f64::from(f32::MIN), f64::from(f32::MAX)) as f32);

        window.with_element_offset(point(px(x), px(y)), |window| {
            child.prepaint(window, cx);
        });
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: gpui_kit::Bounds<Pixels>,
        child: &mut AnyElement,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        child.paint(window, cx);
    }
}
