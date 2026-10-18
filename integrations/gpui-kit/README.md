# gpui-motion-kit

**Declarative animation for native Rust apps built with GPUI Kit.**

Keep the motion state in your view and describe its next destination. The adapter
connects the shared `gpui-ce-motion` engine to Kit's elements, executor clock,
frame scheduling, and reduced-motion preference. No JavaScript or renderer fork.

[Crates.io](https://crates.io/crates/gpui-motion-kit) ·
[Published API reference](https://docs.rs/gpui-motion-kit/latest/gpui_motion_kit/) ·
[Product examples](#product-examples) · [Text gallery](#text-gallery)

**Development note:** the native-helper expansion below is unreleased. Published
`0.2.0` has transitions/presence, scalar values, position layout, vectors and text.
To use the added helpers now, replace the registry dependency with a checkout:

```toml
gpui-motion-kit = { path = "../gpui-motion/integrations/gpui-kit" }
```

## Install

```sh
cargo add gpui-motion-kit@0.2
cargo add gpui-kit@=0.7.3 --features component,assets
```

Or add these dependencies to your application:

```toml
[dependencies]
gpui-motion-kit = "0.2"
gpui-kit = { version = "=0.7.3", features = ["component", "assets"] }
```

Requires **Rust 1.95+** and GPUI Kit's native build prerequisites. This release
supports **Kit 0.7.3 with its default `gpui-pre` backend**. Use the same Kit version
throughout your application; the alternative `gpui-fast` backend is not supported.
macOS is covered by native CI. Linux API documentation builds in CI; Linux/Windows
application behavior has not been validated.

You do not need a direct `gpui-ce-motion` dependency. Portable types such as
`Motion`, `Target`, and `Transition` are available through `gpui_motion_kit::core`.
Import native states from `gpui_motion_kit`, and UI types from `gpui_kit`.
Do not enable the core engine's CE-specific `gpui` feature for this integration:
Kit and CE native types belong to different GPUI crates and cannot be mixed.

## Quick start

This complete application animates a panel containing a normal Kit button.
Click repeatedly while it moves to interrupt the spring smoothly.

```rust
use gpui_kit::{
    AppContext, Context, Render, Window, WindowOptions,
    assets::Assets,
    component::{ActiveTheme, button::Button},
    div, prelude::*,
};

use gpui_motion_kit::{
    MotionState,
    core::{Motion, Target},
};

#[derive(Default)]
struct Panel {
    expanded: bool,
    motion: MotionState,
}

impl Render for Panel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let width = if self.expanded { 320.0 } else { 180.0 };

        let target = Target::default()
            .width(width)
            .height(96.0)
            .border_radius(16.0);

        let frame = self.motion.frame(Motion::new(target), window, cx);

        let panel = div()
            .p_4()
            .bg(cx.theme().secondary)
            .child(
                Button::new("resize-panel")
                    .label("Resize panel")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.expanded = !this.expanded;
                        cx.notify();
                    })),
            );

        div()
            .size_full()
            .p_8()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(frame.apply(panel))
    }
}

fn main() {
    gpui_kit::application().with_assets(Assets).run(|cx| {
        gpui_kit::init(cx);

        gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| {
            cx.new(|_| Panel::default())
        })
        .expect("open animation example");

        cx.activate(true);
    });
}
```

`Motion` is a declaration you can recreate each render. `MotionState` is retained
history: keep one state per logical object. Springs preserve position and velocity
when interrupted. The adapter requests another frame only while moving; notify
the view after an input changes its target. The host app supplies Kit's reduced-motion
preference, which the adapter reads when sampling.

The button keeps its Kit behavior. `Frame::apply` animates its wrapper's relative
`x`/`y`, opacity, width, height, and border radius. Width and height perform real
layout; they are not GPU scale transforms. Applying a corner radius does not
clip children automatically. This integration does not automatically animate
every internal property of every Kit component.

## Enter and exit

Keep declaring the element after `open` becomes false. `Frame::render` retains
it during the exit and returns an empty element once the exit finishes.

```rust
use gpui_kit::{AnyElement, App, Window, div, prelude::*};
use gpui_motion_kit::{MotionState, core::{Motion, Target}};

fn details(state: &mut MotionState, open: bool, window: &mut Window, cx: &App) -> AnyElement {
    let motion = Motion::new(Target::default())
        .initial(Target::default().y(12.0).opacity(0.0))
        .exit(Target::default().y(-8.0).opacity(0.0))
        .present(open);

    let frame = state.frame(motion, window, cx);

    frame.render(|frame| frame.apply(div().p_4().child("Details")))
}
```

Keep `state` on the owner. Before removing focused controls, restore focus to a
remaining control, and guard outgoing input with `frame.is_present()`.

## Text and numbers

Retain `TextMotionState` and `NumberMotionState` on your view. These elements use
Kit's native text renderer with the same shared text lifecycle and shaping logic
as the CE adapter.

```rust
use gpui_kit::{IntoElement, SharedString, div, prelude::*};

use gpui_motion_kit::{
    NumberEffect, NumberMotionState, TextMotionState, TextPreset, TextUnit,
    core::StaggerFrom,
};

use std::time::Duration;

fn heading(
    text: &TextMotionState,
    count: &NumberMotionState,
    title: SharedString,
    total: i64,
) -> impl IntoElement {
    let title = text
        .element("heading", title)
        .units(TextUnit::Words)
        .preset(TextPreset::FadeUp)
        .stagger(Duration::from_millis(35), StaggerFrom::Start)
        .when(cfg!(target_os = "macos"), |title| {
            title.font_family("Helvetica Neue")
        });

    let total = count
        .element("total", total)
        .effect(NumberEffect::Roll)
        .when(cfg!(target_os = "macos"), |total| {
            total.font_family("Helvetica Neue")
        });

    div().flex().gap_4().child(title).child(total)
}
```

The macOS font override avoids a stock system-font clipping issue at display
sizes. Text also supports graphemes, wrapped lines, custom targets, matching,
reflow, wipes, scrambling, playback and scrubbing. Numbers support Roll, Fade,
Slide, and Count. See the [text contracts](https://github.com/1bye/gpui-motion/blob/main/docs/TEXT.md).

## Layout positions

Keep one `LayoutGroup` per coordinate scope and stable IDs for its children.
When normal layout moves an item, this wrapper animates from its previous position.

```rust
use gpui_kit::{IntoElement, div, prelude::*};
use gpui_motion_kit::LayoutGroup;

fn rows(layout: &LayoutGroup, order: &[usize]) -> impl IntoElement {
    div().flex().flex_col().gap_2().children(order.iter().map(|&id| {
        let row = div()
            .id(("row", id))
            .p_3()
            .child(format!("Task {id}"));

        layout.position(("row-position", id), row)
    }))
}
```

Paint, hitboxes, and accessibility bounds move together. Sizes change immediately;
this is position-only animation, not size projection or cross-parent shared layout.

## Animated values

Use `ValueState` when a Kit control or your own drawing consumes a numeric value.
This helper animates a progress fill in logical pixels:

```rust
use gpui_kit::{App, IntoElement, Window, div, prelude::*, px, rgb};
use gpui_motion_kit::{ValueState, core::Transition};

fn progress(
    state: &mut ValueState,
    target_width: f64,
    window: &mut Window,
    cx: &App,
) -> impl IntoElement {
    let frame = state.frame(target_width, Transition::default(), window, cx);

    div()
        .w(px(frame.value() as f32))
        .h_2()
        .bg(rgb(0x84cc16))
}
```

Initialize the retained state once with `ValueState::new(0.0)`. Avoid giving the
same property a second interpolator inside a Kit component.

`VectorState` supplies compatible-path morphing and stroke reveal. The
[automation example](https://github.com/1bye/gpui-motion/blob/main/integrations/gpui-kit/examples/showcase/automation.rs)
shows it in a complete flow. The native helpers below reuse the same published
portable engine; their rendering and input types are Kit-native.

## Style transitions

`StyleMotionState` applies colors, per-corner radii, border widths, padding,
margins, gaps and a single outer shadow. Undeclared properties preserve existing
styles; previously introduced tracks retain their destinations. Styles that change
layout perform real layout. Keep the sampler on the owning view.

```rust
use gpui_kit::{App, IntoElement, Window, div, prelude::*};
use gpui_motion_kit::{StyleMotionState, core::{Color, ColorProperty, StyleMotion, StyleTarget}};

fn styled_panel(state: &mut StyleMotionState, expanded: bool, window: &mut Window, cx: &App) -> impl IntoElement {
    let padding = if expanded { 24.0 } else { 12.0 };

    let target = StyleTarget::default()
        .padding([padding; 4])
        .radii([16.0; 4])
        .color(ColorProperty::Background, Color::rgb(0x4338ca))
        .shadow([0.0, 8.0], 20.0, 0.0, Color::rgba(0.0, 0.0, 0.0, 0.2));

    let frame = state.frame(StyleMotion::new(target), window, cx);

    frame.apply(div().child("Settings"))
}
```

Stock Kit 0.7.3 has no public element blur/backdrop-filter API. A portable target
containing `StyleProperty::Blur` or `BackdropBlur` is therefore unsupported.
`frame.try_apply(element)` returns `UnsupportedStyle` (with `property()`);
`apply` panics for those declarations, including radius zero. No filter is silently
ignored. Shadow blur is supported. `color_to_rgba` converts a sampled engine color
to Kit's native color when you paint your own element.

## Hover, press, focus and drag

Retain one `GestureState` per logical control. Configure its drag bounds and
optional visibility policy once, then bind its current targets during render.
Native input resolves interaction flags automatically. Arrow keys move a focused
drag target; Escape, deactivation, blur and a lost mouse button cancel the grab.

```rust
use gpui_kit::{Role, div, prelude::*, px};
use gpui_motion_kit::{GestureElement, GestureState, core::{Drag, GestureTargets, Motion, Target}};

fn drag_state() -> GestureState {
    GestureState::default()
        .with_drag(Drag::new([0.0, 0.0], [0.0, 0.0], [240.0, 0.0]).snap_grid(20.0))
        .in_view(0.5, false)
}

fn draggable(state: &GestureState) -> GestureElement {
    let card = div()
        .id("drag-card")
        .role(Role::Button)
        .aria_label("Drag card; use arrow keys to move")
        .w(px(100.0))
        .h(px(64.0))
        .child("Drag me");

    let targets = GestureTargets::default()
        .while_hover(Target::default().opacity(0.85))
        .while_press(Target::default().opacity(0.65))
        .while_focus(Target::default().y(-2.0));

    state.bind("drag-motion", card, Motion::new(Target::default()), targets)
}
```

`interaction()`, `presented()`, and `focus_handle()` expose current state.
`.in_view(amount, once)` observes the clipped ancestor viewport; use a
`while_in_view` target to animate it. `.enabled(false)` disables input/tab stops
and cancels a grab on the next render; the owner restores focus and supplies its
disabled semantics. `cancel(cx)` cancels explicitly.

The child is a styled stateful native element, including Kit's observed elements
from `.test_support()`. Attach control semantics and activation to that child;
this wrapper does not replace a Kit Button's own internal interaction system.
Use a dedicated drag surface rather than competing with nested editors/buttons
for pointer and focus ownership.
Mouse and keyboard are supported. Multi-touch, OS-wide capture and autoscroll
remain application/platform work.

## Scroll-driven animation

Retain a Kit `ScrollHandle` and attach it to the real scroll container.
`ScrollMotion` reads its measured extent/offset, refreshes after resize, and maps
progress to a numeric target. Mapping is instant unless you choose smoothing.

```rust
use gpui_kit::{IntoElement, ScrollHandle, div, prelude::*, px};
use gpui_motion_kit::{Axis, ScrollMotion, core::{Target, Transition}};

fn scrolled_panel(scroll: &ScrollHandle) -> impl IntoElement {
    let indicator = ScrollMotion::new(
        "scroll-progress",
        scroll.clone(),
        Axis::Y,
        Target::default().width(0.0),
        Target::default().width(300.0),
        div().h(px(4.0)).bg(gpui_kit::rgb(0x4338ca)),
    )
    .transition(Transition::default());

    div().flex().flex_col()
        .child(indicator)
        .child(div().id("scroll-area").h(px(200.0)).overflow_y_scroll().track_scroll(scroll)
            .child(div().h(px(800.0)).child("Scrollable content")))
}
```

`native_scroll_progress(&handle, Axis::Y)` is available for custom mappings.
An unscrollable container reports zero. A dimension declared at only one endpoint
is held at that numeric value; undeclared dimensions keep their element style.

## Measured content sizing

`AutoSize` measures a separate child layout root, reserves its animated size,
and clips paint/input to the reservation. Keep its ID stable. Supply explicit
available constraints; these do not force a child to have that width or height.

```rust
use gpui_kit::{IntoElement, div, prelude::*, px};
use gpui_motion_kit::{AutoSize, SizeAxis};

fn accordion(expanded: bool) -> impl IntoElement {
    let content = div().w(px(300.0)).p_4()
        .child("Details")
        .when(expanded, |content| content.child("Additional content wraps naturally."));

    AutoSize::new("accordion", content)
        .width(300.0)
        .axis(SizeAxis::Height)
}
```

Width, height and both axes are supported. First measurement snaps by default;
`.initial([width, height])` opts into entrance from a numeric size. Unanimated
axes follow content immediately without keeping an animation loop alive.

## Reordering and popped exits

`ReorderGroup` owns a single-axis logical order and measured variable-size items.
Keep the group and domain keys stable. Pointer movement reorders; Option+arrows
on macOS (Alt+arrows elsewhere) moves the focused item. Escape/deactivation cancels.

```rust
use gpui_kit::{IntoElement, div, prelude::*};
use gpui_motion_kit::{Axis, ReorderGroup};

fn tasks(group: &ReorderGroup<usize>) -> impl IntoElement {
    let order = group.order().to_vec();

    div().flex().flex_col().children(order.into_iter().map(|key| {
        let row = div().id(("task", key)).p_4().child(format!("Task {key}"));

        group.item(("task-motion", key), key, row)
    }))
}

let group = ReorderGroup::new(vec![1, 2, 3], Axis::Y).expect("unique task keys");
```

`on_change` reports pointer commit/keyboard changes without borrowing group state.
Capture application entities weakly. `set_order` updates membership transactionally
and cancels a grab; call it on model changes, not every render. The owner handles
persistence, focus after removal, and autoscroll. Multi-column sorting is not implied.

`PopLayout` lets an outgoing item leave normal flow immediately while retaining
its last visible parent-relative bounds. It also animates sibling reflow:

```rust
use gpui_kit::{IntoElement, div, prelude::*};
use gpui_motion_kit::PopLayout;

fn notifications(pop: &PopLayout, first_present: bool) -> impl IntoElement {
    let rows = div().flex().flex_col().gap_2()
        .child(pop.item("first", first_present, div().child("Retained first row")))
        .child(pop.item("second", true, div().child("Second row")));

    pop.container("notifications", rows)
}
```

Pair it with `Motion::present`/`Frame::render` and, for keyed membership,
`core::PresenceGroup` to retain exiting content until its fade completes. Guard
outgoing input and restore focus. Do not add `LayoutGroup` to the same popped
item; both would own its position. An inner child can animate its own exit offsets.

## Shared position and crossfade

`SharedLayout` coordinates one active appearance per key in a common overlay.
Retain outgoing appearances during their normal exits. Sizes change immediately;
this is position/crossfade, not size projection or cross-parent coordinate discovery.

```rust
use gpui_kit::{IntoElement, div, prelude::*, px};
use gpui_motion_kit::SharedLayout;

fn selection(shared: &SharedLayout<usize>, expanded: bool) -> impl IntoElement {
    let old = div().absolute().left(px(0.0)).w(px(100.0)).h(px(60.0));
    let new = div().absolute().left(px(240.0)).w(px(180.0)).h(px(100.0));

    div().relative().h(px(220.0))
        .child(shared.member(1, "old-selection", !expanded, old))
        .child(shared.member(1, "new-selection", expanded, new))
}
```

The owner suppresses outgoing input and duplicate semantics. Call `remove(&key)`
when an identity is finished; `len`/`is_empty` expose retained membership.

## Prepared playback and timelines

Keep clips and native players in the view. `AnimationState::frame` returns a
Kit-ready `Frame`; `Timeline::frame` returns a retained slice indexed by each
track's insertion result. Track setup allocates; sampling reuses the frame buffer.

```rust
use gpui_kit::{App, IntoElement, Window, div, prelude::*};
use gpui_motion_kit::{AnimationState, core::{Animation, Easing, Target}};
use std::time::Duration;

fn make_clip() -> Animation {
    Animation::to(Target::default().x(200.0), Duration::from_millis(400), Easing::EaseOut)
}

fn animated_card(player: &mut AnimationState, clip: &Animation, window: &mut Window, cx: &App) -> impl IntoElement {
    let frame = player.frame(clip, window, cx);

    frame.apply(div().child("Prepared animation"))
}
```

Playback controls use the **same Kit executor epoch**, not `Instant::now()`:

```rust
use gpui_kit::App;
use gpui_motion_kit::AnimationState;
use std::time::Duration;

fn scrub(player: &mut AnimationState, cx: &App) {
    let now = player.now(cx);

    player.pause(now);
    player.seek(Duration::from_millis(200), now);
    player.set_speed(0.5, now).expect("finite speed");
    player.play(now);
}
```

Players also support `stop`, `restart`, `finish`, `iteration`, and `is_playing`.
`Timeline` supports pause/play, restart, seek, and speed including negative rates:

```rust
use gpui_kit::{App, IntoElement, Window, div, prelude::*};
use gpui_motion_kit::{Timeline, core::{Animation, Easing, Target}};
use std::time::Duration;

fn make_timeline() -> Timeline {
    let slide = Animation::to(Target::default().x(120.0), Duration::from_millis(300), Easing::EaseOut);

    let mut timeline = Timeline::default();

    timeline.at(Duration::ZERO, slide.clone());
    timeline.label("after-first").expect("unique label");
    timeline.then(slide).expect("finite predecessor");

    timeline
}

fn sequence(timeline: &mut Timeline, window: &mut Window, cx: &App) -> impl IntoElement {
    let frames = timeline.frame(window, cx);

    div().flex().flex_col().children(frames.iter().copied().enumerate().map(|(id, frame)| {
        frame.apply(div().id(("track", id)).child(format!("Track {id}")))
    }))
}
```

`at_label` adds overlapping tracks at a named cursor plus an offset. Future tracks
still schedule frames even before their first movement. Paused/settled/reduced-motion
players stop requesting frames, including forever clips. As with portable playback,
a loop can deliberately jump at its boundary. Do not mix explicit `sample` epochs
with native `frame`/`now` calls on one retained owner.

## SVG drawing transforms

Kit supports SVG-specific signed scale, rotation and translation. Retain a
`SvgMotionState`; the shared engine preserves unwrapped angles and interruption
momentum. Only the SVG drawing transforms, not its layout, hitbox, clipping, text
or children. Provide a fixed padded parent for interaction.

```rust
use gpui_kit::{App, IntoElement, Window, div, prelude::*, px, svg};
use gpui_motion_kit::{SvgMotionState, core::{SvgMotion, SvgTarget}};

fn icon(state: &mut SvgMotionState, expanded: bool, window: &mut Window, cx: &App) -> impl IntoElement {
    let target = SvgTarget::default()
        .scale(if expanded { 1.4 } else { 1.0 })
        .rotate_degrees(if expanded { 90.0 } else { 0.0 });

    let frame = state.frame(SvgMotion::new(target), window, cx);

    div().p_8().child(frame.apply(svg().path("icons/arrow-right.svg").size(px(32.0))))
}
```

`transformation()` produces a Kit native center-origin transform;
`transformation_about(bounds_size, pivot)` compensates for a custom pixel pivot.
These replace the drawing's existing transform. `Frame`, `StyleFrame`, and
`SvgFrame` accept their portable core frames through `From` for custom clocks.
General subtree transforms and full layout projection remain renderer limitations.

## Helper gallery

```sh
cargo run --manifest-path integrations/gpui-kit/Cargo.toml --features demo --example helpers
```

Eight scenes exercise native styles, bounded drag, manual scrolling/in-view,
measured content sizing, popped exits/reorder, shared positions, clip/timeline
controls, and SVG transforms. Use Change state during travel, pause/seek/replay,
drag variable-size rows, and enable reduced motion. Option/Alt+arrows reorder a
focused row. The gallery uses actual Kit controls; its production interaction
tests dispatch native pointer/keyboard/scroll input.

## Product examples

Six interactive product examples are included in the repository and crate archive.
Run from the repository root:

```sh
cargo run --manifest-path integrations/gpui-kit/Cargo.toml --features demo --example showcase
```

Use the tabs to switch examples. Each example keeps its state. You can also open
one directly by appending `-- inbox`, `-- assets`, `-- downloads`, `-- palette`,
`-- kanban`, or `-- automation`.

| Example | Try it | Motion |
| --- | --- | --- |
| Inbox | Select messages, archive them, and undo during or after the exit. | Keyed presence, spring reflow, smooth interruption |
| Assets | Filter or search the artwork, inspect a thumbnail, and close the inspector. | Animated filtering, crossfades, inspector width |
| Downloads | Start the simulated transfers, pause, resume, cancel, and add downloads. | Scalar progress, enter/exit, measured row positions |
| Commands | Search documents, preview with arrows, and open with Enter. | Measured palette height, interrupted preview transitions |
| Planner | Drag cards between columns or into a new order. Arrow buttons offer the same move by keyboard. | Insertion previews, native dragging, spring settling |
| Automation | Run the simulated release, pause it, let publishing fail, and retry. | Progress, vector stroke reveal, coordinated steps, expanding errors |

The inbox supports Command+Backspace to archive and Command+Z to undo on macOS;
use Control on other desktop platforms. Command+Q/Control+Q quits. Kit controls
keep their native keyboard, focus, and accessibility behavior. Finite transitions
honor GPUI's reduced-motion preference. Transfers and releases are explicitly
simulated; these examples do not access email, download files, or publish anything.
Artwork is embedded in the executable and decoded once at startup. The showcase
was validated on macOS; Linux/Windows runs and screen-reader behavior are unverified.

## Checks

```sh
cargo fmt --manifest-path integrations/gpui-kit/Cargo.toml --all --check
cargo test --manifest-path integrations/gpui-kit/Cargo.toml --locked --features demo,native-tests
cargo test --manifest-path integrations/gpui-kit/Cargo.toml --locked --features demo,native-tests --example showcase
cargo test --manifest-path integrations/gpui-kit/Cargo.toml --locked --features demo,native-tests --example helpers
cargo clippy --manifest-path integrations/gpui-kit/Cargo.toml --locked --all-targets --features demo,native-tests -- -D warnings
python3 scripts/check_spacing.py --project integrations/gpui-kit
```

Append `--fix` to the spacing command to apply the same shared Dylint policy as
the CE crate. The repository's pinned Dylint nightly and CLI are required; see
[development instructions](https://github.com/1bye/gpui-motion/blob/main/docs/DEVELOPMENT.md#rust-spacing).
The spacing command also compiles every Rust example in this README against Kit.
Tests and benchmarks reuse CE fixtures and run from the repository; the published
archive contains the library, runnable demos, their assets, and licenses.

Native interaction tests render the production views and dispatch real Kit
pointer/keyboard input. TestPlatform substitutes the display/GPU, so those tests
verify behavior and geometry rather than GPU presentation.

## Performance

The helper benchmark compares 100 direct elements with styles, scroll bindings,
clip/timeline playback, gestures, sizing, popped exits, reorder, shared positions,
vectors, and direct/animated SVG drawing. It uses five fresh contexts, 24 warmup
and 240 measured frames per batch, retargeting every 24 frames:

```sh
cargo bench --manifest-path integrations/gpui-kit/Cargo.toml --features demo,native-tests --bench helpers_native
```

These are CPU construction/layout/prepaint/paint-preparation timings on
TestPlatform with synthetic fonts/display/GPU. They do not measure GPU execution,
OS input latency, frame presentation or energy. Direct and animated SVG cases
use identical fixed SVG data and caching; the comparison includes normal builder
costs. See [helper measurements](https://github.com/1bye/gpui-motion/blob/main/PERFORMANCE.md#kit-native-helpers--2026-10-18).


```sh
cargo bench --manifest-path integrations/gpui-kit/Cargo.toml --features demo,native-tests --bench kit_layout
```

The benchmark compares direct layout, position tracking without interpolation,
and interrupted position animation for 400 elements over seven batches of 240
measured frames, after warmup. It reports CPU construction/layout/prepaint/paint
preparation on TestPlatform, not GPU completion or display latency. Keep profile,
machine, and workload the same when comparing results. Live-window checks are
still needed for presentation, input feel, and platform behavior.

Local release-profile result on macOS ARM64 with Rust 1.95.0, recorded on
2026-10-18 after other builds had finished:

| Workload | Median batch mean, ms/frame | Aggregate frame P95, ms |
| --- | ---: | ---: |
| Direct layout | 1.259 | 1.397 |
| Position tracking, interpolation disabled | 1.779 | 2.004 |
| Animated measured positions with interruption | 2.014 | 2.294 |

This is one shared-desktop CPU comparison, not a frame-rate guarantee. The
animated wrapper adds roughly 0.76 ms to the median batch mean in this workload.
These figures do not measure all six product screens or GPU execution.

## Text gallery

The native text API shares its renderer and portable engine with the CE adapter.
Keep `TextMotionState` or `NumberMotionState` from `gpui_motion_kit` in the owner.
Text grouping, targets, presets and number effects are re-exported alongside them;
existing transitions, prepared animations and stagger origins come from `core`.

```sh
cargo run --manifest-path integrations/gpui-kit/Cargo.toml --features demo --example text_motion
```

Try all twelve modes, edit the input, or press Change text repeatedly while it
moves. Replay/Pause/Resume control text playback; Scrub exposes a draggable reveal
slider. Numbers demonstrates Roll/Fade/Slide/Count with increment/decrement buttons.
The reduced-motion checkbox settles every mode. The [text API and contracts](https://github.com/1bye/gpui-motion/blob/main/docs/TEXT.md)
cover Unicode shaping, matching, clipping, playback and the finite scrub horizon.

The text-native benchmark compares ordinary text replacement, settled adapter
rendering, interrupted word/grapheme movement and deterministic scrambling for
40 labels. Like the layout benchmark, it measures CPU preparation on TestPlatform.

The final local run measured 0.271 ms/frame for ordinary text, 0.419 ms for
interrupted words, 0.542 ms for graphemes and 0.560 ms for scrambling. These are
median batch means; see [measurement details and P95](https://github.com/1bye/gpui-motion/blob/main/PERFORMANCE.md#native-text-motion--2026-10-18)
for workload differences, repeat-run variation and the stock-CE comparison.

MIT OR Apache-2.0. See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).
