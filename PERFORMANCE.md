# Performance history

Results below were recorded in the original 1keep workspace before extraction;
Git-pinned CE results are not measurements of the standalone registry demos.
Use the commands below to reproduce measurements in this repository.

# Motion validation — 2026-10-16

Host: macOS 26.6.2, ARM64, Rust 1.95.0, native Metal, GPUI CE revision
`094fe979fc4bd9a7dd4544d75b147a9278afcecf`. Other platforms are unverified.

## Sampling

`cargo bench -p gpui-ce-motion --bench sampling --locked --offline`, release
optimization, no default features. Twenty-five batches of 1,000 independent
objects × 240 samples, alternating destinations every 24 samples. Setup and
state allocation are outside the timed region; declarations, retargeting, and
sampling are inside. Results are passed through `black_box`.

| Workload | Median ns/object/sample | Batch P95 ns/object/sample | Median ms/1,000 objects/frame |
| --- | ---: | ---: | ---: |
| Settled baseline | 7.5 | 7.6 | 0.007 |
| Tween and interruption | 13.0 | 13.1 | 0.013 |
| Spring and interruption | 27.9 | 28.4 | 0.028 |

This measures sampler CPU work, not GPUI element construction, frame requests,
layout, painting, GPU execution, or scanout. The sampler stores fixed arrays and
scalar state and performs no heap allocation; GPUI rendering has its own costs.

## Native rendering smoke comparison

`cargo run --release --features demo --example gpui_eval -- 400 8 baseline` and the same command
with `motion`. Both use 400 text-labelled cards in a 960×640 logical window,
one second of warmup, and eight seconds of measurement. The baseline computes
a shared sinusoidal offset; motion samples an independent spring for each card,
retargeted between ±3 logical pixels every 200 ms. Identical drawing code then
uses the sampled offset. This isolates sampler overhead in a native scene;
it does not stress presence or nested animated subtrees.

| Measurement | Baseline | Motion |
| --- | ---: | ---: |
| Measured render callbacks | 960 | 942 |
| Inactive callbacks | 0 | 5 |
| Active interval samples | 960 | 937 |
| Median render interval | 8.331 ms | 8.332 ms |
| Active render interval P95 | 8.767 ms | 9.233 ms |
| Construction median | 0.161 ms | 0.184 ms |
| Construction P95 | 0.182 ms | 0.196 ms |
| Maximum interval, including inactive time | 18.852 ms | 53.735 ms |

Both foreground workloads sustained approximately 120 render callbacks/second.
The observed construction median difference was 0.023 ms/frame. These are
single-run smoke results on a shared developer desktop, not an isolated
statistical regression test or a guarantee of GPU frame deadlines.

An earlier baseline spent 192 callbacks inactive and had a 33.381 ms median;
it was excluded from this foreground comparison. Occlusion/activation affects
scheduling on this host. Keep the window visible, click to activate it, and
inspect active samples and maximum intervals rather than silently discarding
long gaps. Neither callback timing nor view construction proves GPU completion
or physical screen scanout. Repeat isolated runs before making release claims.

## Behavior and release checks

- External-consumer tests cover duration endpoints, analytic spring damping
  regimes and known critical solution, retargeting between samples, preserved
  spring momentum, transition switching, exit retention and interrupted re-entry,
  initial absence, backward timestamps, zero duration, reduced motion, invalid
  spring parameters, opacity bounds, and extreme finite targets.
- The optional adapter accepts stateful GPUI elements. Native and portable
  doctests compile public usage. Default-feature tests need no GPU.
- Native demo checked with its real keyboard actions and accessible status:
  exit progressed through `Exiting` to `Absent`; re-entry, repeated reversal,
  spring/tween switching, and reduced-motion immediate exit worked. Native menus
  were opened and exercised. Command+W and Command+Q ended the process.
- After settling, sampled process CPU time remained 1.47 seconds across a
  three-second idle interval; RSS was 97,248 KiB. This short sample checks for a
  busy frame loop, not a general energy or memory-leak guarantee.
- Workspace format, check, Clippy with warnings denied, tests, and docs pass.
  The crate archive verifies independently; its optional native feature builds
  and its tests pass against registry `gpui-ce =0.2.2`, separately from the Git
  dependency used by the running app. Nothing has been published.

