//! The same gallery runs against two stock native adapters.
use crate::{motion, ui};

use gpui_ce_motion::{
    ChildrenOrder, Color, Drag, Easing, FixedNumber, GestureTargets, GraphSubscription, Motion,
    MotionConfig, MotionNode, MotionTree, NumberEffect, NumberFormat, PathMeasure, PathShape,
    ReorderAxis, RichText, Spring, StaggerFrom, SvgMotion, SvgTarget, Target, TextPreset, TextSpan,
    TextUnit, TrackId, Transition, TypedAnimation, ValueId, Variants,
};

use motion::{
    Axis, GestureState, GraphState, MotionAnchor, MotionState, MotionTreeState, Motions,
    NumberMotionState, ReorderGroup, ScrollTracker, SharedOverlay, SvgMotionState, TextMotionState,
    TypedTimeline, VectorState,
};

use std::{cell::Cell, rc::Rc, time::Duration};

use ui::{
    AnyElement, Context, IntoElement, MouseButton, Render, Role, ScrollHandle, Window, div,
    prelude::*, px, rgb,
};

const SCENES: [&str; 10] = [
    "Connected",
    "Sequence",
    "Timeline",
    "Scroll",
    "Drag",
    "Reorder",
    "Transfer",
    "Paths",
    "Typography",
    "Collection",
];

fn spring() -> Transition {
    Transition::Spring(Spring::visual(Duration::from_millis(320), 0.08).unwrap())
}

