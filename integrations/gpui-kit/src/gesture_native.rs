//! Native input adapter with a persistent owner and frame-scoped listeners.
use crate::core::{
    Drag, DragBounds, DragEvent, GesturePhase, GestureTargets, InView, Interaction, LayoutRect,
    Motion, PinchGesture, Target,
};

use crate::{MotionAnchor, MotionState};

use gpui_kit::{
    AnyElement, App, Bounds, DispatchPhase, Div, Element, ElementId, EntityId, FocusHandle,
    GlobalElementId, Hitbox, HitboxBehavior, InspectorElementId, IntoElement, KeyDownEvent,
    KeyUpEvent, LayoutId, MouseButton, MouseDownEvent, MouseExitEvent, MouseMoveEvent,
    MouseUpEvent, PinchEvent, Pixels, Stateful, TouchPhase, Window, prelude::*, px,
};

use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

type DragCallback = Rc<dyn Fn(DragEvent, &mut Window, &mut App)>;
type PinchCallback = Rc<dyn Fn(PinchGesture, &mut Window, &mut App)>;

#[derive(Default)]
struct Data {
    interaction: Interaction,
    motion: MotionState,
    drag: Option<Drag>,
    focus: Option<FocusHandle>,
    visibility: Option<InView>,
    epoch: Option<Instant>,
    owner: Option<EntityId>,
    pointer: bool,
    key: Option<String>,
    presented: Target,
    constraints: Option<MotionAnchor>,
    view_root: Option<MotionAnchor>,
    view_margins: [f64; 4],
    enabled: bool,
    touch: bool,
    touch_enabled: bool,
    pointer_origin: [f64; 2],
    last_pointer: [f64; 2],
    last_event: Duration,
    drag_callback: Option<DragCallback>,
    pending_drag: Option<DragEvent>,
    pinch_callback: Option<PinchCallback>,
    pinch_active: bool,
    pinch_scale: f64,
    pinch_center: [f64; 2],
    pending_pinch: Option<PinchGesture>,
}

impl Data {
    fn now(&self, cx: &App) -> Duration {
        self.epoch.map_or(Duration::ZERO, |epoch| {
            cx.background_executor()
                .now()
                .saturating_duration_since(epoch)
        })
    }

    fn cancel(&mut self, cx: &App) {
        let now = self.now(cx);

        if self.pointer {
            self.record(GesturePhase::Cancelled, self.last_pointer, now);
        }

        if self.pinch_active {
            self.pending_pinch = Some(PinchGesture {
                phase: GesturePhase::Cancelled,
                center: self.pinch_center,
                scale: self.pinch_scale,
                delta: 0.0,
            });
            self.pinch_active = false;
        }

        self.touch = false;

        if let Some(drag) = &mut self.drag {
            drag.cancel(now);
        }

        self.pointer = false;
        self.key = None;
        self.interaction.pressed = false;
        self.interaction.dragging = false;
    }

    fn record(&mut self, phase: GesturePhase, point: [f64; 2], now: Duration) {
        if self.drag_callback.is_none() {
            return;
        }

        let dt = now.saturating_sub(self.last_event).as_secs_f64();
        let delta = std::array::from_fn(|i| point[i] - self.last_pointer[i]);

        self.pending_drag = Some(DragEvent {
            phase,
            point,
            delta,
            offset: std::array::from_fn(|i| point[i] - self.pointer_origin[i]),
            velocity: delta.map(|v| if dt > 0.0 { v / dt } else { 0.0 }),
            locked_axis: self.drag.as_ref().and_then(Drag::locked_axis),
        });
        self.last_pointer = point;
        self.last_event = now;
    }

    fn begin(&mut self, pointer: [f64; 2], window: &mut Window, cx: &mut App) -> bool {
        if !self.enabled || self.pointer || !window.is_window_active() {
            return false;
        }

        self.pointer = true;
        self.touch = false;
        self.key = None;
        self.interaction.pressed = true;
        let now = self.now(cx);

        self.pointer_origin = pointer;
        self.last_pointer = pointer;
        self.last_event = now;

        if let Some(drag) = &mut self.drag {
            drag.begin(pointer, now);
            self.interaction.dragging = true;
        }

        self.record(GesturePhase::Started, pointer, now);

        if let Some(focus) = &self.focus {
            focus.focus(window, cx);
        }

        self.notify(cx);

        true
    }

