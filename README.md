# gpui-ce-motion

An experimental native Rust motion crate: declarative targets, duration/easing
transitions, physical springs, enter/exit presence, and interruptible retargeting.
Animate x/y, opacity, width, height, and border radius using stock GPUI CE APIs.
Prepared keyframes, playback controls, timelines, stagger, variants, gestures,
scalar/color values, drag inertia, scroll mapping, and keyed presence are included.
SVG-only GPU scale and rotation use CE’s existing SVG drawing transform.
Automatic position-only layout animation measures native elements after layout.
Native gesture/scroll/reorder bindings, popped exits, measured content sizing,
shared position/crossfade, style tracks, and compatible vector morph/reveal are included.
No React, JavaScript runtime, async executor, or per-animation tasks.

Source repository: [1bye/gpui-motion](https://github.com/1bye/gpui-motion).
Crate name: `gpui-ce-motion`. Version 0.1.0 is prepared but not published.
Rust 1.95+, edition 2024, MIT OR Apache-2.0. The default feature set has no
third-party dependencies. Enable `gpui` for the native adapter. It is tested
against registry `gpui-ce =0.2.2`. The original app also tested official CE
revision `094fe979fc4bd9a7dd4544d75b147a9278afcecf`; this repository uses registry dependencies only.

## Native usage

Keep `MotionState` in your GPUI view, one per logical object. Construct the
`Motion` declaration each render; retain the same state through removal.

```rust,ignore
use gpui::{div, prelude::*};
use gpui_ce_motion::{Motion, MotionState, Target, Transition};

// Persistent view field, initialized once:
// panel: MotionState,

// Inside Render::render, after model updates call cx.notify():
let frame = self.panel.frame(
    Motion::new(Target::default()
        .width(if self.expanded { 320.0 } else { 240.0 })
        .height(160.0).border_radius(12.0))
        .initial(Target::default().width(0.0).height(160.0).border_radius(0.0).opacity(0.0))
        .exit(Target::default().width(0.0).opacity(0.0))
        .transition(Transition::default()) // settling spring
        .present(self.open),
    window,
    cx,
);
frame.render(|frame| frame.apply(div().overflow_hidden())
    .child(div().w(gpui::px(320.0)).child("Panel")))
```

Use `Transition::Tween(Duration::from_millis(250), Easing::EaseInOut)` for a
finite duration, `Transition::Spring(Spring::new(240.0, 26.0, 1.0)?)` for explicit
physical parameters, or `Transition::Instant` to snap. Stiffness, damping, and
mass must be finite and positive. Targets accept finite logical pixel offsets
and opacity in `0..=1`. Width, height, and radius must be finite and non-negative.
Invalid builder values are programmer errors and panic.

The default first mount is immediate. `initial` opts into entrance and is used
once per state. `present(false)` starts exit; `Frame::render` omits the element
only when exit completes. Continue declaring it while false. Re-entry during
exit uses the current presentation and velocity. Drop the state to cancel it;
create a new one to reset mount history. In a list, key states by domain ID,
retain removed items until their exit becomes `Absent`, then remove them.

Retargeting first advances the old trajectory to the current time. Springs
preserve both position and velocity; tweens preserve position but restart their
easing and may change velocity. Changing the transition also interrupts.
The closed-form spring handles under-, critical-, and over-damping without
frame-dependent integration or substep loops. It settles at 0.001 channel units plus a frequency-scaled velocity tolerance. Spring opacity may
overshoot internally; drawable opacity is clamped while momentum is retained.

`frame` must run inside a GPUI render/element drawing phase. It reads
`App::reduce_motion()` and requests frames only while animating. The application
owns synchronization of OS preferences into that flag; this crate does not
install an OS preference observer. Reduced motion snaps and completes exits.
Model changes still require `cx.notify()`. No hidden animation worker runs when
a window is occluded; the next sample accounts for elapsed monotonic time.

## Layout transitions

`Target::width`, `height`, and `border_radius` accept logical pixels, including
zero. Their readers return `Option<f32>` so an undeclared property is distinct
from zero. `Frame::apply` leaves undeclared sizes and radii untouched. Once a
layout channel has been introduced, omitting it from later declarations retains
its previous destination; partial exits can fade out without losing their size.
Reset `MotionState` to release those overrides and return to the element's styles.
The existing `velocity()` reader keeps its x/y/opacity contract;
`layout_velocity()` returns optional width/height/radius velocities.

An omitted initial dimension inherits the target dimension. If a size first
appears later, it snaps to that value: the sampler cannot infer `auto`, percent,
or content-measured sizes. To animate an entrance, provide numeric `initial`
and destination values, such as width `0` to `320`. Springs may undershoot zero
internally; drawable sizes/radius clamp to zero while momentum is retained.

Width and height trigger ordinary GPUI layout and can reflow siblings and text.
For sidebar reveals, animate an outer container with `overflow_hidden()` and
keep its inner content at a fixed width. Radius applies to all four corners,
does not automatically clip children, and follows GPUI's normal corner painting.
Flex shrink, min/max dimensions, padding, and borders still constrain resolved
layout; use `flex_shrink_0()` on a panel that must reserve its sampled width.

`Frame::apply` applies declared sizes/radius, opacity, and **relative left/top
layout insets** to a styled element.
It replaces its position mode and left/top. Use an outer absolute container for
absolute positioning. The sampled layout dimensions remain active through exit.
These offsets are not GPU transforms, and they do not project sibling layout changes. `render`
does not manage focus or suppress input during exit: move focus out before
removal and guard handlers using `frame.is_present()`.

## Automatic layout positions

Enable `gpui` and retain one `LayoutGroup` in the view. Wrap each styled element
with a stable domain ID; no numeric destinations or measurement callbacks are needed:

```rust,ignore
use gpui::{div, prelude::*, px};
use gpui_ce_motion::LayoutGroup;

// View field, initialized once: positions: LayoutGroup
let mut row = div().flex().flex_wrap().gap_3().w(px(self.board_width));
for item in &self.items {
    let card = div().id(("card", item.id)).w(px(128.)).h(px(64.)).child(item.name.clone());
    row = row.child(self.positions.position(("position", item.id), card));
}
// Change board_width, item order, alignment, or membership, then cx.notify().
```

The wrapper measures the resolved origin in CE's prepaint phase and applies a
translation before children register paint bounds, pointer hitboxes and accessibility
bounds. It adds no layout node and preserves the child's flex/style behavior.
Sibling reflow, parent movement and flex wrapping are followed automatically.
Springs are the default; `.transition(Transition::Tween(duration, easing))` selects
a tween. Interruptions use the current presentation; springs also retain velocity.
Only active animations request another frame. The native executor's monotonic
clock drives sampling, including catch-up after an occlusion.

Use the **same retained group for nested animated elements** so child positions
are measured relative to their closest animated ancestor. This avoids applying a
parent movement twice. Independent groups must not nest. IDs must be unique under
the enclosing CE ID and stable through reordering. History belongs to CE's keyed
element state, survives ordinary rerenders, and ends on unmount or reparenting to
a different CE ID scope. Newly mounted/virtualized items snap to their first
measured position. The group does not provide cross-parent/shared-element identity.

Sizes change immediately; siblings use the final layout while presentation moves
toward it. Scroll offsets and manual prepaint offsets pass through immediately.
Normal ancestor clipping still applies. This uses stock prepaint translation,
without a renderer patch, subtree scale/rotation, or size projection. Configure
styles and handlers on the original element before wrapping. Avoid independently
animating x/y on the same element with `MotionState`; those changing layout insets
would become new measured destinations. Opacity/color and explicit size transitions
can be composed on the child.

`.enabled(false)` and `App::reduce_motion()` snap immediately and stop scheduling
frames. The host still owns OS-preference synchronization and normal model-change
notifications. `LayoutPositionState` exposes the allocation-free portable sampler
for tests or custom measurement adapters; it does not measure native layout itself.

## Prepared clips, controls, and interaction

Prepare clips once, retain playback state in your view, and sample during render:

```rust
use gpui_ce_motion::{Animation, AnimationState, Easing, Repeat, RepeatMode, Target};
use std::time::Duration;
let clip = Animation::to(Target::default().x(120.0), Duration::from_millis(600), Easing::EaseInOut)
    .repeat(Repeat::Forever, RepeatMode::Mirror, Duration::from_millis(100))?;
let mut state = AnimationState::new(Target::default());
state.sample(&clip, Duration::ZERO, false);
let now = Duration::from_millis(300);
state.pause(now);
let held = state.sample(&clip, now, false);
assert_eq!(held.target().x_px(), 60.0);
assert!(!held.is_animating());
# Ok::<(), gpui_ce_motion::AnimationError>(())
```

With the native feature, `state.frame(&clip, window, cx)` samples/schedules the
clip, and `state.now()` gives controls the matching clock. Otherwise use one
application `Instant` and call `window.request_animation_frame()` only while
samples are active. Notify the view after input/control changes. `Animation`
clones share prepared keyframes; constructing a new clip intentionally interrupts.
Physical spring clips preserve velocity; finite clips use an explicit deadline.

`Motion::delay` and `transition_for` configure property timing independently.
`Keyframe::current` captures the starting presentation or holds a preceding
keyframe; `Animation::keyframes` validates normalized times. `AnimationState`
provides pause/play, seek, speed, restart, finish, completion edges and repeat
iteration. `Timeline` supports append, overlap, labels, and group playback;
`stagger` computes child delays without a scheduler or per-child task.

`Variants<K>` resolves named `Motion` templates. Use `GestureTargets::resolve`
on the template's target, then `Motion::with_target` to retain its timing policy.
Pass variants to children explicitly. `Interaction` is supplied by normal GPUI
hover, pointer, and focus handlers; the library does not install event listeners.
`Drag` provides bounded motion, elasticity, snap grid, inertia, regrab, cancel,
and `move_by` for keyboard access. The host owns capture, release outside the
hitbox, cancellation on lost focus, and accessible control semantics.

Use `MotionValue` for other scalar properties, velocity, direct set/jump,
spring following, and bounded `Inertia`. `ColorState` samples interruptible
linear-light colors (`Color::to_rgba` with `gpui`). `transform` maps derived values;
`scroll_progress` and `InView` consume measured native scroll/layout metrics.
`PresenceGroup<K>` retains outgoing keys in sync/wait modes until the owner
acknowledges each completed exit. `reorder_index` and `move_item` preserve stable
list identity; `LayoutGroup::position` follows the resulting native reflow automatically.

See [COVERAGE.md](COVERAGE.md) for exact native equivalents, ownership contracts,
and exclusions. Scale/rotation for general GPUI subtrees and full size/shared-element layout
projection and general renderer transforms remain deferred. Browser-specific
features are not emulated.

## Native integration APIs

Enable `gpui`; normal consumers do not need `native-tests`. The integration
handles are retained view fields, not freshly created inside `render`.

| API | Retained owner and behavior |
| --- | --- |
| `GestureState::bind(id, control, motion, targets)` | Supplies hover/press/focus, optional bounded `Drag`, stable keyboard focus, arrow-key movement, outside release, lost-button/Escape/blur/deactivation cancellation. `in_view(amount, once)` measures clipped visibility. `.enabled(false)` cancels input and removes tab stops; the owner restores focus and supplies disabled accessibility metadata on the child. |
| `ScrollMotion::new(id, handle, axis, from, to, child)` | Maps native scroll offsets/extents into partial targets; instant by default, `.transition(...)` smooths. Resize requests a follow-up measurement frame. |
| `ReorderGroup::new(keys, axis)` | Owns a single-axis list's order/geometry. Build `group.item(id, key, card)` from `group.order()`. Pointer or Alt+Arrow changes order; Escape/deactivation cancels a pointer gesture. `on_change` runs after a changed commit without borrowing the group. |
| `PopLayout::{container,item}` | Retains measured container/item coordinates. `item(id, present, child)` detaches exiting content from flow while normal presence owns its fade/lifetime. Owns positional reflow and freezes the visible exit, so restoring during exit starts at that position. Borders, padding and flex gaps do not shift the exit. |
| `AutoSize::new(id, content)` | Measures intrinsic content, reserves/interpolates its height and clips input/paint. `.axis(SizeAxis::Width/Both)` selects other axes, `.width/.height` supply available constraints, `.initial` opts into entrance. |
| `SharedLayout::member(key, id, active, child)` | A view-owned common-overlay position/crossfade registry. Exactly one active destination per shared key. Outgoing content/lifetime/input/focus remain explicitly owned; sizes are immediate. Call `remove` when done. |

Native bindings use CE frame-scoped listeners and the owning view's invalidation,
not per-control tasks. They support desktop mouse and keyboard, not OS-wide capture
or multi-touch. `ReorderGroup` owns ordering; persist changes in `on_change` and use
`set_order` for external membership changes, not every render. The host owns
removal focus restoration and scrolling/autoscroll. Do not capture a strong clone
of the group in its own callback; use the payload or weak application entities.

For popped exits, retain children with `PresenceGroup` and `MotionState` as before.
Wrap them with `pop.item(id, present, child).transition(transition)` and place the
list inside `pop.container(id, list)`. Items own positional reflow; do not add a
second `LayoutGroup::position` wrapper. For x/y exit motion, animate inner content
inside a fixed-size item child. After `Frame::is_mounted()` becomes false,
acknowledge the exit and drop that child. Disable input/tab stops/accessibility for
outgoing controls and move focus before removal. `PopLayout` adds no input policy.

`AutoSize` measures a separate content root before parent layout, so supply width
for wrapping text. It does not discover arbitrary parent constraints. Its sizes
cause real layout; other dimensions snap without unnecessary animation frames.
Shared overlays preserve ordinary ancestor clipping and do not reconcile independent
scrolling coordinate spaces. They do not provide arbitrary subtree scale projection.

## Style tracks and spring controls

`StyleMotionState`/`StyleMotion` mirror retained-state/declarative ownership for
`StyleTarget`: background/text/border/shadow colors, separate corner radii, border
widths, padding/margins/gaps, one outer shadow and Gaussian blur/backdrop blur.
Colors interpolate in linear light. Scalar/color transitions can be overridden
per property. Omitted tracks preserve existing styles until introduced, then retain
their last destination; reset the state to release those overrides.

```rust
use gpui_ce_motion::{Color, ColorProperty, StyleMotion, StyleMotionState, StyleTarget};
use std::time::Duration;
let mut state = StyleMotionState::default();
let target = StyleTarget::default().padding([12.; 4]).radii([4., 16., 4., 16.])
    .color(ColorProperty::Background, Color::rgb(0x334466));
let frame = state.sample(StyleMotion::new(target), Duration::ZERO, false);
assert!(!frame.is_animating());
```

With `gpui`, `state.frame(declaration, window, cx).apply(styled_element)` schedules
native redraws. The style adapter replaces shadow/filter lists when declared.
Layout tracks reflow; blur/backdrop blur need offscreen GPU rendering and should
be profiled separately from sampling or CPU preparation.

Prepare `Spring::visual(duration, bounce)?` once to choose damping through bounce
(`-1..1`, excluding 1). Our convention reaches 90% of a resting step for the first
time at the requested duration; it is not Motion's exact visual-duration solver.
`spring.rest(distance, speed)?` sets positive finite settling thresholds. Existing
physical `Spring::new` and finite-deadline clips keep their contracts.
`CustomEasing::new(fn(t) -> (value, derivative))?` prepares a copyable custom easing
with stable identity. Its callback must return finite values throughout `[0,1]`.

## Vector path drawing and morphing

Prepare `PathShape` from typed absolute `PathCommand::{Move,Line,Quadratic,Cubic,Close}`
commands. `PathMorphState` samples matching topology with per-control-point spring
momentum and no sampling allocation. Invalid geometry/topology returns `PathError`
without partially changing the sampler. Unrelated topologies, SVG text parsing and
arc commands are not normalized automatically.

Retain native `VectorState::new(initial_shape)` and call
`state.element(id, target_shape, [width, height], color)?`, then `.stroke(width)` or
`.fill()`, `.transition(...)`, and `.reveal(StrokeReveal::new(length, offset, spacing)?)`.
Length/spacing are fractions in `0..=1`; offset shifts in total path lengths. The
finite pattern budget permits at most 4096 periods. Stroke length/reveal approximate
curves with 32 equal-parameter segments; fills preserve exact curve commands.
Native tessellation/paint can allocate; `VectorState::error` exposes tessellation
failure. This draws native vector geometry without mutating SVG assets each frame.

## SVG scale and rotation

Retain `SvgMotionState` alongside each logical SVG.
`SvgTarget::translate(x, y)` adds interruptible drawing translation;
`SvgFrame::translation_velocity()` reads its velocity independently of the existing
three-value scale/rotation velocity reader. `transformation_about(bounds_size, pivot)`
accepts a logical-pixel pivot from the SVG’s top-left and compensates for CE’s
center origin. Apply it with `Svg::with_transformation`; native layout/hitbox stay fixed.
 Its first mount is immediate
unless `initial` is supplied; subsequent target changes interrupt the current
presentation, with spring velocity preserved on all three channels.

```rust,ignore
use gpui::{svg, prelude::*, px};
use gpui_ce_motion::{SvgMotion, SvgMotionState, SvgTarget};

// Persistent view field: icon_motion: SvgMotionState
// Inside Render::render:
let frame = self.icon_motion.frame(
    SvgMotion::new(SvgTarget::default().scale(1.3).rotate_degrees(90.0))
        .initial(SvgTarget::default()),
    window, cx,
);
frame.apply(svg().path("icons/arrow.svg").size(px(24.0)))
```

The SVG asset is supplied by the application's ordinary CE asset registry/source.
The library adds no asset loader, platform bootstrap or dependency. Prefer a
stable asset path and fixed base dimensions while animating, so CE can reuse its
raster cache; rebuilding SVG data or changing raster dimensions has separate costs.

`SvgTarget::scale_xy(x, y)` supports independent axes, negative scale for
mirroring, and zero for collapse. `rotate_degrees` is absolute and unwrapped:
`0 -> 720` travels two full turns; `350 -> 370` travels twenty degrees across
zero. The adapter reduces only the drawing angle modulo one turn. It never
changes the retained angle/velocity or automatically chooses a shortest path.
`SvgMotion::transition` supports existing spring/tween/instant timing; `delay`
holds each new destination. Pure `sample` needs no GPUI dependency. Native `frame`
reads the app's reduced-motion flag and requests frames only while active.

`SvgFrame::apply` accepts only a CE `Svg`, replacing its drawing transformation.
Scaling occurs before rotation around the SVG bounds center. Its **layout,
hitbox, and surrounding native text/children do not transform**; clipping remains
CE's existing axis-aligned clipping. Compose with ordinary motion on an outer
container for opacity/presence. Put interaction on a sufficiently padded fixed
native parent, supply keyboard access, and label the native control. This is not
a transformed native button or a container for GPUI children. The SVG's own shapes
and SVG text transform together as a monochrome cached drawing; large zooms may
soften it. Arbitrary native subtree transforms and layout projection stay deferred.

## Demo and verification

From this repository (macOS needs Xcode command-line tools):

```sh
cargo run --features demo --example motion
cargo run --features demo --example sidebar
cargo run --features demo --example motion_features
cargo run --features demo --example svg_motion
cargo run --features demo --example layout_motion
cargo run --features demo --example motion_native
cargo test -p gpui-ce-motion --no-default-features
cargo test -p gpui-ce-motion --features gpui
cargo bench -p gpui-ce-motion --bench sampling
cargo test -p gpui-ce-motion --features native-tests
cargo bench -p gpui-ce-motion --features native-tests --bench layout_native
cargo bench -p gpui-ce-motion --features native-tests --bench advanced_native
cargo run --release --features demo --example gpui_eval -- 400 8 baseline
cargo run --release --features demo --example gpui_eval -- 400 8 motion
cargo run --release --features demo --example gpui_eval -- 400 8 layout-baseline
cargo run --release --features demo --example gpui_eval -- 400 8 layout
cargo run --release --features demo --example gpui_eval -- 400 8 keyframes-baseline
cargo run --release --features demo --example gpui_eval -- 400 8 keyframes
cargo run --release --features demo --example gpui_eval -- 400 8 svg-baseline
cargo run --release --features demo --example gpui_eval -- 400 8 svg
cargo run --release --features demo --example gpui_eval -- 400 8 position-baseline
cargo run --release --features demo --example gpui_eval -- 400 8 position
```

The demo's native Motion menu and keyboard shortcuts exercise show/hide (Space),
mid-flight movement reversal (R), width/height/radius changes (W), spring/tween switching (T), and reduced motion (M).
It requests no animation frames after settling. Command+W/Q work on macOS;
Ctrl+W/Q on other desktop targets. The standalone demo belongs to the consumer
app so the library does not depend on platform bootstrapping.

The `sidebar` example is a complete app shell with pointer/keyboard navigation,
a collapsing sidebar, and content reflow. It derives the reserved sidebar width
from `Target::width` on a clipping container and uses `Frame::render` to retain
the fixed-width inner panel through exit.
Command/Ctrl+B toggles it; Escape closes it and returns focus when necessary.
Its View menu switches transitions and reduced motion; Command/Ctrl+Shift+T
and Command/Ctrl+Shift+M provide keyboard equivalents. Controls stop accepting
input during exit. No library API changes are needed for this composition.

The `motion_features` gallery demonstrates prepared keyframes, mirrored repeats,
stagger/timeline controls, variants and partial gesture targets, color springs,
bounded pointer/keyboard drag, keyed wait-mode replacements, reorder springs,
and real scroll-linked/in-view updates. Shortcuts appear in the window.

The `svg_motion` example shows SVG shapes/text rotating and scaling together,
mirroring, full turns, interruption, spring/tween timing and reduced motion. Its
fixed native stage remains the pointer/keyboard target throughout the animation.

The `layout_motion` demo demonstrates automatic wrapping, reordering, insertion/removal
and alignment changes, interrupted springs/tweens, pointer/keyboard activation, and
reduced motion. C/R/A/J change layout, T changes timing, M reduces motion, and B
disables interpolation. First mounts and sizes remain immediate.

The `motion_native` gallery exercises the integrated APIs in six pages. G toggles
the current example, R removes/restores an exit, T switches timing, M toggles reduced
motion, Tab navigates controls, and Alt+Arrow reorders a focused list item.

The pure benchmark measures style tracks, vector morph/reveal, automatic position sampling, SVG transforms, prepared playback, color, inertial drag, settled-state baseline, interrupted tweens, and
interrupted springs over 25 batches of 1,000 objects × 240 samples. It excludes
GPUI layout/paint, allocations for elements, and GPU execution. The native probe
compares the same 400-card scene, with a direct sinusoidal-offset baseline versus
400 independent spring samplers retargeted every 200 ms. The additional `layout`
mode animates width, height, and radius through `Frame::apply`; `layout-baseline`
uses one shared sampler to produce identical geometry for all 400 cards. The
keyframes modes compare shared and independent prepared players with the same
geometry. SVG modes use five seconds of font/raster-cache warmup and compare one
shared SVG sampler with 400 independent samplers drawing identical cached graphics.
Position modes compare direct reflow with automatic measured movement, using five
seconds of warmup. Their view-construction timer excludes the wrapper’s prepaint
work. The opt-in `layout_native` benchmark measures full CPU view construction,
Taffy layout, prepaint and paint preparation on CE TestPlatform instead. Its
`native-tests` feature enables CE test support only for these development checks;
normal consumers enable just `gpui`.
All probe modes force redraws for measurement. Keep the window visible and active; occluded windows can pause.
Probe output includes construction costs, active intervals, and maximum gaps.
These are render callbacks, not screen scanout or GPU completion timestamps.

Measured results and their scope are recorded in [PERFORMANCE.md](PERFORMANCE.md).

## Publishing

See [PUBLISHING.md](PUBLISHING.md) for package validation and the manual release
procedure. No automated workflow publishes this crate.

The library uses registry `gpui-ce =0.2.2`; the optional `demo` feature adds
`gpui_ce_platform =0.1.0` to run native windows. Consumers normally enable just
`gpui`. Keep one CE source/version across your application and UI dependencies.
The default feature set remains dependency-free.

This library and its demos were extracted from 1keep at commit `0cb95d0`.
The original MIT/Apache-2.0 license notices are retained.

## Recording showcase

```sh
cargo run --release --features demo --example motion_showcase
cargo test --example motion_showcase --features demo,native-tests
```

The **Motion studio** example is a compact, dark-and-lime recording canvas with
seven interactive scenes and a guided tour that pauses for hands-on scrolling.
The borderless playground shows the current experiment number, Play/Pause button,
and seven numbered tiles.
Click a tile to switch experiments: a spring-driven highlight follows the
selection, and the new scene fades and slides in. Reduced motion skips both
animations. Keyboard controls remain available: **R** starts a fresh take,
**P** pauses/resumes, **Left/Right**
selects a scene, and **G** reverses its target (including mid-animation). **M**
toggles reduced motion, which pauses the tour and snaps the animations. Switching
away from the window pauses the tour, too.

| Experiment | Scene | Features on screen |
| --- | --- | --- |
| 01 | Spring to life | Interrupted springs, stagger, size/radius transitions, SVG scale/rotation |
| 02 | Room to grow | Measured content height, padding, asymmetric corners, color and shadow |
| 03 | Make an exit | Enter/exit opacity, popped exits and automatic position reflow |
| 04 | Stay in control | Animated reorder; manually drag the handle for constraints, inertia and snapping |
| 05 | Keep the connection | Shared position/crossfade and manual scroll-linked progress |
| 06 | Draw the feeling | Compatible path morph, stroke reveal and repeating keyframes |
| 07 | Make it yours | Draggable tabs, spring reorder and animated selected content |

For a video, record just the window at its default size and press R. Each automatic
scene runs for six seconds. Experiment 05 pauses the tour: scroll the playlist
with a wheel/trackpad or the up/down buttons, then click **Continue** (or press P)
to resume at 06. Selecting 05 and clicking Play also enters this exploration mode.
Your scroll position stays under your control throughout.

Select 07 to drag tabs into a new order; the selected content follows the tab's
identity. Tab navigates controls, Enter/Space selects a focused tab, Alt+Left/Right
reorders it, and Escape cancels an active drag. Interacting with tabs pauses the
automatic tour. Scene 04 still supports dragging the handle and tracks, with
Alt+Up/Down to reorder a focused track. Command/Ctrl+W closes the window.

Experiment 06 rests on a complete wave; the guided sequence retracts and reveals
the stroke while morphing it. The dot is a looping keyframe indicator. The native
regression tests cover card-size continuity, stable reflow anchoring, dot alignment,
wave tessellation, manual scroll handoff, and pointer/keyboard tab interactions
using CE's test platform; they do not measure GPU presentation.

This is a visual tour of the main feature families, not every configuration or
API method. The galleries above cover more controls and the coverage matrix
records remaining stock-CE limits. Everything here uses the existing crate and
stock GPUI CE, with no new dependencies or renderer patches.

The published CE version lacks the newer `aria_disabled`/`aria_hidden` builders.
The sidebar and exit gallery remove keyboard stops, guard activation, and describe
closing controls as unavailable; outgoing content remains in the accessibility
tree until its exit finishes. The showcase retains named controls and tab roles.