For an actual release, repeat native checks against the registry-backed consumer,
confirm package-name availability and repository metadata, and exercise each
claimed desktop platform. OS reduced-motion preference synchronization belongs
to the host application; this demo explicitly toggles GPUI's application flag.

## Width, height, and radius update — 2026-10-17

Same host, toolchain, and official CE revision. The sampler now retains six
channels; width, height, and radius are optional and inherit previously declared
destinations when omitted. No CE source modifications or renderer fork are used.

The release sampling benchmark (25 batches, same 1,000 × 240 workload) reports:

| Workload | Median ns/object/sample | Batch P95 ns/object/sample | Median ms/1,000 objects/frame |
| --- | ---: | ---: | ---: |
| Settled, no layout targets | 14.3 | 14.6 | 0.014 |
| Tween and interruption, no layout targets | 23.1 | 23.5 | 0.023 |
| Spring and interruption, no layout targets | 40.6 | 41.1 | 0.041 |
| Six-channel spring and interruption | 40.6 | 41.0 | 0.041 |

The original three-channel results above are historical, not a claim that the
expanded sampler has zero overhead. The new timing includes declaration
resolution and all six channels, but still excludes native rendering.

For the native comparison, run `gpui_eval -- 400 8 layout-baseline` and
`gpui_eval -- 400 8 layout` in release mode. Both paint identical text cards in
400 fixed slots. Width alternates between 30/42 logical pixels, height 18/26,
and radius 2/8 every 200 ms. Both apply these values through the real
`Frame::apply` adapter and GPUI layout. The baseline uses one shared sampler;
the independent mode uses 400 retained samplers with the same time and targets.

| Measurement | Shared sampler baseline | 400 independent samplers |
| --- | ---: | ---: |
| Measured callbacks | 922 | 922 |
| Inactive callbacks | 0 | 0 |
| Median render interval | 8.333 ms | 8.333 ms |
| Active render interval P95 | 11.210 ms | 11.135 ms |
| Render interval P99 | 21.190 ms | 17.390 ms |
| Maximum render interval | 39.043 ms | 33.760 ms |
| Construction median | 0.281 ms | 0.310 ms |
| Construction P95 | 0.318 ms | 0.357 ms |

The observed construction difference is 0.029 ms/frame. Both runs delivered
about 115 callbacks/second over the measurement window, with an 8.333 ms median
cadence and occasional longer intervals. The small P95 difference is noise in
this single shared-desktop comparison, not evidence that independent motion is
faster. Neither run measures GPU completion or scanout, and these results do
not establish uninterrupted 120 Hz presentation.

The first independent run included 45 inactive callbacks; it was excluded from
the foreground comparison and repeated with the window activated. Activation
and occlusion remain material measurement constraints.

New tests cover numeric layout endpoints, omitted dimensions, first introduction
of a size, dimension momentum during reversal/re-entry, partial exit inheritance,
non-negative drawable bounds, invalid inputs, reduced-motion exit, and the
adapter's preservation/replacement of actual GPUI styles. The native feature
has 23 passing external-consumer tests and two passing doctests. The independent
crate archive also passes these tests against registry `gpui-ce =0.2.2`.

The playground's W command changes 240×160/radius 8 to 360×200/radius 36.
Native checks covered both endpoints, repeated retargeting, spring/tween
switching, reduced motion, collapse to 0×0 on exit, and restoration on re-entry.
The sidebar consumer now animates its outer clipping width directly, retaining
fixed-width inner content and its existing focus/input policy.

Sidebar runtime checks also confirmed Projects selection, Escape exit, visible
focus restoration to Show sidebar, content reflow after removal, and Command+B
re-entry with the same selection.

## Prepared playback and interaction update — 2026-10-17

Same host/toolchain and unmodified official CE revision. The scalar engine now
owns each channel's independent transition/delay. Prepared clips, timelines,
colors, and interaction helpers share that engine; default dependencies remain
empty. These results supersede the earlier sampler numbers for current code.

`cargo bench -p gpui-ce-motion --bench sampling --offline`: release, 25 batches
of 1,000 objects × 240 samples. Setup is outside measurement. The three advanced
workloads keep one monotonically advancing clock across batches: repeating
keyframes, retargeted linear-light colors (including sRGB conversion), and
bounded drag/release. The original workloads reset states between batches.

