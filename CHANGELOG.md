# Changelog

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
