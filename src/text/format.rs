//! Exact fixed-point number formatting and immutable rich display text.
use crate::Color;
use std::{fmt, sync::Arc};

/// An invalid fixed-point scale or number-format configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NumberFormatError;

impl fmt::Display for NumberFormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid fixed-point number format")
    }
}

impl std::error::Error for NumberFormatError {}

/// Exact signed decimal coefficient with zero through eighteen fractional places.
/// For example `FixedNumber::new(12345, 2)` represents 123.45 without float rounding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FixedNumber {
    coefficient: i64,
    scale: u8,
}

impl FixedNumber {
    /// Validates a decimal scale. All i64 coefficients, including MIN, are supported.
    pub fn new(coefficient: i64, scale: u8) -> Result<Self, NumberFormatError> {
        if scale > 18 {
            return Err(NumberFormatError);
        }

        Ok(Self { coefficient, scale })
    }

    /// Exact signed coefficient.
    pub fn coefficient(self) -> i64 {
        self.coefficient
    }

    /// Number of fractional decimal places.
    pub fn scale(self) -> u8 {
        self.scale
    }
}

impl From<i64> for FixedNumber {
    fn from(coefficient: i64) -> Self {
        Self {
            coefficient,
            scale: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Format {
    prefix: Arc<str>,
    suffix: Arc<str>,
    decimal: Arc<str>,
    separator: Arc<str>,
    primary: usize,
    secondary: usize,
    digits: [char; 10],
}

/// Reusable locale-configurable formatting, with explicit separators, affixes,
/// digit alphabet and primary/secondary groups. No implicit OS locale or CLDR
/// dependency is installed. Currency/percent conversion and rounding belong to
/// the caller; this formatter preserves the provided fixed-point value exactly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NumberFormat(Arc<Format>);

impl Default for NumberFormat {
    fn default() -> Self {
        Self(Arc::new(Format {
            prefix: "".into(),
            suffix: "".into(),
            decimal: ".".into(),
            separator: "".into(),
            primary: 3,
            secondary: 3,
            digits: ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'],
        }))
    }
}

impl NumberFormat {
    /// Sets a fixed prefix, such as a currency symbol.
    pub fn prefix(mut self, prefix: impl Into<Arc<str>>) -> Self {
        Arc::make_mut(&mut self.0).prefix = prefix.into();

        self
    }

    /// Sets a fixed suffix, such as a unit or percentage symbol.
    pub fn suffix(mut self, suffix: impl Into<Arc<str>>) -> Self {
        Arc::make_mut(&mut self.0).suffix = suffix.into();

        self
    }

    /// Sets a nonempty decimal separator. Panics for an empty string.
    pub fn decimal(mut self, separator: impl Into<Arc<str>>) -> Self {
        let separator = separator.into();

        assert!(!separator.is_empty(), "decimal separator must not be empty");
        Arc::make_mut(&mut self.0).decimal = separator;

        self
    }

    /// Sets integer grouping: (3,3) is 1,234,567; (3,2) is 12,34,567.
    /// Panics unless group sizes are between one and eighteen. An empty separator disables grouping.
    pub fn grouping(
        mut self,
        separator: impl Into<Arc<str>>,
        primary: usize,
        secondary: usize,
    ) -> Self {
        assert!(
            (1..=18).contains(&primary) && (1..=18).contains(&secondary),
            "invalid digit groups"
        );
        let format = Arc::make_mut(&mut self.0);

        format.separator = separator.into();
        format.primary = primary;
        format.secondary = secondary;

        self
    }

    /// Supplies ten distinct Unicode digit characters. Panics for duplicates or
    /// characters that are not numeric. Formatting records UTF-8 byte identities.
    pub fn digits(mut self, digits: [char; 10]) -> Self {
        assert!(
            digits
                .iter()
                .enumerate()
                .all(|(i, c)| c.is_numeric() && !digits[..i].contains(c)),
            "invalid digit alphabet"
        );
        Arc::make_mut(&mut self.0).digits = digits;

        self
    }

    /// Formats an exact value with semantic digit slots independent of separators,
    /// affix length and integer growth. At most 256 leading integer digits are permitted.
    /// Preparation allocates; keep the result when manually animating formatted text.
    pub fn format(&self, value: FixedNumber, min_integer_digits: usize) -> FormattedNumber {
        assert!(min_integer_digits <= 256, "minimum digit count exceeds 256");
        let scale = usize::from(value.scale);
        let magnitude = value.coefficient.unsigned_abs().to_string();

        let width = magnitude
            .len()
            .max(scale + 1)
            .max(scale + min_integer_digits);

        let padded = format!("{magnitude:0>width$}");
        let integer = padded.len() - scale;
        let mut text = String::new();
        let mut slots = Vec::new();

        let push =
            |text: &mut String, slots: &mut Vec<(usize, usize)>, string: &str, slot: usize| {
                for (index, c) in string.char_indices() {
                    slots.push((text.len(), slot + index * 8));
                    text.push(c);
                }
            };

        if value.coefficient < 0 {
            push(&mut text, &mut slots, "-", 4);
        }

        push(&mut text, &mut slots, &self.0.prefix, 2);

        for (index, digit) in padded.bytes().enumerate() {
            if index == integer && scale > 0 {
                push(&mut text, &mut slots, &self.0.decimal, 5);
            }

            if index > 0 && index < integer && !self.0.separator.is_empty() {
                let remaining = integer - index;

                if remaining == self.0.primary
                    || (remaining > self.0.primary
                        && (remaining - self.0.primary).is_multiple_of(self.0.secondary))
                {
                    push(
                        &mut text,
                        &mut slots,
                        &self.0.separator,
                        6 + remaining * self.0.separator.len() * 8,
                    );
                }
            }

            let slot = if index < integer {
                (integer - index - 1) * 8
            } else {
                (index - integer) * 8 + 1
            };

            slots.push((text.len(), slot));
            text.push(self.0.digits[usize::from(digit - b'0')]);
        }

        push(&mut text, &mut slots, &self.0.suffix, 3);

        FormattedNumber {
            text: text.into(),
            slots: slots.into(),
        }
    }
}

/// Prepared text plus semantic place identities. Slot identities encode decimal place and token kind; affixes and
/// separators remain distinct even with long UTF-8 prefixes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormattedNumber {
    text: Arc<str>,
    slots: Arc<[(usize, usize)]>,
}

impl FormattedNumber {
    /// Full display/accessibility text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Stable slot containing a UTF-8 byte. Invalid offsets return `None`.
    pub fn slot_at(&self, byte: usize) -> Option<usize> {
        if byte >= self.text.len() {
            return None;
        }

        let index = self.slots.partition_point(|(start, _)| *start <= byte);

        index.checked_sub(1).map(|index| self.slots[index].1)
    }
}

/// One immutable display-text span. Font size and line height remain label-wide,
/// matching the pinned GPUI shaping API. Selection and editing are not provided.
#[derive(Clone, Debug, PartialEq)]
pub struct TextSpan {
    pub(crate) text: Arc<str>,
    pub(crate) color: Option<Color>,
    pub(crate) family: Option<Arc<str>>,
    pub(crate) weight: Option<f32>,
}

impl TextSpan {
    /// Creates a span inheriting the label's text style.
    pub fn new(text: impl Into<Arc<str>>) -> Self {
        Self {
            text: text.into(),
            color: None,
            family: None,
            weight: None,
        }
    }

    /// Overrides this span's color.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);

        self
    }

    /// Overrides this span's native font family.
    pub fn font_family(mut self, family: impl Into<Arc<str>>) -> Self {
        self.family = Some(family.into());

        self
    }

    /// Overrides font weight. Panics outside finite 1..=1000; actual face selection
    /// depends on the font and backend, rather than promising variable-font axes.
    pub fn font_weight(mut self, weight: f32) -> Self {
        assert!(
            weight.is_finite() && (1.0..=1000.0).contains(&weight),
            "invalid font weight"
        );
        self.weight = Some(weight);

        self
    }

    /// Optional span color override.
    pub fn text_color(&self) -> Option<Color> {
        self.color
    }

    /// Optional font family override.
    pub fn family(&self) -> Option<&str> {
        self.family.as_deref()
    }

    /// Optional native font weight override.
    pub fn weight(&self) -> Option<f32> {
        self.weight
    }

    /// Logical span text.
    pub fn text(&self) -> &str {
        &self.text
    }
}

#[derive(Clone, Debug, PartialEq)]
struct RichData {
    text: Arc<str>,
    spans: Vec<TextSpan>,
}

/// Immutable, reusable styled display text. The complete string is shaped before
/// animation units are split, preserving GPUI's font fallback and cluster boundaries.
#[derive(Clone, Debug, PartialEq)]
pub struct RichText(Arc<RichData>);

impl RichText {
    /// Concatenates spans once; empty spans are discarded.
    pub fn new(spans: impl IntoIterator<Item = TextSpan>) -> Self {
        let spans: Vec<_> = spans
            .into_iter()
            .filter(|span| !span.text.is_empty())
            .collect();

        let text = spans
            .iter()
            .map(|span| span.text.as_ref())
            .collect::<String>();

        Self(Arc::new(RichData {
            text: text.into(),
            spans,
        }))
    }

    /// Complete logical and accessibility label.
    pub fn text(&self) -> &str {
        &self.0.text
    }

    /// Prepared spans, in logical text order.
    pub fn spans(&self) -> &[TextSpan] {
        &self.0.spans
    }

    /// Span containing a UTF-8 byte offset, or `None` beyond the text.
    pub fn span_at(&self, byte: usize) -> Option<&TextSpan> {
        let mut end = 0;

        self.0.spans.iter().find(|span| {
            end += span.text.len();

            byte < end
        })
    }
}
