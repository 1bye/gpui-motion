//! Measured spatial reorder, dedicated handles and elapsed-time edge autoscroll.
use crate::{
    AnimationError, AutoScroll, LayoutRect, MotionValue, ReorderAxis, Transition, move_item,
};

use crate::{Axis, MotionAnchor};

use gpui::{
    AnyElement, App, Bounds, DispatchPhase, Div, Element, ElementId, EntityId, FocusHandle,
    GlobalElementId, Hitbox, HitboxBehavior, InspectorElementId, IntoElement, KeyDownEvent,
    LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, ScrollHandle,
    Stateful, Window, point, prelude::*, px,
};

use std::{
    cell::{Ref, RefCell},
    collections::HashMap,
    hash::Hash,
    rc::Rc,
    time::{Duration, Instant},
};

struct Metrics {
    bounds: Bounds<Pixels>,
    displayed: Bounds<Pixels>,
    values: [MotionValue; 2],
    focus: FocusHandle,
    measured: bool,
    // Logical slot represented by this geometry, independent of newer input edits.
    slot: usize,
}

struct Grab<K> {
    key: K,
    offset: [f64; 2],
    pointer: [f64; 2],
    original: Vec<K>,
    axis: ReorderAxis,
}

struct EdgeScroll {
    handle: ScrollHandle,
    viewport: MotionAnchor,
    policy: AutoScroll,
    last: Option<Instant>,
}

type Change<K> = Rc<dyn Fn(&[K], &mut Window, &mut App)>;

struct Data<K> {
    order: Vec<K>,
    metrics: HashMap<K, Metrics>,
    grab: Option<Grab<K>>,
    mode: ReorderAxis,
    epoch: Option<Instant>,
    change: Option<Change<K>>,
    rects: Vec<LayoutRect>,
    indices: Vec<usize>,
    scroll: Option<EdgeScroll>,
    scroll_pending: bool,
    owner: Option<EntityId>,
    touch: bool,
    layout_ready: bool,
}

fn rect(bounds: Bounds<Pixels>, offset: [f64; 2]) -> LayoutRect {
    LayoutRect::new(
        [
            f64::from(bounds.origin.x) + offset[0],
            f64::from(bounds.origin.y) + offset[1],
        ],
        [f64::from(bounds.size.width), f64::from(bounds.size.height)],
    )
    .expect("finite native bounds")
}

impl<K: Clone + Eq + Hash> Data<K> {
    fn refresh(&mut self, offset: [f64; 2]) {
        self.rects.clear();
        self.indices.clear();

        for key in &self.order {
            if let Some(metric) = self.metrics.get(key).filter(|metric| metric.measured) {
                self.rects.push(rect(metric.bounds, offset));
                self.indices.push(metric.slot);
            }
        }
    }

    fn begin(&mut self, key: &K, pointer: [f64; 2], window: &mut Window, cx: &mut App) -> bool {
        if self.grab.is_some() || !self.layout_ready || !window.is_window_active() {
            return false;
        }

        let Some(metric) = self.metrics.get(key).filter(|metric| metric.measured) else {
            return false;
        };

        let origin = rect(metric.displayed, [0.0; 2]).origin();
        let focus = metric.focus.clone();

        self.refresh([0.0; 2]);
        self.grab = Some(Grab {
            key: key.clone(),
            pointer,
            offset: std::array::from_fn(|i| pointer[i] - origin[i]),
            original: self.order.clone(),
            axis: self.mode.resolve(&self.rects),
        });
        self.touch = false;

        if let Some(scroll) = &mut self.scroll {
            scroll.last = Some(cx.background_executor().now());
        }

        focus.focus(window, cx);

        true
    }

    fn cancel(&mut self) {
        if let Some(grab) = self.grab.take() {
            self.order = grab.original;
        }

        self.touch = false;

        if let Some(scroll) = &mut self.scroll {
            scroll.last = None;
        }
    }

    fn reorder(&mut self, scroll_delta: [f64; 2]) -> bool {
        let Some(grab) = &self.grab else {
            return false;
        };

        let from = self
            .order
            .iter()
            .position(|key| *key == grab.key)
            .expect("retained key");

        let axis = grab.axis;

        let Some(metric) = self.metrics.get(&grab.key) else {
            return false;
        };

        let size = rect(metric.bounds, [0.0; 2]).size();
        let center = std::array::from_fn(|i| grab.pointer[i] - grab.offset[i] + size[i] * 0.5);

        self.refresh(scroll_delta);

        let Some(destination) = axis
            .destination(center, &self.rects)
            .map(|index| self.indices[index])
        else {
            return false;
        };

        if from == destination {
            return false;
        }

        move_item(&mut self.order, from, destination).expect("measured indices");

        true
    }

