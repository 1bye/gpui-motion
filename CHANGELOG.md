# Changelog

## Unreleased

- Use a display-safe font for all animated labels and counters in both macOS
  text galleries to avoid clipped glyphs also seen in ordinary system-font text.

- Correct masked text travel to use measured unit heights. Slide/roll and wipe
  exits fade independently over 100 ms, preventing lingering fragments during
  rapid replacements. Expose `y_relative` and independent `opacity_transition`
  for custom text and number transitions.

- Native text motion on stock GPUI CE and GPUI Kit: whole labels, words,
  graphemes and visual lines, with presets, custom targets, stagger, content
  matching, position reflow, masks, scramble and prepared keyframes.
- Interruptible text presence, playback/scrubbing, reduced motion, semantic
  accessibility labels, and rolling/fading/sliding/interpolated counts.
- Interactive text galleries for both backends, public API and native behavior
  tests, allocation checks, and portable/native CPU benchmarks.
- Optional `text` support adds Unicode segmentation; default features remain
  dependency-free. Native adapters enable text support automatically.

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