    fn notify(&self, cx: &mut App) {
        if let Some(owner) = self.owner {
            cx.notify(owner);
        }
    }
}

/// Retain one handle per logical control. Clones share input/motion state.
/// Mouse release outside the hitbox, lost button, Escape, blur and deactivation
/// clear ownership. Pinch and supported native touch gestures are opt-in; raw
/// multi-touch availability depends on the backend/platform.
#[derive(Clone, Default)]
pub struct GestureState(Rc<RefCell<Data>>);

impl GestureState {
    /// Adds bounded native pointer/arrow-key dragging. Configure once in the view.
    pub fn with_drag(self, drag: Drag) -> Self {
        self.0.borrow_mut().drag = Some(drag);

        self
    }

    /// Enables stock Kit touch-drag ownership. Claiming a touch prevents the
    /// containing scroll view from also handling that gesture. Platform delivery
    /// remains native; this does not synthesize touch input on desktop macOS.
    pub fn touch_drag(self, enabled: bool) -> Self {
        self.0.borrow_mut().touch_enabled = enabled;

        self
    }

    /// Measures drag bounds against a retained native container anchor. Resize
    /// changes retarget from the current presentation rather than resetting it.
    pub fn constraints(self, container: MotionAnchor) -> Self {
        self.0.borrow_mut().constraints = Some(container);

        self
    }

    /// Reports structured pointer start/move/end/cancel events outside state borrows.
    /// Also enables a host-controlled pan surface without `with_drag`.
    pub fn on_drag(self, callback: impl Fn(DragEvent, &mut Window, &mut App) + 'static) -> Self {
        self.0.borrow_mut().drag_callback = Some(Rc::new(callback));

        self
    }

    /// Reports native pinch gestures without assuming ordinary elements can scale.
    pub fn on_pinch(
        self,
        callback: impl Fn(PinchGesture, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.0.borrow_mut().pinch_callback = Some(Rc::new(callback));

        self
    }

    /// Starts from a separate handle or scrubber. Available after the target's
    /// first layout; focus, cancellation and outside release remain target-owned.
    pub fn start_drag(&self, pointer: [f64; 2], window: &mut Window, cx: &mut App) -> bool {
        assert!(pointer.iter().all(|v| v.is_finite()), "invalid pointer");
        let started = self.0.borrow_mut().begin(pointer, window, cx);

        emit_pending(&self.0, window, cx);

        started
    }

    /// Uses another measured viewport for in-view checks, intersected with normal
    /// ancestor clipping. Missing measurements preserve the previous visibility.
    pub fn view_root(self, root: MotionAnchor) -> Self {
        self.0.borrow_mut().view_root = Some(root);

        self
    }

    /// Insets top/right/bottom/left visibility edges; negative values expand them.
    pub fn view_margins(self, margins: [f64; 4]) -> Self {
        assert!(
            margins.iter().all(|v| v.is_finite() && v.abs() <= 1e12),
            "invalid viewport margins"
        );
        self.0.borrow_mut().view_margins = margins;

        self
    }

    /// Enables automatically measured clipped viewport visibility. Configure once.
    pub fn in_view(self, amount: f64, once: bool) -> Self {
        self.0.borrow_mut().visibility = Some(InView::new(amount, once));

        self
    }

    /// Current resolved interaction flags, supplied automatically by the element.
    pub fn interaction(&self) -> Interaction {
        self.0.borrow().interaction
    }

    /// Last presented target, including drag displacement.
    pub fn presented(&self) -> Target {
        self.0.borrow().presented
    }

    /// The stable focus handle, available after the first layout.
    pub fn focus_handle(&self) -> Option<FocusHandle> {
        self.0.borrow().focus.clone()
    }

    /// Cancels pointer/key state and bounded dragging, notifying the owning view.
    pub fn cancel(&self, cx: &mut App) {
        let mut data = self.0.borrow_mut();

        data.cancel(cx);
        data.notify(cx);
    }

    /// Wraps a styled native control and automatically binds hover/press/focus,
    /// viewport and optional drag. Supply role, label and click behavior on the
    /// child. The adapter supplies a stable tab-stop focus handle. Configure
    /// layout styles before binding; x/y are owned by this adapter, not a second
    /// `LayoutGroup` on the same control. Native activation remains Kit's behavior.
    ///
    /// ```
    /// use gpui_kit::{div, prelude::*, Role};
    /// use gpui_motion_kit::{GestureState, core::{GestureTargets, Motion, Target}};
    /// let state = GestureState::default(); // persistent view field
    /// let control = state.bind("motion-button", div().id("button").role(Role::Button).child("Press"),
    ///     Motion::new(Target::default()), GestureTargets::default().while_press(Target::default().opacity(0.5)));
    /// ```
    pub fn bind<E: IntoElement + StatefulInteractiveElement + Styled + 'static>(
        &self,
        id: impl Into<ElementId>,
        child: E,
        motion: Motion,
        targets: GestureTargets,
    ) -> GestureElement<E> {
        GestureElement {
            id: id.into(),
            child: Some(child),
            state: self.clone(),
            motion,
            targets,
            enabled: true,
            drag_listener: true,
        }
    }
}