    fn keyboard(&mut self, key: &K, direction: &str) -> Option<(usize, usize)> {
        if !self.layout_ready {
            return None;
        }

        self.refresh([0.0; 2]);
        let mode = self.mode.resolve(&self.rects);

        let (axis, sign) = match direction {
            "left" => (0, -1.0),
            "right" => (0, 1.0),
            "up" => (1, -1.0),
            "down" => (1, 1.0),
            _ => return None,
        };

        if (mode == ReorderAxis::X && axis == 1) || (mode == ReorderAxis::Y && axis == 0) {
            return None;
        }

        let from = self.order.iter().position(|old| old == key)?;
        let origin_slot = self.indices.iter().position(|slot| *slot == from)?;
        let origin = self.rects[origin_slot].center();

        let destination = self
            .rects
            .iter()
            .enumerate()
            .filter_map(|(i, rect)| {
                let center = rect.center();
                let forward = (center[axis] - origin[axis]) * sign;

                (forward > 0.01).then_some((
                    self.indices[i],
                    forward + (center[1 - axis] - origin[1 - axis]).abs() * 2.0,
                ))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|value| value.0)?;

        Some((from, destination))
    }

    fn scroll_velocity(&self) -> [f64; 2] {
        let Some(grab) = &self.grab else {
            return [0.0; 2];
        };

        let Some(scroll) = &self.scroll else {
            return [0.0; 2];
        };

        let Some(viewport) = scroll.viewport.rect() else {
            return [0.0; 2];
        };

        let offset = scroll.handle.offset();
        let max = scroll.handle.max_offset();

        std::array::from_fn(|i| {
            if (grab.axis == ReorderAxis::X && i == 1) || (grab.axis == ReorderAxis::Y && i == 0) {
                return 0.0;
            }

            let velocity =
                scroll
                    .policy
                    .velocity(grab.pointer[i], viewport.origin()[i], viewport.size()[i]);

            let current = if i == 0 {
                -f64::from(offset.x)
            } else {
                -f64::from(offset.y)
            };

            let extent = if i == 0 {
                f64::from(max.x).abs()
            } else {
                f64::from(max.y).abs()
            };

            if (velocity < 0.0 && current > 0.01) || (velocity > 0.0 && current < extent - 0.01) {
                velocity
            } else {
                0.0
            }
        })
    }
}

/// Retained logical order and measured spatial geometry. Mouse/keyboard input,
/// cancellation and optional edge autoscroll share the portable engine. Automatic
/// axis detection occurs on acquisition. Host owns model persistence and focus
/// restoration after removal. Cross-list transfer and virtualization are separate
/// application policies; keep this group's mounted geometry in one scroll source.
#[derive(Clone)]
pub struct ReorderGroup<K>(Rc<RefCell<Data<K>>>);

impl<K: Clone + Eq + Hash + 'static> ReorderGroup<K> {
    /// Creates a compatible horizontal/vertical group; duplicate keys fail without mutation.
    pub fn new(order: Vec<K>, axis: Axis) -> Result<Self, AnimationError> {
        Self::spatial(
            order,
            match axis {
                Axis::X => ReorderAxis::X,
                Axis::Y => ReorderAxis::Y,
            },
        )
    }

    /// Creates an automatically detected row/column/grid or explicit spatial mode.
    pub fn spatial(order: Vec<K>, mode: ReorderAxis) -> Result<Self, AnimationError> {
        if order
            .iter()
            .enumerate()
            .any(|(i, key)| order[..i].contains(key))
        {
            return Err(AnimationError::Index);
        }

        let capacity = order.len();

        Ok(Self(Rc::new(RefCell::new(Data {
            order,
            metrics: HashMap::new(),
            grab: None,
            mode,
            epoch: None,
            change: None,
            rects: Vec::with_capacity(capacity),
            indices: Vec::with_capacity(capacity),
            scroll: None,
            scroll_pending: false,
            owner: None,
            touch: false,
            layout_ready: false,
        }))))
    }

