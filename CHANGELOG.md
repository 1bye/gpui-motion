# Changelog

## Unreleased — GPUI Kit helpers

- Add Kit-native style application, hover/press/focus/drag/in-view bindings,
  measured scroll mapping, intrinsic sizing, reorder, popped exits and shared
  position/crossfade over the existing published engine.
- Add Kit-native prepared clip/timeline clocks and SVG transform adapters, with
  smooth interruption, explicit playback epochs and reduced-motion scheduling.
- Accept styled stateful Kit elements, including observed elements, in input/reorder
  wrappers. Report unsupported element/backdrop blur explicitly on stock Kit.
- Add an eight-scene helper gallery, native-control tests, shared CE/Kit geometry
  regression fixtures, CPU preparation benchmarks and executable README examples.


## gpui-motion-kit 0.2.0

- Publish the Kit-native adapter as a separate crates.io package over the shared
  `gpui-ce-motion` 0.2 engine, targeting GPUI Kit 0.7.3's default backend.
- Add registry setup, a complete Kit application, and native API examples to the
  README; compile and lint Kit README examples in CI.
- Include runnable product/text demos, assets, and license notices in the archive;
  verify the package and its examples against the published core before release.
- Cover the hosted Kit API documentation's Linux build in CI.

## 0.2.0

- Add native text motion on stock GPUI CE and GPUI Kit: whole labels, words,
  graphemes and visual lines, with presets, custom targets, stagger, content
  matching, position reflow, masks, scramble and prepared keyframes.
- Add interruptible text presence, playback and scrubbing, reduced motion,
  semantic accessibility labels, and rolling/fading/sliding/interpolated counts.
- Add a GPUI Kit adapter and six product examples: inbox, assets, downloads,
  command palette, planner, and automation, with native interaction tests.
- Add editable text galleries for both backends, public API and native behavior
  tests, allocation checks, and portable/native CPU benchmarks.
- Add optional `text` support for Unicode segmentation; default features remain
  dependency-free. Native adapters enable text support automatically.
- Correct masked text travel using measured unit heights. Slide/roll and wipe
  exits fade independently over 100 ms, preventing lingering fragments during
  rapid replacements. Expose `y_relative` and `opacity_transition` for custom timing.
- Use a display-safe font for all animated labels and counters in both macOS
  text galleries to avoid clipped glyphs also seen in ordinary system-font text.
- Build hosted API documentation on Linux and verify that target in CI.

## 0.1.0

First public release of the native Rust motion engine and stock GPUI CE adapters.

- Declarative targets, easing, physical and visual springs, smooth interruption,
  and enter/exit presence.
- Prepared keyframes, playback controls, timelines, stagger, variants, and motion values.
- Native gestures, bounded drag with inertia/snapping, scroll-linked animation,
  measured reorder, popped exits, and automatic position-only layout animation.
- Measured content sizing, shared position/crossfade, and animated colors,
  dimensions, padding, corners, shadows, and supported paint styles.
- SVG-specific GPU scale/rotation and compatible vector path morph/stroke reveal.
- Eight runnable demos, including a seven-experiment recording showcase with
  manual scrolling and draggable tabs, plus public API tests and CPU benchmarks.

The default feature set is dependency-free. Native adapters use published
`gpui-ce =0.2.2`; no renderer fork or Git override is required. General element
scale/rotation, size projection, browser APIs, and DOM/CSS parity are outside
this release. See COVERAGE.md and PERFORMANCE.md for limits and measurement scope.
