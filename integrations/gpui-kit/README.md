# gpui-motion-kit

**Declarative animation for native Rust apps built with GPUI Kit.**

Keep the motion state in your view and describe its next destination. The adapter
connects the shared `gpui-ce-motion` engine to Kit's elements, executor clock,
frame scheduling, and reduced-motion preference. No JavaScript or renderer fork.

[Crates.io](https://crates.io/crates/gpui-motion-kit) ·
[API reference](https://docs.rs/gpui-motion-kit/latest/gpui_motion_kit/) ·
[Product examples](#product-examples) · [Text gallery](#text-gallery)

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

`VectorState` supplies native compatible-path morphing and stroke reveal; the
[automation example](https://github.com/1bye/gpui-motion/blob/main/integrations/gpui-kit/examples/showcase/automation.rs)
shows it in a complete flow. Other portable APIs are available through `core`,
but CE-specific native helpers such as `GestureState`, `ScrollMotion`, `AutoSize`,
and `SharedLayout` do not yet have Kit bindings. Drag/drop in the planner uses
Kit input plus the shared motion engine. General subtree transforms remain out
of scope on the stock renderer.

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
