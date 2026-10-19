# Demos and development

[← README](../README.md) · [API recipes](API.md) · [Performance results](../PERFORMANCE.md) · [Publishing](../PUBLISHING.md)

## Demos

Clone the repository and run an example with the `demo` feature. On macOS, install
Xcode command-line tools first; native windows need a graphical login session.
The library does not create the application or load your assets for you.

```sh
git clone https://github.com/1bye/gpui-motion.git
cd gpui-motion
cargo run --release --features demo --example motion_showcase
```

Replace `motion_showcase` with any example below:

| Example | What to explore |
| --- | --- |
| [motion_showcase](../examples/motion_showcase.rs) | Seven experiments on a recording canvas, manual scrolling, draggable tabs |
| [motion](../examples/motion.rs) | Entrance/exit, interrupted movement, dimensions/radius, spring/tween timing |
| [sidebar](../examples/sidebar.rs) | Collapsing app sidebar, content reflow, focus restoration |
| [motion_features](../examples/motion_features.rs) | Keyframes, timeline controls, variants, colors, drag, scroll, keyed presence |
| [motion_native](../examples/motion_native.rs) | Native gestures, reorder, measured sizing, popped exits, styles, vectors |
| [layout_motion](../examples/layout_motion.rs) | Animated wrapping, alignment, insertion/removal, and reordered positions |
| [text_motion](../examples/text_motion.rs) | Twelve text modes, matching/reflow, scramble, playback and animated counts |
| [svg_motion](../examples/svg_motion.rs) | SVG scale/rotation, mirroring, full turns, interruption |
| [gpui_eval](../examples/gpui_eval.rs) | Native redraw/performance comparison; see below |