| Workload | Median ns/object/sample | Batch P95 ns/object/sample | Median ms/1,000 objects/frame |
| --- | ---: | ---: | ---: |
| Repeating prepared keyframes | 96.6 | 113.4 | 0.097 |
| Color spring, interruption and sRGB conversion | 139.6 | 140.8 | 0.140 |
| Drag and bounded inertial release | 55.6 | 58.5 | 0.056 |
| Settled six-channel state | 66.8 | 67.3 | 0.067 |
| Tween and interruption | 95.0 | 99.0 | 0.095 |
| Spring and interruption | 130.9 | 134.3 | 0.131 |
| Six independent layout springs and interruption | 208.9 | 213.2 | 0.209 |

Independent timing costs more than the previous common-matrix sampler: the
six-channel spring case increased from 0.041 to 0.209 ms/1,000 objects/frame on
this host. This is a measured regression in that CPU-only workload in exchange
for property-specific timing and shared scalar behavior, not a claim of free
feature expansion. The current workload uses about 2.5% of an 8.33 ms CPU frame
budget before native rendering. A thread-local allocator-observation test also
checks zero allocations across first bind, retained clip/timeline sampling,
retargeting, colors, dragging, and inertia. It excludes preparation, membership
updates, and GPUI element construction.

The added native probe modes `keyframes-baseline` and `keyframes` use identical
400-card layouts, numeric endpoints, 400 ms mirrored keyframes and easing. One
player supplies every baseline card; the independent run samples 400 retained
players. Both use `Frame::apply`, real layout/text/paint, and forced redraws,
with one second warmup and eight seconds measurement. Both windows were visible
and activated; neither run recorded inactive callbacks.

| Measurement | Shared prepared player | 400 independent players |
| --- | ---: | ---: |
| Measured callbacks | 937 | 946 |
| Inactive callbacks | 0 | 0 |
| Median callback interval | 8.332 ms | 8.330 ms |
| Active interval P95 | 9.296 ms | 9.291 ms |
| Interval P99 | 16.504 ms | 12.683 ms |
| Maximum interval | 58.205 ms | 64.547 ms |
| Construction median | 0.292 ms | 0.327 ms |
| Construction P95 | 0.313 ms | 0.359 ms |

The observed median construction difference is 0.035 ms/frame. These shared
Desktop single-run results show roughly 117–118 callbacks/second, with occasional
long gaps. The nearly equal P95 is not evidence of an improvement. They do not
measure GPU completion, energy, scanout, or guarantee 120 Hz presentation.
Raw probe logs are in ignored `target/gpui-ce-eval/ce/keyframes*.txt`.

The native gallery was exercised in the actual CE/Metal app: pause/resume,
variant width changes, keyed wait-mode replacement, stable-ID reordered items,
pointer drag with bounded snapped release, focused arrow-key motion, real scroll
progress (0%, 25%, 50%), row-10 visibility triggering and reduced-motion snap.
The gallery cancels pointer state on window deactivation and Escape. Retained
clips and colors are configured once, with no per-frame preparation allocation.
The tests additionally cover backward time, pause/seek/rate/reverse playback,
nonuniform/wildcard keyframes, reverse versus mirror easing, billion-cycle skips,
velocity-continuous spring clip replacement, independent completion timing,
analytic inertial frame-rate independence, presence reentry, validation and
allocation behavior. The native feature now passes 42 external-consumer tests
and three doctests, also verified from the independent archive against registry
`gpui-ce =0.2.2`. Default/no-default-feature tests, workspace checks, Clippy and
documentation checks pass. These are CPU/public API tests; other desktop platforms
and native execution of a standalone registry consumer remain unverified.

Portable retained-state sizes from `size_of` in the same release benchmark:
`MotionState` 1,352 bytes, `AnimationState` 352 bytes, `ColorState` 896 bytes,
`Drag` 560 bytes. These exclude shared clip/keyframe storage and container
allocations; the native feature also stores an `Instant` in motion/playback
states. Sample costs depend on active channels and keyframe count. The expanded
state trades memory for independently interruptible property clocks.

## SVG-only scale and rotation — 2026-10-17

Same macOS ARM64/Rust 1.95.0 host and official CE pin. `SvgMotionState` reuses
three independent scalar channels (X/Y scale and unwrapped degrees). The native
adapter sends scale and rotation to CE's SVG sprite transform. Neither the
renderer nor its shaders were patched. Existing general-element motion is unchanged.

