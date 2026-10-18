//! A native single-axis reorder group with measured item geometry.
use crate::{AnimationError, Axis, MotionValue, Transition, move_item};
use gpui::{
    AnyElement, App, Bounds, DispatchPhase, Div, Element, ElementId, FocusHandle, GlobalElementId,
    Hitbox, HitboxBehavior, InspectorElementId, IntoElement, KeyDownEvent, LayoutId, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Stateful, Window, point, prelude::*, px,
};
// notify is a trait method in the official pin, inherent in registry CE 0.2.2.
#[allow(unused_imports)]
use gpui::AppContext;
use std::{
    cell::{Ref, RefCell},
    collections::HashMap,
    hash::Hash,
    rc::Rc,
    time::Instant,
};
struct Metrics {
    bounds: Bounds<Pixels>,
    values: [MotionValue; 2],
    focus: FocusHandle,
}
struct Grab<K> {
    key: K,
    offset: f64,
    pointer: f64,
    original: Vec<K>,
}
type Change<K> = Rc<dyn Fn(&[K], &mut Window, &mut App)>;
struct Data<K> {
    order: Vec<K>,
    metrics: HashMap<K, Metrics>,
    grab: Option<Grab<K>>,
    axis: Axis,
    epoch: Option<Instant>,
    change: Option<Change<K>>,
}
/// Retain in the view. Owns order, stable-key geometry and pointer gesture lifetime.
/// Supports one-dimensional lists, variable item sizes, keyboard Alt+Arrow reorder,
/// Escape/deactivation cancellation, and release outside item bounds. Supply native
/// roles/labels/click handlers on each card. Host owns persistence, removal focus
/// restoration and scrolling/autoscroll policy; multi-column sorting is not implied.
#[derive(Clone)]
pub struct ReorderGroup<K>(Rc<RefCell<Data<K>>>);
impl<K: Clone + Eq + Hash + 'static> ReorderGroup<K> {
    /// Creates a group, rejecting duplicate keys transactionally.
    ///
    /// ```
    /// use gpui::{div, prelude::*};
    /// use gpui_ce_motion::{Axis, ReorderGroup};
    /// let group = ReorderGroup::new(vec![1usize, 2, 3], Axis::Y)?; // persistent view field
    /// let mut list = div().flex().flex_col();
    /// for &key in group.order().iter() {
    ///     list = list.child(group.item(("row", key), key, div().id(("card", key)).child(key.to_string())));
    /// }
    /// # Ok::<(), gpui_ce_motion::AnimationError>(())
    /// ```
    pub fn new(order: Vec<K>, axis: Axis) -> Result<Self, AnimationError> {
        if order
            .iter()
            .enumerate()
            .any(|(i, k)| order[..i].contains(k))
        {
            return Err(AnimationError::Index);
        }

        Ok(Self(Rc::new(RefCell::new(Data {
            order,
            metrics: HashMap::new(),
            grab: None,
            axis,
            epoch: None,
            change: None,
        }))))
    }
    /// Current logical order. Drop this borrow before processing input or updating.
    pub fn order(&self) -> Ref<'_, [K]> {
        Ref::map(self.0.borrow(), |v| v.order.as_slice())
    }
    /// Registers a callback after pointer commit or keyboard reorder. The callback
    /// runs without borrowing group state and may update application entities.
    pub fn on_change(self, callback: impl Fn(&[K], &mut Window, &mut App) + 'static) -> Self {
        self.0.borrow_mut().change = Some(Rc::new(callback));

        self
    }
    /// Updates membership/order and cancels an active gesture. Duplicate keys fail
    /// without mutation. Invoke on model changes, not every render. Notify the view.
    pub fn set_order(&self, order: Vec<K>) -> Result<(), AnimationError> {
        if order
            .iter()
            .enumerate()
            .any(|(i, k)| order[..i].contains(k))
        {
            return Err(AnimationError::Index);
        }

        let mut data = self.0.borrow_mut();

        data.grab = None;
        data.metrics.retain(|k, _| order.contains(k));
        data.order = order;

        Ok(())
    }
    /// Wraps a styled item in measured position interpolation and native handlers.
    /// The key must belong to this group. Preserve the same CE ID through reorder.
    pub fn item(
        &self,
        id: impl Into<ElementId>,
        key: K,
        child: Stateful<Div>,
    ) -> ReorderElement<K> {
        assert!(self.0.borrow().order.contains(&key), "unknown reorder key");

        ReorderElement {
            group: self.clone(),
            id: id.into(),
            key,
            child: Some(child),
            transition: Transition::default(),
        }
    }
    /// Whether a pointer currently owns a reorder gesture.
    pub fn is_dragging(&self) -> bool {
        self.0.borrow().grab.is_some()
    }
}
/// Native measured reorder item.
pub struct ReorderElement<K> {
    group: ReorderGroup<K>,
    id: ElementId,
    key: K,
    child: Option<Stateful<Div>>,
    transition: Transition,
}
impl<K> ReorderElement<K> {
    /// Selects sibling/release position timing.
    pub fn transition(mut self, transition: Transition) -> Self {
        self.transition = transition;

        self
    }
}
fn coordinate(bounds: Bounds<Pixels>, axis: Axis) -> (f64, f64) {
    match axis {
        Axis::X => (f64::from(bounds.origin.x), f64::from(bounds.size.width)),
        Axis::Y => (f64::from(bounds.origin.y), f64::from(bounds.size.height)),
    }
}
fn pointer(position: gpui::Point<Pixels>, axis: Axis) -> f64 {
    match axis {
        Axis::X => f64::from(position.x),
        Axis::Y => f64::from(position.y),
    }
}

