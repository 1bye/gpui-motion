use crate::{Animation, Easing, PresenceMode, StaggerFrom, Transition};
use std::{ops::Range, sync::Arc, time::Duration};
use unicode_segmentation::UnicodeSegmentation;

/// Grouping for text motion. Native adapters merge boundaries inside shaped clusters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextUnit {
    /// The complete string moves together.
    #[default]
    Whole,
    /// Unicode word-boundary segments, including punctuation and spacing.
    Words,
    /// Extended grapheme clusters, including combining marks and joined emoji.
    Graphemes,
    /// Visual wrapped lines, supplied by the native text layout.
    Lines,
}

/// Identity policy when the requested text changes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextMatch {
    /// Replace units when content changes; preserve them on layout-only changes.
    #[default]
    Replace,
    /// Match equal content deterministically in stable occurrence order.
    Content,
    /// Match equal content in the same slot. Numbers use slots counted from the right.
    Position,
}

/// Stationary clipping around each unit's final layout reservation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextClip {
    /// Allow offsets and glyph overhangs outside the unit reservation.
    #[default]
    None,
    /// Clip moving ink to the unit's reservation, useful for rolling text.
    Unit,
}

/// Convenience effects. A subsequent `enter` or `exit` overrides that target.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextPreset {
    /// Fade incoming and outgoing text.
    #[default]
    Fade,
    /// Enter from below and leave above, with an opacity transition.
    FadeUp,
    /// Slide through the measured unit height, with masking and a short exit fade.
    SlideUp,
    /// Reveal from the left edge using a stationary mask.
    WipeRight,
    /// Reveal from the right edge using a stationary mask.
    WipeLeft,
    /// Reveal vertically from the top edge.
    WipeDown,
    /// Reveal vertically from the bottom edge.
    WipeUp,
    /// Reveal complete units at their stagger times, without partial opacity.
    Typewriter,
}

/// Text-relative offsets, opacity, and horizontal reveal fraction.
/// Builders panic on non-finite offsets or fractions outside zero through one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextTarget {
    values: [f64; 4],
    relative_y: bool,
}

impl Default for TextTarget {
    fn default() -> Self {
        Self {
            values: [0., 0., 1., 1.],
            relative_y: false,
        }
    }
}

impl TextTarget {
    /// Horizontal offset in multiples of the resolved font size.
    pub fn x_em(mut self, value: f32) -> Self {
        assert!(value.is_finite(), "text offset must be finite");
        self.values[0] = value.into();

        self
    }

    /// Vertical offset in multiples of the resolved font size.
    pub fn y_em(mut self, value: f32) -> Self {
        assert!(value.is_finite(), "text offset must be finite");
        self.values[1] = value.into();
        self.relative_y = false;

        self
    }

    /// Vertical offset as a fraction of this unit's measured height. One moves the
    /// complete unit clear of its stationary mask, including line spacing.
    /// The last `y_em` or `y_relative` declaration wins. Panics for non-finite input.
    pub fn y_relative(mut self, value: f32) -> Self {
        assert!(value.is_finite(), "text offset must be finite");
        self.values[1] = value.into();
        self.relative_y = true;

        self
    }

    /// Presentation opacity, clamped at the drawing boundary during spring overshoot.
    pub fn opacity(mut self, value: f32) -> Self {
        assert!(
            (0. ..=1.).contains(&value),
            "text opacity must be in zero through one"
        );
        self.values[2] = value.into();

        self
    }

    /// Horizontal ink reveal fraction; this always enables a stationary mask.
    pub fn reveal(mut self, value: f32) -> Self {
        assert!(
            (0. ..=1.).contains(&value),
            "text reveal must be in zero through one"
        );
        self.values[3] = value.into();

        self
    }

    pub(crate) fn values(self) -> [f64; 4] {
        self.values
    }

    pub(crate) fn resolved_values(self, em: f64, height: f64) -> [f64; 4] {
        let mut values = self.values;

        if self.relative_y {
            values[1] *= height / em;
        }

        values
    }
}

/// Metadata supplied once when preparing each unit's custom entrance.
pub struct TextUnitInfo<'a> {
    /// Logical unit index, independent of visual bidi ordering.
    pub index: usize,
    /// Number of prepared units.
    pub count: usize,
    /// Unit content from the complete shaped string.
    pub text: &'a str,
}