    /// Enables elapsed-time edge scrolling in a measured native viewport. Pointer
    /// coordinates remain fixed while scrolled geometry participates in reorder.
    /// After a stalled frame, one tick is capped to 50ms to avoid a scroll jump.
    pub fn autoscroll(
        self,
        handle: ScrollHandle,
        viewport: MotionAnchor,
        policy: AutoScroll,
    ) -> Self {
        self.0.borrow_mut().scroll = Some(EdgeScroll {
            handle,
            viewport,
            policy,
            last: None,
        });

        self
    }

    /// Current order. Release the borrow before sending input or changing membership.
    pub fn order(&self) -> Ref<'_, [K]> {
        Ref::map(self.0.borrow(), |data| data.order.as_slice())
    }

    /// Invokes after pointer commit or keyboard reorder, outside all group borrows.
    /// Capture application entities weakly to avoid a retained self-reference cycle.
    pub fn on_change(self, callback: impl Fn(&[K], &mut Window, &mut App) + 'static) -> Self {
        self.0.borrow_mut().change = Some(Rc::new(callback));

        self
    }

    /// Changes external membership/order and cancels a held gesture. Duplicate keys
    /// fail transactionally. Use on model changes, not on every render.
    pub fn set_order(&self, order: Vec<K>) -> Result<(), AnimationError> {
        if order
            .iter()
            .enumerate()
            .any(|(i, key)| order[..i].contains(key))
        {
            return Err(AnimationError::Index);
        }

        let mut data = self.0.borrow_mut();

        if data.grab.is_none() && data.order == order {
            return Ok(());
        }

        data.cancel();
        data.layout_ready = false;
        data.metrics.retain(|key, _| order.contains(key));
        let extra_rects = order.len().saturating_sub(data.rects.len());
        let extra_indices = order.len().saturating_sub(data.indices.len());

        data.rects.reserve(extra_rects);
        data.indices.reserve(extra_indices);
        data.order = order;

        Ok(())
    }

    /// Starts dragging from an external handle after the item was measured.
    pub fn start_drag(
        &self,
        key: &K,
        pointer: [f64; 2],
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        assert!(
            pointer.iter().all(|v| v.is_finite()),
            "invalid reorder pointer"
        );
        let mut data = self.0.borrow_mut();
        let started = data.begin(key, pointer, window, cx);

        if started && let Some(owner) = data.owner {
            cx.notify(owner);
        }

        started
    }

    /// Rolls back the current pointer gesture without firing a commit callback.
    pub fn cancel(&self, cx: &mut App) {
        let mut data = self.0.borrow_mut();

        data.cancel();

        if let Some(owner) = data.owner {
            cx.notify(owner);
        }
    }

    /// Whether a pointer currently owns the group.
    pub fn is_dragging(&self) -> bool {
        self.0.borrow().grab.is_some()
    }

    /// Wraps one stable-key item. Child semantics, styles and normal click handlers
    /// remain native. Use `drag_listener(false)` when providing a separate handle.
    pub fn item<E: IntoElement + StatefulInteractiveElement + 'static>(
        &self,
        id: impl Into<ElementId>,
        key: K,
        child: E,
    ) -> ReorderElement<K, E> {
        let slot = self
            .0
            .borrow()
            .order
            .iter()
            .position(|old| old == &key)
            .expect("unknown reorder key");

        ReorderElement {
            group: self.clone(),
            id: id.into(),
            key,
            slot,
            child: Some(child),
            transition: Transition::default(),
            listener: true,
        }
    }

    fn finish(&self, key: &K, window: &mut Window, cx: &mut App) {
        let mut data = self.0.borrow_mut();

        if !data.grab.as_ref().is_some_and(|grab| grab.key == *key) {
            return;
        }

        let changed = data.grab.take().expect("owned grab").original != data.order;

        data.touch = false;

        if let Some(scroll) = &mut data.scroll {
            scroll.last = None;
        }

        let callback = if changed {
            data.change
                .clone()
                .map(|callback| (callback, data.order.clone()))
        } else {
            None
        };

        let owner = data.owner;

        drop(data);

        if let Some((callback, order)) = callback {
            callback(&order, window, cx);
        }

        if let Some(owner) = owner {
            cx.notify(owner);
        }
    }
}

/// Native spatial reorder item.
pub struct ReorderElement<K, E = Stateful<Div>> {
    group: ReorderGroup<K>,
    id: ElementId,
    key: K,
    slot: usize,
    child: Option<E>,
    transition: Transition,
    listener: bool,
}