impl<K: Clone + Eq + Hash + 'static> IntoElement for ReorderElement<K> {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl<K: Clone + Eq + Hash + 'static> Element for ReorderElement<K> {
    type RequestLayoutState = AnyElement;
    type PrepaintState = Hitbox;
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
        w: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        let mut data = self.group.0.borrow_mut();

        data.epoch
            .get_or_insert_with(|| cx.background_executor().now());

        if (!w.is_window_active()
            || data.grab.as_ref().is_some_and(|g| {
                data.metrics
                    .get(&g.key)
                    .is_some_and(|m| !m.focus.is_focused(w))
            }))
            && let Some(grab) = data.grab.take()
        {
            data.order = grab.original;
            w.request_animation_frame();
        }

        let metric = data
            .metrics
            .entry(self.key.clone())
            .or_insert_with(|| Metrics {
                bounds: Bounds::default(),
                values: [MotionValue::new(0.), MotionValue::new(0.)],
                focus: cx.focus_handle().tab_stop(true),
            });

        let focus = metric.focus.clone();

        drop(data);
        let mut child = self
            .child
            .take()
            .expect("one layout")
            .track_focus(&focus)
            .tab_index(0)
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
    ) -> Hitbox {
        let mut data = self.group.0.borrow_mut();

        let now = cx
            .background_executor()
            .now()
            .saturating_duration_since(data.epoch.expect("epoch"));

        let raw = bounds.origin - w.pixel_snap_point(w.element_offset());
        let target = [f64::from(raw.x), f64::from(raw.y)];
        let axis = data.axis;

        let drag = data
            .grab
            .as_ref()
            .filter(|g| g.key == self.key)
            .map(|g| g.pointer - g.offset);

        let metric = data.metrics.get_mut(&self.key).expect("layout first");
        let first = metric.bounds.size.width == px(0.) && metric.bounds.size.height == px(0.);

        metric.bounds = bounds;
        let mut presented = target;
        let mut active = false;

        for i in 0..2 {
            if first {
                metric.values[i].jump(target[i], now);
            }

            if let Some(position) = drag
                && i == match axis {
                    Axis::X => 0,
                    Axis::Y => 1,
                }
            {
                let offset = if i == 0 {
                    f64::from(w.element_offset().x)
                } else {
                    f64::from(w.element_offset().y)
                };

                metric.values[i].set(position - offset, now);
            } else {
                metric.values[i].animate(target[i], self.transition, now);
            }

            let frame = metric.values[i].sample(now, cx.reduce_motion());

            presented[i] = frame.value();
            active |= frame.is_animating();
        }

        if active {
            w.request_animation_frame();
        }

        drop(data);
        let offset = point(
            px((presented[0] - target[0]) as f32),
            px((presented[1] - target[1]) as f32),
        );

        let displayed = Bounds {
            origin: bounds.origin + offset,
            size: bounds.size,
        };

        let hitbox = w.insert_hitbox(displayed, HitboxBehavior::Normal);

        w.with_element_offset(offset, |w| {
            child.prepaint(w, cx);
        });

        hitbox
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        child: &mut AnyElement,
        hitbox: &mut Hitbox,
        w: &mut Window,
        cx: &mut App,
    ) {
        let owner = w.current_view();
        let group = self.group.clone();
        let key = self.key.clone();
        let region = hitbox.clone();

        w.on_mouse_event(move |event: &MouseDownEvent, phase, w, cx| {
            if phase == DispatchPhase::Bubble
                && event.button == MouseButton::Left
                && region.is_hovered(w)
            {
                let mut data = group.0.borrow_mut();

                if data.grab.is_some() {
                    return;
                }

                let p = pointer(event.position, data.axis);
                let original = data.order.clone();

                data.grab = Some(Grab {
                    key: key.clone(),
                    offset: p - coordinate(region.bounds, data.axis).0,
                    pointer: p,
                    original,
                });

                if let Some(metric) = data.metrics.get(&key) {
                    metric.focus.focus(w, cx);
                }

                cx.notify(owner);
                cx.stop_propagation();
            }
        });
        let group = self.group.clone();
        let key = self.key.clone();

        w.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
            if phase != DispatchPhase::Capture {
                return;
            }

            let mut data = group.0.borrow_mut();

            if !data.grab.as_ref().is_some_and(|g| g.key == key) {
                return;
            }

            if event.pressed_button != Some(MouseButton::Left) {
                let original = data.grab.take().expect("grab").original;

                data.order = original;
                cx.notify(owner);

                return;
            }

            let p = pointer(event.position, data.axis);
            let axis = data.axis;
            let grab = data.grab.as_mut().expect("grab");

            grab.pointer = p;
            let start = p - grab.offset;

            let length = data
                .metrics
                .get(&key)
                .map_or(0., |m| coordinate(m.bounds, axis).1);

            let center = start + length * 0.5;

            let destination = data
                .order
                .iter()
                .enumerate()
                .filter_map(|(i, k)| {
                    data.metrics.get(k).map(|m| {
                        let (start, length) = coordinate(m.bounds, axis);

                        (i, (start + length * 0.5 - center).abs())
                    })
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|v| v.0);

            if let Some(to) = destination {
                let from = data.order.iter().position(|k| *k == key).expect("key");

                move_item(&mut data.order, from, to).expect("indices");
            }

            cx.notify(owner);
        });
        let group = self.group.clone();
        let key = self.key.clone();

        w.on_mouse_event(move |event: &MouseUpEvent, phase, w, cx| {
            if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                let mut data = group.0.borrow_mut();

                if data.grab.as_ref().is_some_and(|g| g.key == key) {
                    let changed = data.grab.take().expect("grab").original != data.order;

                    let notification = if changed {
                        data.change
                            .clone()
                            .map(|change| (change, data.order.clone()))
                    } else {
                        None
                    };

                    drop(data);

                    if let Some((change, order)) = notification {
                        change(&order, w, cx);
                    }

                    cx.notify(owner);
                }
            }
        });
        let group = self.group.clone();
        let key = self.key.clone();

        w.on_key_event(move |event: &KeyDownEvent, phase, w, cx| {
            if phase != DispatchPhase::Capture {
                return;
            }

            let mut data = group.0.borrow_mut();

            if !data
                .metrics
                .get(&key)
                .is_some_and(|m| m.focus.is_focused(w))
            {
                return;
            }

            if event.keystroke.key == "escape" {
                if let Some(grab) = data.grab.take() {
                    data.order = grab.original;
                    cx.notify(owner);
                }

                return;
            }

            if !event.keystroke.modifiers.alt {
                return;
            }

            let delta = match (data.axis, event.keystroke.key.as_str()) {
                (Axis::Y, "up") | (Axis::X, "left") => -1,
                (Axis::Y, "down") | (Axis::X, "right") => 1,
                _ => return,
            };

            let from = data.order.iter().position(|k| *k == key).expect("key");
            let to = from.saturating_add_signed(delta).min(data.order.len() - 1);

            if from != to {
                move_item(&mut data.order, from, to).expect("indices");
                let notification = data
                    .change
                    .clone()
                    .map(|change| (change, data.order.clone()));

                drop(data);

                if let Some((change, order)) = notification {
                    change(&order, w, cx);
                }

                cx.notify(owner);
                cx.stop_propagation();
            }
        });
        // Register key listeners on this wrapper before child paint selects a descendant node.
        child.paint(w, cx);
    }
}
