# API recipes

[← README](../README.md) · [Source reference](#public-export-index) · [Exact coverage and contracts](../COVERAGE.md)

Each example is independent. Native helpers require the `gpui` feature and an
existing GPUI view. **Initialize states, groups, clips, and timelines once in that
view**, then pass them into the helpers during render. Recreating declarations
such as `Motion` and `StyleMotion` is expected; recreating their state resets them.

**Basics:** [Targets](#targets-and-transitions) · [Timing](#timing) · [Presence](#enter-and-exit) · [Keyed lists](#keyed-presence)  
**Playback:** [Clips](#clips-and-playback) · [Timelines](#timelines-and-stagger) · [Variants](#variants)  
**Input:** [Gestures](#native-gestures) · [Drag](#dragging) · [Scroll/visibility](#scroll-and-visibility) · [Reorder](#reordering)  
**Layout:** [Positions](#layout-positions) · [Popped exits](#popped-exits) · [Content size](#content-sizing) · [Shared visuals](#shared-visuals)  
**Drawing:** [Styles](#style-animation) · [SVG](#svg-transforms) · [Paths](#vector-paths) · [Values](#motion-values) · [Colors](#colors)  
**Reference:** [Lifecycle](#lifecycle-and-accessibility) · [Validation](#validation) · [Every export](#public-export-index)

## Targets and transitions

`Target` supports `x`, `y`, `opacity`, `width`, `height`, and `border_radius`.
Offsets and sizes are logical pixels. A changed destination interrupts from the
current presentation; springs retain velocity. Ordinary motion defaults to a spring.

```rust
use gpui_ce_motion::{Motion, MotionState, Target};
use std::time::Duration;

let mut state = MotionState::default(); // Retain this.
let motion = Motion::new(Target::default().x(120.).width(240.).height(100.))
    .initial(Target::default().x(0.).width(120.).height(100.));
let frame = state.sample(motion, Duration::ZERO, false);
assert!(frame.is_animating());
```

In a GPUI view, use `state.frame(motion, window, cx)` instead of `sample`, then
`frame.apply(element)`. It applies relative left/top insets, opacity, and declared
sizes/radius. Sizes reflow layout; they are not GPU transforms. Use an outer
wrapper when the element also needs absolute positioning. Radius does not clip
children automatically. A first dimension without a numeric initial endpoint snaps.
`Target::overlay` merges partial declarations; getters expose the sampled values.

## Timing

Pick a duration-based tween, physical spring, visual spring, or instant change.
Prepare visual springs and custom easing once. The visual duration means first
arrival at 90% of a resting step, rather than total settling time.

```rust
use gpui_ce_motion::{Bezier, CustomEasing, Easing, Motion, Property, Spring, Target, Transition};
use std::time::Duration;

let physical = Spring::new(240., 26., 1.).expect("positive physical parameters");
let visual = Spring::visual(Duration::from_millis(320), 0.2)
    .expect("valid duration and bounce")
    .rest(0.01, 0.01).expect("positive settling thresholds");
let bezier = Bezier::new(0.2, 0., 0.2, 1.).expect("valid control points");
let custom = CustomEasing::new(|t| (t * t, 2. * t)).expect("valid easing");
let choices = [
    Transition::Spring(physical),
    Transition::Spring(visual),
    Transition::Tween(Duration::from_millis(200), Easing::Bezier(bezier)),
    Transition::Tween(Duration::from_millis(200), Easing::Custom(custom)),
    Transition::Instant,
];
let motion = Motion::new(Target::default().x(100.).opacity(1.))
    .transition(choices[0])
    .delay(Duration::from_millis(50))
    .transition_for(Property::Opacity,
        Transition::Tween(Duration::from_millis(120), Easing::EaseOut), Duration::ZERO);
```

`Property` covers the six target channels. Built-in easing includes linear,
ease-in, ease-out, and ease-in-out. Custom easing returns both value and derivative.
Tweens preserve position when interrupted, but may change velocity.

## Enter and exit

Keep declaring the element while `present(false)` so its exit can finish.
`Frame::render` returns an empty element after the exit completes. Re-entry during
exit continues from the current presentation, using the same retained state.

```rust
use gpui::{div, AnyElement, App, Window, prelude::*};
use gpui_ce_motion::{Motion, MotionState, Target};

fn panel(state: &mut MotionState, open: bool, window: &mut Window, cx: &App) -> AnyElement {
    let frame = state.frame(
        Motion::new(Target::default())
            .initial(Target::default().y(12.).opacity(0.))
            .exit(Target::default().y(-8.).opacity(0.))
            .present(open),
        window, cx,
    );
    frame.render(|frame| frame.apply(div().id("panel").child("Hello")))
}
```

Use `frame.presence()` to inspect `Entering`, `Present`, `Exiting`, or `Absent`.
Before removing a focused control, restore focus elsewhere. Guard outgoing event
handlers with `frame.is_present()`; rendering an exit does not disable its input.

## Keyed presence

`PresenceGroup<K>` retains removed keys until you acknowledge their completed
exits. Use `Sync` to enter/exit together or `Wait` to delay new keys until exits finish.

```rust
use gpui_ce_motion::{PresenceGroup, PresenceMode};

let mut group = PresenceGroup::new(PresenceMode::Wait);
group.update([1_u32, 2]).expect("unique keys");
group.update([2, 3]).expect("unique keys");
assert!(group.children().iter().any(|child| *child.key() == 1 && !child.is_present()));
// After key 1's retained MotionState reaches Presence::Absent:
group.finish_exit(&1);
assert!(group.children().iter().any(|child| *child.key() == 3));
```

Keep each child's motion state in a map keyed by its domain ID. Iterate
`children()`, pass `child.is_present()` into `Motion::present`, and remove the
state only after `finish_exit`. See [the native gallery](../examples/motion_native.rs).

## Clips and playback

Prepare the clip once. Its clones share keyframes; constructing a new clip means
a replacement trajectory. Keyframe times increase strictly from `0` to `1`.
`Keyframe::current(0.)` captures the displayed start; a later `current` holds the
previous target. Segment easing belongs to the keyframe ending that segment.

```rust
use gpui_ce_motion::{Animation, AnimationState, Easing, Keyframe, Repeat, RepeatMode, Target};
use std::time::Duration;

let clip = Animation::keyframes(Duration::from_millis(800), [
    Keyframe::current(0.),
    Keyframe::new(0.5, Target::default().x(100.)).easing(Easing::EaseOut),
    Keyframe::new(1., Target::default().x(0.)),
]).expect("valid keyframes")
    .delay(Duration::from_millis(100))
    .repeat(Repeat::Forever, RepeatMode::Mirror, Duration::from_millis(80))
    .expect("positive repeat period");
let mut player = AnimationState::new(Target::default());
player.sample(&clip, Duration::ZERO, false);
let now = Duration::from_millis(200);
player.pause(now);
player.seek(Duration::from_millis(400), now);
player.set_speed(0.5, now).expect("finite speed");
player.play(now);
let frame = player.sample(&clip, now, false);
```

Also available: `Animation::to` for a two-endpoint tween, `Animation::spring` for
a physical spring with an explicit deadline, `Repeat::Count(n)` for **n extra
cycles**, and `Loop`/`Reverse`/`Mirror` repeat modes. Loop can jump at its boundary.

| Control/readout | Meaning |
| --- | --- |
| `pause(now)` / `stop(now)` | Hold the current playhead; stop is a pause alias |
| `play(now)` | Resume |
| `seek(playhead, now)` | Scrub, including backward seeks |
| `set_speed(speed, now)` | Change rate; negative values play backward |
| `restart(now)` / `finish()` | Start over / complete an `AnimationState` |
| `iteration()` / `is_playing()` | Read cycle count / playback state |
| `Playback` | A standalone clock with time, pause/play, seek, and speed controls |

For native rendering, call `player.frame(&clip, window, cx).apply(element)`.
**Use `player.now()` in its input handlers**, rather than another epoch. Notify the
view after controls change. Portable calls use one application epoch consistently.

## Timelines and stagger

Each timeline track corresponds to an element you own. Prepare tracks once;
`sample` returns a retained frame slice indexed by the track IDs.

```rust
use gpui_ce_motion::{Animation, Easing, StaggerFrom, Target, Timeline, stagger};
use std::time::Duration;

let clip = Animation::to(Target::default().x(100.), Duration::from_millis(300), Easing::EaseOut);
let mut timeline = Timeline::default();
let first = timeline.at(Duration::ZERO, clip.clone());
timeline.label("after-first").expect("unique label");
let next = timeline.then(clip.clone()).expect("finite predecessor");
timeline.at_label("after-first", Duration::from_millis(80), clip.clone()).expect("known label");
let delay = stagger(Duration::from_millis(40), 2, 5, StaggerFrom::Center).expect("valid index");
timeline.at(delay, clip);
timeline.restart(Duration::ZERO);
let frames = timeline.sample(Duration::from_millis(150), false);
assert_eq!(frames.len(), 4);
```

`Start`, `End`, and `Center` choose the stagger origin. Timeline controls include
pause/play, seek, speed, and restart. In native rendering, apply its sampled frames
to the matching elements and request another frame while `timeline.is_animating()`.

## Variants

Select named declarations with typed keys. A variant stores both target and timing.
Layer partial gesture targets using `resolve`, then retain timing with `with_target`.
Pass the selected key to children explicitly.

```rust
use gpui_ce_motion::{GestureTargets, Interaction, Motion, Target, Variants};

let variants = Variants::new([
    ("idle", Motion::new(Target::default().width(180.))),
    ("expanded", Motion::new(Target::default().width(320.))),
]).expect("unique names");
let base = variants.get(&"expanded").expect("known variant");
let mut interaction = Interaction::default();
interaction.hovered = true;
let target = GestureTargets::default()
    .while_hover(Target::default().opacity(0.8))
    .while_focus(Target::default().y(-2.))
    .resolve(base.target(), interaction);
let motion = base.with_target(target);
```

## Native gestures

`GestureState` binds hover, press, focus, drag, and clipped in-view targets to a
native control. Retain it in your view. Its partial target precedence is in-view,
focus, hover, press, then drag.

```rust
use gpui::{div, Role, prelude::*};
use gpui_ce_motion::{GestureElement, GestureState, GestureTargets, Motion, Target};

fn control(state: &GestureState) -> GestureElement {
    state.bind(
        "action-motion",
        div().id("action").role(Role::Button).aria_label("Run action").child("Run"),
        Motion::new(Target::default()),
        GestureTargets::default()
            .while_hover(Target::default().opacity(0.8))
            .while_press(Target::default().y(2.))
            .while_focus(Target::default().opacity(0.9)),
    ).enabled(true)
}
```

Read `interaction()`, `presented()`, or `focus_handle()`; use `cancel(cx)` when
switching contexts. `.enabled(false)` cancels input and removes tab stops; the
app supplies disabled semantics and restores focus. Mouse and keyboard are
supported; OS-wide capture, multi-touch, and automatic autoscroll are not installed.

## Dragging

Add a bounded `Drag` to a retained gesture state for native handlers. Equal min/max
coordinates lock an axis. Grid snap, elasticity, and momentum are optional.

```rust
use gpui_ce_motion::{Drag, GestureState};

let drag = Drag::new([0., 0.], [0., 0.], [300., 0.])
    .elasticity(0.15)
    .snap_grid(20.)
    .momentum(true);
let state = GestureState::default().with_drag(drag);
// Store state in the view and use state.bind(...) as above.
```

The portable `Drag` exposes `begin`, `move_to`, `end`, `cancel`, `move_by` (keyboard
movement), `sample`, and `is_dragging`. Its host must supply pointer timestamps,
release/cancellation handling, focus, and frame requests. A still hold before
release clears stale momentum. Re-grabbing starts at the displayed position.

## Scroll and visibility

Use a retained native `ScrollHandle` for both the scroll container and its progress
binding. Mapping is instant by default; `.transition(...)` adds smoothing.

```rust
use gpui::{div, rgb, ScrollHandle, prelude::*};
use gpui_ce_motion::{Axis, ScrollMotion, Target, native_scroll_progress};

fn playlist(handle: &ScrollHandle) -> impl IntoElement {
    let progress = ScrollMotion::new("progress", handle.clone(), Axis::Y,
        Target::default().width(0.), Target::default().width(220.),
        div().h_1().bg(rgb(0xd4f580)));
    div().child(progress).child(
        div().id("playlist").h_32().overflow_y_scroll().track_scroll(handle)
            .children((0..20).map(|i| div().h_8().child(format!("Track {i}"))))
    )
}
```

`native_scroll_progress(handle, axis)` exposes measured progress separately.
Portable helpers accept measured metrics directly; visibility bounds are
`[x, y, width, height]` in one coordinate space:

```rust
use gpui_ce_motion::{InView, scroll_progress, transform};

let progress = scroll_progress(150., 1000., 400.);
let offset = transform(progress, [0., 1.], [0., 200.], true);
let mut visibility = InView::new(0.5, true); // 50% visible; keep true after first entry.
assert!(visibility.update([0., 0., 400., 300.], [20., 20., 100., 100.]));
```

For automatic native visibility, initialize `GestureState::default().in_view(0.5,
true)` and bind `GestureTargets::while_in_view(...)`. The native adapter measures
clipped bounds. `InView::is_visible` reads the portable observer's latest result.

## Reordering

Use a retained `ReorderGroup` for lists or draggable tabs. Render in `order()` and
keep each item's IDs and key stable. Group items accept a `Stateful<Div>`.

```rust
use gpui::{div, prelude::*};
use gpui_ce_motion::{ReorderGroup, Axis};

fn tabs(group: &ReorderGroup<usize>) -> impl IntoElement {
    div().flex().children(group.order().iter().map(|&key| {
        group.item(("motion-tab", key), key,
            div().id(("tab", key)).w_32().h_10().child(format!("Tab {key}")))
    }).collect::<Vec<_>>())
}
let group = ReorderGroup::new(vec![0, 1, 2], Axis::X).expect("unique keys");
```

Choose `Axis::Y` for a vertical list. The group animates measured reflow and handles
pointer release outside the item, keyboard reorder, and Escape cancellation.
On macOS, keyboard reorder is **Option (⌥) + arrows**; elsewhere it is Alt + arrows.
A tab strip's active selection and panel content remain application state keyed by ID.

Use `on_change(|keys, window, cx| ...)` to persist a changed order; capture application
entities weakly. Call `set_order` for external model changes, not every render.
`is_dragging` reports pointer ownership. Removing focus and edge autoscroll belong
to the host. For custom bindings, `reorder_index` finds the nearest measured center
and `move_item` rearranges a slice while preserving identity.

## Layout positions

Retain one `LayoutGroup` and wrap elements with stable IDs. Change order, wrapping,
alignment, or parent layout normally; the group measures and animates positions.

```rust
use gpui::{div, px, prelude::*};
use gpui_ce_motion::LayoutGroup;

fn cards(group: &LayoutGroup, keys: &[usize]) -> impl IntoElement {
    div().flex().flex_wrap().gap_3().children(keys.iter().map(|&key| {
        group.position(("position", key),
            div().id(("card", key)).w(px(128.)).h(px(64.)).child(format!("Card {key}")))
    }).collect::<Vec<_>>())
}
```

`.transition(...)` changes timing; `.enabled(false)` snaps. Share the **same group**
through nested animated elements to avoid double movement. First mounts and size
changes are immediate; keyed history ends at unmount/reparenting. This translates
paint, hitboxes, and accessibility bounds, with ordinary ancestor clipping.
Avoid competing x/y motion on the same element. Portable `LayoutPositionState`
accepts measured origins via `sample`/`sample_axes`; its frame reports layout origin,
presented position, offset, velocity, activity, and completion. `reset` clears history.

## Popped exits

`PopLayout` removes outgoing items from flow immediately and retains their visible
position while their opacity exit plays. It owns positional reflow too.

```rust
use gpui::{div, AnyElement, prelude::*};
use gpui_ce_motion::{Frame, PopLayout};

fn row(pop: &PopLayout, frame: Frame, present: bool) -> AnyElement {
    let list = div().flex().gap_3().child(frame.render(|frame| {
        pop.item("item", present, frame.apply(div().w_32().h_16().child("Card")))
    }));
    pop.container("row", list).into_any_element()
}
```

Get `frame` from the retained child's `MotionState`, with an opacity exit and
`present` flag as in [enter and exit](#enter-and-exit). Keep the child until absent;
then call `PresenceGroup::finish_exit` if using keyed presence. Do not add another
`LayoutGroup::position` wrapper to the same item. For x/y exits, animate inner
content in a fixed-size wrapper. Input suppression and focus restoration are yours.

## Content sizing

`AutoSize` measures intrinsic content and animates its reserved size. Supply the
available width for wrapping text. It defaults to height; `SizeAxis::Width` or
`Both` selects other axes.

```rust
use gpui::{div, prelude::*};
use gpui_ce_motion::{AutoSize, SizeAxis};

let content = div().w_64().child("Content can change between renders.");
let panel = AutoSize::new("details", content)
    .width(256.)
    .axis(SizeAxis::Height);
```

Keep the element ID stable. `.height(...)` supplies a height constraint,
`.transition(...)` changes timing, and `.initial([width, height])` opts into
entrance. It clips paint/input while resizing. Paint an outer card around the
reservation if its background should follow the animated bounds. This is real
layout sizing, not subtree scaling or automatic ancestor constraint discovery.

## Shared visuals

Retain a `SharedLayout<K>` and declare exactly one active destination per shared
key in a common overlay. The outgoing position/crossfade belongs to that shared
identity; content lifetime and input policy remain explicit.

```rust
use gpui::{div, px, prelude::*};
use gpui_ce_motion::SharedLayout;

let shared = SharedLayout::<u32>::default(); // Retain this.
let old = shared.member(1, "old", false, div().absolute().left(px(0.)).child("Old"));
let new = shared.member(1, "new", true, div().absolute().left(px(200.)).child("New"));
let overlay = div().relative().child(old).child(new);
```

Members accept `.transition(...)`. Call `remove(&key)` when that identity is no
longer needed; `len`/`is_empty` inspect retained entries. Sizes change immediately,
normal clipping applies, and independent scrolling coordinate spaces are not
reconciled. This is position/crossfade, not full shared-element size projection.

## Style animation

Retain `StyleMotionState` for typed scalar/color tracks. Use `StyleTarget` for
individual corners, borders, spacing, one shadow, blur, and backdrop blur.
Colors are background, text, border, and shadow; they interpolate in linear light.

```rust
use gpui::{div, App, Window, prelude::*};
use gpui_ce_motion::{Color, ColorProperty, StyleMotion, StyleMotionState, StyleProperty, StyleTarget};

fn styled(state: &mut StyleMotionState, window: &mut Window, cx: &App) -> impl IntoElement {
    let target = StyleTarget::default()
        .padding([12.; 4]).margin([4.; 4]).gap(8., 8.)
        .radii([16., 4., 16., 4.]).border_widths([1.; 4])
        .color(ColorProperty::Background, Color::rgb(0x334466))
        .shadow([0., 4.], 12., 0., Color::rgba(0., 0., 0., 0.2))
        .value(StyleProperty::Blur, 0.);
    state.frame(StyleMotion::new(target), window, cx).apply(div().id("styled"))
}
```

Corners run top-left, top-right, bottom-right, bottom-left. Edge arrays run top,
right, bottom, left. `value` addresses individual `StyleProperty` channels,
including `BackdropBlur`. `initial`, `transition`, `transition_for`, and
`color_transition` set timing. Portable `sample` returns a `StyleFrame` with target,
activity, and completion. `StyleTarget::get`/`get_color` inspect declared channels.

Omitted styles remain unchanged until introduced, then retain their last target.
Reset the state to release overrides. Shadow/filter declarations replace their
lists. Spacing reflows layout; blur requires separate GPU profiling.

## SVG transforms

Retain `SvgMotionState`; apply its frame to a normal GPUI SVG. Keep the asset path
and base dimensions stable so CE can reuse its cached drawing.

```rust
use gpui::{svg, px, App, Window, prelude::*};
use gpui_ce_motion::{SvgMotion, SvgMotionState, SvgTarget};

fn icon(state: &mut SvgMotionState, window: &mut Window, cx: &App) -> impl IntoElement {
    let motion = SvgMotion::new(SvgTarget::default()
        .scale_xy(1.2, 1.2).rotate_degrees(90.).translate(8., 0.))
        .initial(SvgTarget::default());
    state.frame(motion, window, cx).apply(svg().path("icons/arrow.svg").size(px(24.)))
}
```

The app supplies the SVG asset through its normal `AssetSource`. `scale` sets both
axes; signed `scale_xy` allows mirroring. Angles are absolute and unwrapped:
`0 → 720` makes two turns, and `350 → 370` crosses zero by twenty degrees.
`transition`/`delay` set timing. `SvgFrame` exposes target, scale/rotation velocity,
translation velocity, activity, completion, and `transformation()`.
`transformation_about(size, pivot)` accepts a logical-pixel pivot for
`Svg::with_transformation`. Portable `sample` works without GPUI.

**Only the SVG drawing transforms.** Native children, layout, clipping, and hitboxes
do not scale or rotate. Use a fixed, sufficiently padded native interaction parent.
Large zooms can soften the cached raster drawing. Add outer ordinary motion for
opacity/presence; arbitrary GPUI subtree transforms remain deferred.

## Vector paths

Prepare absolute commands once. `Move`, `Line`, `Quadratic`, `Cubic`, and `Close`
are supported. A morph needs the same command sequence and coordinate topology.

```rust
use gpui_ce_motion::{Color, PathCommand, PathShape, StrokeReveal, VectorState};

let start = PathShape::new([
    PathCommand::Move([0., 40.]), PathCommand::Line([100., 40.]),
]).expect("valid path");
let end = PathShape::new([
    PathCommand::Move([0., 20.]), PathCommand::Line([100., 60.]),
]).expect("compatible path");
let vector = VectorState::new(start); // Retain this.
let drawing = vector.element("line", end, [120., 80.], Color::rgb(0xd4f580))
    .expect("valid bounds and topology")
    .stroke(3.)
    .reveal(StrokeReveal::new(0.7, 0., 1.).expect("valid reveal"));
```

Use `.fill()` for a filled path, `.transition(...)` for timing, and `VectorState::error`
for the most recent tessellation error. Reveal affects strokes only.
`StrokeReveal` specifies normalized length, offset in path lengths, and spacing;
`values()` reads them. At most 4096 reveal periods are accepted. Stroke curves
use 32 segments per curve; fills retain curve commands. SVG parsing, arcs, and
unrelated-shape normalization are outside this API.

For portable/custom rendering, `PathShape::commands`/`compatible` inspect shapes.
Retain `PathMorphState`, call `validate`/`sample`, then read the borrowed `PathFrame`'s
commands, length, activity, and completion. `stroke_segments(reveal, callback)`
emits line segments. Invalid topology does not partially mutate the sampler.

## Motion values

`MotionValue` handles a scalar in your own units. `set` records a directly driven
value and velocity; `jump` resets velocity. `animate`/`animate_delayed` follow a
target; `coast` applies bounded inertia. `get`/`velocity` read the latest sample.

```rust
use gpui_ce_motion::{Inertia, MotionValue, Transition, transform};
use std::time::Duration;

let mut value = MotionValue::new(0.);
value.animate(100., Transition::default(), Duration::ZERO);
let frame = value.sample(Duration::from_millis(100), false);
let opacity = transform(frame.value(), [0., 100.], [0., 1.], true);
value.coast(Inertia::new(Duration::from_millis(250)).bounds(0., 200.),
    Duration::from_millis(100));
```

`ValueFrame` reports value, velocity, activity, and completion. `Inertia::spring`
sets the boundary spring; `project` estimates its destination. Derive dependent
values in ordinary Rust in a defined sampling order; no subscription graph is installed.

## Colors

Colors are typed sRGB inputs with linear-light interpolation. Use `rgb(0xRRGGBB)`,
`rgba(r, g, b, a)` in `0..=1`, `channels()` for sRGB values, and `mix` for a direct blend.

```rust
use gpui_ce_motion::{Color, ColorState, Transition};
use std::time::Duration;

let mut color = ColorState::new(Color::rgb(0x334466));
let (current, active) = color.sample(Color::rgb(0xd4f580), Transition::default(),
    Duration::from_millis(100), false);
let native_color = current.to_rgba(); // Requires gpui.
```

Retain the state for interruptible color springs/tweens. `sample` returns the
sampled color and an activity flag. `to_rgba()` converts it to a GPUI color.

## Lifecycle and accessibility

`Frame`, `ValueFrame`, `SvgFrame`, `StyleFrame`, `LayoutPositionFrame`, and
`PathFrame` expose `is_animating()` and `just_completed()`. Completion is a sampled
edge, not a replay of every missed repeat; read `AnimationState::iteration()` for cycles.
`Frame` additionally reports ordinary/layout velocities and presence state.

Native adapters honor `App::reduce_motion()` and stop requesting frames after
settling. Portable APIs take a `reduced_motion` flag:

```rust
use gpui_ce_motion::{Motion, MotionState, Target};
use std::time::Duration;

let mut state = MotionState::default();
let frame = state.sample(Motion::new(Target::default().x(100.))
    .initial(Target::default()), Duration::ZERO, true);
assert_eq!(frame.target().x_px(), 100.);
assert!(!frame.is_animating());
```

Reduced motion completes even exits and infinite clips; a completed clip stays
complete until restart/replacement. The app synchronizes OS preferences, notifies
on input changes, restores focus on removal, and scopes keyboard shortcuts around
text editing. No per-animation task, global registry, or platform bootstrap is added.

## Validation

| Error type | Returned by |
| --- | --- |
| `InvalidSpring` | Invalid physical parameters, visual timing, or settling tolerances |
| `InvalidEasing` | Invalid Bézier controls or custom easing preparation |
| `AnimationError` | Invalid keyframes/times, repeats, speed, labels, keys, or indices |
| `PathError` | Invalid path geometry/topology, reveal, drawing bounds, or tessellation |

These implement `std::error::Error`. Static recipe inputs use `expect` for brevity;
propagate errors when values come from users. Basic numeric builders such as
`Target::width` assert their contracts: pass finite values, nonnegative dimensions,
and opacity within `0..=1`. See the linked source declarations for each constructor's details.

## Public export index

Every root export is listed below. Links open its declaration in the tagged
0.1.0 source, with methods in the same module. Native-only types require `gpui`.
Element wrapper/frame types are usually returned by higher-level APIs.

For generated method-level docs, run `cargo doc --no-deps --features gpui --open`.
See [documentation build status](DEVELOPMENT.md#api-documentation) for the hosted
0.1.0 reference.

| Group | Public exports | Required feature |
| --- | --- | --- |
| motion | [`Frame`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/motion.rs#L257), [`Motion`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/motion.rs#L152), [`MotionState`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/motion.rs#L333), [`Presence`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/motion.rs#L244), [`Property`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/motion.rs#L226), [`Target`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/motion.rs#L16) | None |
| transition | [`Bezier`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/transition.rs#L295), [`CustomEasing`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/transition.rs#L106), [`Easing`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/transition.rs#L6), [`InvalidEasing`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/transition.rs#L342), [`InvalidSpring`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/transition.rs#L265), [`Spring`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/transition.rs#L137), [`Transition`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/transition.rs#L278) | None |
| value | [`Inertia`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/value.rs#L7), [`MotionValue`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/value.rs#L94), [`ValueFrame`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/value.rs#L58) | None |
| animation | [`Animation`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/animation.rs#L95), [`AnimationError`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/animation.rs#L10), [`AnimationState`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/animation.rs#L423), [`Keyframe`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/animation.rs#L35), [`Playback`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/animation.rs#L355), [`Repeat`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/animation.rs#L65), [`RepeatMode`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/animation.rs#L78), [`StaggerFrom`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/animation.rs#L552), [`Timeline`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/animation.rs#L586), [`stagger`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/animation.rs#L561) | None |
| color | [`Color`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/color.rs#L6), [`ColorState`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/color.rs#L66) | None |
| interaction | [`Drag`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/interaction.rs#L99), [`GestureTargets`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/interaction.rs#L48), [`InView`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/interaction.rs#L299), [`Interaction`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/interaction.rs#L34), [`Variants`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/interaction.rs#L7), [`move_item`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/interaction.rs#L368), [`reorder_index`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/interaction.rs#L352), [`scroll_progress`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/interaction.rs#L284), [`transform`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/interaction.rs#L266) | None |
| presence | [`PresenceChild`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/presence.rs#L15), [`PresenceGroup`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/presence.rs#L32), [`PresenceMode`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/presence.rs#L6) | None |
| svg | [`SvgFrame`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/svg.rs#L103), [`SvgMotion`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/svg.rs#L68), [`SvgMotionState`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/svg.rs#L136), [`SvgTarget`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/svg.rs#L11) | None |
| layout | [`LayoutPositionFrame`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/layout.rs#L8), [`LayoutPositionState`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/layout.rs#L46) | None |
| layout native | [`LayoutGroup`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/layout_native.rs#L14), [`LayoutPositionElement`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/layout_native.rs#L51) | `gpui` |
| style motion | [`ColorProperty`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/style_motion.rs#L62), [`StyleFrame`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/style_motion.rs#L233), [`StyleMotion`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/style_motion.rs#L192), [`StyleMotionState`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/style_motion.rs#L255), [`StyleProperty`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/style_motion.rs#L8), [`StyleTarget`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/style_motion.rs#L78) | None |
| measured size | [`AutoSize`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/measured_size.rs#L24), [`SizeAxis`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/measured_size.rs#L11) | `gpui` |
| pop layout | [`PopElement`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/pop_layout.rs#L65), [`PopLayout`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/pop_layout.rs#L14), [`PopRequest`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/pop_layout.rs#L96) | `gpui` |
| gesture native | [`GestureElement`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/gesture_native.rs#L119), [`GestureState`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/gesture_native.rs#L58) | `gpui` |
| scroll native | [`Axis`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/scroll_native.rs#L10), [`ScrollMotion`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/scroll_native.rs#L36), [`native_scroll_progress`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/scroll_native.rs#L18) | `gpui` |
| shared layout | [`SharedElement`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/shared_layout.rs#L72), [`SharedLayout`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/shared_layout.rs#L22) | `gpui` |
| reorder native | [`ReorderElement`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/reorder_native.rs#L124), [`ReorderGroup`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/reorder_native.rs#L44) | `gpui` |
| vector | [`PathCommand`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/vector.rs#L6), [`PathError`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/vector.rs#L48), [`PathFrame`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/vector.rs#L302), [`PathMorphState`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/vector.rs#L217), [`PathShape`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/vector.rs#L69), [`StrokeReveal`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/vector.rs#L112) | None |
| vector native | [`VectorElement`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/vector_native.rs#L73), [`VectorState`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/vector_native.rs#L19) | `gpui` |