fn tile(id: usize) -> ui::Stateful<ui::Div> {
    div()
        .id(("tile", id))
        .rounded_xl()
        .w(px(112.))
        .h(px(82.))
        .flex_shrink_0()
        .bg(rgb(if id.is_multiple_of(3) {
            0xd8f58b
        } else {
            0x2b332b
        }))
        .text_color(rgb(if id.is_multiple_of(3) {
            0x182018
        } else {
            0xf0f4e8
        }))
        .flex()
        .items_center()
        .justify_center()
        .child(format!("{:02}", id + 1))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Part {
    Panel,
    Title,
    Progress,
    Notice,
}

pub struct Lab {
    scene: usize,
    forward: bool,
    graph: GraphState,
    progress: ValueId,
    follower: ValueId,
    colors: [ValueId; 4],
    _subscription: GraphSubscription,
    tree: MotionTreeState<u8, bool>,
    timeline: TypedTimeline,
    color_track: TrackId<Color, 4>,
    position_track: TrackId<Target, 6>,
    svg_track: TrackId<SvgTarget, 5>,
    svg: SvgMotionState,
    drag: GestureState,
    drag_viewport: MotionAnchor,
    zoom: Rc<Cell<f32>>,
    pub(crate) grid: ReorderGroup<u8>,
    pub(crate) grid_viewport: MotionAnchor,
    pub(crate) grid_scroll: ScrollHandle,
    scroll: ScrollHandle,
    viewport: MotionAnchor,
    target: MotionAnchor,
    tracker: ScrollTracker,
    overlay: SharedOverlay<u8>,
    shapes: [PathShape; 2],
    path: PathMeasure,
    vector: VectorState,
    line: VectorState,
    curve: PathShape,
    text: TextMotionState,
    wipe: TextMotionState,
    number: NumberMotionState,
    rich: [RichText; 2],
    format: NumberFormat,
    numeric: MotionState,
    motions: Motions<Part>,
}

impl Lab {
    pub fn new() -> Self {
        let mut graph = GraphState::default();
        let progress = graph.source(0.);

        let follower = graph
            .derive(&[progress], |values| values[0] * 260.)
            .unwrap();

        let colors = graph.typed_source(Color::rgb(0xd8f58b));
        let subscription = graph.subscribe(progress, |_| {}).unwrap();

        let mut tree = MotionTree::default()
            .config(MotionConfig::default().stagger(Duration::from_millis(45)));

        for key in 0..4 {
            let variants = Variants::new([
                (
                    false,
                    Motion::new(Target::default().y(0.)).transition(spring()),
                ),
                (
                    true,
                    Motion::new(Target::default().y(-50.)).transition(spring()),
                ),
            ])
            .unwrap();

            let node = MotionNode::new(key, Target::default(), variants);

            tree.add(if key == 0 {
                node.when(ChildrenOrder::BeforeChildren)
            } else {
                node.parent(0)
            })
            .unwrap();
        }

        let mut timeline = TypedTimeline::default();

        let color_track = timeline.at(
            Duration::ZERO,
            Color::rgb(0xd8f58b),
            TypedAnimation::to(
                Color::rgb(0xf7ac8b),
                Duration::from_millis(750),
                Easing::EaseInOut,
            )
            .unwrap(),
        );

        timeline.label("move", Duration::from_millis(160)).unwrap();
        let position_track = timeline
            .at_label(
                "move",
                0.,
                Target::default().x(0.).width(140.).border_radius(18.),
                TypedAnimation::spring(
                    Target::default().x(230.).width(200.).border_radius(42.),
                    Spring::default(),
                    Duration::from_millis(1000),
                )
                .unwrap(),
            )
            .unwrap();

        let svg_track = timeline
            .at_label(
                "move",
                0.1,
                SvgTarget::default(),
                TypedAnimation::to(
                    SvgTarget::default().rotate_degrees(360.).scale(1.5),
                    Duration::from_millis(650),
                    Easing::EaseInOut,
                )
                .unwrap(),
            )
            .unwrap();

        let drag_viewport = MotionAnchor::default();
        let zoom = Rc::new(Cell::new(1.));
        let scale = zoom.clone();
        let pinch_start = Cell::new(1.0_f32);

        let drag = GestureState::default()
            .with_drag(Drag::unconstrained([0.; 2]).direction_lock(8.))
            .constraints(drag_viewport.clone())
            .on_pinch(move |event, _, _| {
                if event.phase == gpui_ce_motion::GesturePhase::Started {
                    pinch_start.set(scale.get());
                }

                scale.set((f64::from(pinch_start.get()) * event.scale).clamp(0.5, 2.) as f32);
            });

        let drag = crate::configure_drag(drag);
        let grid_scroll = ScrollHandle::new();
        let grid_viewport = MotionAnchor::default();

        let grid = ReorderGroup::spatial((0..18).collect(), ReorderAxis::Auto)
            .unwrap()
            .autoscroll(
                grid_scroll.clone(),
                grid_viewport.clone(),
                gpui_ce_motion::AutoScroll::default(),
            );

        let scroll = ScrollHandle::new();
        let viewport = MotionAnchor::default();
        let target = MotionAnchor::default().scroll_source(scroll.clone());

        let tracker = ScrollTracker::new(scroll.clone(), viewport.clone(), target.clone(), Axis::Y)
            .margins([16., 0., 16., 0.])
            .transition(spring());

        let circle =
            PathShape::parse_svg("M180 100 A80 80 0 1 1 20 100 A80 80 0 1 1 180 100 Z").unwrap();

        let triangle = PathShape::parse_svg("M100 20 L185 170 L15 170 Z").unwrap();
        let (a, b) = circle.normalize_for_morph(&triangle, 72).unwrap();
        let curve = PathShape::parse_svg("M10 130 C100 0 180 220 310 60").unwrap();
        let path = PathMeasure::new(&curve).unwrap();

        let rich = [false, true].map(|active| {
            RichText::new([
                TextSpan::new("Café ").font_weight(500.),
                TextSpan::new("in motion.")
                    .font_weight(700.)
                    .color(Color::rgb(if active { 0xf7ac8b } else { 0xd8f58b })),
            ])
        });

        let vector = VectorState::new(a.clone());
        let line = VectorState::new(curve.clone());

        Self {
            scene: 0,
            forward: false,
            graph,
            progress,
            follower,
            colors,
            _subscription: subscription,
            tree: MotionTreeState::new(tree),
            timeline,
            color_track,
            position_track,
            svg_track,
            svg: SvgMotionState::default(),
            drag,
            drag_viewport,
            zoom,
            grid,
            grid_viewport,
            grid_scroll,
            scroll,
            viewport,
            target,
            tracker,
            overlay: SharedOverlay::default(),
            shapes: [a, b],
            path,
            vector,
            line,
            curve,
            text: TextMotionState::default(),
            wipe: TextMotionState::default(),
            number: NumberMotionState::default(),
            rich,
            format: NumberFormat::default()
                .prefix("€")
                .decimal(",")
                .grouping(" ", 3, 3),
            numeric: MotionState::default(),
            motions: Motions::with_capacity(4),
        }
    }

    pub(crate) fn play(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.forward = !self.forward;
        let now = self.graph.now(cx);

        self.graph
            .animate(
                self.progress,
                if self.forward { 1. } else { 0. },
                spring(),
                now,
            )
            .unwrap();
        self.graph
            .animate_typed(
                self.colors,
                Color::rgb(if self.forward { 0xf7ac8b } else { 0xd8f58b }),
                spring(),
                now,
            )
            .unwrap();
        let now_tree = self.tree.now(cx);

        self.tree
            .set_variant(&self.forward, now_tree, cx.reduce_motion())
            .unwrap();
        let now_timeline = self.timeline.now(cx);

        self.timeline.restart(now_timeline);
        self.wipe.restart(window, cx);
        cx.notify();
    }

    pub(crate) fn select_scene(&mut self, scene: usize) {
        self.scene = scene;
    }

    fn collection(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        // Numeric targets resolve the gallery's rem scale into logical pixels.
        let unit = f32::from(window.rem_size());
        let width = if self.forward { 24. } else { 18. } * unit;

        let panel_motion = Motion::new(Target::default().width(width).height(13. * unit))
            .initial(Target::default().width(18. * unit).height(13. * unit))
            .transition(spring());

        let title_motion = Motion::new(
            Target::default()
                .y(if self.forward { 0. } else { 0.4 * unit })
                .opacity(if self.forward { 1. } else { 0.65 }),
        )
        .transition(spring());

        let progress_motion =
            Motion::new(Target::default().width(if self.forward { 18. * unit } else { 5. * unit }))
                .initial(Target::default().width(5. * unit))
                .transition(spring());

        let [panel, title, progress] = self.motions.frame(
            [
                (Part::Panel, panel_motion),
                (Part::Title, title_motion),
                (Part::Progress, progress_motion),
            ],
            window,
            cx,
        );

        let mut stage = div().w_96().h_80().flex().flex_col().gap_4().child(
            panel.apply(
                div()
                    .id("collection-panel")
                    .debug_selector(|| "collection-panel".into())
                    .rounded_xl()
                    .bg(rgb(0x1c241c))
                    .flex_shrink_0()
                    .p_6()
                    .flex()
                    .flex_col()
                    .justify_between()
                    .child(
                        title.apply(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(div().text_xl().child("A little room to grow."))
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(0xa3b19c))
                                        .child("Your next release"),
                                ),
                        ),
                    )
                    .child(progress.apply(div().h_2().rounded_full().bg(rgb(0xd8f58b)))),
            ),
        );

        // Keep declaring the outgoing notice until it is absent, then release its key.
        if self.forward || self.motions.contains_key(&Part::Notice) {
            let notice_motion = Motion::new(Target::default())
                .initial(Target::default().y(0.5 * unit).opacity(0.))
                .exit(Target::default().y(-0.5 * unit).opacity(0.))
                .present(self.forward)
                .transition(spring());

            let notice = self
                .motions
                .frame_one(Part::Notice, notice_motion, window, cx);

            if !notice.is_mounted() {
                self.motions.remove(&Part::Notice);
            }

            stage = stage.child(notice.render(|frame| {
                frame.apply(
                    div()
                        .id("collection-notice")
                        .debug_selector(|| "collection-notice".into())
                        .role(Role::Status)
                        .rounded_lg()
                        .bg(rgb(0xd8f58b))
                        .text_color(rgb(0x182018))
                        .px_4()
                        .py_3()
                        .child("Ready to share"),
                )
            }));
        }

        stage.into_any_element()
    }

    fn stage(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        match self.scene {
            0 => {
                self.graph.frame(window, cx);
                let x = self.graph.get(self.follower).unwrap() as f32;
                let color = self.graph.typed_value(Color::rgb(0), self.colors).unwrap();

                div()
                    .w(px(400.))
                    .h(px(220.))
                    .relative()
                    .child(
                        div()
                            .absolute()
                            .top(px(38.))
                            .left(px(x))
                            .w(px(120.))
                            .h(px(120.))
                            .rounded_xl()
                            .bg(crate::native_color(color)),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(180.))
                            .left(px(260. - x))
                            .w(px(120.))
                            .h(px(8.))
                            .rounded_full()
                            .bg(rgb(0xf0f4e8)),
                    )
                    .into_any_element()
            }
            1 => {
                self.tree.frame(window, cx);
                let mut children = div().flex().gap_3();

                for key in 1..4 {
                    children =
                        children.child(self.tree.get(&key).unwrap().apply(tile(key as usize)));
                }

                self.tree
                    .get(&0)
                    .unwrap()
                    .apply(
                        div()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child(div().text_sm().child("Your next idea starts here."))
                            .child(children),
                    )
                    .into_any_element()
            }
            2 => {
                self.timeline.frame(window, cx);
                let color = self.timeline.value(self.color_track).unwrap();
                let target = self.timeline.value(self.position_track).unwrap();
                let pose = self.timeline.value(self.svg_track).unwrap();

                let frame = self.numeric.frame(
                    Motion::new(target).transition(Transition::Instant),
                    window,
                    cx,
                );

                let svg = self.svg.frame(
                    SvgMotion::new(pose).transition(Transition::Instant),
                    window,
                    cx,
                );

                div()
                    .w(px(500.))
                    .h(px(200.))
                    .child(
                        frame.apply(
                            div()
                                .h(px(140.))
                                .bg(crate::native_color(color))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(svg.apply(
                                    crate::native_svg().size(px(48.)).text_color(rgb(0x182018)),
                                )),
                        ),
                    )
                    .into_any_element()
            }
            3 => {
                let sample = self.tracker.frame(window, cx);

                div()
                    .flex()
                    .gap_8()
                    .items_center()
                    .child(
                        div()
                            .w(px(120.))
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(format!("{:02}%", (sample.progress * 100.).round() as i32))
                            .child(
                                div()
                                    .h(px(6.))
                                    .w(px((sample.progress * 120.).clamp(0., 120.) as f32))
                                    .bg(rgb(0xd8f58b))
                                    .rounded_full(),
                            ),
                    )
                    .child(
                        self.viewport.element(
                            "scroll-viewport",
                            div()
                                .id("manual-scroll")
                                .w(px(300.))
                                .h(px(280.))
                                .overflow_scroll()
                                .track_scroll(&self.scroll)
                                .bg(rgb(0x1c241c))
                                .rounded_xl()
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_4()
                                        .p_5()
                                        .child(
                                            div()
                                                .h(px(220.))
                                                .flex_shrink_0()
                                                .child("Scroll to find the card ↓"),
                                        )
                                        .child(self.target.element("scroll-target", tile(2)))
                                        .child(
                                            div()
                                                .h(px(350.))
                                                .flex_shrink_0()
                                                .child("Keep exploring."),
                                        ),
                                ),
                        ),
                    )
                    .into_any_element()
            }
            4 => {
                let drag = self.drag.clone();
                let pose = SvgTarget::default().scale(self.zoom.get());

                let svg = self
                    .svg
                    .frame(SvgMotion::new(pose).transition(spring()), window, cx);

                self.drag_viewport
                    .element(
                        "drag-bounds",
                        div()
                            .w(px(440.))
                            .h(px(260.))
                            .rounded_xl()
                            .bg(rgb(0x1c241c))
                            .p_5()
                            .child(
                                self.drag
                                    .bind(
                                        "drag-card",
                                        div()
                                            .id("draggable")
                                            .debug_selector(|| "draggable".into())
                                            .role(Role::Button)
                                            .aria_label("Drag card; use arrow keys to move")
                                            .w(px(140.))
                                            .h(px(130.))
                                            .rounded_xl()
                                            .bg(rgb(0xd8f58b))
                                            .text_color(rgb(0x182018))
                                            .flex()
                                            .flex_col()
                                            .items_center()
                                            .justify_center()
                                            .gap_3()
                                            .child(
                                                svg.apply(
                                                    crate::native_svg()
                                                        .size(px(40.))
                                                        .text_color(rgb(0x182018)),
                                                ),
                                            )
                                            .child(
                                                div()
                                                    .id("drag-handle")
                                                    .child("Drag handle")
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        move |event, window, cx| {
                                                            drag.start_drag(
                                                                [
                                                                    f64::from(event.position.x),
                                                                    f64::from(event.position.y),
                                                                ],
                                                                window,
                                                                cx,
                                                            );
                                                        },
                                                    ),
                                            ),
                                        Motion::new(Target::default()),
                                        GestureTargets::default(),
                                    )
                                    .drag_listener(false),
                            ),
                    )
                    .into_any_element()
            }
            5 => {
                let mut grid = div().flex().flex_wrap().gap_3().w(px(362.)).flex_shrink_0();

                for key in self.grid.order().iter().copied() {
                    grid = grid.child(
                        self.grid.item(
                            ("grid-item", usize::from(key)),
                            key,
                            tile(usize::from(key))
                                .role(Role::Button)
                                .aria_label(format!("Reorder item {}", key + 1)),
                        ),
                    );
                }

                self.grid_viewport
                    .element(
                        "grid-viewport",
                        div()
                            .id("grid-scroll")
                            .w(px(390.))
                            .h(px(280.))
                            .overflow_scroll()
                            .track_scroll(&self.grid_scroll)
                            .child(grid),
                    )
                    .into_any_element()
            }
            6 => {
                self.overlay
                    .select(0, if self.forward { "today" } else { "tomorrow" });

                div()
                    .relative()
                    .w(px(500.))
                    .h(px(260.))
                    .flex()
                    .justify_between()
                    .child(
                        div()
                            .w(px(200.))
                            .h_full()
                            .rounded_xl()
                            .bg(rgb(0x1c241c))
                            .p_5()
                            .overflow_hidden()
                            .child("Today")
                            .child(self.overlay.anchor(
                                0,
                                "today",
                                div().mt_5().w(px(148.)).h(px(100.)),
                            )),
                    )
                    .child(
                        div()
                            .w(px(200.))
                            .h_full()
                            .rounded_xl()
                            .bg(rgb(0x1c241c))
                            .p_5()
                            .overflow_hidden()
                            .child("Tomorrow")
                            .child(self.overlay.anchor(
                                0,
                                "tomorrow",
                                div().mt_5().w(px(148.)).h(px(100.)),
                            )),
                    )
                    .child(
                        self.overlay.element(
                            0,
                            "transfer-card",
                            div()
                                .w(px(148.))
                                .h(px(100.))
                                .rounded_xl()
                                .bg(rgb(0xd8f58b))
                                .text_color(rgb(0x182018))
                                .p_4()
                                .child("Make something move."),
                        ),
                    )
                    .into_any_element()
            }
            7 => {
                self.graph.frame(window, cx);
                let progress = self.graph.get(self.progress).unwrap().clamp(0., 1.);
                let point = self.path.point(progress);

                div()
                    .flex()
                    .items_center()
                    .gap_6()
                    .child(
                        self.vector
                            .element(
                                "morph",
                                self.shapes[usize::from(self.forward)].clone(),
                                [200., 200.],
                                Color::rgb(0xd8f58b),
                            )
                            .unwrap()
                            .transition(spring()),
                    )
                    .child(
                        div()
                            .w(px(320.))
                            .h(px(210.))
                            .relative()
                            .child(
                                self.line
                                    .element(
                                        "path",
                                        self.curve.clone(),
                                        [320., 200.],
                                        Color::rgb(0x667766),
                                    )
                                    .unwrap(),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .left(px(point.position[0] as f32 - 7.))
                                    .top(px(point.position[1] as f32 - 7.))
                                    .size(px(14.))
                                    .rounded_full()
                                    .bg(rgb(0xf7ac8b)),
                            ),
                    )
                    .into_any_element()
            }
            8 => div()
                .flex()
                .flex_col()
                .gap_6()
                .items_start()
                .child(
                    self.text
                        .rich_element("rich-text", &self.rich[usize::from(self.forward)])
                        .animate_initial(false)
                        .color_transition(spring())
                        .animate_font_size(if self.forward { 56. } else { 46. }, spring())
                        .text_size(px(46.)),
                )
                .child(
                    self.wipe
                        .element(
                            "vertical-reveal",
                            if self.forward {
                                "A fresh perspective."
                            } else {
                                "Room for another idea."
                            },
                        )
                        .preset(TextPreset::WipeDown)
                        .units(TextUnit::Words)
                        .stagger(Duration::from_millis(28), StaggerFrom::Start)
                        .text_size(px(28.)),
                )
                .child(
                    self.number
                        .formatted_element(
                            "currency",
                            FixedNumber::new(if self.forward { 120900 } else { 99999 }, 2).unwrap(),
                            &self.format,
                        )
                        .effect(NumberEffect::Roll)
                        .transition(spring())
                        .text_size(px(64.)),
                )
                .into_any_element(),
            9 => self.collection(window, cx),
            _ => unreachable!(),
        }
    }
}

