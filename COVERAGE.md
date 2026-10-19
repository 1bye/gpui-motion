# Motion-like API coverage

This is a native Rust API inspired by Motion, using stock GPUI CE and GPUI Kit.
It is not a
source-compatible port of Motion's React/JavaScript APIs or a claim of complete
Motion feature parity. Retained owners, explicit clocks, typed keys, and GPUI
input handlers replace hooks, DOM observation, selectors, and component context.

| Motion concept | Native implementation | Boundary |
| --- | --- | --- |
| Declarative animate / initial / exit | `Motion`, `MotionState`, `Target` | x/y offsets, opacity, numeric width/height/radius |
| SVG drawing transforms | `SvgTarget`, `SvgMotion`, `SvgMotionState`, `SvgFrame::{apply,transformation_about}` | Signed scale, unwrapped degrees, x/y drawing translation, center/custom pixel pivot; unchanged native layout/hitbox |
| Tween, easing, spring, interruption | `Transition`, `Spring`, `Easing`, `Bezier`, `CustomEasing` | Analytic physics; explicit derivative for custom easing; spring interruptions retain position and velocity |
| Visual spring controls | `Spring::visual`, `Spring::rest` | Native first-90%-arrival timing, damping through bounce, independent distance/speed settling thresholds; not Motion’s exact solver |
| Declarative native styles | `StyleTarget`, `StyleMotion`, `StyleMotionState`, `StyleFrame::apply` | Partial colors, per-corner radii, border widths, padding/margins/gap, one outer shadow, blur/backdrop blur; typed pixel/color tracks, no CSS string parser |
| Per-value transitions and delays | `Motion::transition_for(Property, transition, delay)` | Six typed target properties; other values use `MotionValue` |
| Keyframes and times | Prepared `Animation::keyframes`, `Keyframe` | Strictly increasing normalized times, per-segment easing; first current captures presentation, later current holds previous target |
| Delay, repeat, repeat delay | `Animation::delay`, `repeat` | Finite/forever; loop, reverse, mirror; absolute playhead skips missed cycles without a loop |
| Finite duration spring | `Animation::spring` | Explicit physical spring plus deadline; endpoint snaps at the deadline; `Spring::visual` is a separate native timing convention |
| Playback controls | `AnimationState`, `Playback`, `Timeline` | Pause/play/stop (pause alias), seek, restart, speed including negative; finish on `AnimationState` |
| Lifecycle edges/futures | `Frame::just_completed`, `ValueFrame::just_completed`, repeat iteration | Graph subscriptions dispatch after batched sampling; `Completion` futures resolve on completion/cancellation; host owns the executor |
| Named / dynamic variants | `Variants<K>`, `Motion::with_target`, `Target::overlay` | Enum/string keys; `MotionTree` handles inheritance, before/after gates, explicit defaults, dynamic nodes and nested exits |
| Stagger and sequences | `stagger`, `Timeline::{at,then,label,at_label}` | `TypedTimeline` composes scalar, color, style, SVG and text channels; signed overlaps, explicit labels, `MotionTree` child gates |
| AnimatePresence | `Motion::present`, `PresenceGroup<K>` | Sync/wait, keyed exit retention, interrupted reentry; owner acknowledges completed exits and manages child state/focus |
| Hover, tap/press, focus, drag, in-view targets | `Interaction`, `GestureTargets`, native `GestureState::bind` | Automatic native handlers, focus/tab stop, cancellation, disabled binding, clipped viewport measurement; partial target precedence; mouse/keyboard and native trackpad pinch; Kit recognized touch drag; CE raw touch pending |
| Drag, constraints, elasticity, momentum, snapping | `Drag`, `Inertia` | Measured anchors or numeric bounds, external handles, direction locking, structured callbacks, elastic resistance, exponential inertia and spring bounce, snap grid, cancellation and keyboard equivalent |
| Automatic position-only layout | `LayoutGroup::position`, `LayoutPositionState` | Measured native reflow/wrapping/reorder, nested ancestor coordination, interrupted springs/tweens; translated paint/hitboxes/accessibility bounds, immediate sizes |
| Reorder | `ReorderGroup`, `ReorderElement`; portable `reorder_index`, `move_item` | Measured row/column/2D grid, auto axis, external handles, physical keyboard neighbors, native edge autoscroll, outside release, cancellation; host owns persistence/removal focus |
| Popped presence exits | `PopLayout::{container,item}` + `PresenceGroup`/`MotionState` | Detached exiting child layout removes flow space/gaps immediately and freezes the visible parent-relative position with continuous reentry; owns positional reflow; host retains exits, guards input and restores focus |
| Intrinsic size transitions | `AutoSize`, `SizeAxis` | Measured content roots with explicit available width/height; interpolated width/height/both, clipping, first-mount policy and reduced motion; real layout, not scale projection |
| Shared position/crossfade | `SharedLayout::member`, `SharedOverlay` + anchors | Window-root position/crossfade across separate containers; one active destination per key, explicit outgoing lifetime, immediate sizes, ordinary clipping; call `remove` to release identity history |
| Vector drawing and compatible morphs | `PathShape`, `PathMorphState`, `StrokeReveal`, native `VectorState` | Typed commands or optional SVG path-data parser with arcs; bounded normalization to corresponding contours; retained per-coordinate springs; normalized length/offset/spacing; stroke curves approximate with 32 segments, fills retain curves |
| Motion values, velocity, jump, spring smoothing | `MotionValue`, `MotionGraph`, native `GraphState` | Explicit sources, multi-input derived maps, velocity, typed bindings and subscription handles; retained graph, no global registry |
| Mapped/derived values | `transform`, ordinary Rust calculations | Explicit dependency order; map values to native styles, painting parameters, text, or custom elements |
| Color interpolation | `Color`, `ColorState` | Linear-light straight RGBA springs/tweens; convert to GPUI sRGB with optional adapter |
| Scroll-linked animation | `ScrollMotion`, `native_scroll_progress`; portable `scroll_progress`, `transform`, `MotionValue` | Native handle-to-target binding, resize follow-up, optional smoothing; measured metrics available separately |
| Scroll-triggered animation | `GestureState::in_view`, `GestureTargets::while_in_view`; portable `InView` | Native clipped prepaint measurement with threshold/once; portable metric input remains available |
| Reduced motion | `sample(..., true)` / native `App::reduce_motion()` | Completes even forever clips and exits; host synchronizes OS preference |
| Global defaults / inherited config | `MotionConfig` templates, `MotionTree` variant inheritance | Explicit application ownership rather than a hidden provider or React context |

