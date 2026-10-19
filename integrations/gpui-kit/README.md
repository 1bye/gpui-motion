# gpui-motion-kit

**Declarative animation for native Rust apps built with GPUI Kit.**

Keep the motion state in your view and describe its next destination. The adapter
connects the shared `gpui-ce-motion` engine to Kit's elements, executor clock,
frame scheduling, and reduced-motion preference. No JavaScript or renderer fork.

[Crates.io](https://crates.io/crates/gpui-motion-kit) ·
[Published API reference](https://docs.rs/gpui-motion-kit/latest/gpui_motion_kit/) ·
[Product examples](#product-examples) · [Text gallery](#text-gallery)

Version **0.5.0** uses the shared **gpui-ce-motion 0.4.0** engine and adds
`Motions<K>` for keyed transitions, synchronized batches, and explicit cleanup.
Explore scene **10 Collection** in `motion_lab` or the
[eight-scene helper gallery](#helper-gallery).

## Install

```toml
[dependencies]
gpui-motion-kit = "0.5"
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

`Motion` is a declaration you can recreate each render. `MotionState` is its
retained history: keep one state per logical object. Springs preserve position
and velocity when interrupted. The adapter requests another frame only while
moving; notify the view after an input changes its target. The host app supplies
Kit's reduced-motion preference, which the adapter reads when sampling.

Prepare custom easing, keyframe clips, and parsed/measured paths outside `render`,
and reuse them until their inputs change. Each `CustomEasing::new(...)` creates a
new identity and can restart transitions; rebuilding clips and paths repeats setup work.

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

## Multiple animations with Motions

Keep one `Motions<Part>` field when a component animates several independent
parts. Each key owns its own ordinary `MotionState` internally. The collection
simplifies ownership; it does not make independent trajectories share a spring.

```rust
use gpui_kit::{Context, Render, Window, div, prelude::*};
use gpui_motion_kit::{Motions, core::{Motion, Target}};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Part {
    Panel,
    Title,
}

#[derive(Default)]
struct Card {
    expanded: bool,
    motions: Motions<Part>,
}

impl Render for Card {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let width = if self.expanded { 320.0 } else { 180.0 };

        let opacity = if self.expanded { 1.0 } else { 0.5 };

        let panel_motion = Motion::new(Target::default().width(width));

        let title_motion = Motion::new(Target::default().opacity(opacity));

        let [panel, title] = self.motions.frame(
            [
                (Part::Panel, panel_motion),
                (Part::Title, title_motion),
            ],
            window,
            cx,
        );

        panel.apply(
            div().p_4().child(
                title.apply(div().child("Project details")),
            ),
        )
    }
}
```

Change `expanded` from your input handler and call `cx.notify()`. Both entries
sample at one timestamp. Springs preserve position and velocity on interruption;
per-entry delays, transitions, property overrides, and presence work as usual.
A native batch queues at most one animation-frame request, and none when all
returned frames are settled or absent. Reduced motion snaps submitted entries.

Results follow the array order; history belongs to the keys. Reordering entries
is safe. Keys need `Eq + Hash`, not `Clone` or `Default`; use enums or stable item
IDs rather than a list's current indices. Duplicate keys in one batch panic as a
programmer error. A batch is not transactional: earlier entries may be sampled
before a duplicate is encountered. Use small array batches for fixed parts;
`frame_one` is also convenient for individual entries or dynamic list loops.

### Explicit cleanup

Omitting a key neither deletes it nor pauses its clock. It does not request frames
while omitted; when submitted again, its old trajectory is evaluated at the new
time. To animate removal, keep declaring `present(false)` until its frame is
absent, then remove the key. Do not recreate an already-cleaned outgoing entry:

```rust
use gpui_kit::{AnyElement, App, Empty, Window, div, prelude::*};
use gpui_motion_kit::{Motions, core::{Motion, Target}};

fn notice(
    motions: &mut Motions<&'static str>,
    visible: bool,
    window: &mut Window,
    cx: &App,
) -> AnyElement {
    if !visible && !motions.contains_key(&"notice") {
        return Empty.into_any_element();
    }

    let declaration = Motion::new(Target::default())
        .initial(Target::default().y(8.0).opacity(0.0))
        .exit(Target::default().y(-8.0).opacity(0.0))
        .present(visible);

    let frame = motions.frame_one("notice", declaration, window, cx);

    if !frame.is_mounted() {
        motions.remove(&"notice");
    }

    frame.render(|frame| frame.apply(div().child("Changes saved")))
}
```

Re-entry during an exit preserves momentum. Removing a key cancels it immediately;
reusing it starts fresh from `initial`. Guard input and restore focus for interactive
exits, just as with individual states.

| Method | Contract |
| --- | --- |
| `new()` / `default()` | Empty owner; no allocation |
| `with_capacity(n)` / `reserve(additional)` | Reserve table storage without creating trajectories |
| `frame([(key, motion), ...], window, cx)` | Fixed-size array in, same-size array of native frames out |
| `frame_one(key, motion, window, cx)` | Sample one native entry |
| `sample(batch, now, reduced_motion)` / `sample_one(key, motion, now, reduced_motion)` | Explicit elapsed `Duration`; backward times clamp; do not mix with native clocks |
| `len()` / `is_empty()` / `contains_key(&key)` | Inspect retained membership, including settled and absent entries |
| `remove(&key)` | Cancel immediately; return whether the key existed |
| `retain(keep)` | Cancel entries rejected by the predicate; does not wait for exits |
| `clear()` | Cancel all entries; keep allocated table capacity and the clock |
| `capacity()` / `shrink_to_fit()` | Inspect capacity / release unused table storage |

Existing entries reuse storage. Each new key allocates one state; the table may
also grow. `with_capacity` reserves table space, not those states. Keys that
allocate when constructed remain the caller's cost. Array inputs and outputs use O(batch size)
stack storage. `retain` visits table capacity; do cleanup/reservation outside hot
loops. Dropping the owner releases all its storage. `Motions` covers ordinary
six-channel `Motion` declarations; text, SVG, styles and typed playback retain
their specialized APIs. A graph is still useful when many effects derive from
one progress value.

Try **10 Collection** in `motion_lab` on either backend: expand/reverse the panel,
watch its independently animated title and progress, and dismiss/re-enter the notice.
Its outgoing state is removed only after the exit completes. See the
[sampling, memory and native measurements](../../PERFORMANCE.md#keyed-collections--2026-10-19).

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
Mouse/keyboard and native pinch are supported. Recognized touch dragging is
opt-in with `touch_drag(true)`. See the measured-input and spatial-reorder sections
for handles, direction locking and edge scrolling. OS-wide capture remains platform work.

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

`ReorderGroup` owns logical order and measured variable-size items. `new` chooses
a single axis; `spatial` also supports automatic detection and 2D grids.
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
persistence and focus after removal. See [spatial reorder](#spatial-reorder-and-edge-scrolling)
for grid collision behavior and optional edge autoscroll.

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

## Reactive values and subscriptions

`MotionGraph` retains sources, derived values, and subscriptions. A sample updates
all dependencies before dispatching events. Keep the subscription handle alive;
dropping it unsubscribes. Prepare mappings once, outside rendering.

```rust
use gpui_motion_kit::core::{MotionGraph, MotionEventKind, Transition};
use std::time::Duration;

let mut graph = MotionGraph::default();
let progress = graph.source(0.0);
let x = graph.derive(&[progress], |values| values[0] * 240.0).unwrap();
let opacity = graph.derive(&[progress], |values| 0.4 + values[0] * 0.6).unwrap();

let subscription = graph.subscribe(x, |event| {
    if event.kind == MotionEventKind::Complete {
        println!("Arrived at {}", event.value);
    }
}).unwrap();

graph.animate(progress, 1.0, Transition::default(), Duration::ZERO).unwrap();
graph.sample(Duration::from_millis(100), false);

let position = graph.get(x).unwrap();
let alpha = graph.get(opacity).unwrap();
```

`set` records direct input velocity; `jump` resets momentum. `animate` preserves
spring momentum. Events are `Start`, `Cancel`, `Change`, and `Complete`. Multiple
writes coalesce per sample. Callbacks may cancel subscriptions or update another
owner; defer edits to the graph being sampled until callbacks return.

Use `typed_source`, `animate_typed`, and `typed_value` for colors, `Target`,
`StyleTarget`, `SvgTarget`, `TextTarget`, arrays, or your own `MotionData<N>` type.
`target(base, &[(Property, ValueId)])` binds graph values to native target fields.

## Parent and child sequences

A retained `MotionTree` propagates variants, staggers siblings, and sequences
parents before or after children. Add parents first. Keys are application data.

```rust
use gpui_motion_kit::core::{ChildrenOrder, Motion, MotionConfig, MotionNode, MotionTree, Target, Variants};
use std::time::Duration;

let variants = || Variants::new([
    (true, Motion::new(Target::default().y(0.0).opacity(1.0))),
    (false, Motion::new(Target::default()).present(false)
        .exit(Target::default().y(12.0).opacity(0.0))),
]).unwrap();

let mut tree = MotionTree::default()
    .config(MotionConfig::default().stagger(Duration::from_millis(40)));

tree.add(MotionNode::new("panel", Target::default(), variants())
    .when(ChildrenOrder::BeforeChildren)
    .exit_when(ChildrenOrder::AfterChildren)).unwrap();
tree.add(MotionNode::new("title", Target::default(), variants())
    .parent("panel")).unwrap();

tree.set_variant(&false, Duration::ZERO, false).unwrap();
tree.sample(Duration::from_millis(100), false);

let retain_panel = tree.is_mounted(&"panel");
let title = tree.get(&"title").unwrap();
```

`Together`, `BeforeChildren`, and `AfterChildren` choose the timing relationship.
`exit_when` optionally chooses a separate policy for leaving.
`inherit(false)` blocks ancestor variant propagation into that subtree;
`set_node_variant` explicitly selects one subtree. `set_motion` accepts a dynamic
Rust-computed declaration for one node. Identical declarations do not restart.
Gated animations hold their local clock and momentum. Nested exits keep a parent
mounted until every participating descendant exits; a non-exiting child keeps it
mounted. The application removes the view and restores focus afterward.

`MotionConfig::transition(...).motion(target)` supplies explicit defaults to your
render helpers. It installs no global provider and does not override OS preferences.

## Typed clips and mixed timelines

`TypedAnimation<T, N>` supports scalars, colors, numeric targets, styles, SVG
transforms, text targets, and arrays. Implement `MotionData<N>` for another fixed
numeric representation. Seed and clip must declare the same property mask.

```rust
use gpui_motion_kit::core::{Color, Easing, SvgTarget, TypedAnimation, TypedTimeline};
use std::time::Duration;

let mut timeline = TypedTimeline::default();

let color = timeline.at(
    Duration::ZERO,
    Color::rgb(0x223344),
    TypedAnimation::to(Color::rgb(0xd8f58b), Duration::from_millis(400), Easing::EaseInOut).unwrap(),
);

timeline.label("turn", Duration::from_millis(120)).unwrap();

let icon = timeline.at_label(
    "turn",
    -0.04,
    SvgTarget::default(),
    TypedAnimation::to(SvgTarget::default().rotate_degrees(360.0), Duration::from_millis(600), Easing::EaseInOut).unwrap(),
).unwrap();

timeline.sample(Duration::from_millis(300), false);

let background = timeline.value(color).unwrap();
let transform = timeline.value(icon).unwrap();
```

Use `TypedKeyframe::new/current/easing`, `TypedAnimation::keyframes/to/spring`,
`delay`, and `repeat` to prepare clips. `TypedState::sample` plays one typed value
with per-channel spring interruption. `TypedTimeline::after` accepts signed
seconds for gaps or overlaps; adding after a forever track returns an error.
Track IDs belong to their creating timeline. Setup allocates; sampling reuses state.

Both owners support pause, play, seek, restart, speed (including reverse), and
completion. `TypedState::finish` finishes even a forever clip. Applying sampled
values uses the existing numeric/style/SVG/text adapters. Kit still rejects blur
and backdrop blur through `StyleFrame::try_apply`; typed tracks add no renderer hooks.

## Completion futures

`AnimationState`, `TypedState`, `TypedTimeline`, and `MotionTree` expose
`completion()`. Requesting a future allocates observer storage; ordinary sampling
does not. Await it on your application's executor while native frames drive the
animation. Completion, replacement, explicit cancellation, restart, and owner drop
all resolve the observer instead of leaving a waiting task behind.

```rust
use gpui_motion_kit::core::{Animation, AnimationState, CompletionStatus, Easing, Target};
use std::time::Duration;

let clip = Animation::to(Target::default().x(80.0), Duration::from_millis(300), Easing::Linear);
let mut state = AnimationState::default();

state.sample(&clip, Duration::ZERO, false);

let finished = state.completion();

state.sample(&clip, Duration::from_millis(300), false);

async move {
    match finished.await {
        CompletionStatus::Completed => println!("Finished"),
        CompletionStatus::Cancelled => println!("Replaced or cancelled"),
    }
};
```

## SVG path preparation and following

`PathShape::parse_svg` accepts SVG path data, including relative, smooth, implicit,
and elliptical arc commands. Enable `svg-path` for portable use; the native
adapters enable it. This parses path data, not an SVG document or CSS.

```rust
use gpui_motion_kit::core::{PathMeasure, PathShape};

let circle = PathShape::parse_svg("M180 100 A80 80 0 1 1 20 100 A80 80 0 1 1 180 100 Z").unwrap();
let triangle = PathShape::parse_svg("M100 20 L185 170 L15 170 Z").unwrap();

let (from, to) = circle.normalize_for_morph(&triangle, 72).unwrap();
let path = PathShape::parse_svg("M10 130 C100 0 180 220 310 60").unwrap();
let measure = PathMeasure::new(&path).unwrap();

let point = measure.point(0.4);
let position = point.position;
let angle_degrees = point.tangent_degrees;
```

Feed the normalized pair to `PathMorphState` or `VectorState`; feed measured
position/tangent to a numeric target or SVG transform. Normalize and measure once.
Queries allocate nothing. Curves use 32 subdivisions; normalization resamples to
line contours with your chosen point budget. Contour count, order, open/closed
status, start points, and winding must correspond. It does not infer semantic
shape correspondence. Subpath gaps do not become visible connecting segments.

## Rich text, vertical reveals, and decimal counters

```rust
use gpui_motion_kit::core::{Color, FixedNumber, NumberFormat, RichText, TextSpan};

let title = RichText::new([
    TextSpan::new("Café ").font_weight(500.0),
    TextSpan::new("in motion.").font_weight(700.0).color(Color::rgb(0xd8f58b)),
]);

let euros = NumberFormat::default()
    .prefix("€")
    .decimal(",")
    .grouping(" ", 3, 3);

let value = FixedNumber::new(120900, 2).unwrap();
let formatted = euros.format(value, 0);

assert_eq!(formatted.text(), "€1 209,00");
```

`TextSpan::font_family` and `font_weight` choose native faces per span. The full
string is shaped before units split. Font size and line height remain label-wide.
This is a display-label API; selection/editing and variable-font-axis interpolation
are outside its contract. Numeric endpoints use an exact i64 coefficient plus
0–18 decimal places. Separators, affixes, digit alphabets, and 3/3 or 3/2 grouping
are explicit; it does not bundle automatic CLDR/ICU locale rules. Currency conversion
and rounding remain application decisions. Digit identities follow decimal places
across grouping/affix changes and integer growth. Affixes and separators have
distinct identities; the chosen effect applies to changed glyphs.

## Native graph and tree owners

Retain `GraphState` or `MotionTreeState` in your view. Call `frame` once before
reading bound values. Use `now(cx)` for input/control timestamps; do not mix that
executor clock with an unrelated `Instant` or a manually sampled epoch. Notify
the owning view after edits. Idle or paused owners stop requesting frames.

```rust
use gpui_motion_kit::{GraphState, MotionTreeState};
use gpui_motion_kit::core::MotionTree;

let graph = GraphState::default();
let tree = MotionTreeState::new(MotionTree::<u8, bool>::default());
```

`TypedState` and `TypedTimeline` also provide native `frame` and `now` methods.
Reduced motion settles pending transitions and bypasses tree gates/staggers.

## Measured scroll targets and viewport roots

Retain one `MotionAnchor` per measured element and mount each once. Anchors use
logical window coordinates, keep native input/accessibility, and request a follow-up
frame after changed measurements. Clear an anchor before reusing it in another window.

```rust
use gpui_kit::{ScrollHandle, div, prelude::*, px};
use gpui_motion_kit::{Axis, MotionAnchor, ScrollTracker};
use gpui_motion_kit::core::{ScrollEdge, ScrollOffsets};

let scroll = ScrollHandle::new();
let viewport = MotionAnchor::default();
let target = MotionAnchor::default().scroll_source(scroll.clone());

let mut tracker = ScrollTracker::new(scroll.clone(), viewport.clone(), target.clone(), Axis::Y)
    .offsets(ScrollOffsets::new(
        ScrollEdge::new(0.0, 1.0, 0.0),
        ScrollEdge::new(1.0, 0.0, 0.0),
    ))
    .margins([16.0, 0.0, 16.0, 0.0]);

let panel = viewport.element("viewport", div().id("scroll")
    .w(px(300.0)).h(px(200.0)).overflow_scroll().track_scroll(&scroll)
    .child(target.element("target", div().h(px(600.0)))));
```

Call `tracker.frame(window, cx)` during rendering. The returned `ScrollFrame`
contains readiness, progress, velocity, offsets, and active status; `target(from,
to)` maps progress to numeric targets. Optional `transition` smooths with the shared
solver. Negative margins expand the viewport; `ScrollOffsets::unclamped` permits
anticipation. Explicit `scroll_source` avoids stale measurements between prepaints.
For nested scrolling, track the nearest source and measure again after ancestor
scrolling/layout; this is not an automatic browser-style ancestor observer.

`GestureState::view_root(anchor).view_margins([top, right, bottom, left])` customizes
in-view observation while preserving normal ancestor clipping.

## Measured drag bounds, handles, and pinch

```rust
use gpui_motion_kit::{GestureState, MotionAnchor};
use gpui_motion_kit::core::{Drag, GesturePhase};

let bounds = MotionAnchor::default();

let gesture = GestureState::default()
    .with_drag(Drag::unconstrained([0.0, 0.0]).direction_lock(8.0))
    .constraints(bounds.clone())
    .on_drag(|event, _, _| {
        if event.phase == GesturePhase::Ended {
            println!("Released at {:?}", event.offset);
        }
    })
    .on_pinch(|event, _, _| println!("Pinch scale: {}", event.scale));
```

Mount `bounds.element(...)`, then wrap the control with `gesture.bind(...)`.
Measured constraints update on resize. Set `.drag_listener(false)` on the binding
and call `gesture.start_drag([x, y], window, cx)` from a separate handle's mouse-down
handler. Focus, keyboard equivalents, cancellation, release outside, and smooth
inertial interruption remain supported. `DragEvent` reports phase, point, delta,
offset, velocity, and direction lock. Callbacks run after internal borrows end;
use weak view/entity captures rather than retaining the same owner in its callback.
Pinch reports native trackpad lifecycle; mapping its scale to an SVG or another
supported property is explicit. It cannot scale an ordinary element subtree.

On stock Kit, `.touch_drag(true)` opts into its recognized touch-drag stream.
Spatial reorder also consumes that stream. The OS/backend determines which
touch and pinch gestures are delivered; physical mobile-device acceptance is separate.

## Spatial reorder and edge scrolling

```rust
use gpui_kit::ScrollHandle;
use gpui_motion_kit::{MotionAnchor, ReorderGroup};
use gpui_motion_kit::core::{AutoScroll, ReorderAxis};

let scroll = ScrollHandle::new();
let viewport = MotionAnchor::default();

let group = ReorderGroup::spatial(vec!["one", "two", "three"], ReorderAxis::Auto)
    .unwrap()
    .autoscroll(scroll.clone(), viewport.clone(), AutoScroll::default());
```

Render `group.order()` inside your ordinary row/column/wrapped grid, wrapping each
stable-key item with `group.item(...)`. `Auto` measures whether it is a row,
column, or grid; explicit `X`, `Y`, and `XY` are available. Pointer collisions use
nearest measured slots. Batched pointer/key input keeps the latest measured slot
map stable until layout catches up. Option/Alt + arrows choose a physical neighbor
in grids. Auto chooses its axis at acquisition.
Use `.drag_listener(false)` plus `start_drag(&key, point, window, cx)` for handles. External `set_order` changes require a fresh layout before acquiring
another drag.
Edge scrolling uses elapsed time and native bounds, and stops on release, Escape,
blur, deactivation, or reaching a limit. One pointer owns a group. This is a measured
reorder list/grid, not a virtualized arbitrary drag/drop data-transfer framework.

## Cross-container shared movement

```rust
use gpui_kit::{div, prelude::*, px};
use gpui_motion_kit::SharedOverlay;

let overlay = SharedOverlay::<u8>::default();
let source = overlay.anchor(1, "inbox", div().w(px(140.0)).h(px(80.0)));
let destination = overlay.anchor(1, "today", div().w(px(140.0)).h(px(80.0)));

overlay.select(1, "today");

let floating = overlay.element(1, "moving-card", div()
    .w(px(140.0)).h(px(80.0)).child("Read this later"));
```

Anchors may live in separate clipped containers. Render floating appearances in
an **unclipped window-root overlay** so movement can cross them. `scroll_anchor`
tracks a scroll source; measurements must belong to the same native window.
`active(false)` crossfades a retiring appearance; retain it while exiting, disable
its input, and manage focus in the host. `match_size(true)` uses destination sizes
immediately through real layout. Position uses interruptible shared springs;
this provides no GPU size projection. `remove(&key)` releases completed history.

## Native rich text and formatted numbers

```rust
use gpui_kit::{div, prelude::*, px};
use gpui_motion_kit::{NumberMotionState, TextMotionState};
use gpui_motion_kit::core::{Color, FixedNumber, NumberFormat, RichText, TextPreset, TextSpan, Transition};

let text = TextMotionState::default();
let number = NumberMotionState::default();

let rich = RichText::new([
    TextSpan::new("Café "),
    TextSpan::new("in motion.").color(Color::rgb(0xd8f58b)),
]);

let label = text.rich_element("title", &rich)
    .preset(TextPreset::WipeDown)
    .color_transition(Transition::default())
    .animate_font_size(48.0, Transition::default());

let format = NumberFormat::default().prefix("€").grouping(",", 3, 3);
let count = number.formatted_element("balance", FixedNumber::new(120900, 2).unwrap(), &format);

let content = div().font_family("Helvetica Neue").text_size(px(40.0))
    .child(label)
    .child(count);
```

`WipeDown` and `WipeUp` complement existing horizontal reveals;
`reveal_axis(TextRevealAxis::Vertical)` configures custom wipes. Colors interpolate
in linear light, retaining per-glyph state. `NumberElement` also forwards
`color_transition` and `animate_font_size`. Font size animates by native shaping
and layout, so it costs more than opacity/translation. Pause/play freezes glyph
motion, font-size and color transitions; normalized seek/restart controls the
entrance sequence, not style history. Keep one mounted element per text state.

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

## Motion lab

Run `cargo run --features demo --example motion_lab` from this directory.
Ten interactive scenes cover connected values, child sequences, mixed timelines,
scroll intersections, measured handles/pinch, grid reorder/autoscroll, cross-container
transfer, parsed paths/morphs/following, rich labels and currency counters, and
keyed batches with explicit notice cleanup.


The expanded engine's CPU benchmarks, allocation checks and native acceptance are
recorded in [the performance report](../../PERFORMANCE.md#expanded-shared-engine--core-030--kit-040).