impl Render for Lab {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let stage = self.stage(window, cx);
        let mut tabs = div().flex().flex_wrap().gap_2();

        for (index, title) in SCENES.iter().enumerate() {
            let label = format!("{:02}  {title}", index + 1);

            tabs = tabs.child(crate::button(
                ("scene", index),
                label,
                self.scene == index,
                cx.listener(move |this, _, _, cx| {
                    this.select_scene(index);
                    cx.notify();
                }),
            ));
        }

        div()
            .id("motion-lab")
            .size_full()
            .flex()
            .flex_col()
            .p_8()
            .gap_6()
            .bg(rgb(0x101710))
            .text_color(rgb(0xf0f4e8))
            .font_family(if cfg!(target_os = "macos") {
                "Helvetica Neue"
            } else {
                "sans-serif"
            })
            .child(tabs)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(stage),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .child(crate::button(
                                "play",
                                "Play / reverse".into(),
                                true,
                                cx.listener(|this, _, window, cx| this.play(window, cx)),
                            ))
                            .child(crate::button(
                                "reduce",
                                if cx.reduce_motion() {
                                    "Reduced motion"
                                } else {
                                    "Full motion"
                                }
                                .into(),
                                false,
                                cx.listener(|_, _, _, cx| {
                                    let reduce = !cx.reduce_motion();

                                    cx.set_reduce_motion(reduce);
                                    cx.notify();
                                }),
                            )),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0xa3b19c))
                            .child(match self.scene {
                                3 => "Scroll inside the panel",
                                4 => "Drag the handle · pinch the SVG",
                                5 => "Drag tiles · Option/Alt + arrows · Escape cancels",
                                _ => "Interrupt at any time",
                            }),
                    ),
            )
    }
}