`cargo bench -p gpui-ce-motion --bench sampling --offline`, release, no default
features, 25 batches × 1,000 SVG states × 240 samples. Scale/angle targets
are retargeted every 200 ms; all objects receive the same timestamp/declaration.
State allocation is outside timing; declaration creation, interruption and
sampling are inside. Results pass through `black_box`.

| Workload | Median ns/object/sample | Batch P95 ns/object/sample | Median ms/1,000 objects/frame |
| --- | ---: | ---: | ---: |
| SVG scale/rotation springs with interruption | 136.6 | 149.2 | 0.137 |

The portable retained SVG state occupies 680 bytes on this host. The optional
native adapter also stores an `Instant`. The allocator test now exercises SVG
first bind and retargeting alongside the existing engines and observes zero
sampling allocations. This benchmark excludes native transform conversion,
element construction, SVG loading/rasterization, painting and GPU execution.

The native probe adds `svg-baseline` / `svg`, each drawing 400 copies of the same
embedded SVG (including SVG text) at a fixed 18×18 logical base size. Unit scale
is replaced by spring destinations 0.7/1.1 with rotation -30/+30 degrees, toggled
every 200 ms. Both modes use the same SVG adapter, fixed slots, asset path, cache
and geometry; only the number of retained samplers differs. Fixed raster size
and asset identity allow CE to reuse its SVG raster cache across transforms.
There are five seconds of warmup for font/raster caches and eight measured
seconds. Earlier probe modes retain their original one-second warmup.

| Recorded run | Shared sampler, foreground | 400 samplers, inactive diagnostic |
| --- | ---: | ---: |
| Callbacks | 955 | 212 |
| Inactive callbacks | 0 | 212 |
| Active interval samples | 955 | 0 |
| Median interval | 8.333 ms | 40.770 ms |
| Active interval P95 | 9.421 ms | unavailable |
| Maximum interval | 17.063 ms | 66.163 ms |
| Construction median | 0.229 ms | 0.304 ms |
| Construction P95 | 0.677 ms | 0.796 ms |

**These columns are not a controlled foreground performance comparison.** The
independent run was inactive and throttled; its slower callbacks do not establish
an animation-engine regression, nor does the construction difference establish
sampler overhead. Other independent attempts also included inactivity, including
one 16.9-second occlusion gap. UI automation then reported macOS ScreenCaptureKit
error -3811 (capture stream failure), preventing reliable activation/observation
of a final foreground comparison. The baseline demonstrates foreground rendering;
independent-player frame cadence/GPU performance needs a repeat on an uninterrupted
desktop. Raw logs, including excluded runs, are retained in ignored
`target/gpui-ce-eval/svg/`.

The native SVG demo was visually exercised before the capture failure: scale
1.0/1.5, rotation, multiple-turn angle presentation, horizontal mirroring of both
shapes and SVG text, reset, and reduced-motion immediate completion. Its original
bounds outline and surrounding native text stayed fixed. Input is attached to
a padded native stage, never to an assumed transformed hitbox. The final layout
was enlarged and its help text shortened after inspecting clipped bottom copy;
that layout adjustment and the final Tab/Enter path could not be visually rechecked
after capture failed. Spring/tween interruption and native transformation mapping
are additionally covered by deterministic external-consumer tests.

All workspace checks, Clippy with denied warnings, no-default/native feature
tests, rustdoc, and archive verification pass. The independent archive passes
51 public API tests and five doctests against registry `gpui-ce =0.2.2` (no Git
patch), including nine SVG tests. No new dependency was added; the default core
still has no third-party dependencies. No package was published. Windows/Linux,
a native registry-backed app launch, and transformed visual hit testing are not
verified; CE intentionally leaves SVG layout/hitbox unchanged. Large zooms may
soften its cached rasterized graphic, and clipping follows CE's axis-aligned masks.

## Automatic position-only layout — 2026-10-17

Same macOS ARM64/Rust 1.95.0 host, official CE revision
`094fe979fc4bd9a7dd4544d75b147a9278afcecf`. No CE fork, renderer/shader
patch or new runtime dependency. `LayoutGroup::position` delegates the child's
layout node, measures its final native origin in prepaint, and translates the
child's presentation and input bounds together. Positions are spring/tween-driven;
size is immediate. Nested wrappers share a coordinate group.

Portable release benchmark (`cargo bench -p gpui-ce-motion --bench sampling
--offline`), 25 batches × 1,000 states × 240 samples, retargeting every 200 ms:

| Workload | Median ns/object/sample | Batch P95 ns/object/sample | Median ms/1,000 objects/frame |
| --- | ---: | ---: | ---: |
| Measured position springs with interruption | 73.2 | 75.3 | 0.073 |

State allocation is outside timing; measurement input, interruption and sampling
are inside. The allocator test includes first measurement and retargeting and
observes zero portable sampling allocations. Native element construction, keyed
CE state and frame callbacks can allocate; they are not covered by that claim.

### Full native CPU pipeline

```sh
cargo bench -p gpui-ce-motion --features native-tests --bench layout_native --offline
```

This benchmark uses real CE views, Taffy layout, prepaint, hitbox/scene registration,
paint preparation, frame callback dispatch and frame cleanup on CE TestPlatform.
Only the platform/window/display/GPU are test doubles. Each mode runs seven fresh
windows, 24 warmup frames and 240 measured frames each, advancing the fake
monotonic clock by 8,333 µs per frame. Four hundred 20×16 rectangles wrap inside a
480/600-pixel board that changes width every 24 frames and reverses order every 48.
Layout changes interrupt unsettled springs. Input geometry/content are identical
across modes; direct/disabled modes snap, animated mode moves toward final layout.

| Mode | Median of batch means, ms/frame | Aggregate frame P95, ms |
| --- | ---: | ---: |
| Direct CE elements | 1.671 | 1.838 |
| Position wrapper, interpolation disabled | 2.234 | 2.529 |
| Automatic measured positions + interruption | 2.539 | 4.834 |

The observed total difference is 0.868 ms/frame for 400 elements. This final run
had no concurrent Cargo builds. An earlier run overlapping validation builds
measured 1.632 / 2.097 / 2.327 ms batch means and 1.772 / 2.269 / 2.500 ms
frame P95 in the same mode order; the higher final animated tail illustrates
shared-desktop timing variation. Both runs are retained in
`target/gpui-ce-eval/layout/cpu-native.csv`. This includes
wrapper traversal, retained-state lookup, measurement, sampling and scheduling;
it is not a standalone spring cost. Normal clipping and translated child bounds
use CE's existing pipeline. No GPU submission/completion, Metal rendering, OS
input, display scanout or 120 Hz guarantee is measured here. Small rectangles
also do not model expensive text or complex subtrees. Modes run in fixed order
on a shared desktop; timings are local measurements, not release thresholds.

### Native demo and redraw probe

`layout_motion` was launched as a release CE/Metal app and visually checked.
Pointer controls and shortcuts exercised compact wrapping, repeated reorder/
relayout interruption, insertion, alignment, spring/tween switching, disabled
animation and reduced motion. Card selection and Tab/Enter activation updated
the live status; focus borders and help copy were visible without clipping.

`gpui_eval` adds `position-baseline` / `position`: 400 text-labelled cards, board
width 720/900 and reversed stable-ID order every 200 ms, five seconds of warmup
and eight measured seconds. View-construction timing ends before the wrapper's
prepaint sampling, so it cannot measure this feature's full CPU cost. The
TestPlatform benchmark above covers that missing work. The native baseline
attempt stalled without output and was terminated after a 35-second bound.
A retry's activation check failed with macOS ScreenCaptureKit error -3811.
There is no valid foreground callback comparison or GPU timing for this change;
repeat these modes on an uninterrupted desktop. Diagnostic logs are retained in
ignored `target/gpui-ce-eval/layout/`. The probe forces redraws independently
of the wrapper's own scheduling.

Seven portable layout tests cover first mount, two-axis settling/completion,
velocity-continuous interruption, between-sample retargeting, backward clocks,
tweens, reset, finite input validation and reduced motion. Five headless native
tests exercise real flex wrapping/reorder/insertion, immediate size changes,
translated pointer hitboxes, nested movement without double offsets, scrolling,
reduced/disabled interpolation and zero pending frame callbacks after settling.
The opt-in `native-tests` feature enables CE's test-support dependencies; default
and normal `gpui` builds do not enable it. Cross-parent shared layout, animated
size projection, general subtree scale/rotation, Windows/Linux runtime behavior
and alternate display scales remain unverified or deferred. Nothing was published.

