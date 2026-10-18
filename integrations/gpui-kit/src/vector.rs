//! Native vector fill/stroke with reusable path morph ownership.
use gpui_ce_motion::{
    Color, MotionValue, PathCommand, PathError, PathMorphState, PathShape, StrokeReveal, Transition,
};

use gpui_kit::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    Path, PathBuilder, Pixels, Style, Window, point, px, size,
};

use std::{cell::RefCell, rc::Rc, time::Instant};

struct Data {
    morph: PathMorphState,
    epoch: Option<Instant>,
    reveal: [MotionValue; 3],
    error: Option<PathError>,
}

/// Retain one vector state per drawing. Clones share morph/reveal storage.
/// Path sampling allocates no memory; native tessellation/paint preparation does.
#[derive(Clone)]
pub struct VectorState(Rc<RefCell<Data>>);

impl VectorState {
    /// Creates prepared coordinate/reveal state.
    ///
    /// ```
    /// use gpui_motion_kit::VectorState;
    /// use gpui_ce_motion::{PathShape, PathCommand, Color};
    /// let path = PathShape::new([PathCommand::Move([0.,0.]), PathCommand::Line([100.,0.])])?;
    /// let state = VectorState::new(path.clone()); // persistent view field
    /// let drawing = state.element("path", path, [120., 20.], Color::rgb(0xffffff))?.stroke(2.);
    /// # Ok::<(), gpui_ce_motion::PathError>(())
    /// ```
    pub fn new(shape: PathShape) -> Self {
        Self(Rc::new(RefCell::new(Data {
            morph: PathMorphState::new(shape),
            epoch: None,
            reveal: [
                MotionValue::new(1.),
                MotionValue::new(0.),
                MotionValue::new(1.),
            ],
            error: None,
        })))
    }

    /// Most recent native tessellation error; invalid topology is returned by `element`.
    pub fn error(&self) -> Option<PathError> {
        self.0.borrow().error
    }

    /// Creates a native drawing, validating topology before any mutation.
    /// Geometry uses logical coordinates relative to the fixed reservation.
    pub fn element(
        &self,
        id: impl Into<ElementId>,
        target: PathShape,
        bounds: [f32; 2],
        color: Color,
    ) -> Result<VectorElement, PathError> {
        self.0.borrow().morph.validate(&target)?;

        if !bounds.iter().all(|v| v.is_finite() && *v >= 0.) {
            return Err(PathError::Geometry);
        }

        Ok(VectorElement {
            id: id.into(),
            state: self.clone(),
            target,
            bounds,
            color,
            stroke: Some(2.),
            transition: Transition::default(),
            reveal: StrokeReveal::default(),
        })
    }
}

/// Fixed-size native vector drawing. This paints vector geometry, not an SVG
/// sprite or a GPUI subtree; it does not install pointer semantics.
pub struct VectorElement {
    id: ElementId,
    state: VectorState,
    target: PathShape,
    bounds: [f32; 2],
    color: Color,
    stroke: Option<f32>,
    transition: Transition,
    reveal: StrokeReveal,
}

impl VectorElement {
    /// Selects coordinate and stroke reveal timing.
    pub fn transition(mut self, transition: Transition) -> Self {
        self.transition = transition;

        self
    }

    /// Selects full-path fill. Reveal applies only to strokes.
    pub fn fill(mut self) -> Self {
        self.stroke = None;

        self
    }

    /// Selects a finite positive stroke width.
    pub fn stroke(mut self, width: f32) -> Self {
        assert!(width.is_finite() && width > 0.);
        self.stroke = Some(width);

        self
    }

    /// Animates normalized path length/offset/spacing. Springs are clamped at the
    /// drawing boundary; the coordinate/reveal momentum remains retained.
    pub fn reveal(mut self, reveal: StrokeReveal) -> Self {
        self.reveal = reveal;

        self
    }
}

impl IntoElement for VectorElement {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for VectorElement {
    type RequestLayoutState = ();
    type PrepaintState = Option<Path<Pixels>>;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        w: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let style = Style {
            size: size(px(self.bounds[0]).into(), px(self.bounds[1]).into()),
            flex_shrink: 0.,
            ..Default::default()
        };

        (w.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        w: &mut Window,
        cx: &mut App,
    ) -> Option<Path<Pixels>> {
        let mut data = self.state.0.borrow_mut();

        let first = data.epoch.is_none();

        let epoch = *data
            .epoch
            .get_or_insert_with(|| cx.background_executor().now());

        let now = cx
            .background_executor()
            .now()
            .saturating_duration_since(epoch);

        let mut reveal = [0.; 3];
        let mut active = false;

        for (i, value) in data.reveal.iter_mut().enumerate() {
            if first {
                value.jump(self.reveal.values()[i], now);
            }

            value.animate(self.reveal.values()[i], self.transition, now);
            let frame = value.sample(now, cx.reduce_motion());

            reveal[i] = frame.value();
            active |= frame.is_animating();
        }

        // Clamping only affects drawing. Avoid tiny spring-overshoot dash periods.
        let reveal = StrokeReveal::new(reveal[0].clamp(0., 1.), reveal[1], reveal[2].clamp(0., 1.))
            .unwrap_or(self.reveal);

        let frame = data
            .morph
            .sample(&self.target, self.transition, now, cx.reduce_motion())
            .expect("validated topology");

        active |= frame.is_animating();

        if active {
            w.request_animation_frame();
        }

        let point_at = |p: [f64; 2]| bounds.origin + point(px(p[0] as f32), px(p[1] as f32));

        let mut builder = self
            .stroke
            .map_or_else(PathBuilder::fill, |width| PathBuilder::stroke(px(width)));

        if self.stroke.is_some() {
            let mut previous = None;

            frame.stroke_segments(reveal, |a, b| {
                if previous != Some(a) {
                    builder.move_to(point_at(a));
                }

                builder.line_to(point_at(b));
                previous = Some(b);
            });
        } else {
            for &command in frame.commands() {
                match command {
                    PathCommand::Move(p) => builder.move_to(point_at(p)),
                    PathCommand::Line(p) => builder.line_to(point_at(p)),
                    PathCommand::Quadratic(control, to) => {
                        builder.curve_to(point_at(to), point_at(control))
                    }
                    PathCommand::Cubic(a, b, to) => {
                        builder.cubic_bezier_to(point_at(to), point_at(a), point_at(b))
                    }
                    PathCommand::Close => builder.close(),
                }
            }
        }

        match builder.build() {
            Ok(path) => {
                data.error = None;

                Some(path)
            }
            Err(_) => {
                data.error = Some(PathError::Tessellation);

                None
            }
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        path: &mut Option<Path<Pixels>>,
        w: &mut Window,
        _: &mut App,
    ) {
        if let Some(path) = path.take() {
            let [r, g, b, a] = self.color.channels();

            w.paint_path(path, gpui_kit::Rgba { r, g, b, a });
        }
    }
}