#[cfg(all(test, feature = "native-tests"))]
mod tests {
    use super::*;

    #[ui::test]
    fn every_scene_uses_the_real_play_control_and_handles_interruption(
        cx: &mut ui::TestAppContext,
    ) {
        crate::init_test(cx);
        let window = cx.open_window(ui::size(px(960.), px(640.)), |_, _| Lab::new());

        cx.update_window(window.into(), |_, window, _| window.activate_window())
            .unwrap();
        cx.run_until_parked();

        for scene in 0..SCENES.len() {
            window
                .update(cx, |view, _, cx| {
                    view.scene = scene;
                    cx.notify();
                })
                .unwrap();
            draw(cx, window);
            let before = window.update(cx, |view, _, _| view.forward).unwrap();

            let position = ui::VisualTestContext::from_window(window.into(), cx)
                .debug_bounds("play")
                .unwrap()
                .center();

            ui::VisualTestContext::from_window(window.into(), cx)
                .simulate_click(position, ui::Modifiers::default());
            assert_ne!(
                window.update(cx, |view, _, _| view.forward).unwrap(),
                before
            );
            draw(cx, window);
            cx.executor().advance_clock(Duration::from_millis(100));
            draw(cx, window);
            ui::VisualTestContext::from_window(window.into(), cx)
                .simulate_click(position, ui::Modifiers::default());
            draw(cx, window);
            cx.executor().advance_clock(Duration::from_secs(3));
            draw(cx, window);
            window
                .update(cx, |view, _, _| {
                    assert!(view.text.error().is_none());
                    assert!(view.wipe.error().is_none());
                    assert!(view.number.text_state().error().is_none());
                    assert!(view.vector.error().is_none());
                })
                .unwrap();
        }
    }

