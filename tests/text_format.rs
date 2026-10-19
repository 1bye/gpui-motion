//! Covers: exact fixed-point display and semantic identities across localized grouping.
//! Real: shared formatting. Doubles: none. Requires: text feature.
#![cfg(feature = "text")]
use gpui_ce_motion::*;

#[test]
fn fixed_point_currency_is_exact_including_i64_min_and_nonwestern_grouping() {
    let format = NumberFormat::default()
        .prefix("€")
        .decimal(",")
        .grouping("\u{202f}", 3, 3);

    assert_eq!(
        format
            .format(FixedNumber::new(-123456789, 2).unwrap(), 0)
            .text(),
        "-€1\u{202f}234\u{202f}567,89"
    );
    assert_eq!(
        NumberFormat::default()
            .format(FixedNumber::new(i64::MIN, 2).unwrap(), 0)
            .text(),
        "-92233720368547758.08"
    );
    assert_eq!(
        NumberFormat::default()
            .grouping(",", 3, 2)
            .format(1234567.into(), 0)
            .text(),
        "12,34,567"
    );
    assert!(FixedNumber::new(1, 19).is_err());
}

#[test]
fn semantic_digit_slots_survive_prefixes_groups_and_decimal_places() {
    let format = NumberFormat::default().prefix("USD ").grouping(",", 3, 3);
    let a = format.format(FixedNumber::new(99999, 2).unwrap(), 0);
    let b = format.format(FixedNumber::new(199999, 2).unwrap(), 0);

    assert_eq!(a.text(), "USD 999.99");
    assert_eq!(b.text(), "USD 1,999.99");
    assert_eq!(a.slot_at(a.text().len() - 1), b.slot_at(b.text().len() - 1));
    assert_eq!(a.slot_at(4), b.slot_at(6));
    assert_eq!(a.slot_at(0), b.slot_at(0));
    let arabic = NumberFormat::default()
        .digits(['٠', '١', '٢', '٣', '٤', '٥', '٦', '٧', '٨', '٩'])
        .format(FixedNumber::new(120, 2).unwrap(), 0);

    assert_eq!(arabic.text(), "١.٢٠");
    assert_eq!(arabic.slot_at(0), Some(0));
}

#[test]
fn rich_text_keeps_logical_text_and_explicit_span_styles() {
    let rich = RichText::new([
        TextSpan::new("Hello "),
        TextSpan::new("世界")
            .color(Color::rgb(0x44aaff))
            .font_weight(700.0),
    ]);

    assert_eq!(rich.text(), "Hello 世界");
    assert_eq!(rich.spans().len(), 2);
}