Verification passes: 54 portable tests + four doctests without default features;
58 tests + seven doctests with normal `gpui`; 63 tests + seven doctests with
`native-tests`, including five real-CE headless tests. The independent packaged
archive passes both native configurations against registry `gpui-ce =0.2.2`
without a Git override. Workspace all-target checks, Clippy with warnings denied,
formatting, rustdoc with warnings denied, and Cargo package verification pass.
The registry crate remains unpublished; no third-party dependency is enabled
by default (`cargo tree -p gpui-ce-motion --no-default-features`).

## Remaining stock-CE integrations — 2026-10-17

Same macOS ARM64/Rust 1.95.0 host and official CE pin. New APIs use public
layout, prepaint offsets, hitboxes, input dispatch, style/filter and PathBuilder
hooks. No renderer source, shader, CE dependency revision or runtime dependency
was changed. Gaussian blur/backdrop blur exist in both supported CE sources.

### Portable sampling

Release `cargo bench -p gpui-ce-motion --bench sampling --offline`, 25 batches
of 1,000 states × 240 samples with targets interrupted every 200 ms. Setup,
path preparation and state allocation are outside timing; declarations, channel
sampling, compatible topology checks, length updates and stroke iteration are
inside. Outputs pass through `black_box`.

| Workload | Median ns/object/sample | Batch P95 ns/object/sample | Median ms/1,000 objects/frame |
| --- | ---: | ---: | ---: |
| Eight padding/radius tracks + linear-light background color | 483.3 | 660.2 | 0.483 |
| Prepared cubic morph + measured stroke reveal | 544.4 | 607.8 | 0.544 |
| Five-channel SVG transform state, interrupted scale/rotation | 170.3 | 189.9 | 0.170 |

The final run is reported above. The earlier run measured style sampling at
503.5 ns and morph/reveal at 551.5 ns; desktop timing varies between runs.
These exclude native element creation, native style vectors, tessellation, GPU
work, input dispatch and layout. Portable allocation checks include style first
bind/retargeting, cubic morphing and stroke iteration and observe zero allocations
after preparation. Curve reveal uses 32 equal-parameter segments per curve,
not an exact arc-length solver. Native tessellation can allocate and fail;
`VectorState::error` reports that boundary. Patterns are capped at 4096 periods.

Retained sizes in this release benchmark: `MotionState` 1,496 bytes,
`AnimationState` 352, `ColorState` 992, `Drag` 608 and `SvgMotionState` 1,248.
New configurable settling tolerances enlarge physical spring state, and SVGs
now retain two additional translation channels. These exclude vector/clip heap
storage and native element allocations; the native SVG clock adds its `Instant`.
They are historical measurements, not stable ABI promises.

### Full native CPU preparation

```sh
cargo bench -p gpui-ce-motion --features native-tests --bench advanced_native --offline
```

Real CE views, Taffy, prepaint/hitboxes, style/filter scene preparation, frame
callback dispatch, path tessellation and frame cleanup on TestPlatform. The
platform/window/display/GPU are test doubles. Five fresh contexts per mode,
24 warmup + 240 measured frames each, fake clock advancing 8,333 µs/frame.
Each mode constructs 100 small elements in a 480-pixel wrapping board. Targets
change every 24 frames; real sizes alternate 16/24 pixels. Native reorder mode
programmatically reverses keys; its handlers are installed but pointer input is
covered by behavioral tests instead. Shared mode measures one active appearance
per identity. Vector mode morphs one cubic per element and changes stroke reveal.

| Workload | Median batch mean, ms/frame | Aggregate frame P95, ms |
| --- | ---: | ---: |
| Direct reflow | 0.694 | 1.467 |
| Native style tracks | 0.711 | 1.832 |
| Blur CPU preparation only | 0.756 | 1.280 |
| Gesture bindings + interruptible opacity | 1.904 | 3.665 |
| Intrinsic height measurement | 1.049 | 2.649 |
| Popped retained exits | 0.796 | 1.475 |
| Measured reorder + native handlers | 1.133 | 1.506 |
| Shared position bindings | 0.582 | 0.674 |
| Cubic morph/reveal + tessellation | 1.311 | 1.772 |

These are final-run timings after the popped-reentry fix. An earlier run
measured direct reflow at 0.430 ms mean / 0.531 ms P95, versus 0.694 / 1.467
in this run despite no change to that control. This variation prevents treating
small differences between modes as measured feature overhead. No Cargo builds
ran concurrently with either benchmark.