## Renderer limits and deliberate exclusions

Scale/rotation/skew/perspective and custom transform origins for arbitrary GPUI
subtrees remain unsupported by the pinned CE's public subtree rendering API.
Consequently full transform-based size/layout projection, descendant scale
correction, and unrestricted shared-element projection are deferred. SVG-only
drawing transforms and common-overlay position/crossfade are supported; neither
provides general transformed clipping or transformed native hit regions.

Numeric dimensions and `AutoSize` perform real layout rather than GPU scaling.
The SVG adapter uses a cached monochrome raster sprite, so large zooms can soften
it. Vector drawing uses CE's public PathBuilder/tessellation, not per-frame SVG
asset mutation/raster-cache churn. It accepts typed absolute commands; SVG path parsing includes arcs; `normalize_for_morph` resamples corresponding
contours at setup time. It does not parse full documents or infer semantic shape
correspondence. `PathMeasure` provides allocation-free position/tangent queries. Reveal periods are bounded to 4096 per path.

Stock CE supports Gaussian blur/backdrop blur, which the typed style adapter now
exposes. Other CSS filters, CSS variables/complex string interpolation, browser
selectors/WAAPI/scroll timelines, React hooks, and Motion+ product components are
not native drop-in APIs. Native Rust ownership and explicitly prepared values
replace framework lifecycle and subscriptions. This is not complete JavaScript
API compatibility or bit-for-bit Motion physics.

## Operational contracts

- Construct clips/variant tables/timelines once; rebuilding an `Animation` means
  a new trajectory. A clone shares its immutable clip. Sampling allocates no
  memory, including first bind; preparation/membership changes can allocate.
- Pass one monotonic application epoch to pure sampling and controls. Native
  `AnimationState::frame` uses its own epoch; use `state.now()` for its controls.
  Backward real timestamps hold; explicit seeks may move the virtual playhead.
- Completion is an edge on the sampled state, not a log of every missed repeat.
  Read `iteration()` to observe repeat skips. A new spring clip captures both
  current presentation and velocity. Tweens preserve position and can change
  velocity. Loop repeats may deliberately jump at their boundary.
- A timeline retains its frame buffer. It samples each track's absolute local
  time, so pause, seek, occlusion, and long frame gaps do not accumulate error.
  Restart clears reduced-motion completion. A reduced-motion snap otherwise
  stays complete until restart or replacement, avoiding an unexpected replay.
- Native `GestureState` binds desktop mouse/keyboard input, outside release,
  cancellation, focus, measured bounds and viewport measurements. `ReorderGroup` owns
  one pointer gesture and spatial order, with optional edge autoscroll. Portable
  `Drag` remains host-driven. Neither adapter installs OS-wide pointer capture or
  arbitrary raw multi-touch routing; Kit recognized touch drag is opt-in for gestures. Scope
  shortcuts so text editing and control activation remain available.
- `PresenceGroup` keeps mount order and stable keys. Keep removed child states
  until their exit completes, call `finish_exit`, then drop their state. Wait
  mode delays new keys until all outgoing keys finish. Preserve focus explicitly.