The separate [Kit text gallery](../integrations/gpui-kit/README.md#text-gallery)
adds editable input, pointer-driven scrubbing and count controls using Kit-native
components. See [text contracts](TEXT.md) for the complete API.

### Showcase controls

Click a numbered tile to select an experiment. The highlight follows the selection
with a spring; the new scene fades and slides in. Play starts a guided tour.

| Experiment | Features |
| --- | --- |
| 01 | Springs, stagger, dimensions/radius, SVG scale/rotation |
| 02 | Measured height, padding, corners, color, shadow |
| 03 | Enter/exit, popped exits, automatic position reflow |
| 04 | Drag constraints, inertia, snapping, reordered tracks |
| 05 | Shared position/crossfade and manually controlled scroll progress |
| 06 | Compatible path morphs, stroke reveal, repeating keyframes |
| 07 | Draggable tabs, spring reorder, selected-content transitions |

- **P / Play:** play or pause. Experiment **05 pauses the tour** for exploration:
  scroll with the trackpad/wheel or ↑/↓ buttons. **Continue / P** resumes at 06.
- **Left/Right:** change experiment. **G:** reverse the current target, even in flight.
- **R:** restart the tour. **M:** toggle reduced motion. Deactivating the window pauses the tour.
- **07:** drag tabs into a new order. Tab focuses controls; Enter/Space selects a tab;
  **Option (⌥) + Left/Right** reorders it on macOS (Alt elsewhere). Escape cancels a drag.
- **04:** drag the handle/tracks; Option/Alt + Up/Down reorders the focused track.
- **Command+W/Q** closes/quits on macOS; Ctrl+W/Q on other desktop targets.

The wave in 06 rests fully drawn; its tour retracts/reveals while morphing. Its dot
is a playback indicator, not a slider. The default window is suitable for recording.

Other demos show their shortcuts in the window or their source header. In the
basic demo, Space shows/hides, R reverses movement, W changes dimensions, T changes
timing, and M reduces motion. The sidebar uses Command/Ctrl+B and Escape.

### Accessibility boundaries

The published CE version lacks the newer `aria_disabled`/`aria_hidden` builders.
The sidebar and exit gallery remove keyboard stops, guard activation, and describe
closing controls as unavailable; outgoing content remains in the accessibility
tree until its exit finishes. Native windows and paint still need hands-on checks;
TestPlatform tests do not measure GPU presentation.

## Verification

These checks use the repository's stable Rust toolchain, edition 2024, Rust 1.95+.

```sh
cargo fmt --all --check
cargo test --locked --no-default-features
cargo test --locked --no-default-features --features text
cargo test --locked --features gpui
cargo test --locked --features demo,native-tests
cargo test --locked --features demo,native-tests --example motion_showcase
cargo clippy --locked --all-targets --features demo,native-tests -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps --features gpui
cargo build --locked --release --features demo --examples
```

`native-tests` enables CE's headless TestPlatform for real layout/prepaint/input
checks. It is a development feature, not a normal consumer requirement. CI checks
portable code on Linux and native integration on macOS. This does not establish
native Linux/Windows presentation or GPU performance.

## API documentation

Generate the full method/variant reference locally:

```sh
cargo doc --no-deps --features gpui --open
```

Version 0.2.0 builds the [hosted API reference](https://docs.rs/gpui-ce-motion/0.2.0/gpui_ce_motion/)
on `x86_64-unknown-linux-gnu` with the native `gpui` feature enabled. CI checks the
same documentation target on Linux with warnings denied. The documentation target
does not establish native Linux window-system or GPU behavior.

The older [0.1.0 build](https://docs.rs/crate/gpui-ce-motion/0.1.0/builds/4898447)
failed while cross-compiling macOS dependencies on the Linux builder. Its published
metadata cannot be replaced; use the 0.2.0 reference or generate docs locally.

## Benchmarks

```sh
cargo bench --locked --bench sampling
cargo bench --locked --features native-tests --bench layout_native
cargo bench --locked --features native-tests --bench advanced_native
cargo bench --locked --features text --bench text_sampling
cargo bench --locked --features native-tests --bench text_native
```

| Benchmark | Measures | Does not measure |
| --- | --- | --- |
| `sampling` | Prepared sampler work across targets, clips, colors, values, SVGs, styles, paths | Native layout/paint or GPU execution |
| `layout_native` | View construction, layout, prepaint, paint preparation on TestPlatform | GPU completion or presentation |
| `text_sampling` | Prepared text-unit sampling and deterministic scramble | Matching, shaping or native rendering |
| `text_native` | Text shaping, cache invalidation, matching, layout and glyph preparation | GPU execution or screen presentation |
| `advanced_native` | Native style, gesture, sizing, reorder, shared-layout, and vector preparation | Actual blur GPU cost or screen scanout |

For a native-window probe:

```sh
cargo run --release --features demo --example gpui_eval -- 400 8 baseline
cargo run --release --features demo --example gpui_eval -- 400 8 motion
```

Use the same count/duration with these paired modes:

| Baseline | Animated |
| --- | --- |
| `baseline` | `motion` |
| `layout-baseline` | `layout` |
| `keyframes-baseline` | `keyframes` |
| `svg-baseline` | `svg` |
| `position-baseline` | `position` |

The probe forces redraws and reports view construction and render callback
intervals. Keep its window visible and active; occlusion can pause it. SVG/position
modes include warmup. Layout prepaint work is outside its construction timer;
use the TestPlatform benchmarks for full CPU preparation. These timings are not
GPU completion timestamps. [PERFORMANCE.md](../PERFORMANCE.md) records exact
workloads, historical baselines, measured results, and their scope.

## Releases

[CHANGELOG.md](../CHANGELOG.md) records versions. Follow
[PUBLISHING.md](../PUBLISHING.md) to inspect and validate the package before a
manual release. CI has no crates.io publishing credentials or upload step.

## Rust spacing

Run the shared Dylint policy on every native source target and all README Rust
examples:

```sh
rustup toolchain install nightly-2026-08-20 --profile minimal --component rustc-dev,llvm-tools-preview,rust-src
cargo +nightly-2026-08-20 install cargo-dylint dylint-link --version 6.2.0 --locked
python3 scripts/check_spacing.py
python3 scripts/check_spacing.py --fix
```

The first Python command checks without editing files. The second applies only
machine-applicable whitespace suggestions, then checks again. The rules come
from the full workflow revision pinned in `workspace.metadata.dylint` in
`Cargo.toml`; no local sibling checkout is needed. Keep the application on its
normal stable toolchain. Native targets require macOS or the usual CE platform
prerequisites. Python 3.9+ is sufficient; no Python packages are required.

After updating the pinned lint revision, `cargo dylint list --all` should list
`statement_padding`, `item_padding`, `multiple_empty_lines`, and `padded_blocks`.
If it still lists older rules, clear the generated `target/dylint/libraries`
cache in the affected workspace and rerun the check to rebuild the lint library.

Consecutive single-line `let` declarations may stay together. Other declarations
need a blank line afterward. Standalone control flow needs padding, and returns
and final expressions need separation from earlier work. Blocks have no padding
inside their braces, and files permit at most one consecutive empty line.

Type definitions, implementations, functions, and methods need one blank line
between them. Documentation and attributes stay attached to their definition.
Struct fields and enum variants stay compact, as do adjacent single-line imports,
module declarations, constants, statics, or type aliases within their own group.

README examples compile as isolated consumer scopes against their own backend:
the root README uses CE, and the Kit README uses the Kit adapter. Run the latter
with `python3 scripts/check_spacing.py --project integrations/gpui-kit`. Diagnostics
report README line numbers; fixes preserve prose and non-whitespace code. Ignored
and compile-fail examples are rejected rather than silently skipped.

```sh
python3 -m unittest discover -s scripts -p test_readme.py -v
python3 scripts/test_readme_integration.py -v
```

## Kit helper gallery and performance checks

The Kit helpers use stock `gpui-kit =0.7.3` and the published portable engine.
Their public API examples and registry setup live in the
[Kit README](../integrations/gpui-kit/README.md).

```sh
cargo run --manifest-path integrations/gpui-kit/Cargo.toml --features demo --example helpers
cargo test --manifest-path integrations/gpui-kit/Cargo.toml --features demo,native-tests --example helpers
cargo test --manifest-path integrations/gpui-kit/Cargo.toml --features demo,native-tests --test helpers_native --test playback_native
cargo bench --manifest-path integrations/gpui-kit/Cargo.toml --features demo,native-tests --bench helpers_native
```

`tests/common/advanced_fixture.rs` runs the same layout, hitbox, cancellation,
scroll-resize, clipped visibility and reordering contracts against CE and Kit.
Kit-specific tests cover native style conversion, unsupported filters, playback
clocks/controls, future timeline scheduling, SVG conversion and idle frames.
The helper gallery tests use real Kit controls, including observed drag/reorder
children. TestPlatform and the benchmark substitute fonts, display and GPU;
keep CPU preparation measurements separate from real presentation profiling.

## Expanded Motion lab

```sh
cargo run --features demo --example motion_lab
cargo run --manifest-path integrations/gpui-kit/Cargo.toml --features demo --example motion_lab
cargo test --features demo,native-tests --test spatial_native --example motion_lab
cargo test --manifest-path integrations/gpui-kit/Cargo.toml --features demo,native-tests --test spatial_native --example motion_lab
cargo bench --bench graph
cargo bench --features demo,native-tests --bench motion_lab
cargo bench --manifest-path integrations/gpui-kit/Cargo.toml --features demo,native-tests --bench motion_lab
```

The two `motion_lab/view.rs` files intentionally contain the same gallery code;
wrappers select native controls, SVG assets and Kit touch opt-in. Both package
archives contain their own complete example source. Keep these views identical.
Tests click the production Play button in every scene and interrupt playback.
Manual scroll and drag/reorder scenes are explored with native input; Option is
macOS's name for Alt. Pinch delivery depends on the host platform. Headless input
checks establish native dispatch/layout, not physical touch-device or GPU behavior.
