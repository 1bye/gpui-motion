# gpui-ce-motion

**Declarative animation for native Rust apps on GPUI CE.**

Describe the next state. Keep the motion state in your view. The library handles
transitions, springs, and interruptions using stock GPUI CE—no React or renderer fork.

[Crates.io](https://crates.io/crates/gpui-ce-motion) ·
[All public APIs](docs/API.md#public-export-index) ·
[API recipes](docs/API.md) · [Demos](docs/DEVELOPMENT.md#demos) ·
[Changelog](CHANGELOG.md)

## Install

```sh
cargo add gpui-ce-motion --features gpui
```

Or add it to an existing GPUI CE application:

```toml
[dependencies]
gpui-ce-motion = { version = "0.1", features = ["gpui"] }
```

Requires **Rust 1.95+**. Native adapters use **registry `gpui-ce =0.2.2`**. Use the
same CE source and version throughout your app so its element types match.

| Feature | Use it for |
| --- | --- |
| Default (none) | Portable animation math, explicit clocks, no third-party dependencies |
| `gpui` | Native element adapters, input bindings, and automatic frame scheduling |
| `demo` | Running this repository's examples; adds the platform backends |
| `native-tests` | Development checks using CE's TestPlatform |

## Quick start

This is a GPUI component for an existing app. Click the card to change its width.
Changing a target during animation continues from the displayed position; springs
preserve velocity too.

```rust
use gpui::{div, prelude::*, rgb, Context, Render, Window};
use gpui_ce_motion::{Motion, MotionState, Target};

#[derive(Default)]
struct Card {
    expanded: bool,
    motion: MotionState, // Keep this between renders.
}

impl Render for Card {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let frame = self.motion.frame(
            Motion::new(Target::default()
                .width(if self.expanded { 320. } else { 180. })
                .height(100.)
                .border_radius(16.))
                .initial(Target::default().y(12.).opacity(0.)),
            window, cx,
        );

        frame.apply(div().id("card").bg(rgb(0xd4f580)).text_color(rgb(0x101310)).p_4())
            .on_click(cx.listener(|this, _, _, cx| {
                this.expanded = !this.expanded;
                cx.notify();
            }))
            .child("Click to resize")
    }
}
```

`Motion` is a declaration you can recreate every render. `MotionState` is its
retained history. Keep one state per logical object; key list items by stable IDs.
Native adapters request frames while active. Notify the view after input changes.
See [a complete runnable app](examples/motion.rs) or try the showcase:

```sh
git clone https://github.com/1bye/gpui-motion.git
cd gpui-motion
cargo run --release --features demo --example motion_showcase
```

## Choose an API

Every API family has a short, compile-checked recipe. Native recipes require `gpui`.

| I want to… | Main APIs | Recipe |
| --- | --- | --- |
| Animate position, opacity, width, height, or radius | `Motion`, `MotionState`, `Target`, `Frame` | [Targets and transitions](docs/API.md#targets-and-transitions) |
| Choose easing, springs, or property-specific timing | `Transition`, `Spring`, `Easing`, `Bezier`, `CustomEasing`, `Property` | [Timing](docs/API.md#timing) |
| Animate entrance, exit, and interrupted re-entry | `Motion::initial`, `exit`, `present`, `Presence` | [Enter and exit](docs/API.md#enter-and-exit) |
| Keep removed list items alive for their exit | `PresenceGroup`, `PresenceMode`, `PresenceChild` | [Keyed presence](docs/API.md#keyed-presence) |
| Play keyframes, repeat, or control playback | `Animation`, `Keyframe`, `Repeat`, `RepeatMode`, `AnimationState`, `Playback` | [Clips and playback](docs/API.md#clips-and-playback) |
| Sequence, overlap, label, or stagger animations | `Timeline`, `stagger`, `StaggerFrom` | [Timelines and stagger](docs/API.md#timelines-and-stagger) |
| Select named states and layer interaction targets | `Variants`, `GestureTargets`, `Interaction` | [Variants](docs/API.md#variants) |
| Bind hover, press, focus, drag, and visibility | `GestureState`, `GestureElement` | [Native gestures](docs/API.md#native-gestures) |
| Add bounds, inertia, elasticity, or grid snapping | `Drag`, `Inertia` | [Dragging](docs/API.md#dragging) |
| Animate from scroll position or viewport visibility | `ScrollMotion`, `native_scroll_progress`, `scroll_progress`, `InView`, `Axis` | [Scroll and visibility](docs/API.md#scroll-and-visibility) |
| Build draggable lists or tabs | `ReorderGroup`, `ReorderElement`, `reorder_index`, `move_item` | [Reordering](docs/API.md#reordering) |
| Animate automatic reflow and wrapping | `LayoutGroup`, `LayoutPositionElement`, `LayoutPositionState`, `LayoutPositionFrame` | [Layout positions](docs/API.md#layout-positions) |
| Remove exiting items from flow immediately | `PopLayout`, `PopElement`, `PopRequest` | [Popped exits](docs/API.md#popped-exits) |
| Animate measured content dimensions | `AutoSize`, `SizeAxis` | [Content sizing](docs/API.md#content-sizing) |
| Move a shared visual between overlay positions | `SharedLayout`, `SharedElement` | [Shared visuals](docs/API.md#shared-visuals) |
| Animate spacing, corners, colors, shadows, or blur | `StyleTarget`, `StyleMotion`, `StyleMotionState`, `StyleFrame`, `StyleProperty`, `ColorProperty` | [Style animation](docs/API.md#style-animation) |
| Scale, rotate, translate, or mirror an SVG drawing | `SvgTarget`, `SvgMotion`, `SvgMotionState`, `SvgFrame` | [SVG transforms](docs/API.md#svg-transforms) |
| Draw, morph, or reveal compatible vector paths | `PathShape`, `PathCommand`, `PathMorphState`, `PathFrame`, `StrokeReveal`, `VectorState`, `VectorElement` | [Vector paths](docs/API.md#vector-paths) |
| Animate a scalar, read velocity, or map values | `MotionValue`, `ValueFrame`, `transform` | [Motion values](docs/API.md#motion-values) |
| Interpolate colors in linear light | `Color`, `ColorState` | [Colors](docs/API.md#colors) |
| Respond to completion or honor reduced motion | `just_completed`, `is_animating`, `App::reduce_motion` | [Lifecycle and accessibility](docs/API.md#lifecycle-and-accessibility) |
| Handle invalid declarations | `AnimationError`, `InvalidSpring`, `InvalidEasing`, `PathError` | [Validation](docs/API.md#validation) |

The [complete export index](docs/API.md#public-export-index) links every public
type and function to its source declaration. Recipes cover both native
adapters and the portable sampling APIs.

## Ownership in four rules

1. **Retain state.** Store motion states, groups, clips, and timelines in the view.
   Creating a new state resets its history; creating a new clip replaces playback.
2. **Declare the destination.** Build targets during render. First mounts snap
   unless you provide `initial`; undeclared dimensions preserve existing styles.
3. **Keep exits mounted.** Continue declaring `present(false)` until the exit is
   absent. Restore focus and guard outgoing input yourself.
4. **Use one clock.** Portable `sample` and controls use one monotonic epoch.
   Native clip controls use `state.now()`. Native adapters honor `cx.reduce_motion()`;
   your app synchronizes the OS preference.

## What stock GPUI CE can animate

| Supported | Boundary |
| --- | --- |
| Dimensions, spacing, and measured content sizing | Real layout; text and siblings can reflow |
| Automatic position animation and shared position/crossfade | Sizes change immediately; no size projection |
| SVG scale and rotation | Drawing only; native hitboxes, clipping, and children stay untransformed |
| Native paths and stroke reveal | Matching command topology; no arbitrary SVG parser or shape normalization |
| Blur and backdrop blur | Need separate GPU profiling; CPU sampling timings do not measure these effects |

Arbitrary element-tree scale/rotation/skew/perspective and full layout projection
are deferred. This is a Motion-inspired Rust API, not JavaScript/React API parity.
Read [COVERAGE.md](COVERAGE.md) for exact contracts and exclusions.

## Explore and contribute

- [Demos and showcase controls](docs/DEVELOPMENT.md#demos)
- [Tests and benchmarks](docs/DEVELOPMENT.md#verification)
- [Measured performance and its limits](PERFORMANCE.md)
- [Publishing procedure](PUBLISHING.md)
- [Changes by version](CHANGELOG.md)

MIT OR Apache-2.0. The library and demos were extracted from 1keep at commit
`0cb95d0`; the original [MIT](LICENSE-MIT) and [Apache-2.0](LICENSE-APACHE)
notices are retained.