    #[ui::test]
    fn collection_notice_exits_before_its_state_is_removed(cx: &mut ui::TestAppContext) {
        crate::init_test(cx);
        cx.update(|cx| cx.set_reduce_motion(false));
        let window = cx.open_window(ui::size(px(960.), px(640.)), |_, _| {
            let mut lab = Lab::new();

            lab.select_scene(9);

            lab
        });

        cx.update_window(window.into(), |_, window, _| window.activate_window())
            .unwrap();
        cx.run_until_parked();
        draw(cx, window);
        assert_eq!(
            window.update(cx, |view, _, _| view.motions.len()).unwrap(),
            3
        );
        let position = ui::VisualTestContext::from_window(window.into(), cx)
            .debug_bounds("play")
            .unwrap()
            .center();

        ui::VisualTestContext::from_window(window.into(), cx)
            .simulate_click(position, ui::Modifiers::default());
        draw(cx, window);
        cx.executor().advance_clock(Duration::from_millis(100));
        draw(cx, window);
        assert_eq!(
            window.update(cx, |view, _, _| view.motions.len()).unwrap(),
            4
        );
        assert!(
            ui::VisualTestContext::from_window(window.into(), cx)
                .debug_bounds("collection-notice")
                .is_some()
        );
        ui::VisualTestContext::from_window(window.into(), cx)
            .simulate_click(position, ui::Modifiers::default());
        draw(cx, window);
        assert_eq!(
            window.update(cx, |view, _, _| view.motions.len()).unwrap(),
            4
        );
        cx.executor().advance_clock(Duration::from_secs(3));
        draw(cx, window);
        assert_eq!(
            window.update(cx, |view, _, _| view.motions.len()).unwrap(),
            3
        );
        assert!(
            ui::VisualTestContext::from_window(window.into(), cx)
                .debug_bounds("collection-notice")
                .is_none()
        );
        // Rendering again must not recreate the already-cleaned outgoing entry.
        window.update(cx, |_, _, cx| cx.notify()).unwrap();
        draw(cx, window);
        assert_eq!(
            window.update(cx, |view, _, _| view.motions.len()).unwrap(),
            3
        );
    }