These are different feature workloads, not identical rendered-output comparisons
or isolated sampler overheads. In particular, popped exits change occupancy,
shared elements move, vector geometry differs from rectangles, and gesture mode
registers focus/input listeners. Fixed-order modes on a shared desktop are local
CPU measurements, not release thresholds or a 120 Hz presentation guarantee.
They do not measure GPU filter execution, energy, native OS input latency or
screen scanout. Blur may be much more expensive than its CPU preparation suggests.
The six-page release gallery verifies actual Metal rendering separately.

### Behavior and packaging evidence

The release `motion_native` gallery was launched on CE/Metal. Native checks
exercised content-measured expansion, popped removal, hover/activation, bounded
pointer drag and release, focused arrow-key motion, real scroll progress, pointer
and Alt+Arrow reorder commits with visible focus, shared position/crossfade,
vector morph/reveal/fill controls, SVG custom-pivot controls, style changes,
Gaussian blur/backdrop-filter rendering, reduced motion, and Command+Q/W exit.
All six pages were inspected; transient overlapping reflow settles normally.
No GPU completion or foreground callback-cadence measurement was captured.

Native verification found a keyboard dispatch bug with text-bearing controls:
listeners registered after child paint landed on its deepest dispatch node.
A new external-consumer test failed at zero movement before the correction and
passes at the requested ten-pixel keyboard movement after registering handlers
on the wrapper before child paint. Reorder fixtures now include text too.
A second regression reproduced a position jump when restoring a popped item
during exit. Popped items now own their reflow state and freeze the visible
position instead of following a hidden layout placeholder. The restore test
passes at the same displayed position before and after reentry.
Other tests cover borders/gaps in popped exits, clipped input during size changes,
nonanimated size axes with no pending frame, first active shared-member ordering,
disabled binding/cancellation, focus-loss pointer cancellation, reentrant reorder
callbacks and reduced-motion idle.

Final checks pass: 63 portable tests + six doctests, 68 normal-`gpui` tests +
15 doctests, and 88 `native-tests` tests + 15 doctests. The latter includes 20
real-CE headless tests (five existing layout tests plus 15 integration tests).
Workspace all-target Clippy and rustdoc pass with warnings denied, formatting
and the release gallery build pass, and Cargo package verification passes.

The default core remains dependency-free. Native consumers still enable only
`gpui`; opt-in `native-tests` adds CE's existing test-support feature. The package
is self-contained and independently checked against registry `gpui-ce =0.2.2`.
Test/benchmark activation uses a small adapter for that release's `activate_window`
and the official pin's `activate` name. Runtime code needs no compatibility flag.
Shadow conversion uses the common public constructor. Disabled accessibility
metadata and removal focus remain host responsibilities across CE releases.
Windows/Linux execution, alternate display scales, screen-reader interaction,
OS-wide pointer capture and GPU performance remain unverified. General subtree
transforms/full size projection and browser APIs remain outside the implemented
stock-CE equivalents. No crate was published.


## GPUI Kit integration — 2026-10-18

The standalone [Kit showcase](integrations/gpui-kit/README.md) binds the portable
engine to registry GPUI Kit 0.7.3 / gpui-pre 0.3.9, with no CE or Kit fork.
Its release benchmark compares 400 identical small rectangles under direct
layout, disabled position interpolation, and animated measured positions.
Seven fresh TestPlatform contexts per mode run 24 warmup + 240 measured frames,
advancing the executor clock by 8,333 microseconds per frame. Width changes every
24 frames and keyed order reverses every 48 frames, interrupting ongoing motion.

On macOS ARM64 / Rust 1.95.0, the final run after other builds finished measured
1.259 ms median batch mean / 1.397 ms aggregate P95 for direct layout,
1.779 / 2.004 ms for tracking without interpolation, and 2.014 / 2.294 ms for
animated positions. Run commands and the comparison table live in the linked
README. These are CPU construction/layout/prepaint/paint preparation timings;
TestPlatform substitutes the display/GPU. They establish neither GPU completion
nor screen presentation, and do not benchmark the six product screens.

Production-view interaction tests cover inbox archive/undo, mid-exit keyboard
undo without a position jump, asset search/inspector collapse, simulated transfer
pause/cancel/completion, command search/Enter, native planner drag/drop, and
pipeline failure/retry preserving completed steps. Adapter tests exercise clock
advancement, reversal continuity, and reduced-motion unmounting. Live macOS
window inspection covers all six screens separately from the CPU benchmark.