/// Deterministic visual substitutions. The logical accessibility text is unchanged.
#[derive(Clone, Debug, PartialEq)]
pub struct Scramble {
    pub(crate) alphabet: Arc<[String]>,
    pub(crate) duration: Duration,
    pub(crate) seed: u64,
}

impl Scramble {
    /// Prepares a nonempty alphabet of extended graphemes. Panics if empty.
    pub fn new(alphabet: &str) -> Self {
        let alphabet: Vec<_> = alphabet.graphemes(true).map(str::to_owned).collect();

        assert!(!alphabet.is_empty(), "scramble alphabet must not be empty");

        Self {
            alphabet: alphabet.into(),
            duration: Duration::from_millis(500),
            seed: 0,
        }
    }

    /// Sets the settling duration per unit. Zero disables substitutions.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;

        self
    }

    /// Sets a stable deterministic seed; identical declarations do not restart.
    pub fn seed(mut self, seed: u64) -> Self {
        self.seed = seed;

        self
    }

    /// Prepared replacement graphemes, for native shaping or a custom renderer.
    pub fn alphabet(&self) -> &[String] {
        &self.alphabet
    }
}

/// Digit transitions. Rolling and sliding retain unchanged digits by place value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NumberEffect {
    /// Slide digits through stationary per-place masks.
    #[default]
    Roll,
    /// Fade changed digits in the same places.
    Crossfade,
    /// Slide changed digits without a mask.
    Slide,
    /// Interpolate an integer count, rounding each displayed sample.
    Interpolate,
}

/// Axis of an axis-aligned text reveal mask.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextRevealAxis {
    /// Reveal across the unit's width.
    #[default]
    Horizontal,
    /// Reveal across its measured line-height reservation.
    Vertical,
}

/// Portable text configuration. Native element builders expose the same options.
#[derive(Clone, Debug, PartialEq)]
pub struct TextOptions {
    /// Requested grouping.
    pub unit: TextUnit,
    /// Identity policy.
    pub matching: TextMatch,
    /// Initial pose for entering units.
    pub enter: TextTarget,
    /// Pose for retiring units.
    pub exit: TextTarget,
    /// Pose transition.
    pub transition: Transition,
    /// Optional independent opacity timing; movement and reveal keep `transition`.
    pub opacity_transition: Option<Transition>,
    /// Optional natural-position transition. Sizes take their destination immediately.
    pub reflow: Option<Transition>,
    /// Per-unit delay.
    pub stagger: Duration,
    /// Delay origin.
    pub stagger_from: StaggerFrom,
    /// Incoming/outgoing coordination.
    pub presence_mode: PresenceMode,
    /// Per-unit clipping.
    pub clip: TextClip,
    /// Reverses the reveal to anchor at the trailing edge of the selected axis.
    pub wipe_left: bool,
    /// Axis used by reveals; `wipe_left` chooses the trailing edge on either axis.
    pub reveal_axis: TextRevealAxis,
    /// Whether the first mounted text enters. Later changes always follow the effect.
    pub animate_initial: bool,
    /// Prepared optional substitutions.
    pub scramble: Option<Scramble>,
    /// Optional retained clip; positional channels are logical pixels.
    pub animation: Option<Animation>,
}

impl Default for TextOptions {
    fn default() -> Self {
        Self {
            unit: TextUnit::Whole,
            matching: TextMatch::Replace,
            enter: TextTarget::default().opacity(0.),
            exit: TextTarget::default().opacity(0.),
            transition: Transition::default(),
            opacity_transition: None,
            reflow: None,
            stagger: Duration::ZERO,
            stagger_from: StaggerFrom::Start,
            presence_mode: PresenceMode::Sync,
            clip: TextClip::None,
            wipe_left: false,
            reveal_axis: TextRevealAxis::Horizontal,
            animate_initial: true,
            scramble: None,
            animation: None,
        }
    }
}

