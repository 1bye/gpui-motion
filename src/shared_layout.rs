//! Explicit shared identity in a common coordinate space, without size projection.
use crate::{LayoutPositionState, MotionValue, Transition};
use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Styled, Window, point, px,
};
use std::{cell::RefCell, collections::HashMap, hash::Hash, rc::Rc, time::Instant};
struct Entry {
    position: LayoutPositionState,
    destination: [f64; 2],
    member: Option<GlobalElementId>,
    epoch: Instant,
    measured: bool,
}
/// View-owned shared identity registry. Exactly one active destination per key
/// is required. Render incoming/outgoing appearances in a common overlay; retain
/// outgoing content until its normal presence exit completes. Native clipping
/// still applies, sizes change immediately, and independent scrolling coordinate
/// spaces are not reconciled. This is position/crossfade, not full `layoutId`.
/// Clones share storage. Call `remove` when a logical shared identity is finished.
#[derive(Clone)]
pub struct SharedLayout<K>(Rc<RefCell<HashMap<K, Entry>>>);
impl<K> Default for SharedLayout<K> {
    fn default() -> Self {
        Self(Rc::new(RefCell::new(HashMap::new())))
    }
}
impl<K: Eq + Hash + Clone + 'static> SharedLayout<K> {
    /// Forgets a shared identity. Existing native member history ends on unmount.
    pub fn remove(&self, key: &K) {
        self.0.borrow_mut().remove(key);
    }
    /// Number of explicitly retained shared identities.
    pub fn len(&self) -> usize {
        self.0.borrow().len()
    }
    /// Whether no identities are retained.
    pub fn is_empty(&self) -> bool {
        self.0.borrow().is_empty()
    }
    /// Creates a crossfading appearance. Styles and absolute positioning belong
    /// on the child. Inactive retained appearances follow the active position.
    /// The owner must suppress input and restore focus for outgoing appearances,
    /// just as with normal presence exits; opacity does not disable handlers.
    ///
    /// ```
    /// use gpui::{div, prelude::*, px};
    /// use gpui_ce_motion::SharedLayout;
    /// let shared = SharedLayout::<u32>::default(); // persistent view field
    /// let first = shared.member(1, "old", false, div().absolute().left(px(0.)).child("Old"));
    /// let second = shared.member(1, "new", true, div().absolute().left(px(200.)).child("New"));
    /// let overlay = div().relative().child(first).child(second);
    /// ```
    pub fn member<E: IntoElement + Styled + 'static>(
        &self,
        key: K,
        id: impl Into<ElementId>,
        active: bool,
        child: E,
    ) -> SharedElement<K, E> {
        SharedElement {
            group: self.clone(),
            key,
            id: id.into(),
            active,
            child: Some(child),
            transition: Transition::default(),
        }
    }
}
/// Native position/crossfade appearance constructed by `SharedLayout::member`.
pub struct SharedElement<K, E> {
    group: SharedLayout<K>,
    key: K,
    id: ElementId,
    active: bool,
    child: Option<E>,
    transition: Transition,
}
impl<K, E> SharedElement<K, E> {
    /// Chooses interruption-aware positional and crossfade timing.
    pub fn transition(mut self, transition: Transition) -> Self {
        self.transition = transition;

        self
    }
}
struct Fade {
    value: MotionValue,
    epoch: Instant,
}
impl<K: Eq + Hash + Clone + 'static, E: IntoElement + Styled + 'static> IntoElement
    for SharedElement<K, E>
{
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl<K: Eq + Hash + Clone + 'static, E: IntoElement + Styled + 'static> Element
    for SharedElement<K, E>
{
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
        let now = cx.background_executor().now();
        let id = id.expect("shared ID");
        let mut entries = self.group.0.borrow_mut();

        let entry = entries.entry(self.key.clone()).or_insert_with(|| Entry {
            position: LayoutPositionState::default(),
            destination: [0.; 2],
            member: None,
            epoch: now,
            measured: false,
        });

        let initial = if self.active && entry.member.is_none() {
            1.
        } else {
            0.
        };

        if self.active {
            entry.member = Some(id.clone());
        }

        drop(entries);
        let opacity = w.with_element_state(id, |state: Option<Fade>, w| {
            let mut state = state.unwrap_or_else(|| Fade {
                value: MotionValue::new(initial),
                epoch: now,
            });

            let time = now.saturating_duration_since(state.epoch);

            state
                .value
                .animate(if self.active { 1. } else { 0. }, self.transition, time);
            let frame = state.value.sample(time, cx.reduce_motion());

            if frame.is_animating() {
                w.request_animation_frame();
            }

            (frame.value().clamp(0., 1.) as f32, state)
        });

        let mut child = self
            .child
            .take()
            .expect("one layout")
            .opacity(opacity)
            .into_any_element();

        let layout = child.request_layout(w, cx);

        (layout, child)
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut AnyElement,
        w: &mut Window,
        cx: &mut App,
    ) {
        let now = cx.background_executor().now();
        let mut entries = self.group.0.borrow_mut();
        let entry = entries.get_mut(&self.key).expect("layout first");

        if !entry.measured && !self.active {
            drop(entries);
            child.prepaint(w, cx);

            return;
        }

        if self.active {
            entry.measured = true;
            entry.destination = [f64::from(bounds.origin.x), f64::from(bounds.origin.y)];
        }

        let frame = entry.position.sample(
            entry.destination,
            self.transition,
            now.saturating_duration_since(entry.epoch),
            cx.reduce_motion(),
        );

        if frame.is_animating() {
            w.request_animation_frame();
        }

        let presented = frame.position();

        let offset = point(
            px((presented[0] - f64::from(bounds.origin.x))
                .clamp(f64::from(f32::MIN), f64::from(f32::MAX)) as f32),
            px((presented[1] - f64::from(bounds.origin.y))
                .clamp(f64::from(f32::MIN), f64::from(f32::MAX)) as f32),
        );

        drop(entries);
        w.with_element_offset(offset, |w| {
            child.prepaint(w, cx);
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
