# Motion / Kit

Six interactive product examples built with **GPUI Kit 0.7.3** and the portable
`gpui-ce-motion` engine. Run from the repository root:

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

## Adapter

This is a separate development package, **not yet published**. An application can
use a local path dependency on `gpui-motion-kit` and its existing `gpui-kit`
dependency, both selecting Kit 0.7.3's default `gpui-pre` backend.

`MotionState::frame` samples the portable engine with Kit's executor clock and
reduced-motion policy. Its `Frame` supplies Kit-native `apply` and `render`, plus
the core frame's status/velocity getters. `ValueState` binds scalar motion;
`LayoutGroup` animates measured positions; `VectorState` paints compatible paths
and stroke reveals. The `core` re-export exposes the underlying portable API.
The [library example](src/lib.rs) is compiled as a doctest.

The adapter does not enable the engine's CE feature. Kit elements and CE elements
belong to different GPUI crates and cannot be mixed. There are no renderer forks,
Git runtime dependencies, arbitrary subtree transforms, or size projection.

The standalone manifest and lockfile keep Kit dependencies out of the CE crate's
normal build and package. The adapter is not included in the CE crates.io archive.
Kit's own control animations remain owned by Kit; example progress indicators use
Base semantics so a second interpolator does not animate the same value.

## Checks

```sh
cargo fmt --manifest-path integrations/gpui-kit/Cargo.toml --all --check
cargo test --manifest-path integrations/gpui-kit/Cargo.toml --locked --features demo,native-tests
cargo test --manifest-path integrations/gpui-kit/Cargo.toml --locked --features demo,native-tests --example showcase
cargo clippy --manifest-path integrations/gpui-kit/Cargo.toml --locked --all-targets --features demo,native-tests -- -D warnings
python3 scripts/check_spacing.py --project integrations/gpui-kit --sources-only
```

Append `--fix` to the spacing command to apply the same shared Dylint policy as
the CE crate. The repository's pinned Dylint nightly and CLI are required; see
[development instructions](../../docs/DEVELOPMENT.md#rust-spacing).

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
