# Text motion

Text motion is available on stock GPUI CE 0.2.2 and through the separate GPUI Kit
0.7.3 adapter. Retain one `TextMotionState` per logical label and declare its
current content with `element(id, text)` during render. Keep the element mounted
while its state should survive. Cloning a state shares its storage; do not mount
two labels with the same state at once.

The [README examples](../README.md#text-motion) show the consumer API. Run the
interactive gallery with:

```sh
cargo run --manifest-path integrations/gpui-kit/Cargo.toml --features demo --example text_motion
cargo run --features demo --example text_motion
```

The Kit gallery has editable text, twelve modes, replay/pause/resume, reduced
motion, a reveal slider, and rolling/fading/sliding/interpolated counts. Change
text repeatedly during motion to interrupt it. The CE playground exposes the same
effects through its visible controls and E/Space/R/P/M/Up/Down shortcuts.

## Effects and grouping

- `TextPreset::{Fade, FadeUp, SlideUp, WipeRight, WipeLeft, Typewriter}` configure
  convenience effects. A later `enter` or `exit` overrides that pose.
  `transition` controls movement and reveal, including Typewriter's discrete
  timing. `opacity_transition` independently overrides opacity timing.
  SlideUp and the wipe presets use a 100 ms ease-out exit fade so rapid
  replacements do not leave opaque retiring fragments. This opacity override
  also applies when a fading unit returns; positional springs retain momentum.
- `TextUnit::{Whole, Words, Graphemes, Lines}` select complete strings, Unicode
  word-boundary segments, extended grapheme clusters, or measured visual lines.
  Word boundaries include punctuation and whitespace. Stagger follows logical
  text order, including whitespace, rather than left-to-right visual ordering.
- `TextTarget::{x_em,y_em,y_relative,opacity,reveal}` configures an entrance or
  exit. Em offsets use the resolved font size. `y_relative(1.0)` moves by the
  unit's complete measured height, including line spacing; the last `y_em` or
  `y_relative` wins. SlideUp uses this measured height to fully clear its mask.
  Opacity/reveal accept zero through one. Settled offsets are zero,
  opacity/reveal one. Spring overshoot is clamped only for drawable opacity and
  masks. Opacity and position remain independent during scrubbing and resume.
- `stagger(delay, StaggerFrom::{Start,End,Center})` delays incoming units and clip
  starts. Exits begin immediately. `clip(TextClip::Unit)` provides stationary,
  axis-aligned masks around each unit's line-height reservation. Clipping may cut
  intentional font overhang; omit it when overhang is important.
- `enter_with` prepares a pose using `TextUnitInfo::{index,count,text}`. It runs
  when layout/configuration is prepared or on replay, not on every sample.
  Call `restart` after changing a value captured only by this callback.
- `Scramble::new(alphabet).duration(duration).seed(seed)` prepares deterministic
  visual substitutions. Alphabet entries are graphemes. Shaped replacements are
  cached; no string or font layout is rebuilt per scramble tick. Use grapheme
  grouping for a character-by-character scramble.
- `animation(&prepared_clip)` uses the existing `Animation` and repeat APIs per
  unit. Retain the clip outside render. Only x/y/opacity apply; offsets in these
  clips use logical pixels, while `TextTarget` uses em. Prepared clips overlay
  presence offsets and multiply opacity. Retiring units still have a finite
  exit lifetime even when the decorative clip repeats forever.

## Changing text

`TextMatch::Replace` replaces units when the content changes, while keeping
identity on layout-only changes. `Content` matches equal pieces in stable
occurrence order, so repeated letters/words remain distinct. `Position` matches
equal content in the same slot; numbers assign slots from the right.

`reflow(transition)` animates retained units to their newly measured positions.
The complete destination is shaped first. Retained positions account for the
label’s measured origin, so a centered label does not jump when its width changes.
Scroll and ancestor prepaint offsets follow the parent without double animation.
Font changes and destination sizes take effect immediately: this is position
animation, not size projection or scaling.
Outgoing units keep their prepared glyphs until their exits finish. Reintroducing
a matching outgoing unit reclaims its identity and current motion state.

`PresenceMode::Sync` overlaps incoming and outgoing content. `Wait` holds incoming
units until all outgoing units finish. A newer change replaces pending incoming
content, so a burst of updates does not play a stale queue.

Springs preserve presentation and velocity on retargeting. A switch to a tween
preserves position. Repeated identical declarations do not restart animation.
`animate_initial(false)` skips the first entrance without disabling later changes.

## Playback

`pause(window, cx)`, `play(window, cx)`, and `restart(window, cx)` act on retained
state. Controls refresh the window; an owning view should also notify after its
own data changes. Restart preserves incoming identities and reevaluates custom
entrances. It does not resurrect text whose exit already completed.

`seek(progress, window, cx)` holds a normalized finite entrance cursor and returns
whether the seek was accepted. Infinite clips return false. `progress(value)` on
the element declaratively holds the same sequence, including stagger. It panics
if used with an infinite clip. Values must be finite and in zero through one.
Tween horizons use their duration; spring scrubbing uses a documented two-second
horizon, with an exact destination at one. Reflow is not rewound by an entrance
scrub. Resuming after an entrance scrub retargets from the held pose. Normal
pause/resume preserves the existing time and momentum. A declared `progress`
value owns the cursor on every render; remove it before using state playback
controls to advance freely.

All finite and infinite effects honor the application's reduced-motion flag.
Reduced motion removes exits, clears substitutions and settles entrance/reflow
without scheduling ongoing frames. The host owns synchronization with OS settings.

## Numbers

`NumberMotionState::element(id, value)` accepts an `i64` count. `min_digits(n)` pads
the magnitude with zeroes, keeping the sign separate; its maximum is 256.
`NumberEffect::{Roll,Crossfade,Slide}` preserves unchanged digits by decimal place.
Roll masks moving digits; Slide leaves their overhang visible. Both travel by
one measured slot height and fade outgoing digits over 100 ms, while unchanged
digits retain their identities. `opacity_transition(...)` on the number element
can override this fade independently of movement. Changed natural positions also
animate, so proportional digit metrics do not jump immediately.

`Interpolate` samples a numeric count and rounds the displayed intermediate value.
It uses f64 between endpoints; choose a digit effect for exact large-integer
intermediate values. Both endpoints remain exact across the entire i64 range.
`text_state()` exposes the digit layer's presentation and controls; it does not
pause or seek the independent numeric interpolation channel.

## Native text, accessibility, and errors

Complete text runs are shaped before units are prepared. Boundaries inside a
shaped cluster are merged, preserving contextual shaping, kerning, ligatures,
combining marks and emoji. A grapheme may therefore animate with a neighboring
one when the font shapes them into one glyph. Bidi positions come from GPUI's
text layout; motion does not reverse or independently reshape RTL letters.
The pinned GPUI backends can split a mixed-direction Arabic run incorrectly when
wrapping. A live comparison reproduces this with ordinary, unanimated GPUI text
too. Use an unwrapped label or explicit line breaks for that content; this adapter
does not replace the backend’s bidi line-breaking algorithm.

Font family, size, weight, style, features, fallback, color, line height and
alignment inherit from GPUI. This API paints a single-style display label. It
does not implement text editing, selection, inline rich-text spans, or text-run
background/underline/strikethrough decorations. Use normal GPUI controls for
editable or decorated rich text.

On macOS, large `.SystemUIFont` glyphs can show a truncated edge even in ordinary,
unanimated GPUI text. Both galleries use `Helvetica Neue` for all animated text
and counters to avoid this native font-rendering artifact. Other platforms retain
their normal inherited font. The library still inherits the application's font;
use the normal `.font_family(...)` builder to choose a suitable display font when
needed. Increasing the motion mask or adding opacity does not repair a glyph that
the backend already draws with a clipped edge.

The native wrapper exposes one logical accessibility label containing the latest
requested text. Retiring fragments, individual glyphs and scrambled substitutions
are paint-only. Native screen-reader announcements and platform-specific text
behavior still need application acceptance checks.

`error()` reports the most recent shaping/glyph-paint error without logging from
the library. `presentation()` copies diagnostics in unscrolled window-layout
coordinates, including identity, positions, opacity, reveal, substitution and exit
state. It allocates; normal callers do not need it. `is_animating()` reports the last sampled frame's activity.

## Portable engine and costs

The optional `text` feature enables `TextOptions`, `TextPlaybackState`,
`TextLayoutUnit`, `TextPresentation`, and `text_ranges`. It adds
`unicode-segmentation`; the existing no-default-features build remains
independent of third-party dependencies. Native CE and Kit adapters enable this
feature automatically. The portable engine accepts explicit monotonic durations
and renderer-prepared geometry, with no window, task or platform dependency.

Reconciliation, Unicode segmentation, layout and glyph preparation allocate.
Prepared `sample` and scrubbing reuse storage, covered by the allocation test.
The native cache keeps at most four measurement variants per label; outgoing
units retain only their still-live prepared glyph data. Ordinary glyphs paint
through GPUI's atlas without per-character layout nodes. Fading emoji use native
composition to preserve their alpha. Native frame construction and painting have
additional costs and can allocate.

```sh
cargo test --features text --test text --test allocations
cargo test --features native-tests --test text_native
cargo test --manifest-path integrations/gpui-kit/Cargo.toml --features demo,native-tests --test text_native
cargo bench --features text --bench text_sampling
cargo bench --features native-tests --bench text_native
cargo bench --manifest-path integrations/gpui-kit/Cargo.toml --features demo,native-tests --bench text_native
```

Native benchmarks measure CPU construction, text-unit preparation, layout,
prepaint and paint preparation on TestPlatform. Its `NoopTextSystem` supplies
synthetic font metrics and glyphs; platform font shaping, glyph rasterization,
display and GPU execution are outside these timings. They do not establish display
cadence, energy use or screen-reader behavior. Native macOS checks complement them. Blur, arbitrary glyph/subtree
transforms, font-axis interpolation and outline morphing are outside this API.