- Numeric targets require endpoints. `AutoSize` measures content with explicit
  available constraints, adds a reservation node and separate child layout root,
  and clips input/paint while it interpolates. It does not infer constraints from
  arbitrary ancestors or add a transform-based size projection.
- Retain one `LayoutGroup` across renders and share it through nested animated
  wrappers. Independent groups must not nest. CE-scoped stable IDs retain position
  history while mounted, not across reparenting or virtualization/unmount. First
  measurements snap; scroll/manual prepaint offsets pass through immediately.
  Configure child styles before wrapping and avoid competing x/y animations.
- Shared identities and native reorder geometry belong to their retained owners.
  Call `SharedLayout::remove` when finished and `ReorderGroup::set_order` for model
  changes. Change callbacks should capture application entities weakly, avoiding
  a strong self-reference cycle. The owner restores focus before removing items.
- Prepare paths and `CustomEasing`/visual springs once. Native tessellation, style
  vectors and frame-scoped event listeners can allocate; portable sampling and
  stroke iteration reuse prepared storage. Blur needs separate GPU profiling.
- No background worker, global registry, app runtime, or platform bootstrap dependency
  is installed by the crate. Host notification remains explicit; native adapters schedule active frames.

The comparison uses Motion's official [animation](https://motion.dev/docs/react-animation),
[transitions](https://motion.dev/docs/react-transitions),
[gestures](https://motion.dev/docs/react-gestures), and
[scroll animation](https://motion.dev/docs/react-scroll-animations) documentation.

## Text motion

Native CE and Kit text adapters support whole labels, Unicode word/grapheme units,
measured wrapped lines, enter/exit, content matching, position reflow, stagger,
masked reveals, typewriter, scramble, prepared keyframes/repeats, custom entrances,
pause/play/restart and finite entrance scrubbing. `NumberMotionState` adds exact
i64 endpoints, changed-digit rolling/fading/sliding, and rounded numeric sampling.
See [text contracts](docs/TEXT.md) for the two-second spring scrub horizon, numeric
precision, native composition costs and typography/accessibility limits. General
transforms, outline morphs, editable text and variable-font axes remain excluded.
Rich native spans, color/font-size transitions, vertical wipes and fixed-point
number formatting are supported. Font-size animation performs real shaping/layout.

## GPUI Kit native helpers (gpui-motion-kit 0.4.0)

Kit 0.7.3's default `gpui-pre` backend has native bindings for motion/presence,
styles, gestures/drag/in-view, measured scroll mapping, position layout,
intrinsic sizing, reorder, popped exits, shared position/crossfade, vectors,
prepared clips/timelines, SVG drawing transforms, text and numbers. Bindings use
the shared portable engine (core 0.3.0 is required for these additions); no CE native feature or renderer fork is
needed. Shared geometry/input regression fixtures run against both adapters.

Kit does not expose CE's public element/backdrop filter API. `StyleFrame::try_apply`
returns `UnsupportedStyle` for those tracks; `apply` panics rather than dropping
them. Outer shadow blur is supported. General subtree transforms, size projection,
OS-wide pointer capture, arbitrary raw multi-touch and the documented typography/
accessibility limits remain deferred. Spatial reorder/autoscroll, window-root shared
coordinates, native pinch and recognized Kit touch drag are now supported. Kit
native types cannot mix with CE native types. See the
[Kit API examples](integrations/gpui-kit/README.md).

## Expanded native contracts

- `MotionGraph` subscriptions are explicitly owned. Derived maps run synchronously
  in dependency order, must return finite values, and must not edit their graph.
  Setup allocates; prepared sampling and callbacks that do no work allocate nothing.
- `TypedAnimation` masks must match their seeds. Timeline IDs cannot cross owners.
  Signed offsets are seconds, while absolute positions use `Duration`. Completion
  futures allocate only when requested. Owners cancel observers on replacement/drop.
- `MotionAnchor` measurements and `SharedOverlay` destinations belong to one native
  window. Clear stale anchors when unmounting/reusing them. Explicit scroll sources
  update offsets between measurements; nested ancestor changes remeasure next frame.
- Window-root floating appearances must remain outside clipped containers. Ordinary
  child controls follow translated hitboxes; host owns outgoing input/focus. Sizes
  update through layout, never subtree scale projection.
- SVG parsing is bounded (1 MiB input, 65,536 expanded commands), normalization
  uses at most 4096 samples per contour and 65,536 points total. Setup may allocate;
  queries/sample reuse storage. Curve subdivision and polyline normalization are
  approximations with caller-controlled normalization density.
- Fixed-point formatting is exact at endpoints. Numeric interpolation still uses
  f64 between endpoints. Explicit configuration replaces automatic locale discovery.
  Text styles use shared solvers; font-size changes reshape, colors retain glyph
  samplers, pause/play freezes both, and entrance seek does not rewind style history.