    #[ui::test]
    fn consecutive_native_pinches_keep_the_previous_zoom(cx: &mut ui::TestAppContext) {
        crate::init_test(cx);
        let window = cx.open_window(ui::size(px(960.), px(640.)), |_, _| {
            let mut lab = Lab::new();

            lab.select_scene(4);

            lab
        });

        cx.update_window(window.into(), |_, w, _| w.activate_window())
            .unwrap();
        cx.run_until_parked();
        draw(cx, window);

        let position = ui::VisualTestContext::from_window(window.into(), cx)
            .debug_bounds("draggable")
            .unwrap()
            .center();

        for (phase, delta, expected) in [
            (ui::TouchPhase::Started, 0., 1.),
            (ui::TouchPhase::Moved, 0.25, 1.25),
            (ui::TouchPhase::Ended, 0., 1.25),
            (ui::TouchPhase::Started, 0., 1.25),
            (ui::TouchPhase::Moved, 0.2, 1.5),
            (ui::TouchPhase::Ended, 0., 1.5),
        ] {
            cx.update_window(window.into(), |_, w, cx| {
                w.dispatch_event(
                    ui::PlatformInput::Pinch(ui::PinchEvent {
                        position,
                        phase,
                        delta,
                        modifiers: ui::Modifiers::default(),
                    }),
                    cx,
                );
            })
            .unwrap();
            draw(cx, window);

            let zoom = window.update(cx, |lab, _, _| lab.zoom.get()).unwrap();

            assert!((zoom - expected).abs() < 1e-5);
        }
    }

    fn draw(cx: &mut ui::TestAppContext, window: ui::WindowHandle<Lab>) {
        cx.update_window(window.into(), |_, window, cx| {
            window.simulate_next_frame(cx);
            window.draw(cx).clear(cx);
        })
        .unwrap();
    }
}
