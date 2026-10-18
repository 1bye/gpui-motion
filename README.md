# gpui-ce-motion

**Declarative animation for native Rust apps on GPUI CE.**

Describe the next state. Keep the motion state in your view. The library handles
transitions, springs, and interruptions using stock GPUI CE—no React or renderer fork.

[Crates.io](https://crates.io/crates/gpui-ce-motion) ·
[API examples](#api) · [Demos](docs/DEVELOPMENT.md#demos) ·
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

| Feature        | Use it for                                                              |
| -------------- | ----------------------------------------------------------------------- |
| Default (none) | Portable animation math, explicit clocks, no third-party dependencies   |
| `text`         | Portable Unicode text lifecycle and prepared text sampling               |
| `gpui`         | Native adapters, text, input bindings, and automatic frame scheduling    |
| `demo`         | Running this repository's examples; adds the platform backends          |
| `native-tests` | Development checks using CE's TestPlatform                              |

## Quick start

This is a GPUI component for an existing app. Click the card to change its width.
Changing a target during animation continues from the displayed position; springs
preserve velocity too.

```rust
use gpui::{Context, Render, Window, div, prelude::*, rgb};
use gpui_ce_motion::{Motion, MotionState, Target};

#[derive(Default)]
struct Card {
    expanded: bool,
    motion: MotionState,
}

impl Render for Card {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let width = if self.expanded { 320.0 } else { 180.0 };

        let target = Target::default()
            .width(width)
            .height(100.0)
            .border_radius(16.0);

        let animation = Motion::new(target).initial(Target::default().y(12.0).opacity(0.0));

        let frame = self.motion.frame(animation, window, cx);

        let card = div()
            .id("card")
            .bg(rgb(0xd4f580))
            .text_color(rgb(0x101310))
            .p_4()
            .child("Click to resize");

        frame.apply(card).on_click(cx.listener(|this, _, _, cx| {
            this.expanded = !this.expanded;
            cx.notify();
        }))
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

## API

The examples below cover every public API family. Each example stands on its own.
Native elements require the `gpui` feature and an existing GPUI view.

Keep states, groups, clips, and timelines in your view between renders. Build new
targets as the UI changes. The native helpers below receive that retained state
as a function argument; the portable examples use an explicit clock.

With portable sampling, `sample(..., now, reduced_motion)` takes elapsed time and
the user's reduced-motion preference. In a GPUI view, `frame(..., window, cx)`
handles the clock, frame requests, and that preference for you.

- **Animation:** [Targets](#targets-and-transitions), [timing](#timing),
  [enter and exit](#enter-and-exit), [keyed presence](#keyed-presence).
- **Playback:** [Clips](#clips-and-playback), [timelines and stagger](#timelines-and-stagger),
  [variants](#variants).
- **Interaction:** [Gestures](#native-gestures), [dragging](#dragging),
  [scroll and visibility](#scroll-and-visibility), [reordering](#reordering).
- **Layout:** [Positions](#layout-positions), [popped exits](#popped-exits),
  [content sizing](#content-sizing), [shared visuals](#shared-visuals).
- **Drawing:** [Styles](#style-animation), [text and numbers](#text-motion),
  [SVG transforms](#svg-transforms),
  [vector paths](#vector-paths), [motion values](#motion-values), [colors](#colors).
- **Reference:** [Lifecycle and accessibility](#lifecycle-and-accessibility),
  [validation](#validation), [all public exports](#public-export-index).

## Targets and transitions

`Target` supports `x`, `y`, `opacity`, `width`, `height`, and `border_radius`.
Offsets and sizes are logical pixels. A changed destination interrupts from the
current presentation; springs retain velocity. Ordinary motion defaults to a spring.

```rust
use gpui_ce_motion::{Motion, MotionState, Target};
use std::time::Duration;

let mut state = MotionState::default();

let start = Target::default()
    .x(0.0)
    .width(120.0)
    .height(100.0);

let destination = Target::default()
    .x(120.0)
    .width(240.0)
    .height(100.0);

let motion = Motion::new(destination).initial(start);

// Use one clock for every sample. Keep state between calls.
state.sample(motion, Duration::ZERO, false);

let frame = state.sample(motion, Duration::from_millis(100), false);

let current_width = frame.target().width_px();
```

In a GPUI view, use `state.frame(motion, window, cx)`, then `frame.apply(element)`.
Position uses relative left/top offsets. Width and height affect layout, so text
and siblings can reflow as they change. Use an outer wrapper if the element also
needs absolute positioning.

Give dimensions an initial numeric value to animate them on first mount.
Otherwise, the first value is applied immediately. Border radius does not clip
children automatically. `Target::overlay` combines partial targets, and the
getters on `frame.target()` let you read the current values.

## Timing

Use a tween when you want an exact duration, or a spring when you want movement
that responds naturally to interruption. `Transition::Instant` skips animation.

### Tweens

```rust
use gpui_ce_motion::{Easing, Motion, Target, Transition};
use std::time::Duration;

let slide = Transition::Tween(Duration::from_millis(200), Easing::EaseOut);

let motion = Motion::new(Target::default().x(100.0))
    .transition(slide)
    .delay(Duration::from_millis(50));
```

Built-in easing includes `Linear`, `EaseIn`, `EaseOut`, `EaseInOut`,
`CircIn` / `CircOut` / `CircInOut`, `BackIn` / `BackOut` / `BackInOut`,
and `Anticipate`. An interrupted tween keeps its current position; its velocity
can change.

### Springs

Set stiffness, damping, and mass directly:

```rust
use gpui_ce_motion::{Motion, Spring, Target, Transition};

let spring = Spring::new(240.0, 26.0, 1.0).expect("positive stiffness, damping, and mass");

let motion = Motion::new(Target::default().x(100.0)).transition(Transition::Spring(spring));
```

Or describe the spring with a visual duration and bounce. Prepare it once and
reuse it in your view:

```rust
use gpui_ce_motion::{Motion, Spring, Target, Transition};
use std::time::Duration;

let spring = Spring::visual(Duration::from_millis(320), 0.2)
    .expect("positive duration and bounce between -1 and 1");

let spring = spring
    .rest(0.01, 0.01)
    .expect("positive distance and velocity thresholds");

let motion = Motion::new(Target::default().x(100.0)).transition(Transition::Spring(spring));
```

The visual duration is the time to reach 90% of a resting step. The spring can
continue settling afterward. Springs preserve both position and velocity when
you change their target.

### Timing per property

A card can move with a spring and fade with a short tween:

```rust
use gpui_ce_motion::{Easing, Motion, Property, Target, Transition};
use std::time::Duration;

let fade = Transition::Tween(Duration::from_millis(120), Easing::EaseOut);

let motion = Motion::new(Target::default().x(100.0).opacity(1.0))
    .initial(Target::default().opacity(0.0))
    .transition(Transition::default())
    .transition_for(Property::Opacity, fade, Duration::ZERO);
```

`Property` selects `X`, `Y`, `Opacity`, `Width`, `Height`, or `BorderRadius`.
The last argument to `transition_for` is that property's delay.

### Custom easing

Use a cubic Bézier curve for a familiar easing editor workflow:

```rust
use gpui_ce_motion::{Bezier, Easing, Transition};
use std::time::Duration;

let curve = Bezier::new(0.2, 0.0, 0.2, 1.0).expect("valid Bézier control points");

let transition = Transition::Tween(Duration::from_millis(200), Easing::Bezier(curve));
```

A custom function returns the easing value and its derivative. Prepare the
`CustomEasing` once; creating it again gives it a new identity.

```rust
use gpui_ce_motion::{CustomEasing, Easing, Transition};
use std::time::Duration;

let ease_in = CustomEasing::new(|progress| {
    let value = progress * progress;
    let derivative = 2.0 * progress;

    (value, derivative)
})
.expect("finite easing with endpoints at 0 and 1");

let transition = Transition::Tween(Duration::from_millis(200), Easing::Custom(ease_in));
```

## Enter and exit

Keep declaring the element while `present(false)` so its exit can finish.
`Frame::render` returns an empty element after the exit completes. Re-entry during
exit continues from the current presentation, using the same retained state.

```rust
use gpui::{AnyElement, App, Window, div, prelude::*};
use gpui_ce_motion::{Motion, MotionState, Target};

fn panel(state: &mut MotionState, open: bool, window: &mut Window, cx: &App) -> AnyElement {
    let motion = Motion::new(Target::default())
        .initial(Target::default().y(12.0).opacity(0.0))
        .exit(Target::default().y(-8.0).opacity(0.0))
        .present(open);

    let frame = state.frame(motion, window, cx);

    frame.render(|frame| {
        let content = div().id("panel").child("Hello");

        frame.apply(content)
    })
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

let mut notifications = PresenceGroup::new(PresenceMode::Wait);

notifications
    .update([1_u32, 2])
    .expect("unique notification IDs");

// Notification 1 stays mounted for its exit. Notification 3 waits.
notifications
    .update([2, 3])
    .expect("unique notification IDs");

let outgoing = notifications
    .children()
    .iter()
    .find(|child| *child.key() == 1)
    .expect("outgoing notification is retained");

assert!(!outgoing.is_present());

// Call this only after notification 1's motion reaches Presence::Absent.
notifications.finish_exit(&1);
```

Keep each child's motion state in a map keyed by its domain ID. Iterate
`children()`, pass `child.is_present()` into `Motion::present`, and remove the
state only after `finish_exit`. See [the native gallery](examples/motion_native.rs).

## Clips and playback

Prepare the clip once. Its clones share keyframes; constructing a new clip means
a replacement trajectory. Keyframe times increase strictly from `0` to `1`.
`Keyframe::current(0.)` captures the displayed start; a later `current` holds the
previous target. Segment easing belongs to the keyframe ending that segment.

```rust
use gpui_ce_motion::{Animation, AnimationState, Easing, Keyframe, Repeat, RepeatMode, Target};
use std::time::Duration;

let keyframes = [
    Keyframe::current(0.0),
    Keyframe::new(0.5, Target::default().x(100.0)).easing(Easing::EaseOut),
    Keyframe::new(1.0, Target::default().x(0.0)),
];

let clip = Animation::keyframes(Duration::from_millis(800), keyframes)
    .expect("valid keyframe times")
    .delay(Duration::from_millis(100))
    .repeat(
        Repeat::Forever,
        RepeatMode::Mirror,
        Duration::from_millis(80),
    )
    .expect("positive repeat period");

let mut player = AnimationState::new(Target::default());

player.sample(&clip, Duration::ZERO, false);

// In an input handler, pause and scrub using the same clock.
let now = Duration::from_millis(200);

player.pause(now);
player.seek(Duration::from_millis(400), now);
player.set_speed(0.5, now).expect("finite playback speed");
player.play(now);

let frame = player.sample(&clip, now, false);
```

Also available: `Animation::to` for a two-endpoint tween, `Animation::spring` for
a physical spring with an explicit deadline, `Repeat::Count(n)` for **n extra
cycles**, and `Loop`/`Reverse`/`Mirror` repeat modes. Loop can jump at its boundary.

| Control/readout                | Meaning                                                            |
| ------------------------------ | ------------------------------------------------------------------ |
| `pause(now)` / `stop(now)`     | Hold the current playhead; stop is a pause alias                   |
| `play(now)`                    | Resume                                                             |
| `seek(playhead, now)`          | Scrub, including backward seeks                                    |
| `set_speed(speed, now)`        | Change rate; negative values play backward                         |
| `restart(now)` / `finish()`    | Start over / complete an `AnimationState`                          |
| `iteration()` / `is_playing()` | Read cycle count / playback state                                  |
| `Playback`                     | A standalone clock with time, pause/play, seek, and speed controls |

For native rendering, call `player.frame(&clip, window, cx).apply(element)`.
**Use `player.now()` in its input handlers**, rather than another epoch. Notify the
view after controls change. Portable calls use one application epoch consistently.

## Timelines and stagger

Use a timeline to start animations together, one after another, or relative to a
named point. Each track returns an index you can use to read its current frame.
Build the timeline once and keep it in your view.

```rust
use gpui_ce_motion::{Animation, Easing, Target, Timeline};
use std::time::Duration;

let slide = Animation::to(
    Target::default().x(100.0),
    Duration::from_millis(300),
    Easing::EaseOut,
);

let mut timeline = Timeline::default();

let first_card = timeline.at(Duration::ZERO, slide.clone());

// Place a label at the end of the first animation.
timeline.label("after-first").expect("unique label");

// The second card starts when the first finishes.
let second_card = timeline
    .then(slide.clone())
    .expect("finite first animation");

// The third card starts 80 ms after the label, overlapping the second.
let third_card = timeline
    .at_label("after-first", Duration::from_millis(80), slide.clone())
    .expect("known label");

timeline.restart(Duration::ZERO);

let frames = timeline.sample(Duration::from_millis(150), false);

let first_card_position = frames[first_card].target().x_px();
```

To stagger a group, calculate a delay for each item:

```rust
use gpui_ce_motion::{Animation, Easing, Keyframe, StaggerFrom, Target, Timeline, stagger};
use std::time::Duration;

let keyframes = [
    Keyframe::new(0.0, Target::default().opacity(0.0)),
    Keyframe::new(1.0, Target::default().opacity(1.0)).easing(Easing::EaseOut),
];

let fade_in =
    Animation::keyframes(Duration::from_millis(200), keyframes).expect("valid keyframe times");

let mut timeline = Timeline::default();

for index in 0..5 {
    let delay = stagger(Duration::from_millis(40), index, 5, StaggerFrom::Center)
        .expect("item index is within the group");

    timeline.at(delay, fade_in.clone());
}
```

`StaggerFrom::Start`, `End`, and `Center` choose where the stagger begins.
Timeline controls include pause/play, seek, speed, and restart. In native
rendering, apply each frame to its matching element and request another frame
while `timeline.is_animating()`.

## Variants

Give reusable states names such as `"idle"` and `"expanded"`. Each variant stores
both a target and its timing. Keys can also be enums or another type you choose.

Here, the expanded variant determines the width, while hover changes the opacity.
`with_target` keeps the variant's timing when applying the combined target.

```rust
use gpui_ce_motion::{GestureTargets, Interaction, Motion, Target, Variants};

let variants = Variants::new([
    ("idle", Motion::new(Target::default().width(180.0))),
    ("expanded", Motion::new(Target::default().width(320.0))),
])
.expect("unique variant names");

let expanded = variants.get(&"expanded").expect("known variant");

let mut interaction = Interaction::default();

interaction.hovered = true;

let gestures = GestureTargets::default()
    .while_hover(Target::default().opacity(0.8))
    .while_focus(Target::default().y(-2.0));

let target = gestures.resolve(expanded.target(), interaction);

let motion = expanded.with_target(target);
```

## Native gestures

`GestureState` connects native input to animation targets. Store it in your view,
then bind it to the element you want to animate:

```rust
use gpui::{Role, div, prelude::*};
use gpui_ce_motion::{GestureElement, GestureState, GestureTargets, Motion, Target};

fn button(state: &GestureState) -> GestureElement {
    let content = div()
        .id("action")
        .role(Role::Button)
        .aria_label("Run action")
        .child("Run");

    let gestures = GestureTargets::default()
        .while_hover(Target::default().opacity(0.8))
        .while_press(Target::default().y(2.0))
        .while_focus(Target::default().opacity(0.9));

    state.bind(
        "action-motion",
        content,
        Motion::new(Target::default()),
        gestures,
    )
}
```

You can also set `while_drag` and `while_in_view` targets. If several interactions
set the same property, drag wins over press, hover, focus, and in-view, in that order.

Read `interaction()` for input state, `presented()` for the current position, and
`focus_handle()` for focus management. Use `cancel(cx)` when switching contexts.
`.enabled(false)` cancels input and removes tab stops; the app supplies disabled
semantics and restores focus. Mouse and keyboard are supported. OS-wide capture,
multi-touch, and automatic autoscroll require application support.

## Dragging

Attach a `Drag` to your gesture state to make its element draggable. This example
allows horizontal movement from 0 to 300 pixels, with a 20-pixel snap grid.
Keeping the minimum and maximum Y coordinates equal locks vertical movement.

```rust
use gpui_ce_motion::{Drag, GestureState};

let start = [0.0, 0.0];
let min = [0.0, 0.0];
let max = [300.0, 0.0];

let drag = Drag::new(start, min, max)
    .elasticity(0.15)
    .snap_grid(20.0)
    .momentum(true);

let gestures = GestureState::default().with_drag(drag);

// Store gestures in the view, then use gestures.bind(...) when rendering.
```

The portable `Drag` exposes `begin`, `move_to`, `end`, `cancel`, `move_by` (keyboard
movement), `sample`, and `is_dragging`. Its host must supply pointer timestamps,
release/cancellation handling, focus, and frame requests. A still hold before
release clears stale momentum. Re-grabbing starts at the displayed position.

## Scroll and visibility

Use a retained native `ScrollHandle` for both the scroll container and its progress
binding. Mapping is instant by default; `.transition(...)` adds smoothing.

```rust
use gpui::{ScrollHandle, div, prelude::*, rgb};
use gpui_ce_motion::{Axis, ScrollMotion, Target};

fn playlist(handle: &ScrollHandle) -> impl IntoElement {
    let progress_bar = div().h_1().bg(rgb(0xd4f580));

    let progress = ScrollMotion::new(
        "progress",
        handle.clone(),
        Axis::Y,
        Target::default().width(0.0),
        Target::default().width(220.0),
        progress_bar,
    );

    let tracks = (0..20).map(|index| div().h_8().child(format!("Track {index}")));

    let list = div()
        .id("playlist")
        .h_32()
        .overflow_y_scroll()
        .track_scroll(handle)
        .children(tracks);

    div().child(progress).child(list)
}
```

`native_scroll_progress(handle, axis)` exposes measured progress separately.
Portable helpers accept measured metrics directly; visibility bounds are
`[x, y, width, height]` in one coordinate space:

```rust
use gpui_ce_motion::{InView, scroll_progress, transform};

// The viewport is 400 px tall, with 1,000 px of scrollable content.
let progress = scroll_progress(150.0, 1000.0, 400.0);

let offset = transform(progress, [0.0, 1.0], [0.0, 200.0], true);

// Trigger at 50% visibility and stay triggered after the first entry.
let mut visibility = InView::new(0.5, true);

let viewport_bounds = [0.0, 0.0, 400.0, 300.0];
let item_bounds = [20.0, 20.0, 100.0, 100.0];

visibility.update(viewport_bounds, item_bounds);

let is_visible = visibility.is_visible();
```

For automatic native visibility, initialize `GestureState::default().in_view(0.5,
true)` and bind `GestureTargets::while_in_view(...)`. The native adapter measures
clipped bounds. `InView::is_visible` reads the portable observer's latest result.

## Reordering

Use a retained `ReorderGroup` for lists or draggable tabs. Render in `order()` and
keep each item's IDs and key stable. Group items accept a `Stateful<Div>`.

```rust
use gpui::{div, prelude::*};
use gpui_ce_motion::{Axis, ReorderGroup};

fn tab_strip(group: &ReorderGroup<usize>) -> impl IntoElement {
    let tabs = group
        .order()
        .iter()
        .map(|&key| {
            let tab = div()
                .id(("tab", key))
                .w_32()
                .h_10()
                .child(format!("Tab {key}"));

            group.item(("motion-tab", key), key, tab)
        })
        .collect::<Vec<_>>();

    div().flex().children(tabs)
}

// Create this once and store it in your view.
let tabs = ReorderGroup::new(vec![0, 1, 2], Axis::X).expect("unique tab IDs");
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
use gpui::{div, prelude::*, px};
use gpui_ce_motion::LayoutGroup;

fn cards(group: &LayoutGroup, keys: &[usize]) -> impl IntoElement {
    let cards = keys
        .iter()
        .map(|&key| {
            let card = div()
                .id(("card", key))
                .w(px(128.0))
                .h(px(64.0))
                .child(format!("Card {key}"));

            group.position(("position", key), card)
        })
        .collect::<Vec<_>>();

    div().flex().flex_wrap().gap_3().children(cards)
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
use gpui::{AnyElement, div, prelude::*};
use gpui_ce_motion::{Frame, PopLayout};

fn row(pop: &PopLayout, frame: Frame, present: bool) -> AnyElement {
    let card = frame.render(|frame| {
        let content = div().w_32().h_16().child("Card");

        pop.item("item", present, frame.apply(content))
    });

    let list = div().flex().gap_3().child(card);

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
    .width(256.0)
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
use gpui::{div, prelude::*, px};
use gpui_ce_motion::SharedLayout;

let shared = SharedLayout::<u32>::default();

let previous = div().absolute().left(px(0.0)).child("Old");
let destination = div().absolute().left(px(200.0)).child("New");

// Both members represent the same key. Only the destination is active.
let outgoing = shared.member(1, "previous", false, previous);

let incoming = shared.member(1, "destination", true, destination);

let overlay = div().relative().child(outgoing).child(incoming);
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
use gpui::{App, Window, div, prelude::*};

use gpui_ce_motion::{
    Color, ColorProperty, StyleMotion, StyleMotionState, StyleProperty, StyleTarget,
};

fn card(state: &mut StyleMotionState, window: &mut Window, cx: &App) -> impl IntoElement {
    let target = StyleTarget::default()
        .padding([12.0; 4])
        .margin([4.0; 4])
        .gap(8.0, 8.0)
        .radii([16.0, 4.0, 16.0, 4.0])
        .border_widths([1.0; 4])
        .color(ColorProperty::Background, Color::rgb(0x334466))
        .shadow([0.0, 4.0], 12.0, 0.0, Color::rgba(0.0, 0.0, 0.0, 0.2))
        .value(StyleProperty::Blur, 0.0);

    let motion = StyleMotion::new(target);

    let frame = state.frame(motion, window, cx);

    frame.apply(div().id("styled-card"))
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

## Text motion

These text APIs are unreleased and require this repository checkout.

Retain a `TextMotionState` per label. The element measures and shapes complete text,
then coordinates its units, outgoing content, and interrupted transitions.
The native `gpui` feature enables text automatically; portable consumers can enable
`text` alone. Kit consumers import native states from `gpui_motion_kit`.

```rust
use gpui::prelude::*;
use gpui_ce_motion::{StaggerFrom, TextMotionState, TextPreset, TextUnit, Transition};
use std::time::Duration;

fn title(state: &TextMotionState, text: String) -> impl gpui::IntoElement {
    state
        .element("title", text)
        .units(TextUnit::Words)
        .preset(TextPreset::FadeUp)
        .stagger(Duration::from_millis(35), StaggerFrom::Start)
        .transition(Transition::default())
        .text_2xl()
}
```

Groupings are `Whole`, `Words`, `Graphemes`, and measured `Lines`. Presets are
`Fade`, `FadeUp`, `SlideUp`, `WipeRight`, `WipeLeft`, and discrete `Typewriter`.
`animate_initial(false)` skips the first entrance while preserving later changes.

Custom poses and matching compose independently:

```rust
use gpui_ce_motion::{
    PresenceMode, TextClip, TextMatch, TextMotionState, TextTarget, TextUnit, Transition,
};

let state = TextMotionState::default();

let text = state
    .element("status", "Ready when you are.")
    .units(TextUnit::Graphemes)
    .matching(TextMatch::Content)
    .reflow(Transition::default())
    .enter(TextTarget::default().y_relative(1.0).opacity(0.0))
    .exit(TextTarget::default().y_relative(-1.0).opacity(0.0))
    .opacity_transition(Transition::Tween(
        std::time::Duration::from_millis(100),
        gpui_ce_motion::Easing::EaseOut,
    ))
    .clip(TextClip::Unit)
    .presence_mode(PresenceMode::Sync);
```

`TextTarget` supplies `x_em`, `y_em`, measured-height `y_relative`, `opacity`,
and horizontal `reveal`. SlideUp clears the entire measured unit; SlideUp and
wipes fade outgoing text over 100 ms. `opacity_transition(...)` changes that
opacity timing while `transition(...)` keeps control of movement and reveal.
`TextMatch::Content` preserves matching occurrences; `Replace` replaces changed
content and `Position` matches slots. `PresenceMode::Wait` waits for exits and
keeps the latest pending text. Font metrics and sizes change immediately.

Prepare a scramble alphabet once, or calculate each unit's entrance:

```rust
use gpui_ce_motion::{Scramble, TextMotionState, TextTarget, TextUnit};
use std::time::Duration;

let state = TextMotionState::default();

let scramble = Scramble::new("ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789")
    .duration(Duration::from_millis(500))
    .seed(42);

let scrambled = state
    .element("scrambled", "A new idea")
    .units(TextUnit::Graphemes)
    .scramble(scramble);

let another_state = TextMotionState::default();

let alternating = another_state
    .element("alternating", "Make room for the next idea")
    .units(TextUnit::Words)
    .enter_with(|unit| {
        let direction = if unit.index % 2 == 0 { -1.0 } else { 1.0 };

        TextTarget::default()
            .y_em(direction * 0.6)
            .opacity(0.0)
    });
```

`TextUnitInfo` provides `index`, `count`, and `text`. A retained prepared
`Animation` can be passed to `animation(&clip)` for keyframes and repeating
waves/pulses. Its x/y channels use logical pixels and opacity multiplies presence.

Playback controls work from ordinary event handlers:

```rust
use gpui_ce_motion::TextMotionState;

fn controls(state: &TextMotionState, window: &mut gpui::Window, cx: &gpui::App) {
    state.pause(window, cx);

    let accepted = state.seek(0.5, window, cx);

    state.play(window, cx);
    state.restart(window, cx);
}
```

`seek` holds a normalized finite entrance cursor and returns false for infinite
clips. `progress(value)` declaratively scrubs entrances including stagger; it
rejects infinite clips. Spring scrubbing uses a two-second horizon. `presentation`
copies diagnostics, `is_animating` reports activity, and `error` reports native
shaping/paint failures. Reduced motion settles without ongoing frame requests.

Counts have a separate retained state and place-value matching:

```rust
use gpui::prelude::*;
use gpui_ce_motion::{NumberEffect, NumberMotionState, Transition};

let counter = NumberMotionState::default();

let count = counter
    .element("download-count", 199)
    .effect(NumberEffect::Roll)
    .min_digits(3)
    .transition(Transition::default())
    .opacity_transition(Transition::Tween(
        std::time::Duration::from_millis(100),
        gpui_ce_motion::Easing::EaseOut,
    ))
    .text_2xl();
```

Number effects are `Roll`, `Crossfade`, `Slide`, and rounded numeric `Interpolate`.
Counts accept `i64`, including exact endpoints at its limits. Intermediate numeric
interpolation uses f64. `text_state()` exposes the digit layer's controls and
diagnostics, independently of numeric interpolation.

The [text contracts](docs/TEXT.md) cover shaping, clipping, playback, accessibility,
portable `TextPlaybackState`/`TextOptions`, errors, costs, and limits. This is
single-style display text; editing, selection, and inline rich text stay with GPUI.
Run the [Kit text gallery](integrations/gpui-kit/README.md#text-gallery) or stock-CE
`cargo run --features demo --example text_motion` for interactive examples.

## SVG transforms

Retain `SvgMotionState`; apply its frame to a normal GPUI SVG. Keep the asset path
and base dimensions stable so CE can reuse its cached drawing.

```rust
use gpui::{App, Window, prelude::*, px, svg};
use gpui_ce_motion::{SvgMotion, SvgMotionState, SvgTarget};

fn icon(state: &mut SvgMotionState, window: &mut Window, cx: &App) -> impl IntoElement {
    let target = SvgTarget::default()
        .scale(1.2)
        .rotate_degrees(90.0)
        .translate(8.0, 0.0);

    let motion = SvgMotion::new(target).initial(SvgTarget::default());

    let frame = state.frame(motion, window, cx);

    let arrow = svg().path("icons/arrow.svg").size(px(24.0));

    frame.apply(arrow)
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

let horizontal = PathShape::new([
    PathCommand::Move([0.0, 40.0]),
    PathCommand::Line([100.0, 40.0]),
])
.expect("valid path coordinates");

let diagonal = PathShape::new([
    PathCommand::Move([0.0, 20.0]),
    PathCommand::Line([100.0, 60.0]),
])
.expect("valid path coordinates");

// Store this in the view so changes to the target path can animate.
let vector = VectorState::new(horizontal);

let reveal =
    StrokeReveal::new(0.7, 0.0, 1.0).expect("valid reveal length, offset, and spacing");

let drawing = vector
    .element("line", diagonal, [120.0, 80.0], Color::rgb(0xd4f580))
    .expect("compatible paths and valid drawing bounds")
    .stroke(3.0)
    .reveal(reveal);
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

let mut progress = MotionValue::new(0.0);

progress.animate(100.0, Transition::default(), Duration::ZERO);

let now = Duration::from_millis(100);

let frame = progress.sample(now, false);

let opacity = transform(frame.value(), [0.0, 100.0], [0.0, 1.0], true);

// Continue from the current velocity, staying within these bounds.
let inertia = Inertia::new(Duration::from_millis(250)).bounds(0.0, 200.0);

progress.coast(inertia, now);
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

let mut background = ColorState::new(Color::rgb(0x334466));

let destination = Color::rgb(0xd4f580);
let transition = Transition::default();

background.sample(destination, transition, Duration::ZERO, false);

let (current, is_animating) =
    background.sample(destination, transition, Duration::from_millis(100), false);

let native_color = current.to_rgba();
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

let motion = Motion::new(Target::default().x(100.0)).initial(Target::default());

let reduced_motion = true;

let frame = state.sample(motion, Duration::ZERO, reduced_motion);

// Reduced motion reaches the destination without playing the transition.
assert_eq!(frame.target().x_px(), 100.0);
assert!(!frame.is_animating());
```

Reduced motion completes even exits and infinite clips; a completed clip stays
complete until restart/replacement. The app synchronizes OS preferences, notifies
on input changes, restores focus on removal, and scopes keyboard shortcuts around
text editing. No per-animation task, global registry, or platform bootstrap is added.

## Validation

| Error type       | Returned by                                                             |
| ---------------- | ----------------------------------------------------------------------- |
| `InvalidSpring`  | Invalid physical parameters, visual timing, or settling tolerances      |
| `InvalidEasing`  | Invalid Bézier controls or custom easing preparation                    |
| `AnimationError` | Invalid keyframes/times, repeats, speed, labels, keys, or indices       |
| `PathError`      | Invalid path geometry/topology, reveal, drawing bounds, or tessellation |

All four implement `std::error::Error`. The examples use `expect` for fixed,
valid inputs; propagate errors when values come from users. Builders such as
`Target::width` assert their contracts: pass finite values, nonnegative dimensions,
and opacity within `0..=1`. See the linked source declarations for each constructor's details.

## Public export index

Every public root export is listed below. Existing APIs link to tagged 0.1.0
source; the unreleased text additions link to their current source modules.
Methods live in the same modules. Native-only types require `gpui`.
Element wrapper/frame types are usually returned by higher-level APIs.

For generated method-level docs, run `cargo doc --no-deps --features gpui --open`.
See [documentation build status](docs/DEVELOPMENT.md#api-documentation) for the hosted
0.1.0 reference.

| Group          | Public exports                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | Required feature |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------- |
| motion         | [`Frame`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/motion.rs#L257), [`Motion`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/motion.rs#L152), [`MotionState`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/motion.rs#L333), [`Presence`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/motion.rs#L244), [`Property`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/motion.rs#L226), [`Target`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/motion.rs#L16)                                                                                                                                                                                                                                                                                                                                                                                      | None             |
| transition     | [`Bezier`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/transition.rs#L295), [`CustomEasing`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/transition.rs#L106), [`Easing`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/transition.rs#L6), [`InvalidEasing`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/transition.rs#L342), [`InvalidSpring`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/transition.rs#L265), [`Spring`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/transition.rs#L137), [`Transition`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/transition.rs#L278)                                                                                                                                                                                                                                                           | None             |
| value          | [`Inertia`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/value.rs#L7), [`MotionValue`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/value.rs#L94), [`ValueFrame`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/value.rs#L58)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | None             |
| animation      | [`Animation`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/animation.rs#L95), [`AnimationError`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/animation.rs#L10), [`AnimationState`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/animation.rs#L423), [`Keyframe`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/animation.rs#L35), [`Playback`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/animation.rs#L355), [`Repeat`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/animation.rs#L65), [`RepeatMode`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/animation.rs#L78), [`StaggerFrom`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/animation.rs#L552), [`Timeline`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/animation.rs#L586), [`stagger`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/animation.rs#L561) | None             |
| color          | [`Color`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/color.rs#L6), [`ColorState`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/color.rs#L66)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               | None             |
| interaction    | [`Drag`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/interaction.rs#L99), [`GestureTargets`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/interaction.rs#L48), [`InView`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/interaction.rs#L299), [`Interaction`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/interaction.rs#L34), [`Variants`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/interaction.rs#L7), [`move_item`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/interaction.rs#L368), [`reorder_index`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/interaction.rs#L352), [`scroll_progress`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/interaction.rs#L284), [`transform`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/interaction.rs#L266)                                                                  | None             |
| presence       | [`PresenceChild`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/presence.rs#L15), [`PresenceGroup`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/presence.rs#L32), [`PresenceMode`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/presence.rs#L6)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | None             |
| svg            | [`SvgFrame`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/svg.rs#L103), [`SvgMotion`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/svg.rs#L68), [`SvgMotionState`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/svg.rs#L136), [`SvgTarget`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/svg.rs#L11)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           | None             |
| layout         | [`LayoutPositionFrame`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/layout.rs#L8), [`LayoutPositionState`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/layout.rs#L46)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      | None             |
| layout native  | [`LayoutGroup`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/layout_native.rs#L14), [`LayoutPositionElement`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/layout_native.rs#L51)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | `gpui`           |
| style motion   | [`ColorProperty`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/style_motion.rs#L62), [`StyleFrame`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/style_motion.rs#L233), [`StyleMotion`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/style_motion.rs#L192), [`StyleMotionState`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/style_motion.rs#L255), [`StyleProperty`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/style_motion.rs#L8), [`StyleTarget`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/style_motion.rs#L78)                                                                                                                                                                                                                                                                                                                       | None             |
| measured size  | [`AutoSize`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/measured_size.rs#L24), [`SizeAxis`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/measured_size.rs#L11)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | `gpui`           |
| pop layout     | [`PopElement`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/pop_layout.rs#L65), [`PopLayout`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/pop_layout.rs#L14), [`PopRequest`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/pop_layout.rs#L96)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         | `gpui`           |
| gesture native | [`GestureElement`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/gesture_native.rs#L119), [`GestureState`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/gesture_native.rs#L58)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | `gpui`           |
| scroll native  | [`Axis`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/scroll_native.rs#L10), [`ScrollMotion`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/scroll_native.rs#L36), [`native_scroll_progress`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/scroll_native.rs#L18)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | `gpui`           |
| shared layout  | [`SharedElement`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/shared_layout.rs#L72), [`SharedLayout`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/shared_layout.rs#L22)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    | `gpui`           |
| reorder native | [`ReorderElement`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/reorder_native.rs#L124), [`ReorderGroup`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/reorder_native.rs#L44)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | `gpui`           |
| vector         | [`PathCommand`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/vector.rs#L6), [`PathError`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/vector.rs#L48), [`PathFrame`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/vector.rs#L302), [`PathMorphState`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/vector.rs#L217), [`PathShape`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/vector.rs#L69), [`StrokeReveal`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/vector.rs#L112)                                                                                                                                                                                                                                                                                                                                                                     | None             |
| vector native  | [`VectorElement`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/vector_native.rs#L73), [`VectorState`](https://github.com/1bye/gpui-motion/blob/v0.1.0/src/vector_native.rs#L19)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | `gpui`           |
| text options (unreleased) | [`TextUnit`, `TextMatch`, `TextClip`, `TextPreset`, `TextTarget`, `TextUnitInfo`, `Scramble`, `NumberEffect`, `TextOptions`, `TextError`, `text_ranges`](src/text/options.rs) | `text` |
| text playback (unreleased) | [`TextLayoutUnit`, `TextPresentation`, `TextPlaybackState`](src/text/state.rs) | `text` |
| text native (unreleased) | [`TextMotionState`, `TextElement`, `NumberMotionState`, `NumberElement`](src/text/native.rs) | `gpui` |

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

| Supported                                                  | Boundary                                                                       |
| ---------------------------------------------------------- | ------------------------------------------------------------------------------ |
| Dimensions, spacing, and measured content sizing           | Real layout; text and siblings can reflow                                      |
| Automatic position animation and shared position/crossfade | Sizes change immediately; no size projection                                   |
| SVG scale and rotation                                     | Drawing only; native hitboxes, clipping, and children stay untransformed       |
| Native paths and stroke reveal                             | Matching command topology; no arbitrary SVG parser or shape normalization      |
| Blur and backdrop blur                                     | Need separate GPU profiling; CPU sampling timings do not measure these effects |

Arbitrary element-tree scale/rotation/skew/perspective and full layout projection
are deferred. This is a Motion-inspired Rust API, not JavaScript/React API parity.
Read [COVERAGE.md](COVERAGE.md) for exact contracts and exclusions.

## GPUI Kit examples

The [Motion / Kit showcase](integrations/gpui-kit/README.md) contains six product
flows: inbox and undo, asset inspection, downloads, document search, a draggable
planner, and an automation runner. It uses a separate Kit-native adapter over the
same portable engine. GPUI Kit 0.7.3 uses a different GPUI backend from CE, so its
native elements do not mix with the CE adapters above.

```sh
cargo run --manifest-path integrations/gpui-kit/Cargo.toml --features demo --example showcase
```

## Explore and contribute

- [Demos and showcase controls](docs/DEVELOPMENT.md#demos)
- [Tests and benchmarks](docs/DEVELOPMENT.md#verification)
- [Measured performance and its limits](PERFORMANCE.md)
- [Publishing procedure](PUBLISHING.md)
- [Changes by version](CHANGELOG.md)

MIT OR Apache-2.0. The library and demos were extracted from 1keep at commit
`0cb95d0`; the original [MIT](LICENSE-MIT) and [Apache-2.0](LICENSE-APACHE)
notices are retained.
