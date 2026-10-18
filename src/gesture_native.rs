//! Native input adapter with a persistent owner and frame-scoped listeners.
use crate::{Drag, GestureTargets, InView, Interaction, Motion, MotionState, Target};
use gpui::{
    AnyElement, App, Bounds, DispatchPhase, Div, Element, ElementId, EntityId, FocusHandle,
    GlobalElementId, Hitbox, HitboxBehavior, InspectorElementId, IntoElement, KeyDownEvent,
    KeyUpEvent, LayoutId, MouseButton, MouseDownEvent, MouseExitEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Stateful, Window, prelude::*, px,
};
// notify is a trait method in the official pin, inherent in registry CE 0.2.2.
#[allow(unused_imports)]
use gpui::AppContext;
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};
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

        if let Some(drag) = &mut self.drag {
            drag.cancel(now);
        }

        self.pointer = false;
        self.key = None;
        self.interaction.pressed = false;
        self.interaction.dragging = false;
    }
    fn notify(&self, cx: &mut App) {
        if let Some(owner) = self.owner {
            cx.notify(owner);
        }
    }
}
/// Retain one handle per logical control. Clones share input/motion state.
/// Mouse release outside the hitbox, lost button, Escape, blur and deactivation
/// clear ownership. Native desktop mouse events are supported, not multi-touch.
#[derive(Clone, Default)]
pub struct GestureState(Rc<RefCell<Data>>);
impl GestureState {
    /// Adds bounded native pointer/arrow-key dragging. Configure once in the view.
    pub fn with_drag(self, drag: Drag) -> Self {
        self.0.borrow_mut().drag = Some(drag);

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
    /// `LayoutGroup` on the same control. Native activation remains CE's behavior.
    ///
    /// ```
    /// use gpui::{div, prelude::*, Role};
    /// use gpui_ce_motion::{GestureState, GestureTargets, Motion, Target};
    /// let state = GestureState::default(); // persistent view field
    /// let control = state.bind("motion-button", div().id("button").role(Role::Button).child("Press"),
    ///     Motion::new(Target::default()), GestureTargets::default().while_press(Target::default().opacity(0.5)));
    /// ```
    pub fn bind(
        &self,
        id: impl Into<ElementId>,
        child: Stateful<Div>,
        motion: Motion,
        targets: GestureTargets,
    ) -> GestureElement {
        GestureElement {
            id: id.into(),
            child: Some(child),
            state: self.clone(),
            motion,
            targets,
            enabled: true,
        }
    }
}
/// Frame-scoped input/motion wrapper produced by `GestureState::bind`.
pub struct GestureElement {
    id: ElementId,
    child: Option<Stateful<Div>>,
    state: GestureState,
    motion: Motion,
    targets: GestureTargets,
    enabled: bool,
}
impl GestureElement {
    /// Disables native input bindings and keyboard tab stops. Cancels a held
    /// gesture on the next render; the owner restores focus if disabling the
    /// focused control and supplies disabled accessibility metadata on the child
    /// using its CE version’s APIs. Visibility observation remains available.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;

        self
    }
}
impl IntoElement for GestureElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for GestureElement {
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
            || (!focused && (data.key.is_some() || data.pointer))
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

        drop(data);
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

        if let Some(visibility) = &mut data.visibility {
            let viewport = w.content_mask().bounds;

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

        w.on_mouse_event(move |event: &MouseDownEvent, phase, w, cx| {
            if phase == DispatchPhase::Bubble
                && event.button == MouseButton::Left
                && region.is_hovered(w)
            {
                let mut data = shared.borrow_mut();

                if data.pointer {
                    return;
                }

                data.pointer = true;
                data.key = None;
                data.interaction.pressed = true;
                let now = data.now(cx);

                if let Some(drag) = &mut data.drag {
                    drag.begin(
                        [f64::from(event.position.x), f64::from(event.position.y)],
                        now,
                    );
                    data.interaction.dragging = true;
                    cx.stop_propagation();
                }

                if let Some(focus) = &data.focus {
                    focus.focus(w, cx);
                }

                data.notify(cx);
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

            if data.pointer {
                let now = data.now(cx);

                if event.pressed_button != Some(MouseButton::Left) {
                    data.cancel(cx);
                } else if let Some(drag) = &mut data.drag {
                    drag.move_to(
                        [f64::from(event.position.x), f64::from(event.position.y)],
                        now,
                    );
                }

                data.notify(cx);
            } else if changed {
                data.notify(cx);
            }
        });
        let shared = self.state.0.clone();

        w.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
            if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                let mut data = shared.borrow_mut();

                if !data.pointer {
                    return;
                }

                let now = data.now(cx);

                if let Some(drag) = &mut data.drag {
                    drag.end(now);
                }

                data.pointer = false;
                data.interaction.pressed = false;
                data.interaction.dragging = false;
                data.notify(cx);
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
        // Register key listeners on this wrapper before child paint selects a descendant node.
        child.paint(w, cx);
    }
}