impl TextOptions {
    /// Applies a preset, replacing entrance/exit poses and its required masking/timing.
    pub fn preset(&mut self, preset: TextPreset) {
        self.enter = TextTarget::default();
        self.exit = TextTarget::default();
        self.clip = TextClip::None;
        self.wipe_left = false;
        self.reveal_axis = TextRevealAxis::Horizontal;
        self.opacity_transition = None;

        match preset {
            TextPreset::Fade => {
                self.enter = self.enter.opacity(0.);
                self.exit = self.exit.opacity(0.);
            }
            TextPreset::FadeUp => {
                self.enter = self.enter.y_em(0.6).opacity(0.);
                self.exit = self.exit.y_em(-0.6).opacity(0.);
            }
            TextPreset::SlideUp => {
                self.enter = self.enter.y_relative(1.);
                self.exit = self.exit.y_relative(-1.).opacity(0.);
                self.clip = TextClip::Unit;
                self.opacity_transition = Some(Transition::Tween(
                    Duration::from_millis(100),
                    Easing::EaseOut,
                ));
            }
            TextPreset::WipeRight
            | TextPreset::WipeLeft
            | TextPreset::WipeDown
            | TextPreset::WipeUp => {
                self.enter = self.enter.reveal(0.);
                self.exit = self.exit.reveal(0.).opacity(0.);
                self.opacity_transition = Some(Transition::Tween(
                    Duration::from_millis(100),
                    Easing::EaseOut,
                ));
                self.clip = TextClip::Unit;
                self.wipe_left = matches!(preset, TextPreset::WipeLeft | TextPreset::WipeUp);

                if matches!(preset, TextPreset::WipeDown | TextPreset::WipeUp) {
                    self.reveal_axis = TextRevealAxis::Vertical;
                }
            }
            TextPreset::Typewriter => {
                self.enter = self.enter.opacity(0.);
                self.exit = self.exit.opacity(0.);
                self.transition = Transition::Instant;
            }
        }
    }

    pub(crate) fn pose_transition(&self, index: usize) -> Transition {
        if index == 2 {
            self.opacity_transition.unwrap_or(self.transition)
        } else {
            self.transition
        }
    }

    /// Finite entrance duration used for normalized scrubbing. Springs use a two-second
    /// sampling horizon; infinite clips cannot be scrubbed by normalized progress.
    pub fn entrance_duration(&self, count: usize) -> Duration {
        let base = self
            .animation
            .as_ref()
            .and_then(Animation::duration)
            .unwrap_or_else(|| transition_horizon(self.transition))
            .max(
                self.opacity_transition
                    .map_or(Duration::ZERO, transition_horizon),
            )
            .max(
                self.scramble
                    .as_ref()
                    .map_or(Duration::ZERO, |value| value.duration),
            );

        base.saturating_add(
            self.stagger
                .saturating_mul(count.saturating_sub(1).min(u32::MAX as usize) as u32),
        )
    }
}

fn transition_horizon(transition: Transition) -> Duration {
    match transition {
        Transition::Instant => Duration::ZERO,
        Transition::Tween(duration, _) => duration,
        Transition::Spring(_) => Duration::from_secs(2),
    }
}

/// Unicode segmentation for a custom renderer. `Lines` here means explicit newline
/// lines; native adapters replace these with measured visual wrapped lines.
pub fn text_ranges(text: &str, unit: TextUnit) -> Vec<Range<usize>> {
    match unit {
        TextUnit::Whole => {
            if text.is_empty() {
                Vec::new()
            } else {
                std::iter::once(0..text.len()).collect()
            }
        }
        TextUnit::Words => text
            .split_word_bound_indices()
            .map(|(index, value)| index..index + value.len())
            .collect(),
        TextUnit::Graphemes => text
            .grapheme_indices(true)
            .map(|(index, value)| index..index + value.len())
            .collect(),
        TextUnit::Lines => {
            let mut start = 0;

            text.split_inclusive('\n')
                .map(|value| {
                    let range = start..start + value.trim_end_matches('\n').len();

                    start += value.len();

                    range
                })
                .collect()
        }
    }
}

/// Native text failures, independent of the chosen GPUI backend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextError {
    /// Font shaping or text measurement failed.
    Shaping(Arc<str>),
    /// A prepared glyph could not be painted by the backend.
    Painting(Arc<str>),
}

impl std::fmt::Display for TextError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Shaping(message) => write!(formatter, "text shaping failed: {message}"),
            Self::Painting(message) => write!(formatter, "text painting failed: {message}"),
        }
    }
}

impl std::error::Error for TextError {}

impl crate::MotionData<4> for TextTarget {
    fn channels(self) -> [f64; 4] {
        self.values
    }

    fn with_channels(self, values: [f64; 4]) -> Self {
        Self {
            values,
            relative_y: self.relative_y,
        }
    }

    fn compatible(self, other: Self) -> bool {
        self.relative_y == other.relative_y
    }
}
