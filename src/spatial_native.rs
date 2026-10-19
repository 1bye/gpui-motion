/// Shared native scheduling and window-space measurement implementation.
#[doc(hidden)]
#[macro_export]
macro_rules! __motion_spatial_adapter {
    ($ui:ident, $adapter:path) => {
        use std::{
            cell::RefCell,
            ops::{Deref, DerefMut},
            rc::Rc,
            time::{Duration, Instant},
        };
        use $adapter as native;
        use $ui::{
            AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId,
            IntoElement, LayoutId, ScrollHandle, Window,
        };

        #[derive(Debug, Default)]
        struct Clock(Option<Instant>);

        impl Clock {
            fn tick(&mut self, cx: &App) -> Duration {
                let now = cx.background_executor().now();
                now.saturating_duration_since(*self.0.get_or_insert(now))
            }

            fn now(&self, cx: &App) -> Duration {
                self.0.map_or(Duration::ZERO, |epoch| {
                    cx.background_executor()
                        .now()
                        .saturating_duration_since(epoch)
                })
            }
        }

        /// View-owned native clock for a portable motion-value graph.
        /// Keep this state alive and notify after source/control changes.
        #[derive(Debug, Default)]
        pub struct GraphState {
            inner: $crate::MotionGraph,
            clock: Clock,
        }

        impl Deref for GraphState {
            type Target = $crate::MotionGraph;
            fn deref(&self) -> &Self::Target {
                &self.inner
            }
        }

        impl DerefMut for GraphState {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.inner
            }
        }

        impl GraphState {
            /// Native monotonic control time, zero before the first `frame`.
            pub fn now(&self, cx: &App) -> Duration {
                self.clock.now(cx)
            }

            /// Samples once in the owning view, before reading all bound values.
            pub fn frame(&mut self, window: &mut Window, cx: &App) -> &$crate::MotionGraph {
                let now = self.clock.tick(cx);
                self.inner.sample(now, cx.reduce_motion());

                if self.inner.is_animating() {
                    window.request_animation_frame();
                }
                &self.inner
            }
        }

        /// Native owner for a portable variant tree; shares the graph's explicit clock contract.
        #[derive(Debug)]
        pub struct MotionTreeState<K, V> {
            inner: $crate::MotionTree<K, V>,
            clock: Clock,
        }

        impl<K, V> Deref for MotionTreeState<K, V> {
            type Target = $crate::MotionTree<K, V>;
            fn deref(&self) -> &Self::Target {
                &self.inner
            }
        }

        impl<K, V> DerefMut for MotionTreeState<K, V> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.inner
            }
        }

        impl<K: PartialEq, V: PartialEq> MotionTreeState<K, V> {
            /// Retains a prepared node tree.
            pub fn new(tree: $crate::MotionTree<K, V>) -> Self {
                Self {
                    inner: tree,
                    clock: Clock::default(),
                }
            }

            /// Native control timestamp.
            pub fn now(&self, cx: &App) -> Duration {
                self.clock.now(cx)
            }

            /// Samples parent/child gates and schedules pending trajectories/delays.
            pub fn frame(&mut self, window: &mut Window, cx: &App) {
                self.inner.sample(self.clock.tick(cx), cx.reduce_motion());

                if self.inner.is_animating() {
                    window.request_animation_frame();
                }
            }

            /// Retrieves this backend's native frame type.
            pub fn get(&self, key: &K) -> Option<native::Frame> {
                self.inner.get(key).map(Into::into)
            }
        }

        #[derive(Default)]
        struct AnchorData {
            rect: Option<$crate::LayoutRect>,
            scroll: Option<ScrollHandle>,
            offset: [f64; 2],
            window: Option<$ui::AnyWindowHandle>,
        }

        /// Retained measurement of one native element in logical window coordinates.
        /// Mount each anchor once. Clones share storage. `clear` releases a stale
        /// measurement before reusing the owner in another window.
        #[derive(Clone, Default)]
        pub struct MotionAnchor(Rc<RefCell<AnchorData>>);

        impl MotionAnchor {
            /// Tracks current native offsets between measurements, preventing a
            /// frame of scroll lag. Use the target's nearest scrolling ancestor.
            pub fn scroll_source(self, handle: ScrollHandle) -> Self {
                self.0.borrow_mut().scroll = Some(handle);
                self
            }

            /// Last measured rectangle, adjusted for an explicitly tracked scroll source.
            pub fn rect(&self) -> Option<$crate::LayoutRect> {
                let data = self.0.borrow();
                let rect = data.rect?;

                if let Some(handle) = &data.scroll {
                    let offset = handle.offset();
                    $crate::LayoutRect::new(
                        [
                            rect.origin()[0] + f64::from(offset.x) - data.offset[0],
                            rect.origin()[1] + f64::from(offset.y) - data.offset[1],
                        ],
                        rect.size(),
                    )
                    .ok()
                } else {
                    Some(rect)
                }
            }

            /// Releases identity/measurement history; does not keep a native element alive.
            pub fn clear(&self) {
                let mut data = self.0.borrow_mut();
                data.rect = None;
                data.window = None;
            }

            /// Measures an ordinary child, retaining its input and accessibility semantics.
            pub fn element<E: IntoElement + 'static>(
                &self,
                id: impl Into<ElementId>,
                child: E,
            ) -> AnchorElement<E> {
                AnchorElement {
                    id: id.into(),
                    anchor: self.clone(),
                    child: Some(child),
                }
            }
        }

        /// Frame-scoped measuring wrapper created by `MotionAnchor::element`.
        pub struct AnchorElement<E> {
            id: ElementId,
            anchor: MotionAnchor,
            child: Option<E>,
        }

        impl<E: IntoElement + 'static> IntoElement for AnchorElement<E> {
            type Element = Self;
            fn into_element(self) -> Self {
                self
            }
        }

        impl<E: IntoElement + 'static> Element for AnchorElement<E> {
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
                let mut child = self.child.take().expect("one layout").into_any_element();
                let layout = child.request_layout(window, cx);
                (layout, child)
            }

            fn prepaint(
                &mut self,
                _: Option<&GlobalElementId>,
                _: Option<&InspectorElementId>,
                bounds: Bounds<$ui::Pixels>,
                child: &mut AnyElement,
                window: &mut Window,
                cx: &mut App,
            ) {
                let rect = $crate::LayoutRect::new(
                    [f64::from(bounds.origin.x), f64::from(bounds.origin.y)],
                    [f64::from(bounds.size.width), f64::from(bounds.size.height)],
                )
                .expect("finite native bounds");
                let changed = self.anchor.rect() != Some(rect);
                let mut data = self.anchor.0.borrow_mut();
                let handle = window.window_handle();
                assert!(
                    data.window.is_none_or(|old| old == handle),
                    "motion anchors cannot span windows; clear before reusing"
                );
                data.window = Some(handle);
                data.rect = Some(rect);

                if let Some(scroll) = &data.scroll {
                    let offset = scroll.offset();
                    data.offset = [f64::from(offset.x), f64::from(offset.y)];
                }
                drop(data);

                if changed {
                    window.request_animation_frame();
                }
                child.prepaint(window, cx);
            }

            fn paint(
                &mut self,
                _: Option<&GlobalElementId>,
                _: Option<&InspectorElementId>,
                _: Bounds<$ui::Pixels>,
                child: &mut AnyElement,
                _: &mut (),
                window: &mut Window,
                cx: &mut App,
            ) {
                child.paint(window, cx);
            }
        }

        /// Tracks an element's progress through another measured element, with
        /// configurable intersections, margins and optional spring smoothing.
        /// Retain anchors and this state; render their wrappers in the same window.
        pub struct ScrollTracker {
            handle: ScrollHandle,
            container: MotionAnchor,
            target: MotionAnchor,
            offsets: $crate::ScrollOffsets,
            axis: native::Axis,
            margins: [f64; 4],
            transition: $crate::Transition,
            value: $crate::MotionValue,
            clock: Clock,
        }

        impl ScrollTracker {
            /// Creates a target/viewport intersection tracker. Give the target anchor
            /// `scroll_source(handle.clone())` for current offsets before prepaint.
            pub fn new(
                handle: ScrollHandle,
                container: MotionAnchor,
                target: MotionAnchor,
                axis: native::Axis,
            ) -> Self {
                Self {
                    handle,
                    container,
                    target,
                    offsets: Default::default(),
                    axis,
                    margins: [0.0; 4],
                    transition: $crate::Transition::Instant,
                    value: $crate::MotionValue::new(0.0),
                    clock: Default::default(),
                }
            }

            /// Sets start/end intersections.
            pub fn offsets(mut self, offsets: $crate::ScrollOffsets) -> Self {
                self.offsets = offsets;
                self
            }

            /// Insets top/right/bottom/left viewport edges; negative margins expand them.
            pub fn margins(mut self, margins: [f64; 4]) -> Self {
                assert!(
                    margins.iter().all(|v| v.is_finite() && v.abs() <= 1e12),
                    "invalid scroll margins"
                );
                self.margins = margins;
                self
            }

            /// Uses the shared solver for optional smoothing.
            pub fn transition(mut self, transition: $crate::Transition) -> Self {
                self.transition = transition;
                self
            }

            /// Samples available measurements, tracks resize, and schedules only while
            /// smoothing. A missing anchor holds the last value with `ready=false`.
            pub fn frame(&mut self, window: &mut Window, cx: &App) -> $crate::ScrollFrame {
                let now = self.clock.tick(cx);

                for anchor in [&self.container, &self.target] {
                    assert!(
                        anchor
                            .0
                            .borrow()
                            .window
                            .is_none_or(|handle| handle == window.window_handle()),
                        "scroll anchors must belong to this window"
                    );
                }
                let ready = self.container.rect().zip(self.target.rect());

                if let Some((container, target)) = ready {
                    let axis = match self.axis {
                        native::Axis::X => 0,
                        native::Axis::Y => 1,
                    };
                    self.value.animate(
                        self.offsets
                            .progress(container.inset(self.margins), target, axis),
                        self.transition,
                        now,
                    );
                } else {
                    self.value.jump(self.value.get(), now);
                }
                let frame = self.value.sample(now, cx.reduce_motion());

                if frame.is_animating() {
                    window.request_animation_frame();
                }
                let offset = self.handle.offset();
                $crate::ScrollFrame {
                    progress: frame.value(),
                    velocity: frame.velocity(),
                    offset: [-f64::from(offset.x), -f64::from(offset.y)],
                    ready: ready.is_some(),
                    active: frame.is_animating(),
                }
            }
        }

        struct SharedEntry {
            anchors: std::collections::HashMap<ElementId, MotionAnchor>,
            selected: Option<ElementId>,
            values: [$crate::MotionValue; 2],
            last: Option<$crate::LayoutRect>,
            clock: Clock,
        }

        impl Default for SharedEntry {
            fn default() -> Self {
                Self {
                    anchors: Default::default(),
                    selected: None,
                    values: [$crate::MotionValue::new(0.0), $crate::MotionValue::new(0.0)],
                    last: None,
                    clock: Default::default(),
                }
            }
        }

        /// Window-root coordinator connecting anchors in separate containers. Render
        /// its floating appearances in an unclipped root layer. Anchor geometry is
        /// reconciled in window coordinates, including explicitly tracked scrolling.
        /// Sizes can follow measured destinations through real layout; no scale
        /// projection or transformed clipping is installed. One active appearance
        /// per key is required. The host retains exits and owns focus/input policy.
        pub struct SharedOverlay<K>(Rc<RefCell<std::collections::HashMap<K, SharedEntry>>>);

        impl<K> Clone for SharedOverlay<K> {
            fn clone(&self) -> Self {
                Self(self.0.clone())
            }
        }

        impl<K> Default for SharedOverlay<K> {
            fn default() -> Self {
                Self(Rc::new(RefCell::new(Default::default())))
            }
        }

        impl<K: Eq + std::hash::Hash + Clone + 'static> SharedOverlay<K> {
            /// Marks a normal in-flow location. Use stable, domain-derived location IDs.
            pub fn anchor<E: IntoElement + 'static>(
                &self,
                key: K,
                location: impl Into<ElementId>,
                child: E,
            ) -> AnchorElement<E> {
                self.anchor_impl(key, location.into(), None, child)
            }

            /// Marks a location inside a scroll container, tracking its current offset.
            pub fn scroll_anchor<E: IntoElement + 'static>(
                &self,
                key: K,
                location: impl Into<ElementId>,
                scroll: ScrollHandle,
                child: E,
            ) -> AnchorElement<E> {
                self.anchor_impl(key, location.into(), Some(scroll), child)
            }

            fn anchor_impl<E: IntoElement + 'static>(
                &self,
                key: K,
                location: ElementId,
                scroll: Option<ScrollHandle>,
                child: E,
            ) -> AnchorElement<E> {
                let mut entries = self.0.borrow_mut();
                let entry = entries.entry(key).or_default();
                let anchor = entry.anchors.entry(location.clone()).or_default().clone();
                anchor.0.borrow_mut().scroll = scroll;
                anchor.element(location, child)
            }

            /// Selects a destination. It may be measured later in this frame; the
            /// previous presentation remains visible until its first measurement.
            /// Notify the owning view after changing the selection.
            pub fn select(&self, key: K, location: impl Into<ElementId>) {
                self.0.borrow_mut().entry(key).or_default().selected = Some(location.into());
            }

            /// Creates an appearance in the root overlay. Its child can contain live
            /// native controls; use a stable ID to retain the same logical subtree.
            pub fn element<E: IntoElement + $ui::Styled + 'static>(
                &self,
                key: K,
                id: impl Into<ElementId>,
                child: E,
            ) -> SharedOverlayElement<K, E> {
                self.0.borrow_mut().entry(key.clone()).or_default();
                SharedOverlayElement {
                    group: self.clone(),
                    key,
                    id: id.into(),
                    child: Some(child),
                    transition: $crate::Transition::default(),
                    active: true,
                    match_size: false,
                }
            }

            /// Releases anchor and trajectory history after the identity is finished.
            pub fn remove(&self, key: &K) {
                self.0.borrow_mut().remove(key);
            }
        }

        /// Frame-scoped, window-coordinate position/crossfade appearance.
        pub struct SharedOverlayElement<K, E> {
            group: SharedOverlay<K>,
            key: K,
            id: ElementId,
            child: Option<E>,
            transition: $crate::Transition,
            active: bool,
            match_size: bool,
        }

        impl<K, E> SharedOverlayElement<K, E> {
            /// Chooses interruptible position and crossfade timing.
            pub fn transition(mut self, transition: $crate::Transition) -> Self {
                self.transition = transition;
                self
            }

            /// Fades a retiring appearance. Disable its child input and restore focus
            /// in the host before making interactive content inactive.
            pub fn active(mut self, active: bool) -> Self {
                self.active = active;
                self
            }

            /// Follows the measured destination width/height immediately via native
            /// layout. Child sizes are retained when false; neither mode scales text.
            pub fn match_size(mut self, enabled: bool) -> Self {
                self.match_size = enabled;
                self
            }
        }

        struct SharedFade {
            value: $crate::MotionValue,
            clock: Clock,
        }

        impl<K: Eq + std::hash::Hash + Clone + 'static, E: IntoElement + $ui::Styled + 'static>
            IntoElement for SharedOverlayElement<K, E>
        {
            type Element = Self;
            fn into_element(self) -> Self {
                self
            }
        }

        impl<K: Eq + std::hash::Hash + Clone + 'static, E: IntoElement + $ui::Styled + 'static>
            Element for SharedOverlayElement<K, E>
        {
            type RequestLayoutState = AnyElement;
            type PrepaintState = bool;
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
                let entries = self.group.0.borrow();
                let entry = entries.get(&self.key).expect("registered identity");
                let target = entry
                    .selected
                    .as_ref()
                    .and_then(|slot| entry.anchors.get(slot))
                    .and_then(MotionAnchor::rect)
                    .or(entry.last);
                let initial = if self.active && entry.last.is_none() {
                    1.0
                } else {
                    0.0
                };
                drop(entries);
                let opacity = window.with_element_state(
                    id.expect("shared overlay ID"),
                    |state: Option<SharedFade>, window| {
                        let mut state = state.unwrap_or_else(|| SharedFade {
                            value: $crate::MotionValue::new(initial),
                            clock: Clock::default(),
                        });
                        let now = state.clock.tick(cx);
                        state.value.animate(
                            if self.active { 1.0 } else { 0.0 },
                            self.transition,
                            now,
                        );
                        let frame = state.value.sample(now, cx.reduce_motion());

                        if frame.is_animating() {
                            window.request_animation_frame();
                        }
                        (frame.value().clamp(0.0, 1.0) as f32, state)
                    },
                );
                let mut child = self
                    .child
                    .take()
                    .expect("one layout")
                    .absolute()
                    .left($ui::px(0.0))
                    .top($ui::px(0.0))
                    .opacity(opacity);
                if self.match_size
                    && let Some(target) = target
                {
                    child = child
                        .w($ui::px(target.size()[0] as f32))
                        .h($ui::px(target.size()[1] as f32));
                }
                let mut child = child.into_any_element();
                let layout = child.request_layout(window, cx);
                (layout, child)
            }

            fn prepaint(
                &mut self,
                _: Option<&GlobalElementId>,
                _: Option<&InspectorElementId>,
                bounds: Bounds<$ui::Pixels>,
                child: &mut AnyElement,
                window: &mut Window,
                cx: &mut App,
            ) -> bool {
                let mut entries = self.group.0.borrow_mut();
                let entry = entries.get_mut(&self.key).expect("registered identity");
                let anchor = entry
                    .selected
                    .as_ref()
                    .and_then(|slot| entry.anchors.get(slot));
                if let Some(anchor) = anchor {
                    assert!(
                        anchor
                            .0
                            .borrow()
                            .window
                            .is_none_or(|handle| handle == window.window_handle()),
                        "shared overlays cannot span native windows"
                    );
                }
                let measured = anchor.and_then(MotionAnchor::rect);
                let Some(target) = measured.or(entry.last) else {
                    return false;
                };
                let first = entry.last.is_none();
                entry.last = Some(target);
                let now = entry.clock.tick(cx);
                let mut position = [0.0; 2];

                for i in 0..2 {
                    if first {
                        entry.values[i].jump(target.origin()[i], now);
                    }
                    entry.values[i].animate(target.origin()[i], self.transition, now);
                    let frame = entry.values[i].sample(now, cx.reduce_motion());
                    position[i] = frame.value();

                    if frame.is_animating() {
                        window.request_animation_frame();
                    }
                }
                drop(entries);
                let offset = $ui::point(
                    $ui::px((position[0] - f64::from(bounds.origin.x)) as f32),
                    $ui::px((position[1] - f64::from(bounds.origin.y)) as f32),
                );
                window.with_element_offset(offset, |window| {
                    child.prepaint(window, cx);
                });
                true
            }

            fn paint(
                &mut self,
                _: Option<&GlobalElementId>,
                _: Option<&InspectorElementId>,
                _: Bounds<$ui::Pixels>,
                child: &mut AnyElement,
                ready: &mut bool,
                window: &mut Window,
                cx: &mut App,
            ) {
                if *ready {
                    child.paint(window, cx);
                }
            }
        }
    };
}
