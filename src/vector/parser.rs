//! Setup-only SVG path parsing using the backend's existing SVG grammar library.
use super::{PathCommand, PathError, PathShape};
use std::fmt;

/// SVG grammar or bounded-geometry preparation failure.
#[derive(Debug)]
pub enum SvgPathError {
    /// Invalid SVG path data, retaining the parser's source position/error.
    Syntax(svgtypes::Error),
    /// Invalid geometry or a preparation budget exceeded.
    Geometry(PathError),
}

impl fmt::Display for SvgPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(error) => write!(f, "invalid SVG path: {error}"),
            Self::Geometry(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for SvgPathError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Syntax(error) => Some(error),
            Self::Geometry(error) => Some(error),
        }
    }
}

impl PathShape {
    /// Parses an SVG `d` string, resolving relative/implicit/smooth commands and
    /// converting elliptical arcs to cubic Beziers. This does not parse an SVG
    /// document, paints, transforms, or CSS. Prepare once outside rendering.
    /// Input is bounded to 1 MiB and 65,536 expanded commands.
    ///
    /// # Errors
    /// Returns grammar errors with their original position, geometry errors for
    /// non-finite/out-of-range coordinates, or `Complexity` when budgets are exceeded.
    #[cfg_attr(feature = "svg-path", doc = "```")]
    #[cfg_attr(feature = "svg-path", doc = "use gpui_ce_motion::PathShape;")]
    #[cfg_attr(
        feature = "svg-path",
        doc = "let path = PathShape::parse_svg(\"M0 0h20a10 10 0 0 1 10 10z\").unwrap();"
    )]
    #[cfg_attr(feature = "svg-path", doc = "assert!(path.commands().len() >= 4);")]
    #[cfg_attr(feature = "svg-path", doc = "```")]
    pub fn parse_svg(data: &str) -> Result<Self, SvgPathError> {
        if data.len() > 1_048_576 {
            return Err(SvgPathError::Geometry(PathError::Complexity));
        }

        let mut commands = Vec::new();

        for segment in svgtypes::SimplifyingPathParser::from(data) {
            if commands.len() >= 65_536 {
                return Err(SvgPathError::Geometry(PathError::Complexity));
            }

            let command = match segment.map_err(SvgPathError::Syntax)? {
                svgtypes::SimplePathSegment::MoveTo { x, y } => PathCommand::Move([x, y]),
                svgtypes::SimplePathSegment::LineTo { x, y } => PathCommand::Line([x, y]),
                svgtypes::SimplePathSegment::CurveTo {
                    x1,
                    y1,
                    x2,
                    y2,
                    x,
                    y,
                } => PathCommand::Cubic([x1, y1], [x2, y2], [x, y]),
                svgtypes::SimplePathSegment::Quadratic { x1, y1, x, y } => {
                    PathCommand::Quadratic([x1, y1], [x, y])
                }
                svgtypes::SimplePathSegment::ClosePath => PathCommand::Close,
            };

            commands.push(command);
        }

        Self::new(commands).map_err(SvgPathError::Geometry)
    }
}
