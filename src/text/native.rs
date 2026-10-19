/// Shared implementation for the two supported GPUI type universes. This is an
/// integration detail; applications use `TextMotionState` from their native adapter.
#[doc(hidden)]
#[macro_export]
macro_rules! __motion_text_adapter {
    ($ui:ident, $to_color:path, $from_color:path) => {
        use std::{
            cell::RefCell,
            collections::{BTreeMap, BTreeSet, VecDeque},
            rc::Rc,
            sync::Arc,
            time::{Duration, Instant},
        };
        use $crate::{
            Animation, MotionValue, NumberEffect, PresenceMode, Scramble, StaggerFrom, TextClip,
            TextError, TextLayoutUnit, TextMatch, TextOptions, TextPlaybackState, TextPresentation,
            TextPreset, TextTarget, TextUnit, TextUnitInfo, Transition,
        };
        use $ui::{
            AnyElement, App, AvailableSpace, Bounds, ContentMask, Div, Element, ElementId,
            GlobalElementId, InspectorElementId, IntoElement, LayoutId, ParentElement, Pixels,
            Point, SharedString, Size, Stateful, Style, StyleRefinement, Styled, TextAlign,
            TextStyle, WhiteSpace, Window, canvas, div, point, prelude::*, px, size,
        };

        struct PaintGlyph {
            color: Option<$crate::Color>,
            font: $ui::FontId,
            glyph: $ui::GlyphId,
            emoji: bool,
            point: Point<Pixels>,
        }

        struct PaintPiece {
            layout: TextLayoutUnit,
            glyphs: Vec<PaintGlyph>,
            substitutes: Vec<Vec<PaintGlyph>>,
        }

        #[derive(Clone, PartialEq)]
        struct ShapeKey {
            text: SharedString,
            style: TextStyle,
            width: Option<Pixels>,
            rem: Pixels,
            options: TextOptions,
            number: Option<$crate::FormattedNumber>,
            rich: Option<$crate::RichText>,
        }

        struct PreparedText {
            key: ShapeKey,
            pieces: Vec<Arc<PaintPiece>>,
            size: Size<Pixels>,
        }

        struct TextData {
            engine: TextPlaybackState,
            epoch: Option<Instant>,
            prepared: VecDeque<Rc<PreparedText>>,
            committed: Option<ShapeKey>,
            origin: Option<Point<Pixels>>,
            paints: BTreeMap<u64, Arc<PaintPiece>>,
            colors: BTreeMap<(u64, usize), $crate::ColorState>,
            font_size: Option<MotionValue>,
            style_active: bool,
            style_clock: $crate::Playback,
            error: Option<TextError>,
            replay: bool,
        }

        impl Default for TextData {
            fn default() -> Self {
                Self {
                    engine: TextPlaybackState::default(),
                    epoch: None,
                    prepared: VecDeque::new(),
                    committed: None,
                    origin: None,
                    paints: BTreeMap::new(),
                    colors: BTreeMap::new(),
                    font_size: None,
                    style_active: false,
                    style_clock: $crate::Playback::default(),
                    error: None,
                    replay: false,
                }
            }
        }

        impl TextData {
            fn now(&mut self, cx: &App) -> Duration {
                let clock = cx.background_executor().now();

                clock.saturating_duration_since(*self.epoch.get_or_insert(clock))
            }
        }

        /// View-owned native text motion. Clones share lifecycle and shape caches.
        /// Retain one state per logical label, with one mounted element per state.
        /// Fonts and full runs are shaped before splitting; native glyph atlas painting
        /// preserves kerning, contextual forms, bidi positions, and emoji.
        #[derive(Clone, Default)]
        pub struct TextMotionState(Rc<RefCell<TextData>>);

        impl TextMotionState {
            /// Declares a label's current semantic content. Entry/exit effects run on
            /// changes, and on first mount unless `animate_initial(false)` is selected.
            /// Native styles are inherited from the surrounding GPUI element.
            pub fn element(
                &self,
                id: impl Into<ElementId>,
                text: impl Into<SharedString>,
            ) -> TextElement {
                let text = text.into();

                TextElement {
                    container: div().id(id).role($ui::Role::Label).aria_value(text.clone()),
                    text,
                    state: self.clone(),
                    options: TextOptions::default(),
                    enter_with: None,
                    progress: None,
                    number: None,
                    number_layout: None,
                    rich: None,
                    font_motion: None,
                    color_transition: None,
                }
            }

            /// Declares immutable styled runs. Font family/weight/color may vary;
            /// font size and line height remain label-wide. Editing/decorations are
            /// outside this display-label API. The complete text is shaped first.
            pub fn rich_element(
                &self,
                id: impl Into<ElementId>,
                rich: &$crate::RichText,
            ) -> TextElement {
                let mut element = self.element(id, rich.text().to_owned());
                element.rich = Some(rich.clone());
                element
            }

            /// Freezes the current presentation. Notify the owning view after controls.
            pub fn pause(&self, window: &mut Window, cx: &App) {
                let mut data = self.0.borrow_mut();
                let now = data.now(cx);

                data.engine.pause(now);
                data.style_clock.pause(now);
                window.refresh();
            }

            /// Resumes from the held presentation.
            pub fn play(&self, window: &mut Window, cx: &App) {
                let mut data = self.0.borrow_mut();
                let now = data.now(cx);

                data.engine.play(now);
                data.style_clock.play(now);
                window.refresh();
            }

            /// Holds a normalized entrance cursor. Returns false for infinite clips.
            /// Springs use a two-second normalized horizon. Panics outside zero through one.
            pub fn seek(&self, progress: f64, window: &mut Window, cx: &App) -> bool {
                let mut data = self.0.borrow_mut();
                let now = data.now(cx);
                let accepted = data.engine.seek(progress, now);

                window.refresh();

                accepted
            }

            /// Replays current text and reevaluates custom entrance callbacks next render.
            pub fn restart(&self, window: &mut Window, _: &App) {
                let mut data = self.0.borrow_mut();

                data.prepared.clear();
                data.replay = true;
                window.refresh();
            }

            /// Whether the last sample requested ongoing animation.
            pub fn is_animating(&self) -> bool {
                {
                    let data = self.0.borrow();
                    data.engine.is_animating() || data.style_active
                }
            }

            /// Copies presentations in unscrolled window-layout coordinates. This allocates;
            /// normal frame sampling does not need it.
            pub fn presentation(&self) -> Vec<TextPresentation> {
                self.0.borrow().engine.presentation().copied().collect()
            }

            /// Last shaping or glyph-paint error. No application logging is performed.
            pub fn error(&self) -> Option<TextError> {
                self.0.borrow().error.clone()
            }
        }

        type Entrance = Rc<dyn for<'a> Fn(TextUnitInfo<'a>) -> TextTarget>;

        /// Styled native text declaration created by `TextMotionState::element`.
        /// Layout takes destination sizes immediately; use `reflow` for position motion.
        /// This is display text, not an editable or selectable input implementation.
        pub struct TextElement {
            container: Stateful<Div>,
            state: TextMotionState,
            text: SharedString,
            options: TextOptions,
            enter_with: Option<Entrance>,
            progress: Option<f64>,
            number: Option<NumberRequest>,
            number_layout: Option<$crate::FormattedNumber>,
            rich: Option<$crate::RichText>,
            font_motion: Option<(f32, Transition)>,
            color_transition: Option<Transition>,
        }

        impl Styled for TextElement {
            fn style(&mut self) -> &mut StyleRefinement {
                self.container.style()
            }
        }

        impl TextElement {
            /// Animates span/inherited glyph colors using the shared linear-light
            /// color sampler. Normal glyph painting is unchanged when omitted.
            pub fn color_transition(mut self, transition: Transition) -> Self {
                self.color_transition = Some(transition);
                self
            }

            /// Animates the resolved label font size in logical pixels. This performs
            /// native reshaping/layout while moving; it is not GPU scale projection.
            /// Panics for nonpositive/non-finite sizes. Retain the text state.
            pub fn animate_font_size(mut self, pixels: f32, transition: Transition) -> Self {
                assert!(
                    pixels.is_finite() && pixels > 0.0,
                    "invalid animated font size"
                );
                self.font_motion = Some((pixels, transition));
                self
            }

            /// Chooses the mask axis for a custom reveal target.
            pub fn reveal_axis(mut self, axis: $crate::TextRevealAxis) -> Self {
                self.options.reveal_axis = axis;
                self
            }

            /// Chooses whole text, words, Unicode graphemes, or visual wrapped lines.
            pub fn units(mut self, units: TextUnit) -> Self {
                self.options.unit = units;

                self
            }

            /// Selects a convenience effect. Later target builders override its poses.
            pub fn preset(mut self, preset: TextPreset) -> Self {
                self.options.preset(preset);

                self
            }

            /// Selects the per-unit entrance pose.
            pub fn enter(mut self, target: TextTarget) -> Self {
                self.options.enter = target;

                self
            }

            /// Selects the outgoing pose. Exits remain painted until settled.
            pub fn exit(mut self, target: TextTarget) -> Self {
                self.options.exit = target;

                self
            }

            /// Sets spring, tween, or instant timing.
            pub fn transition(mut self, transition: Transition) -> Self {
                self.options.transition = transition;

                self
            }

            /// Sets independent opacity timing. Movement and reveal keep `transition`.
            /// SlideUp and wipe presets default to a 100 ms fade; later calls override it.
            pub fn opacity_transition(mut self, transition: Transition) -> Self {
                self.options.opacity_transition = Some(transition);

                self
            }

            /// Sets stagger delays in logical order, including punctuation and spaces.
            pub fn stagger(mut self, delay: Duration, from: StaggerFrom) -> Self {
                self.options.stagger = delay;
                self.options.stagger_from = from;

                self
            }

            /// Selects identity matching across content changes.
            pub fn matching(mut self, matching: TextMatch) -> Self {
                self.options.matching = matching;

                self
            }

            /// Animates measured natural positions; font metrics and sizes change immediately.
            pub fn reflow(mut self, transition: Transition) -> Self {
                self.options.reflow = Some(transition);

                self
            }

            /// Coordinates incoming content with outgoing exits. Pending changes use latest text.
            pub fn presence_mode(mut self, mode: PresenceMode) -> Self {
                self.options.presence_mode = mode;

                self
            }

            /// Clips ink to each unit's stationary line-height reservation.
            pub fn clip(mut self, clip: TextClip) -> Self {
                self.options.clip = clip;

                self
            }

            /// Controls initial mount only; subsequent content changes still animate.
            pub fn animate_initial(mut self, animate: bool) -> Self {
                self.options.animate_initial = animate;

                self
            }

            /// Prepares deterministic visual substitutions without changing semantic text.
            /// Use grapheme grouping for a character scramble.
            pub fn scramble(mut self, scramble: Scramble) -> Self {
                self.options.scramble = Some(scramble);

                self
            }

            /// Adds a prepared clip per unit. Retain the clip outside render; clones preserve identity.
            /// Only x/y/opacity channels apply, in logical pixels. Exit lifetime is still finite.
            pub fn animation(mut self, animation: &Animation) -> Self {
                self.options.animation = Some(animation.clone());

                self
            }

            /// Prepares an entrance per shaped unit, on layout/config changes or explicit replay.
            pub fn enter_with(
                mut self,
                entrance: impl for<'a> Fn(TextUnitInfo<'a>) -> TextTarget + 'static,
            ) -> Self {
                self.enter_with = Some(Rc::new(entrance));

                self
            }

            /// Scrubs a finite entrance sequence, including stagger. Panics for invalid
            /// progress or an infinite clip; no autonomous entrance frames are requested.
            pub fn progress(mut self, progress: f64) -> Self {
                assert!((0. ..=1.).contains(&progress));
                self.progress = Some(progress);

                self
            }
        }

        impl IntoElement for TextElement {
            type Element = Stateful<Div>;
            fn into_element(self) -> Self::Element {
                self.container.child(TextPainter {
                    state: self.state,
                    text: self.text,
                    options: self.options,
                    enter_with: self.enter_with,
                    progress: self.progress,
                    number: self.number,
                    number_layout: self.number_layout,
                    rich: self.rich,
                    font_motion: self.font_motion,
                    color_transition: self.color_transition,
                })
            }
        }

        struct NumberData {
            scale: u8,
            format: $crate::NumberFormat,
            value: Option<MotionValue>,
            epoch: Option<Instant>,
        }

        struct NumberRequest {
            state: Rc<RefCell<NumberData>>,
            target: i64,
            scale: u8,
            format: $crate::NumberFormat,
            effect: NumberEffect,
            digits: usize,
        }

        /// Retained count animation. Unchanged digits match by place value, from the right.
        #[derive(Clone)]
        pub struct NumberMotionState {
            text: TextMotionState,
            value: Rc<RefCell<NumberData>>,
        }

        impl Default for NumberMotionState {
            fn default() -> Self {
                Self {
                    text: TextMotionState::default(),
                    value: Rc::new(RefCell::new(NumberData {
                        scale: 0,
                        format: $crate::NumberFormat::default(),
                        value: None,
                        epoch: None,
                    })),
                }
            }
        }

        impl NumberMotionState {
            /// Declares a signed integer count. Exact endpoints include all i64 values.
            /// Numeric interpolation uses f64 between endpoints; rolling digits stay exact.
            pub fn element(&self, id: impl Into<ElementId>, value: i64) -> NumberElement {
                let mut element = self.text.element(id, value.to_string());

                element.number = Some(NumberRequest {
                    state: self.value.clone(),
                    target: value,
                    scale: 0,
                    format: self.value.borrow().format.clone(),
                    effect: NumberEffect::Roll,
                    digits: 0,
                });

                NumberElement(element)
            }

            /// Declares an exact fixed-point value with explicit locale formatting.
            /// Digit identities use decimal places, independent of grouping/affixes.
            /// Changing scale rescales the interpolation trajectory and velocity.
            pub fn formatted_element(
                &self,
                id: impl Into<ElementId>,
                value: $crate::FixedNumber,
                format: &$crate::NumberFormat,
            ) -> NumberElement {
                let mut element = self.element(id, value.coefficient());
                let request = element.0.number.as_mut().expect("number declaration");
                request.scale = value.scale();
                request.format = format.clone();
                element
            }

            /// Digit-layer motion for playback controls and diagnostics.
            pub fn text_state(&self) -> &TextMotionState {
                &self.text
            }
        }

        /// Styled count declaration. Number formatting reserves at most 256 minimum digits.
        pub struct NumberElement(TextElement);

        impl Styled for NumberElement {
            fn style(&mut self) -> &mut StyleRefinement {
                self.0.style()
            }
        }

        impl NumberElement {
            /// Animates inherited digit color using the shared linear-light solver.
            pub fn color_transition(mut self, transition: Transition) -> Self {
                self.0.color_transition = Some(transition);
                self
            }

            /// Animates label-wide font size through native shaping and layout.
            pub fn animate_font_size(mut self, pixels: f32, transition: Transition) -> Self {
                self.0 = self.0.animate_font_size(pixels, transition);
                self
            }

            /// Chooses rolling, fading, sliding, or numeric interpolation.
            pub fn effect(mut self, effect: NumberEffect) -> Self {
                self.0.number.as_mut().unwrap().effect = effect;

                self
            }

            /// Pads the magnitude with zeroes. The sign is separate. Panics above 256.
            pub fn min_digits(mut self, digits: usize) -> Self {
                assert!(digits <= 256, "minimum digit count exceeds 256");
                self.0.number.as_mut().unwrap().digits = digits;

                self
            }

            /// Chooses changed-digit or numeric interpolation timing.
            pub fn transition(mut self, transition: Transition) -> Self {
                self.0.options.transition = transition;

                self
            }

            /// Overrides the independent opacity timing, including rolling/sliding exits.
            pub fn opacity_transition(mut self, transition: Transition) -> Self {
                self.0.options.opacity_transition = Some(transition);

                self
            }

            /// Skips initial digit entrance while retaining later transitions.
            pub fn animate_initial(mut self, animate: bool) -> Self {
                self.0.options.animate_initial = animate;

                self
            }
        }

        impl IntoElement for NumberElement {
            type Element = Stateful<Div>;
            fn into_element(mut self) -> Self::Element {
                let request = self.0.number.as_ref().expect("number declaration");
                let semantic = request.format.format(
                    $crate::FixedNumber::new(request.target, request.scale)
                        .expect("validated scale"),
                    0,
                );
                self.0.container = self.0.container.aria_value(semantic.text().to_owned());

                self.0.options.unit = TextUnit::Graphemes;
                self.0.options.matching = TextMatch::Position;
                self.0.options.reflow = Some(self.0.options.transition);
                let opacity_transition = self.0.options.opacity_transition;

                match self.0.number.as_ref().unwrap().effect {
                    NumberEffect::Roll => self.0.options.preset(TextPreset::SlideUp),
                    NumberEffect::Slide => {
                        self.0.options.preset(TextPreset::SlideUp);
                        self.0.options.clip = TextClip::None;
                    }
                    NumberEffect::Crossfade | NumberEffect::Interpolate => {
                        self.0.options.preset(TextPreset::Fade)
                    }
                }

                if let Some(transition) = opacity_transition {
                    self.0.options.opacity_transition = Some(transition);
                }

                self.0.into_element()
            }
        }

        struct DrawPiece {
            piece: Arc<PaintPiece>,
            frame: TextPresentation,
            fallback: Option<AnyElement>,
        }

        struct TextPainter {
            state: TextMotionState,
            text: SharedString,
            options: TextOptions,
            enter_with: Option<Entrance>,
            progress: Option<f64>,
            number: Option<NumberRequest>,
            number_layout: Option<$crate::FormattedNumber>,
            rich: Option<$crate::RichText>,
            font_motion: Option<(f32, Transition)>,
            color_transition: Option<Transition>,
        }

        impl TextPainter {
            fn resolve_number(&mut self, window: &mut Window, cx: &App) {
                let Some(request) = &self.number else {
                    return;
                };

                let mut state = request.state.borrow_mut();
                let clock = cx.background_executor().now();
                let now = clock.saturating_duration_since(*state.epoch.get_or_insert(clock));

                let initialized = state.value.is_some();
                let previous_scale = state.scale;
                state.scale = request.scale;
                let value = state
                    .value
                    .get_or_insert_with(|| MotionValue::new(request.target as f64));
                if initialized && previous_scale != request.scale {
                    value.sample(now, cx.reduce_motion());
                    value
                        .rescale(10_f64.powi(i32::from(request.scale) - i32::from(previous_scale)));
                }

                let displayed = if request.effect == NumberEffect::Interpolate {
                    value.animate(request.target as f64, self.options.transition, now);
                    let frame = value.sample(now, cx.reduce_motion());

                    if frame.is_animating() {
                        window.request_animation_frame();

                        frame.value().round() as i64
                    } else {
                        request.target
                    }
                } else {
                    value.jump(request.target as f64, now);

                    request.target
                };

                let formatted = request.format.format(
                    $crate::FixedNumber::new(displayed, request.scale)
                        .expect("validated number scale"),
                    request.digits,
                );
                self.text = formatted.text().to_owned().into();
                self.number_layout = Some(formatted);

                if request.effect == NumberEffect::Interpolate {
                    self.options.transition = Transition::Instant;
                    self.options.animate_initial = false;
                }
            }
        }

        impl IntoElement for TextPainter {
            type Element = Self;
            fn into_element(self) -> Self {
                self
            }
        }

        impl Element for TextPainter {
            type RequestLayoutState = TextStyle;
            type PrepaintState = Vec<DrawPiece>;
            fn id(&self) -> Option<ElementId> {
                None
            }

            fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
                None
            }

            fn request_layout(
                &mut self,
                _: Option<&GlobalElementId>,
                _: Option<&InspectorElementId>,
                window: &mut Window,
                cx: &mut App,
            ) -> (LayoutId, TextStyle) {
                self.resolve_number(window, cx);
                let mut style = window.text_style();
                {
                    let mut data = self.state.0.borrow_mut();
                    data.style_active = false;

                    if self.color_transition.is_none() {
                        data.colors.clear();
                    }

                    if let Some((target, transition)) = self.font_motion {
                        let wall = data.now(cx);
                        let now = data.style_clock.time(wall);
                        let playing = data.style_clock.is_playing();
                        let font = data.font_size.get_or_insert_with(|| {
                            MotionValue::new(f64::from(
                                style.font_size.to_pixels(window.rem_size()),
                            ))
                        });
                        font.animate(f64::from(target), transition, now);
                        let frame = font.sample(now, cx.reduce_motion());
                        style.font_size =
                            px(frame.value().max(0.1).min(f64::from(f32::MAX)) as f32).into();
                        data.style_active |= frame.is_animating() && playing;

                        if frame.is_animating() && playing {
                            window.request_animation_frame();
                        }
                    } else {
                        data.font_size = None;
                    }
                }
                let state = self.state.clone();
                let text = self.text.clone();
                let options = self.options.clone();
                let style_copy = style.clone();
                let enter_with = self.enter_with.clone();
                let number = self.number_layout.clone();
                let rich = self.rich.clone();

                let layout = window.request_measured_layout(
                    Style::default(),
                    move |known, available, window, cx| {
                        let width = if style_copy.white_space == WhiteSpace::Nowrap {
                            None
                        } else {
                            known.width.or(match available.width {
                                AvailableSpace::Definite(width) => Some(width),
                                _ => None,
                            })
                        };

                        let key = ShapeKey {
                            text: text.clone(),
                            style: style_copy.clone(),
                            width,
                            rem: window.rem_size(),
                            options: options.clone(),
                            number: number.clone(),
                            rich: rich.clone(),
                        };

                        prepare_cached(&state, key, enter_with.as_ref(), window, cx)
                            .map_or(Size::default(), |prepared| prepared.size)
                    },
                );

                (layout, style)
            }

            fn prepaint(
                &mut self,
                _: Option<&GlobalElementId>,
                _: Option<&InspectorElementId>,
                bounds: Bounds<Pixels>,
                style: &mut TextStyle,
                window: &mut Window,
                cx: &mut App,
            ) -> Vec<DrawPiece> {
                assert!(
                    self.progress.is_none()
                        || self
                            .options
                            .animation
                            .as_ref()
                            .is_none_or(|animation| animation.duration().is_some()),
                    "cannot scrub an infinite text clip"
                );
                let key = ShapeKey {
                    text: self.text.clone(),
                    style: style.clone(),
                    width: if style.white_space == WhiteSpace::Nowrap {
                        None
                    } else {
                        Some(bounds.size.width)
                    },
                    rem: window.rem_size(),
                    options: self.options.clone(),
                    number: self.number_layout.clone(),
                    rich: self.rich.clone(),
                };

                let Some(prepared) =
                    prepare_cached(&self.state, key, self.enter_with.as_ref(), window, cx)
                else {
                    return Vec::new();
                };

                let mut data = self.state.0.borrow_mut();
                let now = data.now(cx);
                let ancestor_offset = window.pixel_snap_point(window.element_offset());
                let origin = bounds.origin - ancestor_offset;

                if data.committed.as_ref() != Some(&prepared.key) || data.replay {
                    let layout: Vec<_> = prepared
                        .pieces
                        .iter()
                        .map(|piece| {
                            let mut layout = piece.layout.clone();

                            layout.origin[0] += f64::from(origin.x);
                            layout.origin[1] += f64::from(origin.y);

                            layout
                        })
                        .collect();

                    let ids = data.engine.reconcile(
                        &self.text,
                        &layout,
                        &self.options,
                        now,
                        cx.reduce_motion(),
                    );

                    for (id, piece) in ids.into_iter().zip(&prepared.pieces) {
                        data.paints.insert(id, piece.clone());
                    }

                    let TextData { paints, colors, .. } = &mut *data;

                    colors.retain(|(id, index), _| {
                        paints.get(id).is_some_and(|piece| {
                            *index
                                < piece
                                    .glyphs
                                    .len()
                                    .max(piece.substitutes.iter().map(Vec::len).max().unwrap_or(0))
                        })
                    });

                    data.committed = Some(prepared.key.clone());
                    data.origin = Some(origin);

                    if data.replay {
                        data.engine.restart(now);
                        data.replay = false;
                    }
                }

                if data.origin != Some(origin) {
                    if let Some(previous) = data.origin {
                        let delta = origin - previous;

                        data.engine
                            .translate([f64::from(delta.x), f64::from(delta.y)]);
                    }

                    data.origin = Some(origin);
                }

                if data.engine.sample(now, cx.reduce_motion(), self.progress) {
                    window.request_animation_frame();
                }

                let count = data.engine.presentation().count();

                if data.paints.len() > count {
                    let ids: BTreeSet<_> =
                        data.engine.presentation().map(|frame| frame.id).collect();

                    data.paints.retain(|id, _| ids.contains(id));
                    data.colors.retain(|(id, _), _| ids.contains(id));
                }

                let mut elements = Vec::with_capacity(count);

                for frame in data
                    .engine
                    .presentation()
                    .filter(|frame| frame.visible && frame.opacity > 0.)
                {
                    let Some(piece) = data.paints.get(&frame.id).cloned() else {
                        continue;
                    };

                    let mut frame = *frame;

                    frame.origin[0] += f64::from(ancestor_offset.x);
                    frame.origin[1] += f64::from(ancestor_offset.y);
                    frame.clip_origin[0] += f64::from(ancestor_offset.x);
                    frame.clip_origin[1] += f64::from(ancestor_offset.y);
                    let glyphs = frame
                        .substitution
                        .and_then(|index| piece.substitutes.get(index))
                        .unwrap_or(&piece.glyphs);

                    if frame.opacity >= 1. || !glyphs.iter().any(|glyph| glyph.emoji) {
                        elements.push(DrawPiece {
                            piece,
                            frame,
                            fallback: None,
                        });
                        continue;
                    }

                    // Emoji opacity is owned by GPUI element composition. Monochrome
                    // glyphs paint directly and need no per-unit native layout nodes.
                    let mask = self.options.clip == TextClip::Unit || frame.reveal < 1.;

                    let (clip_origin, clip_size) = mask_geometry(&frame, &self.options);
                    let origin = if mask { clip_origin } else { frame.origin };

                    let offset = [frame.origin[0] - origin[0], frame.origin[1] - origin[1]];
                    let substitution = frame.substitution;
                    let color = style.color;
                    let color_transition = self.color_transition;
                    let font_size = px(piece.layout.em as f32);
                    let error_state = self.state.clone();
                    let fallback_piece = piece.clone();

                    let drawing = canvas(
                        move |_, _, _| (),
                        move |bounds, _, window, cx| {
                            let glyphs = substitution
                                .and_then(|index| fallback_piece.substitutes.get(index))
                                .unwrap_or(&fallback_piece.glyphs);

                            for (glyph_index, glyph) in glyphs.iter().enumerate() {
                                let color = animated_color(
                                    &error_state,
                                    (frame.id, glyph_index),
                                    glyph.color,
                                    color,
                                    color_transition,
                                    window,
                                    cx,
                                );
                                let at = bounds.origin
                                    + glyph.point
                                    + point(px(offset[0] as f32), px(offset[1] as f32));

                                let result = if glyph.emoji {
                                    window.paint_emoji(at, glyph.font, glyph.glyph, font_size)
                                } else {
                                    window.paint_glyph(
                                        at,
                                        glyph.font,
                                        glyph.glyph,
                                        font_size,
                                        color,
                                    )
                                };

                                if let Err(error) = result {
                                    error_state.0.borrow_mut().error =
                                        Some(TextError::Painting(error.to_string().into()));
                                }
                            }
                        },
                    )
                    .size_full();

                    let mut element = div()
                        .relative()
                        .opacity(frame.opacity as f32)
                        .w(px(clip_size[0] as f32))
                        .h(px(clip_size[1] as f32))
                        .when(mask, |element| element.overflow_hidden())
                        .child(drawing)
                        .into_any_element();

                    element.layout_as_root(
                        size(AvailableSpace::MaxContent, AvailableSpace::MaxContent),
                        window,
                        cx,
                    );
                    element.prepaint_at(
                        point(px(origin[0] as f32), px(origin[1] as f32)),
                        window,
                        cx,
                    );
                    elements.push(DrawPiece {
                        piece,
                        frame,
                        fallback: Some(element),
                    });
                }

                elements
            }

            fn paint(
                &mut self,
                _: Option<&GlobalElementId>,
                _: Option<&InspectorElementId>,
                _bounds: Bounds<Pixels>,
                style: &mut TextStyle,
                elements: &mut Vec<DrawPiece>,
                window: &mut Window,
                cx: &mut App,
            ) {
                let mut error = None;

                for draw in elements {
                    if let Some(element) = &mut draw.fallback {
                        element.paint(window, cx);
                        continue;
                    }

                    let frame = draw.frame;
                    let piece = &draw.piece;

                    let mask = if self.options.clip == TextClip::Unit || frame.reveal < 1. {
                        let (origin, extent) = mask_geometry(&frame, &self.options);
                        Some(ContentMask {
                            bounds: Bounds::new(
                                point(px(origin[0] as f32), px(origin[1] as f32)),
                                size(px(extent[0] as f32), px(extent[1] as f32)),
                            ),
                        })
                    } else {
                        None
                    };

                    let origin = point(px(frame.origin[0] as f32), px(frame.origin[1] as f32));

                    let glyphs = frame
                        .substitution
                        .and_then(|index| piece.substitutes.get(index))
                        .unwrap_or(&piece.glyphs);

                    window.with_content_mask(mask, |window| {
                        for (glyph_index, glyph) in glyphs.iter().enumerate() {
                            let color = animated_color(
                                &self.state,
                                (frame.id, glyph_index),
                                glyph.color,
                                style.color,
                                self.color_transition,
                                window,
                                cx,
                            )
                            .opacity(frame.opacity as f32);
                            let at = origin + glyph.point;

                            let result = if glyph.emoji {
                                window.paint_emoji(
                                    at,
                                    glyph.font,
                                    glyph.glyph,
                                    px(piece.layout.em as f32),
                                )
                            } else {
                                window.paint_glyph(
                                    at,
                                    glyph.font,
                                    glyph.glyph,
                                    px(piece.layout.em as f32),
                                    color,
                                )
                            };

                            if let Err(failure) = result {
                                error = Some(TextError::Painting(failure.to_string().into()));
                            }
                        }
                    });
                }

                if error.is_some() {
                    self.state.0.borrow_mut().error = error;
                }
            }
        }

        fn mask_geometry(frame: &TextPresentation, options: &TextOptions) -> ([f64; 2], [f64; 2]) {
            let axis = usize::from(options.reveal_axis == $crate::TextRevealAxis::Vertical);
            let mut origin = frame.clip_origin;
            let mut size = frame.size;
            size[axis] *= frame.reveal;

            if options.wipe_left {
                origin[axis] += frame.size[axis] - size[axis];
            }
            (origin, size)
        }

        fn native_color(color: $crate::Color) -> $ui::Hsla {
            $to_color(color)
        }

        fn animated_color(
            state: &TextMotionState,
            key: (u64, usize),
            desired: Option<$crate::Color>,
            inherited: $ui::Hsla,
            transition: Option<Transition>,
            window: &mut Window,
            cx: &App,
        ) -> $ui::Hsla {
            let Some(transition) = transition else {
                return desired.map_or(inherited, native_color);
            };
            let target = desired.unwrap_or_else(|| $from_color(inherited));
            let mut data = state.0.borrow_mut();
            let wall = data.now(cx);
            let now = data.style_clock.time(wall);
            let playing = data.style_clock.is_playing();
            let value = data
                .colors
                .entry(key)
                .or_insert_with(|| $crate::ColorState::new(target));
            let (color, active) = value.sample(target, transition, now, cx.reduce_motion());
            data.style_active |= active && playing;

            if active && playing {
                window.request_animation_frame();
            }
            native_color(color)
        }

        fn prepare_cached(
            state: &TextMotionState,
            key: ShapeKey,
            entrance: Option<&Entrance>,
            window: &Window,
            cx: &App,
        ) -> Option<Rc<PreparedText>> {
            let mut data = state.0.borrow_mut();

            if let Some(prepared) = data.prepared.iter().find(|prepared| prepared.key == key) {
                return Some(prepared.clone());
            }

            match prepare_text(key, entrance, window, cx) {
                Ok(prepared) => {
                    let prepared = Rc::new(prepared);

                    if data.prepared.len() == 4 {
                        data.prepared.pop_front();
                    }

                    data.prepared.push_back(prepared.clone());
                    data.error = None;

                    Some(prepared)
                }
                Err(error) => {
                    data.error = Some(TextError::Shaping(error.into()));

                    None
                }
            }
        }

        struct PositionedGlyph {
            paint: PaintGlyph,
            byte: usize,
            row: usize,
            right: f32,
        }

        fn prepare_text(
            key: ShapeKey,
            entrance: Option<&Entrance>,
            window: &Window,
            _cx: &App,
        ) -> Result<PreparedText, String> {
            let em = key.style.font_size.to_pixels(key.rem);
            let line_height = key.style.line_height_in_pixels(key.rem);

            let runs = key.rich.as_ref().map(|rich| {
                rich.spans()
                    .iter()
                    .map(|span| {
                        let mut run = key.style.to_run(span.text().len());

                        if let Some(family) = span.family() {
                            run.font.family = family.to_owned().into();
                        }

                        if let Some(weight) = span.weight() {
                            run.font.weight = $ui::FontWeight(weight);
                        }

                        if let Some(color) = span.text_color() {
                            run.color = native_color(color);
                        }
                        run
                    })
                    .collect::<Vec<_>>()
            });
            let inherited = [key.style.to_run(key.text.len())];
            let lines = window
                .text_system()
                .shape_text(
                    key.text.clone(),
                    em,
                    runs.as_deref().unwrap_or(&inherited),
                    key.width,
                    None,
                )
                .map_err(|error| error.to_string())?;

            let mut glyphs = Vec::new();
            let mut row_start = 0;
            let mut byte_start = 0;
            let mut max_width = 0_f32;

            for line in &lines {
                let mut starts = vec![0_f32];

                starts.extend(line.wrap_boundaries.iter().map(|boundary| {
                    f32::from(
                        line.unwrapped_layout.runs[boundary.run_ix].glyphs[boundary.glyph_ix]
                            .position
                            .x,
                    )
                }));
                let mut ends: Vec<_> = starts.iter().skip(1).copied().collect();

                ends.push(f32::from(line.unwrapped_layout.width));
                let widths: Vec<_> = starts
                    .iter()
                    .zip(&ends)
                    .map(|(start, end)| (end - start).max(0.))
                    .collect();

                max_width = widths.iter().copied().fold(max_width, f32::max);
                let baseline =
                    (line_height - line.unwrapped_layout.ascent - line.unwrapped_layout.descent)
                        / 2.
                        + line.unwrapped_layout.ascent;

                let mut row = 0;
                let mut wrap = line.wrap_boundaries.iter().peekable();

                for (run_index, run) in line.unwrapped_layout.runs.iter().enumerate() {
                    for (glyph_index, glyph) in run.glyphs.iter().enumerate() {
                        if wrap.peek().is_some_and(|boundary| {
                            boundary.run_ix == run_index && boundary.glyph_ix == glyph_index
                        }) {
                            row += 1;
                            wrap.next();
                        }

                        let row_width = widths[row];

                        let spare = key
                            .width
                            .map_or(0., |width| (f32::from(width) - row_width).max(0.));

                        let align = match key.style.text_align {
                            TextAlign::Left => 0.,
                            TextAlign::Center => spare / 2.,
                            TextAlign::Right => spare,
                        };

                        let x = f32::from(glyph.position.x) - starts[row] + align;

                        let y =
                            baseline + line_height * (row_start + row) as f32 + glyph.position.y;

                        glyphs.push(PositionedGlyph {
                            paint: PaintGlyph {
                                color: key
                                    .rich
                                    .as_ref()
                                    .and_then(|rich| rich.span_at(byte_start + glyph.index))
                                    .and_then(|span| span.text_color()),
                                font: run.font_id,
                                glyph: glyph.id,
                                emoji: glyph.is_emoji,
                                point: point(px(x), y),
                            },
                            byte: byte_start + glyph.index,
                            row: row_start + row,
                            right: row_width + align,
                        });
                    }
                }

                row_start += starts.len();
                byte_start += line.len() + 1;
            }

            let height = line_height * row_start as f32;

            let width = key
                .width
                .map_or(max_width, |width| max_width.min(f32::from(width)));

            let mut ranges = $crate::text_ranges(&key.text, key.options.unit);
            let mut line_rows = Vec::new();

            if key.options.unit == TextUnit::Lines {
                ranges.clear();

                for row in 0..row_start {
                    let start = glyphs
                        .iter()
                        .filter(|glyph| glyph.row == row)
                        .map(|glyph| glyph.byte)
                        .min();

                    if let Some(start) = start {
                        let last = glyphs
                            .iter()
                            .filter(|glyph| glyph.row == row)
                            .map(|glyph| glyph.byte)
                            .max()
                            .unwrap();

                        let end = glyphs
                            .iter()
                            .map(|glyph| glyph.byte)
                            .filter(|byte| *byte > last)
                            .min()
                            .unwrap_or(key.text.len());

                        ranges.push(start..end);
                        line_rows.push(row);
                    }
                }
            } else {
                let mut safe: BTreeSet<_> = glyphs.iter().map(|glyph| glyph.byte).collect();

                safe.insert(key.text.len());
                let mut merged = Vec::new();
                let mut start = 0;

                for range in ranges {
                    if safe.contains(&range.end) {
                        merged.push(start..range.end);
                        start = range.end;
                    }
                }

                if start < key.text.len() {
                    merged.push(start..key.text.len());
                }

                ranges = merged;
            }

            let count = ranges.len();
            let mut pieces = Vec::with_capacity(count);

            for (index, range) in ranges.into_iter().enumerate() {
                let selected: Vec<_> = glyphs
                    .iter()
                    .filter(|glyph| {
                        if let Some(row) = line_rows.get(index) {
                            glyph.row == *row
                        } else {
                            range.contains(&glyph.byte)
                        }
                    })
                    .collect();

                if selected.is_empty() {
                    continue;
                }

                let first_row = selected.iter().map(|glyph| glyph.row).min().unwrap();
                let last_row = selected.iter().map(|glyph| glyph.row).max().unwrap();

                let x = if matches!(key.options.unit, TextUnit::Whole | TextUnit::Lines) {
                    0.
                } else {
                    selected
                        .iter()
                        .map(|glyph| f32::from(glyph.paint.point.x))
                        .fold(f32::INFINITY, f32::min)
                };

                let y = f32::from(line_height) * first_row as f32;
                let mut right = x;

                for glyph in &selected {
                    let next = glyphs
                        .iter()
                        .filter(|next| {
                            next.row == glyph.row && next.paint.point.x > glyph.paint.point.x
                        })
                        .map(|next| f32::from(next.paint.point.x))
                        .fold(glyph.right, f32::min);

                    right = right.max(next);
                }

                let piece_width = if key.options.unit == TextUnit::Lines {
                    key.width.map_or(max_width, f32::from)
                } else if key.options.unit == TextUnit::Whole {
                    width
                } else {
                    (right - x).max(0.)
                };

                let piece_height = f32::from(line_height) * (last_row - first_row + 1) as f32;

                let content: Arc<str> = if key.options.unit == TextUnit::Lines {
                    let starts: BTreeSet<_> = selected.iter().map(|glyph| glyph.byte).collect();
                    let mut content = String::new();

                    for start in starts {
                        let end = glyphs
                            .iter()
                            .map(|glyph| glyph.byte)
                            .filter(|byte| *byte > start)
                            .min()
                            .unwrap_or(key.text.len());

                        content.push_str(&key.text[start..end]);
                    }

                    content.into()
                } else {
                    Arc::from(&key.text[range.clone()])
                };

                let mut layout = TextLayoutUnit::new(
                    content.clone(),
                    key.number.as_ref().map_or(index, |number| {
                        number.slot_at(range.start).unwrap_or(10_000_000 + index)
                    }),
                    [x.into(), y.into()],
                    [piece_width.into(), piece_height.into()],
                    f32::from(em).into(),
                );

                if let Some(entrance) = entrance {
                    layout.enter = Some(entrance(TextUnitInfo {
                        index,
                        count,
                        text: &content,
                    }));
                }

                let paints = selected
                    .iter()
                    .map(|glyph| PaintGlyph {
                        color: glyph.paint.color,
                        font: glyph.paint.font,
                        glyph: glyph.paint.glyph,
                        emoji: glyph.paint.emoji,
                        point: glyph.paint.point - point(px(x), px(y)),
                    })
                    .collect();

                let mut substitutes = Vec::new();

                if let Some(scramble) = &key.options.scramble {
                    for substitute in scramble.alphabet() {
                        let line = window.text_system().shape_line(
                            substitute.clone().into(),
                            em,
                            &[key.style.to_run(substitute.len())],
                            None,
                        );

                        let baseline =
                            (line_height - line.ascent - line.descent) / 2. + line.ascent;

                        let centering = (px(piece_width) - line.width()) / 2.;

                        let substitute_color = key
                            .rich
                            .as_ref()
                            .and_then(|rich| rich.span_at(range.start))
                            .and_then(|span| span.text_color());
                        substitutes.push(
                            line.runs
                                .iter()
                                .flat_map(|run| {
                                    run.glyphs.iter().map(move |glyph| PaintGlyph {
                                        color: substitute_color,
                                        font: run.font_id,
                                        glyph: glyph.id,
                                        emoji: glyph.is_emoji,
                                        point: glyph.position + point(centering, baseline),
                                    })
                                })
                                .collect(),
                        );
                    }
                }

                pieces.push(Arc::new(PaintPiece {
                    layout,
                    glyphs: paints,
                    substitutes,
                }));
            }

            debug_assert_eq!(
                pieces.iter().map(|piece| piece.glyphs.len()).sum::<usize>(),
                glyphs.len(),
                "every shaped glyph must belong to exactly one text unit"
            );

            Ok(PreparedText {
                key,
                pieces,
                size: size(px(width), height),
            })
        }
    };
}