/// Frame-scoped input/motion wrapper produced by `GestureState::bind`.
pub struct GestureElement<E = Stateful<Div>> {
    id: ElementId,
    child: Option<E>,
    state: GestureState,
    motion: Motion,
    targets: GestureTargets,
    enabled: bool,
    drag_listener: bool,
}

impl<E> GestureElement<E> {
    /// Disables automatic target dragging so a separate handle can call `start_drag`.
    pub fn drag_listener(mut self, enabled: bool) -> Self {
        self.drag_listener = enabled;

        self
    }

    /// Disables native input bindings and keyboard tab stops. Cancels a held
    /// gesture on the next render; the owner restores focus if disabling the
    /// focused control and supplies disabled accessibility metadata on the child
    /// using its Kit version’s APIs. Visibility observation remains available.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;

        self
    }
}

impl<E: IntoElement + StatefulInteractiveElement + Styled + 'static> IntoElement
    for GestureElement<E>
{
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl<E: IntoElement + StatefulInteractiveElement + Styled + 'static> Element for GestureElement<E> {
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
        let mut data = self.state.0.borrow_mut();

        data.owner = Some(w.current_view());
        data.enabled = self.enabled;
        data.epoch
            .get_or_insert_with(|| cx.background_executor().now());
        let focus = data
            .focus
            .get_or_insert_with(|| cx.focus_handle().tab_stop(true))
            .clone()
            .tab_stop(self.enabled);

        let focused = self.enabled && focus.is_focused(w);

        if !self.enabled
            || !w.is_window_active()
            || (!focused && (data.key.is_some() || data.pointer || data.pinch_active))
        {
            data.cancel(cx);
        }

        if !self.enabled || !w.is_window_active() {
            data.interaction.hovered = false;
        }

        data.interaction.focused = focused;
        let now = data.now(cx);
        let target = self.targets.resolve(self.motion.target(), data.interaction);

        let frame = data
            .motion
            .sample(self.motion.with_target(target), now, cx.reduce_motion());

        let mut target = frame.target();
        let mut active = frame.is_animating();

        if let Some(drag) = &mut data.drag {
            let offset = drag.sample(now, cx.reduce_motion());

            active |= offset.is_animating();
            let finite = |a: f32, b: f32| {
                (f64::from(a) + f64::from(b)).clamp(f64::from(f32::MIN), f64::from(f32::MAX)) as f32
            };

            target = target
                .x(finite(target.x_px(), offset.target().x_px()))
                .y(finite(target.y_px(), offset.target().y_px()));
        }

        data.presented = target;

        if active {
            w.request_animation_frame();
        }

        let child = frame
            .apply(
                self.child
                    .take()
                    .expect("one layout")
                    .track_focus(&focus)
                    .tab_index(0),
            )
            .left(px(target.x_px()))
            .top(px(target.y_px()));

        let pending = data.pending_drag.is_some() || data.pending_pinch.is_some();

        drop(data);

        if pending {
            let shared = self.state.0.clone();

            w.defer(cx, move |w, cx| emit_pending(&shared, w, cx));
        }

        let mut child = child.into_any_element();
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
        let hitbox = w.insert_hitbox(bounds, HitboxBehavior::Normal);
        let mut data = self.state.0.borrow_mut();

        let rect = |r: Bounds<Pixels>| {
            LayoutRect::new(
                [f64::from(r.origin.x), f64::from(r.origin.y)],
                [f64::from(r.size.width), f64::from(r.size.height)],
            )
            .expect("finite native bounds")
        };

        let now = data.now(cx);

        if let Some(container) = data.constraints.as_ref().and_then(MotionAnchor::rect) {
            let presented = data.presented;

            let base = LayoutRect::new(
                [
                    f64::from(bounds.origin.x) - f64::from(presented.x_px()),
                    f64::from(bounds.origin.y) - f64::from(presented.y_px()),
                ],
                [f64::from(bounds.size.width), f64::from(bounds.size.height)],
            )
            .expect("finite bounds");

            let limits = DragBounds::within(container, base);

            if let Some(drag) = &mut data.drag {
                let previous = drag.presentation().0;

                drag.set_bounds(limits.min(), limits.max(), now);
                let frame = drag.sample(now, cx.reduce_motion());

                if frame.is_animating() || previous != drag.presentation().0 {
                    w.request_animation_frame();
                }
            }
        }

        let clip = rect(w.content_mask().bounds);

        let viewport = data
            .view_root
            .as_ref()
            .map_or(Some(clip), MotionAnchor::rect)
            .map(|root| root.inset(data.view_margins).intersection(clip));

        if let (Some(visibility), Some(viewport)) = (&mut data.visibility, viewport) {
            let viewport = Bounds {
                origin: gpui_kit::point(
                    px(viewport.origin()[0] as f32),
                    px(viewport.origin()[1] as f32),
                ),
                size: gpui_kit::size(px(viewport.size()[0] as f32), px(viewport.size()[1] as f32)),
            };

            let rect = |r: Bounds<Pixels>| {
                [
                    f64::from(r.origin.x),
                    f64::from(r.origin.y),
                    f64::from(r.size.width),
                    f64::from(r.size.height),
                ]
            };

            if visibility.update(rect(viewport), rect(bounds)) {
                data.interaction.in_view = visibility.is_visible();
                let owner = data.owner.expect("owner");

                w.defer(cx, move |_, cx| cx.notify(owner));
            }
        }

        drop(data);
        child.prepaint(w, cx);

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
        if !self.enabled {
            let region = hitbox.clone();

            w.on_mouse_event(move |_: &MouseDownEvent, phase, w, cx| {
                if phase == DispatchPhase::Capture && region.is_hovered(w) {
                    cx.stop_propagation();
                }
            });
            let region = hitbox.clone();

            w.on_mouse_event(move |_: &MouseUpEvent, phase, w, cx| {
                if phase == DispatchPhase::Capture && region.is_hovered(w) {
                    cx.stop_propagation();
                }
            });
            let region = hitbox.clone();

            w.on_mouse_event(move |_: &MouseMoveEvent, phase, w, cx| {
                if phase == DispatchPhase::Capture && region.is_hovered(w) {
                    cx.stop_propagation();
                }
            });
            let focus = self.state.focus_handle();

            w.on_key_event(move |_: &KeyDownEvent, phase, w, cx| {
                if phase == DispatchPhase::Capture
                    && focus.as_ref().is_some_and(|f| f.is_focused(w))
                {
                    cx.stop_propagation();
                }
            });
            let focus = self.state.focus_handle();

            w.on_key_event(move |_: &KeyUpEvent, phase, w, cx| {
                if phase == DispatchPhase::Capture
                    && focus.as_ref().is_some_and(|f| f.is_focused(w))
                {
                    cx.stop_propagation();
                }
            });
            child.paint(w, cx);

            return;
        }

        let shared = self.state.0.clone();
        let region = hitbox.clone();

        let automatic_drag = self.drag_listener;

        w.on_mouse_event(move |event: &MouseDownEvent, phase, w, cx| {
            if phase == DispatchPhase::Bubble
                && event.button == MouseButton::Left
                && region.is_hovered(w)
                && automatic_drag
            {
                let mut data = shared.borrow_mut();

                if data.begin(
                    [f64::from(event.position.x), f64::from(event.position.y)],
                    w,
                    cx,
                ) && (data.drag.is_some() || data.drag_callback.is_some())
                {
                    cx.stop_propagation();
                }

                drop(data);
                emit_pending(&shared, w, cx);
            }
        });
        let shared = self.state.0.clone();
        let region = hitbox.clone();

        w.on_mouse_event(move |event: &MouseMoveEvent, phase, w, cx| {
            if phase != DispatchPhase::Capture {
                return;
            }

            let mut data = shared.borrow_mut();
            let hover = region.is_hovered(w);
            let changed = data.interaction.hovered != hover;

            data.interaction.hovered = hover;

            if data.pointer && !data.touch {
                let now = data.now(cx);

                if event.pressed_button != Some(MouseButton::Left) {
                    data.cancel(cx);
                } else if let Some(drag) = &mut data.drag {
                    drag.move_to(
                        [f64::from(event.position.x), f64::from(event.position.y)],
                        now,
                    );
                }

                if data.pointer {
                    data.record(
                        GesturePhase::Moved,
                        [f64::from(event.position.x), f64::from(event.position.y)],
                        now,
                    );
                }

                data.notify(cx);
            } else if changed {
                data.notify(cx);
            }

            drop(data);
            emit_pending(&shared, w, cx);
        });
        let shared = self.state.0.clone();

        w.on_mouse_event(move |event: &MouseUpEvent, phase, w, cx| {
            if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                let mut data = shared.borrow_mut();

                if !data.pointer || data.touch {
                    return;
                }

                let now = data.now(cx);

                if let Some(drag) = &mut data.drag {
                    drag.end(now);
                }

                data.record(
                    GesturePhase::Ended,
                    [f64::from(event.position.x), f64::from(event.position.y)],
                    now,
                );
                data.pointer = false;
                data.interaction.pressed = false;
                data.interaction.dragging = false;
                data.notify(cx);
                drop(data);
                emit_pending(&shared, w, cx);
            }
        });
        let shared = self.state.0.clone();

        w.on_mouse_event(move |_: &MouseExitEvent, phase, _, cx| {
            if phase == DispatchPhase::Capture {
                let mut data = shared.borrow_mut();

                if data.interaction.hovered {
                    data.interaction.hovered = false;
                    data.notify(cx);
                }
            }
        });
        let shared = self.state.0.clone();

        w.on_key_event(move |event: &KeyDownEvent, phase, w, cx| {
            if phase != DispatchPhase::Capture {
                return;
            }

            let mut data = shared.borrow_mut();

            if !data.focus.as_ref().is_some_and(|f| f.is_focused(w)) {
                return;
            }

            let key = event.keystroke.key.as_str();
            let now = data.now(cx);

            if key == "escape" {
                data.cancel(cx);
                data.notify(cx);
                drop(data);
                emit_pending(&shared, w, cx);
            } else if ["enter", "space"].contains(&key) && !event.is_held && !data.pointer {
                data.key = Some(key.to_owned());
                data.interaction.pressed = true;
                data.notify(cx);
            } else if let Some(delta) = match key {
                "left" => Some([-10., 0.]),
                "right" => Some([10., 0.]),
                "up" => Some([0., -10.]),
                "down" => Some([0., 10.]),
                _ => None,
            } && let Some(drag) = &mut data.drag
            {
                drag.move_by(delta, now);
                data.notify(cx);
                cx.stop_propagation();
            }
        });
        let shared = self.state.0.clone();

        w.on_key_event(move |event: &KeyUpEvent, phase, _, cx| {
            if phase == DispatchPhase::Capture {
                let mut data = shared.borrow_mut();

                if data.key.as_deref() == Some(event.keystroke.key.as_str()) {
                    data.key = None;
                    data.interaction.pressed = data.pointer;
                    data.notify(cx);
                }
            }
        });
        let shared = self.state.0.clone();
        let region = hitbox.clone();

        w.on_mouse_event(move |event: &PinchEvent, phase, w, cx| {
            if phase != DispatchPhase::Bubble || !event.delta.is_finite() {
                return;
            }

            let mut data = shared.borrow_mut();

            if data.pinch_callback.is_none() {
                return;
            }

            if event.phase == TouchPhase::Started {
                if !region.is_hovered(w) {
                    return;
                }

                data.pinch_active = true;
                data.pinch_scale = 1.0;

                if let Some(focus) = &data.focus {
                    focus.focus(w, cx);
                }
            }

            if !data.pinch_active {
                return;
            }

            data.pinch_scale = (data.pinch_scale * (1.0 + f64::from(event.delta))).clamp(0.0, 1e6);
            data.pinch_center = [f64::from(event.position.x), f64::from(event.position.y)];
            let phase = match event.phase {
                TouchPhase::Started => GesturePhase::Started,
                TouchPhase::Moved => GesturePhase::Moved,
                TouchPhase::Ended => GesturePhase::Ended,
                TouchPhase::Cancelled => GesturePhase::Cancelled,
            };

            data.pending_pinch = Some(PinchGesture {
                phase,
                center: data.pinch_center,
                scale: data.pinch_scale,
                delta: f64::from(event.delta),
            });

            if matches!(event.phase, TouchPhase::Ended | TouchPhase::Cancelled) {
                data.pinch_active = false;
            }

            data.notify(cx);
            drop(data);
            emit_pending(&shared, w, cx);
            cx.stop_propagation();
        });
        let shared = self.state.0.clone();
        let region = hitbox.clone();
        let automatic_drag = self.drag_listener;

        w.on_mouse_event(move |event: &gpui_kit::TouchDragEvent, phase, w, cx| {
            if phase != DispatchPhase::Bubble {
                return;
            }

            let mut data = shared.borrow_mut();

            if !data.enabled || !data.touch_enabled {
                return;
            }

            let point = [f64::from(event.position.x), f64::from(event.position.y)];
            let now = data.now(cx);

            match event.phase {
                TouchPhase::Started => {
                    if !automatic_drag
                        || !region.is_hovered(w)
                        || (data.drag.is_none() && data.drag_callback.is_none())
                    {
                        return;
                    }

                    if data.begin(point, w, cx) {
                        data.touch = true;
                        w.prevent_default();
                    }
                }
                TouchPhase::Moved if data.touch => {
                    if let Some(drag) = &mut data.drag {
                        drag.move_to(point, now);
                    }

                    data.record(GesturePhase::Moved, point, now);
                    data.notify(cx);
                }
                TouchPhase::Ended if data.touch => {
                    if let Some(drag) = &mut data.drag {
                        drag.end(now);
                    }

                    data.record(GesturePhase::Ended, point, now);
                    data.touch = false;
                    data.pointer = false;
                    data.interaction.pressed = false;
                    data.interaction.dragging = false;
                    data.notify(cx);
                }
                TouchPhase::Cancelled if data.touch => {
                    data.cancel(cx);
                    data.notify(cx);
                }
                _ => return,
            }

            drop(data);
            emit_pending(&shared, w, cx);
            cx.stop_propagation();
        });
        // Register key listeners on this wrapper before child paint selects a descendant node.
        child.paint(w, cx);
    }
}

fn emit_pending(shared: &Rc<RefCell<Data>>, window: &mut Window, cx: &mut App) {
    let (drag, drag_callback, pinch, pinch_callback) = {
        let mut data = shared.borrow_mut();

        (
            data.pending_drag.take(),
            data.drag_callback.clone(),
            data.pending_pinch.take(),
            data.pinch_callback.clone(),
        )
    };

    if let (Some(event), Some(callback)) = (drag, drag_callback) {
        callback(event, window, cx);
    }

    if let (Some(event), Some(callback)) = (pinch, pinch_callback) {
        callback(event, window, cx);
    }
}
