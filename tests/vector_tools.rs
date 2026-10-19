//! Covers: SVG grammar conversion, contour normalization and path-following queries.
//! Real: svgtypes parser and shared geometry. Doubles: none. Requires: svg-path feature.
#![cfg(feature = "svg-path")]
use gpui_ce_motion::*;

#[test]
fn parses_relative_implicit_smooth_and_arc_commands_with_original_errors() {
    let path = PathShape::parse_svg(
        "m10 10 10 0h10v10q10 10 20 0t20 0c1 2 3 4 5 6s3 4 5 6a10 15 20 0 1 20 20z",
    )
    .unwrap();

    assert!(
        path.commands()
            .iter()
            .any(|command| matches!(command, PathCommand::Cubic(..)))
    );
    assert_eq!(path.commands()[0], PathCommand::Move([10.0, 10.0]));
    assert_eq!(path.commands()[1], PathCommand::Line([20.0, 10.0]));
    assert!(matches!(
        PathShape::parse_svg("M0 0 A10 nope"),
        Err(SvgPathError::Syntax(_))
    ));
    assert!(PathShape::parse_svg("M1e20 0 L0 1").is_err());
}

#[test]
fn unrelated_single_contours_normalize_once_and_morph_through_shared_solver() {
    let triangle = PathShape::parse_svg("M0 0 L100 0 L50 80 Z").unwrap();
    let circle = PathShape::parse_svg("M0 0 A50 50 0 1 1 100 0 A50 50 0 1 1 0 0 Z").unwrap();
    let (from, to) = triangle.normalize_for_morph(&circle, 64).unwrap();

    assert!(from.compatible(&to));
    let mut state = PathMorphState::new(from);

    assert!(
        state
            .sample(&to, Transition::default(), std::time::Duration::ZERO, false)
            .unwrap()
            .is_animating()
    );
    assert!(
        !state
            .sample(&to, Transition::default(), std::time::Duration::ZERO, true)
            .unwrap()
            .is_animating()
    );
    assert!(
        triangle
            .normalize_for_morph(&PathShape::parse_svg("M0 0 L1 1").unwrap(), 64)
            .is_err()
    );
}

#[test]
fn measured_following_skips_subpath_gaps_and_reports_tangents() {
    let path = PathShape::parse_svg("M0 0 H10 M100 100 V110").unwrap();
    let measure = PathMeasure::new(&path).unwrap();

    assert_eq!(measure.length(), 20.0);
    assert_eq!(measure.point(0.25).position, [5.0, 0.0]);
    assert_eq!(measure.point(0.75).position, [100.0, 105.0]);
    assert_eq!(measure.point(0.75).tangent_degrees, 90.0);
    assert_eq!(measure.point(2.0).position, [100.0, 110.0]);
}