impl<K, E> ReorderElement<K, E> {
    /// Changes sibling/release interpolation without resetting measured identity.
    pub fn transition(mut self, transition: Transition) -> Self {
        self.transition = transition;

        self
    }

    /// Uses external drag acquisition while preserving keyboard reorder controls.
    pub fn drag_listener(mut self, enabled: bool) -> Self {
        self.listener = enabled;

        self
    }
}

impl<K: Clone + Eq + Hash + 'static, E: IntoElement + StatefulInteractiveElement + 'static>
    IntoElement for ReorderElement<K, E>
{
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl<K: Clone + Eq + Hash + 'static, E: IntoElement + StatefulInteractiveElement + 'static> Element
    for ReorderElement<K, E>
{
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
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        let mut data = self.group.0.borrow_mut();

        data.owner = Some(window.current_view());
        data.epoch
            .get_or_insert_with(|| cx.background_executor().now());

        if !window.is_window_active()
            || data.grab.as_ref().is_some_and(|grab| {
                data.metrics
                    .get(&grab.key)
                    .is_some_and(|metric| !metric.focus.is_focused(window))
            })
        {
            let was_dragging = data.grab.is_some();

            data.cancel();

            if was_dragging {
                window.request_animation_frame();
            }
        }

        let metric = data
            .metrics
            .entry(self.key.clone())
            .or_insert_with(|| Metrics {
                bounds: Bounds::default(),
                displayed: Bounds::default(),
                values: [MotionValue::new(0.0), MotionValue::new(0.0)],
                focus: cx.focus_handle().tab_stop(true),
                measured: false,
                slot: 0,
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

        let layout = child.request_layout(window, cx);

        (layout, child)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) -> Hitbox {
        let mut data = self.group.0.borrow_mut();

        data.layout_ready = true;

        let now = cx
            .background_executor()
            .now()
            .saturating_duration_since(data.epoch.expect("layout epoch"));

        let raw = bounds.origin - window.pixel_snap_point(window.element_offset());
        let target = [f64::from(raw.x), f64::from(raw.y)];

        let drag = data
            .grab
            .as_ref()
            .filter(|grab| grab.key == self.key)
            .map(|grab| {
                (
                    std::array::from_fn::<_, 2, _>(|i| grab.pointer[i] - grab.offset[i]),
                    grab.axis,
                )
            });

        let metric = data.metrics.get_mut(&self.key).expect("measured key");
        let first = !metric.measured;

        metric.measured = true;
        metric.slot = self.slot;
        metric.bounds = bounds;
        let mut presented = target;

        for i in 0..2 {
            if first {
                metric.values[i].jump(target[i], now);
            }

            if let Some((position, axis)) = drag
                && (axis == ReorderAxis::XY
                    || (axis == ReorderAxis::X && i == 0)
                    || (axis == ReorderAxis::Y && i == 1))
            {
                let offset = if i == 0 {
                    f64::from(window.element_offset().x)
                } else {
                    f64::from(window.element_offset().y)
                };

                metric.values[i].set(position[i] - offset, now);
            } else {
                metric.values[i].animate(target[i], self.transition, now);
            }

            let frame = metric.values[i].sample(now, cx.reduce_motion());

            presented[i] = frame.value();

            if frame.is_animating() {
                window.request_animation_frame();
            }
        }

        let offset = point(
            px((presented[0] - target[0]) as f32),
            px((presented[1] - target[1]) as f32),
        );

        let displayed = Bounds {
            origin: bounds.origin + offset,
            size: bounds.size,
        };

        metric.displayed = displayed;
        drop(data);
        let hitbox = window.insert_hitbox(displayed, HitboxBehavior::Normal);

        window.with_element_offset(offset, |window| {
            child.prepaint(window, cx);
        });
        schedule_scroll(&self.group, &self.key, window, cx);

        hitbox
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        child: &mut AnyElement,
        hitbox: &mut Hitbox,
        window: &mut Window,
        cx: &mut App,
    ) {
        let group = self.group.clone();
        let key = self.key.clone();
        let region = hitbox.clone();
        let listener = self.listener;

        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
            if listener
                && phase == DispatchPhase::Bubble
                && event.button == MouseButton::Left
                && region.is_hovered(window)
                && group.start_drag(
                    &key,
                    [f64::from(event.position.x), f64::from(event.position.y)],
                    window,
                    cx,
                )
            {
                cx.stop_propagation();
            }
        });
        let group = self.group.clone();
        let key = self.key.clone();

        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
            if phase != DispatchPhase::Capture {
                return;
            }

            let mut data = group.0.borrow_mut();

            if data.touch || !data.grab.as_ref().is_some_and(|grab| grab.key == key) {
                return;
            }

            if event.pressed_button != Some(MouseButton::Left) {
                data.cancel();
            } else {
                data.grab.as_mut().expect("owned grab").pointer =
                    [f64::from(event.position.x), f64::from(event.position.y)];
                data.reorder([0.0; 2]);
            }

            if let Some(owner) = data.owner {
                cx.notify(owner);
            }
        });
        let group = self.group.clone();
        let key = self.key.clone();

        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
            if phase == DispatchPhase::Capture
                && event.button == MouseButton::Left
                && !group.0.borrow().touch
            {
                group.finish(&key, window, cx);
            }
        });
        let group = self.group.clone();
        let key = self.key.clone();

        window.on_key_event(move |event: &KeyDownEvent, phase, window, cx| {
            if phase != DispatchPhase::Capture {
                return;
            }

            let mut data = group.0.borrow_mut();

            if !data
                .metrics
                .get(&key)
                .is_some_and(|metric| metric.focus.is_focused(window))
            {
                return;
            }

            if event.keystroke.key == "escape" {
                data.cancel();

                if let Some(owner) = data.owner {
                    cx.notify(owner);
                }

                return;
            }

            if !event.keystroke.modifiers.alt {
                return;
            }

            let Some((from, to)) = data.keyboard(&key, event.keystroke.key.as_str()) else {
                return;
            };

            if from == to {
                return;
            }

            move_item(&mut data.order, from, to).expect("measured order");
            let callback = data
                .change
                .clone()
                .map(|callback| (callback, data.order.clone()));

            let owner = data.owner;

            drop(data);

            if let Some((callback, order)) = callback {
                callback(&order, window, cx);
            }

            if let Some(owner) = owner {
                cx.notify(owner);
            }

            cx.stop_propagation();
        });
        // Input listeners precede child paint so focused descendants retain correct dispatch ownership.
        child.paint(window, cx);
    }
}

fn schedule_scroll<K: Clone + Eq + Hash + 'static>(
    group: &ReorderGroup<K>,
    key: &K,
    window: &mut Window,
    cx: &App,
) {
    let mut data = group.0.borrow_mut();

    if data.scroll_pending
        || !data.grab.as_ref().is_some_and(|grab| grab.key == *key)
        || data.scroll_velocity() == [0.0; 2]
    {
        return;
    }

    data.scroll_pending = true;

    if let Some(scroll) = &mut data.scroll {
        scroll
            .last
            .get_or_insert_with(|| cx.background_executor().now());
    }

    drop(data);
    let weak = Rc::downgrade(&group.0);

    window.on_next_frame(move |window, cx| {
        let Some(group) = weak.upgrade() else {
            return;
        };

        let mut data = group.borrow_mut();

        data.scroll_pending = false;

        if !window.is_window_active()
            || data.grab.as_ref().is_some_and(|grab| {
                data.metrics
                    .get(&grab.key)
                    .is_some_and(|metric| !metric.focus.is_focused(window))
            })
        {
            data.cancel();

            return;
        }

        let velocity = data.scroll_velocity();

        if velocity == [0.0; 2] {
            return;
        }

        let scroll = data.scroll.as_mut().expect("scroll policy");
        let now = cx.background_executor().now();

        let dt = now
            .saturating_duration_since(scroll.last.replace(now).unwrap_or(now))
            .min(Duration::from_millis(50))
            .as_secs_f64();

        let old = scroll.handle.offset();
        let max = scroll.handle.max_offset();

        let next = point(
            px((f64::from(old.x) - velocity[0] * dt).clamp(-f64::from(max.x).abs(), 0.0) as f32),
            px((f64::from(old.y) - velocity[1] * dt).clamp(-f64::from(max.y).abs(), 0.0) as f32),
        );

        scroll.handle.set_offset(next);
        data.reorder([f64::from(next.x - old.x), f64::from(next.y - old.y)]);

        if let Some(owner) = data.owner {
            cx.notify(owner);
        }
    });
}
