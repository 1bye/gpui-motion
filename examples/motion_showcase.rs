//! Recording-friendly motion studio. P plays the tour; experiment 05 pauses for exploration.
//! G flips
//! the current target, Left/Right selects scenes, R restarts, M reduces motion.
mod common;

use gpui::{
    App, Bounds, Context, Div, FocusHandle, KeyBinding, Render, Role, Stateful, TitlebarOptions,
    Window, WindowBounds, WindowOptions, actions, div, prelude::*, px, rgb, size,
};
use gpui_ce_motion::*;
use std::time::{Duration, Instant};

// Presentation tokens for this deliberately fixed-aspect recording canvas.
const BACK: u32 = 0x101310;
const PANEL: u32 = 0x1b201b;
const LINE: u32 = 0x343c32;
const TEXT: u32 = 0xf0f0df;
const MUTED: u32 = 0xa1ad98;
const ACCENT: u32 = 0xd4f580;
const CARD: u32 = 0x94a765;
const EXPERIMENTS: usize = 7;
const TOUR_SECONDS: f64 = EXPERIMENTS as f64 * 6.;
const TABS: [&str; 4] = ["Overview", "Activity", "Files", "Notes"];
actions!(
    studio,
    [
        /// Play or pause the tour.
        Play,
        /// Reverse the current target.
        Flip,
        /// Select the next scene.
        Next,
        /// Select the previous scene.
        Previous,
        /// Replay from the first scene.
        Restart,
        /// Toggle reduced motion.
        Reduce,
        /// Focus the next control.
        FocusNext,
        /// Focus the previous control.
        FocusPrevious,
        /// Close the studio.
        Close,
        /// Quit the studio.
        Quit
    ]
);
fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
fn button(id: impl Into<gpui::ElementId>, text: impl Into<gpui::SharedString>) -> Stateful<Div> {
    let text = text.into();
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(text.clone())
        .tab_index(0)
        .px_4()
        .py_2()
        .rounded_lg()
        .border_1()
        .border_color(rgb(LINE))
        .bg(rgb(PANEL))
        .text_color(rgb(TEXT))
        .text_sm()
        .hover(|s| s.bg(rgb(LINE)))
        .focus_visible(|s| s.border_color(rgb(ACCENT)))
        .child(text)
}
fn surface() -> Div {
    div()
        .bg(rgb(PANEL))
        .rounded_xl()
        .border_1()
        .border_color(rgb(LINE))
}
fn curve(high: bool) -> PathShape {
    let amplitude = if high { -70. } else { 60. };
    PathShape::new([
        PathCommand::Move([10., 100.]),
        PathCommand::Cubic(
            [90., 100. + amplitude],
            [170., 100. + amplitude],
            [250., 100.],
        ),
        PathCommand::Cubic(
            [330., 100. - amplitude],
            [410., 100. - amplitude],
            [490., 100.],
        ),
    ])
    .expect("prepared compatible curves")
}
fn tour_position(seconds: f64) -> (usize, bool) {
    let seconds = seconds.clamp(0., TOUR_SECONDS);
    let phase = seconds % 6.;
    (
        (seconds as usize / 6).min(EXPERIMENTS - 1),
        (1.0..2.0).contains(&phase) || (2.3..4.5).contains(&phase),
    )
}
struct Studio {
    focus: FocusHandle,
    epoch: Instant,
    clock: Playback,
    scene: usize,
    rendered_scene: usize,
    stage_motion: MotionState,
    selection_motion: MotionState,
    alternate: bool,
    touring: bool,
    exploring: bool,
    spring: Transition,
    bars: [MotionState; 5],
    presence: [MotionState; 4],
    icon: SvgMotionState,
    style: StyleMotionState,
    pop: PopLayout,
    drag: GestureState,
    reorder: ReorderGroup<usize>,
    shared: SharedLayout<usize>,
    scroll: gpui::ScrollHandle,
    vector: VectorState,
    shapes: [PathShape; 2],
    clip: Animation,
    playback: AnimationState,
    tabs: ReorderGroup<usize>,
    active_tab: usize,
    rendered_tab: usize,
    tab_motion: MotionState,
}
impl Studio {
    fn new(w: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        focus.focus(w, cx);
        let mut clock = Playback::default();
        clock.pause(Duration::ZERO);
        let shapes = [curve(false), curve(true)];
        Self {
            focus,
            epoch: Instant::now(),
            clock,
            scene: 0,
            rendered_scene: 0,
            stage_motion: MotionState::default(),
            selection_motion: MotionState::default(),
            alternate: false,
            touring: false,
            exploring: false,
            spring: Transition::Spring(Spring::visual(ms(320), 0.3).unwrap()),
            bars: std::array::from_fn(|_| MotionState::default()),
            presence: std::array::from_fn(|_| MotionState::default()),
            icon: SvgMotionState::default(),
            style: StyleMotionState::default(),
            pop: PopLayout::default(),
            drag: GestureState::default()
                .with_drag(Drag::new([0., 0.], [0., 0.], [160., 100.]).snap_grid(20.))
                .in_view(0.5, false),
            reorder: ReorderGroup::new(vec![0, 1, 2], Axis::Y).unwrap(),
            shared: SharedLayout::default(),
            scroll: gpui::ScrollHandle::new(),
            vector: VectorState::new(shapes[0].clone()),
            shapes,
            clip: Animation::keyframes(
                ms(1100),
                [
                    Keyframe::new(0., Target::default().x(0.)),
                    Keyframe::new(0.5, Target::default().x(260.)).easing(Easing::EaseInOut),
                    Keyframe::new(1., Target::default().x(0.)).easing(Easing::EaseInOut),
                ],
            )
            .unwrap()
            .repeat(Repeat::Forever, RepeatMode::Loop, ms(120))
            .unwrap(),
            playback: AnimationState::default(),
            tabs: ReorderGroup::new(vec![0, 1, 2, 3], Axis::X)
                .unwrap()
                .on_change({
                    let owner = cx.entity().downgrade();
                    move |_, _, cx| {
                        let _ = owner.update(cx, |s, cx| {
                            s.touring = false;
                            s.clock.pause(s.epoch.elapsed());
                            cx.notify();
                        });
                    }
                }),
            active_tab: 0,
            rendered_tab: 0,
            tab_motion: MotionState::default(),
        }
    }
    fn select(&mut self, scene: usize, w: &mut Window, cx: &mut Context<Self>) {
        self.drag.cancel(cx);
        self.scene = scene;
        if scene == 5 {
            self.playback.restart(self.playback.now());
        }
        self.alternate = false;
        self.touring = false;
        self.exploring = false;
        self.clock.pause(self.epoch.elapsed());
        self.clock.seek(
            Duration::from_secs((scene * 6) as u64),
            self.epoch.elapsed(),
        );
        self.focus.focus(w, cx);
        cx.notify();
    }
    fn play(&mut self, cx: &mut Context<Self>) {
        let now = self.epoch.elapsed();
        if self.scene == 4 {
            if !self.exploring {
                self.explore(cx);
            } else {
                self.exploring = false;
                self.clock.seek(Duration::from_secs(30), now);
                self.clock.play(now);
                self.touring = true;
                cx.notify();
            }
            return;
        }
        self.touring = !self.touring;
        if self.touring {
            if self.clock.time(now).as_secs_f64() >= TOUR_SECONDS {
                self.clock.seek(Duration::ZERO, now);
            }
            self.clock.play(now);
        } else {
            self.clock.pause(now);
        }
        cx.notify();
    }
    fn explore(&mut self, cx: &mut Context<Self>) {
        self.touring = false;
        self.exploring = true;
        self.alternate = true;
        self.clock.pause(self.epoch.elapsed());
        cx.notify();
    }
    fn scroll_by(&mut self, delta: f32, cx: &mut Context<Self>) {
        self.explore(cx);
        let y = (f32::from(self.scroll.offset().y) + delta)
            .clamp(-f32::from(self.scroll.max_offset().y).max(0.), 0.);
        self.scroll.set_offset(gpui::point(px(0.), px(y)));
    }
    fn select_tab(&mut self, key: usize, cx: &mut Context<Self>) {
        self.active_tab = key;
        self.touring = false;
        self.clock.pause(self.epoch.elapsed());
        cx.notify();
    }
    fn stage(&mut self, w: &mut Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        let on = self.alternate;
        match self.scene {
            0 => {
                let icon = self.icon.frame(
                    SvgMotion::new(
                        SvgTarget::default()
                            .scale(if on { 1.5 } else { 1. })
                            .rotate_degrees(if on { 180. } else { 0. }),
                    )
                    .transition(self.spring),
                    w,
                    cx,
                );
                let mut bars = div().flex().items_end().gap_3().h(px(170.));
                for i in 0..5 {
                    let frame = self.bars[i].frame(
                        Motion::new(
                            Target::default()
                                .height(if on {
                                    [112., 160., 90., 140., 64.][i]
                                } else {
                                    40.
                                })
                                .width(48.)
                                .border_radius(if on { 12. } else { 24. }),
                        )
                        .transition(self.spring)
                        .delay(ms(i as u64 * 25)),
                        w,
                        cx,
                    );
                    bars =
                        bars.child(frame.apply(div().bg(rgb(if i == 1 { ACCENT } else { LINE }))));
                }
                div()
                    .flex()
                    .items_center()
                    .gap_16()
                    .child(
                        div()
                            .size(px(120.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                icon.apply(
                                    gpui::svg()
                                        .path("motion.svg")
                                        .size(px(64.))
                                        .text_color(rgb(ACCENT)),
                                ),
                            ),
                    )
                    .child(bars)
                    .into_any_element()
            }
            1 => {
                let style = self.style.frame(
                    StyleMotion::new(
                        StyleTarget::default()
                            .padding([if on { 24. } else { 16. }; 4])
                            .radii(if on { [24., 8., 24., 8.] } else { [12.; 4] })
                            .color(
                                ColorProperty::Background,
                                Color::rgb(if on { ACCENT } else { CARD }),
                            )
                            .color(ColorProperty::Text, Color::rgb(BACK))
                            .shadow(
                                [0., 8.],
                                if on { 24. } else { 0. },
                                0.,
                                Color::rgba(0., 0., 0., 0.25),
                            ),
                    )
                    .transition(self.spring),
                    w,
                    cx,
                );
                let content = div()
                    .w(px(372.))
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(div().text_lg().child("A little more possibility."))
                    .child("One component. Every state.")
                    .when(on, |d| {
                        d.child(
                            div()
                                .h(px(60.))
                                .flex()
                                .items_center()
                                .child("Space for the details that matter."),
                        )
                    });
                // Paint the card around the animated reservation; only its content clips.
                style
                    .apply(
                        div()
                            .debug_selector(|| "expansion-card".into())
                            .w(px(420.))
                            .child(
                                AutoSize::new("expansion", content)
                                    .width(372.)
                                    .transition(self.spring),
                            ),
                    )
                    .into_any_element()
            }
            2 => {
                // Keep the centered container stable while its children reflow.
                let mut row = div().w(px(600.)).flex().gap_4().p_4();
                for i in 0..4 {
                    let present = !on || i != 1;
                    let frame = self.presence[i].frame(
                        Motion::new(Target::default().opacity(1.))
                            .initial(Target::default().opacity(0.))
                            .exit(Target::default().opacity(0.))
                            .present(present)
                            .transition(self.spring),
                        w,
                        cx,
                    );
                    if frame.is_mounted() {
                        row = row.child(
                            self.pop
                                .item(
                                    ("exit", i),
                                    present,
                                    frame.apply(
                                        surface()
                                            .debug_selector(move || format!("exit-card-{i}"))
                                            .w(px(130.))
                                            .h(px(150.))
                                            .p_5()
                                            .flex()
                                            .flex_col()
                                            .justify_between()
                                            .child(
                                                div()
                                                    .text_color(rgb(ACCENT))
                                                    .text_2xl()
                                                    .child(format!("0{}", i + 1)),
                                            )
                                            .child(["Explore", "Refine", "Create", "Ship"][i]),
                                    ),
                                )
                                .transition(self.spring),
                        );
                    }
                }
                self.pop.container("exit-container", row).into_any_element()
            }
            3 => {
                let handle = self.drag.bind(
                    "drag",
                    button("handle", "Move me")
                        .w(px(110.))
                        .bg(rgb(ACCENT))
                        .text_color(rgb(BACK)),
                    Motion::new(Target::default()),
                    GestureTargets::default()
                        .while_hover(Target::default().opacity(0.8))
                        .while_press(Target::default().opacity(0.6)),
                );
                let mut list = div().flex().flex_col().gap_3().w(px(280.));
                for &i in self.reorder.order().iter() {
                    list = list.child(
                        self.reorder
                            .item(
                                ("reorder", i),
                                i,
                                button(
                                    ("track", i),
                                    format!(
                                        "{}   {}",
                                        i + 1,
                                        ["Find the rhythm", "Follow the motion", "Make it yours"]
                                            [i]
                                    ),
                                )
                                .w_full()
                                .h(px(50.)),
                            )
                            .transition(self.spring),
                    );
                }
                div()
                    .flex()
                    .gap_8()
                    .child(surface().w(px(290.)).h(px(200.)).p_4().child(handle))
                    .child(list)
                    .into_any_element()
            }
            4 => {
                let progress = ScrollMotion::new(
                    "progress",
                    self.scroll.clone(),
                    Axis::Y,
                    Target::default().width(0.),
                    Target::default().width(220.),
                    div().h(px(4.)).bg(rgb(ACCENT)),
                )
                .transition(self.spring);
                div()
                    .flex()
                    .gap_8()
                    .child(
                        div()
                            .relative()
                            .w(px(340.))
                            .h(px(210.))
                            .child(
                                self.shared
                                    .member(
                                        0,
                                        "shared-a",
                                        !on,
                                        surface()
                                            .absolute()
                                            .left(px(0.))
                                            .top(px(10.))
                                            .w(px(160.))
                                            .h(px(110.))
                                            .p_5()
                                            .child("Same idea."),
                                    )
                                    .transition(self.spring),
                            )
                            .child(
                                self.shared
                                    .member(
                                        0,
                                        "shared-b",
                                        on,
                                        surface()
                                            .absolute()
                                            .left(px(170.))
                                            .top(px(90.))
                                            .w(px(160.))
                                            .h(px(110.))
                                            .p_5()
                                            .bg(rgb(ACCENT))
                                            .text_color(rgb(BACK))
                                            .child("New perspective."),
                                    )
                                    .transition(self.spring),
                            ),
                    )
                    .child(
                        div()
                            .w(px(240.))
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .text_color(rgb(MUTED))
                                    .text_sm()
                                    .child("SCROLL TO EXPLORE"),
                            )
                            .child(progress)
                            .child(
                                div()
                                    .id("playlist")
                                    .debug_selector(|| "playlist".into())
                                    .h(px(140.))
                                    .overflow_y_scroll()
                                    .track_scroll(&self.scroll)
                                    .on_scroll_wheel(cx.listener(|s, _, _, cx| s.explore(cx)))
                                    .child(div().flex().flex_col().children((0..8).map(|i| {
                                        div()
                                            .h(px(44.))
                                            .flex_shrink_0()
                                            .text_sm()
                                            .child(format!("0{}   A new perspective", i + 1))
                                    }))),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .child(
                                        button("scroll-up", "↑")
                                            .aria_label("Scroll playlist up")
                                            .debug_selector(|| "scroll-up".into())
                                            .on_click(
                                                cx.listener(|s, _, _, cx| s.scroll_by(88., cx)),
                                            ),
                                    )
                                    .child(
                                        button("scroll-down", "↓")
                                            .aria_label("Scroll playlist down")
                                            .debug_selector(|| "scroll-down".into())
                                            .on_click(
                                                cx.listener(|s, _, _, cx| s.scroll_by(-88., cx)),
                                            ),
                                    ),
                            ),
                    )
                    .into_any_element()
            }
            5 => {
                let path = self
                    .vector
                    .element(
                        "curve",
                        self.shapes[usize::from(on)].clone(),
                        [510., 190.],
                        Color::rgb(ACCENT),
                    )
                    .unwrap()
                    .stroke(3.)
                    .transition(self.spring)
                    // Rest on a complete wave; retract only during the active demo beat.
                    .reveal(StrokeReveal::new(if on { 0.35 } else { 1. }, 0., 1.).unwrap());
                let frame = self.playback.frame(&self.clip, w, cx);
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_4()
                    .child(path)
                    .child(
                        div()
                            .debug_selector(|| "vector-track".into())
                            .relative()
                            .w(px(280.))
                            .h(px(8.))
                            .rounded_full()
                            .bg(rgb(LINE))
                            .child(
                                div().absolute().left_0().top(px(-6.)).child(
                                    frame.apply(
                                        div()
                                            .debug_selector(|| "vector-dot".into())
                                            .size(px(20.))
                                            .rounded_full()
                                            .bg(rgb(TEXT)),
                                    ),
                                ),
                            ),
                    )
                    .into_any_element()
            }
            _ => {
                let mut tabs = div()
                    .id("workspace-tabs")
                    .role(Role::TabList)
                    .aria_label("Workspace tabs")
                    .flex()
                    .gap_2();
                for &key in self.tabs.order().iter() {
                    let active = self.active_tab == key;
                    tabs = tabs.child(
                        self.tabs
                            .item(
                                ("workspace-tab", key),
                                key,
                                button(("tab", key), TABS[key])
                                    .role(Role::Tab)
                                    .aria_selected(active)
                                    .debug_selector(move || format!("tab-{key}"))
                                    .w(px(144.))
                                    .h(px(44.))
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .when(active, |d| d.bg(rgb(ACCENT)).text_color(rgb(BACK)))
                                    .on_mouse_down(
                                        gpui::MouseButton::Left,
                                        cx.listener(move |s, _, _, cx| s.select_tab(key, cx)),
                                    )
                                    .on_click(cx.listener(move |s, _, _, cx| s.select_tab(key, cx)))
                                    .on_key_down(cx.listener(
                                        move |s, event: &gpui::KeyDownEvent, _, cx| {
                                            if matches!(
                                                event.keystroke.key.as_str(),
                                                "enter" | "space"
                                            ) {
                                                s.select_tab(key, cx);
                                                cx.stop_propagation();
                                            }
                                        },
                                    )),
                            )
                            .transition(self.spring),
                    );
                }
                let changed = self.rendered_tab != self.active_tab;
                if changed {
                    self.tab_motion = MotionState::default();
                    self.rendered_tab = self.active_tab;
                }
                let frame = self.tab_motion.frame(
                    Motion::new(Target::default())
                        .initial(if changed {
                            Target::default().y(10.).opacity(0.)
                        } else {
                            Target::default()
                        })
                        .transition(self.spring),
                    w,
                    cx,
                );
                let descriptions = [
                    "Everything in its place.",
                    "A little progress, every day.",
                    "Room for your next idea.",
                    "Keep the thoughts worth keeping.",
                ];
                div()
                    .w(px(600.))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(tabs)
                    .child(
                        surface().h(px(140.)).p_5().child(
                            frame.apply(
                                div()
                                    .id("tab-panel")
                                    .role(Role::TabPanel)
                                    .aria_label(TABS[self.active_tab])
                                    .flex()
                                    .flex_col()
                                    .gap_3()
                                    .child(
                                        div()
                                            .text_xl()
                                            .text_color(rgb(ACCENT))
                                            .child(TABS[self.active_tab]),
                                    )
                                    .child(descriptions[self.active_tab])
                                    .child(div().flex().gap_2().children((0..3).map(|i| {
                                        div()
                                            .h(px(5.))
                                            .w(px(40. + ((self.active_tab + i) % 4) as f32 * 25.))
                                            .rounded_full()
                                            .bg(rgb(LINE))
                                    }))),
                            ),
                        ),
                    )
                    .child(div().text_sm().text_color(rgb(MUTED)).child(
                        if cfg!(target_os = "macos") {
                            "Drag tabs to reorder · ⌥ + ← / →"
                        } else {
                            "Drag tabs to reorder · Alt + ← / →"
                        },
                    ))
                    .into_any_element()
            }
        }
    }
}
impl Render for Studio {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = self.epoch.elapsed();
        if (cx.reduce_motion() || !w.is_window_active()) && self.touring {
            self.touring = false;
            self.clock.pause(now);
        }
        let elapsed = self.clock.time(now).as_secs_f64();
        if self.touring {
            let (scene, on) = tour_position(elapsed);
            if scene != self.scene {
                self.drag.cancel(cx);
                self.focus.focus(w, cx);
                if scene == 5 {
                    self.playback.restart(self.playback.now());
                }
            }
            self.scene = scene;
            self.alternate = on;
            if self.scene == 3 {
                let order = if on { vec![2, 0, 1] } else { vec![0, 1, 2] };
                if *self.reorder.order() != order {
                    self.reorder.set_order(order).unwrap();
                }
            }
            if self.scene == 4 {
                // Hand control to the presenter; never overwrite their scroll offset.
                self.explore(cx);
                self.clock.seek(Duration::from_secs(24), now);
            }
            if self.scene == 6 {
                let order = if on {
                    vec![2, 0, 1, 3]
                } else {
                    vec![0, 1, 2, 3]
                };
                if *self.tabs.order() != order {
                    self.tabs.set_order(order).unwrap();
                }
                self.active_tab = if on { 2 } else { 0 };
            }
            if elapsed >= TOUR_SECONDS {
                self.touring = false;
                self.clock.pause(now);
            } else if self.touring {
                w.request_animation_frame();
            }
        }
        let changing_scene = self.rendered_scene != self.scene;
        if changing_scene {
            // Each experiment is a new mount; the selection highlight keeps its momentum.
            self.stage_motion = MotionState::default();
            self.rendered_scene = self.scene;
        }
        let entrance = self.stage_motion.frame(
            Motion::new(Target::default())
                .initial(if changing_scene {
                    Target::default().y(18.).opacity(0.)
                } else {
                    Target::default()
                })
                .transition(self.spring),
            w,
            cx,
        );
        let stage = entrance.apply(div().child(self.stage(w, cx)));
        let selection = self.selection_motion.frame(
            Motion::new(Target::default().x(self.scene as f32 * 72.)).transition(self.spring),
            w,
            cx,
        );
        let mut chapters = div().relative().flex().gap(px(8.)).child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .child(selection.apply(div().w(px(64.)).h(px(44.)).rounded_lg().bg(rgb(LINE)))),
        );
        for i in 0..EXPERIMENTS {
            chapters = chapters.child(
                button(("experiment", i), format!("0{}", i + 1))
                    .aria_label(format!("Experiment {}", i + 1))
                    .w(px(64.))
                    .h(px(44.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(gpui::rgba(0x00000000))
                    .when(self.scene == i, |d| d.text_color(rgb(ACCENT)))
                    .on_click(cx.listener(move |s, _, w, cx| s.select(i, w, cx))),
            );
        }
        div()
            .id("studio")
            .track_focus(&self.focus)
            .tab_group()
            .size_full()
            .bg(rgb(BACK))
            .text_color(rgb(TEXT))
            .font_family(".SystemUIFont")
            .p_6()
            .flex()
            .flex_col()
            .on_action(cx.listener(|s, _: &Play, _, cx| s.play(cx)))
            .on_action(cx.listener(|s, _: &Flip, _, cx| {
                s.touring = false;
                s.clock.pause(s.epoch.elapsed());
                s.alternate = !s.alternate;
                if s.scene == 3 {
                    s.reorder
                        .set_order(if s.alternate {
                            vec![2, 0, 1]
                        } else {
                            vec![0, 1, 2]
                        })
                        .unwrap();
                }
                cx.notify();
            }))
            .on_action(
                cx.listener(|s, _: &Next, w, cx| s.select((s.scene + 1) % EXPERIMENTS, w, cx)),
            )
            .on_action(cx.listener(|s, _: &Previous, w, cx| {
                s.select((s.scene + EXPERIMENTS - 1) % EXPERIMENTS, w, cx)
            }))
            .on_action(cx.listener(|s, _: &Restart, w, cx| {
                s.select(0, w, cx);
                s.clock.seek(Duration::ZERO, s.epoch.elapsed());
                s.play(cx);
            }))
            .on_action(cx.listener(|s, _: &Reduce, _, cx| {
                let reduced = !cx.reduce_motion();
                cx.set_reduce_motion(reduced);
                if !reduced {
                    s.playback.restart(s.playback.now());
                }
                cx.notify();
            }))
            .on_action(|_: &FocusNext, w, cx| w.focus_next(cx))
            .on_action(|_: &FocusPrevious, w, cx| w.focus_prev(cx))
            .on_action(|_: &Close, w, _| w.remove_window())
            .child(
                div()
                    .bg(rgb(PANEL))
                    .rounded_xl()
                    .size_full()
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .p_6()
                            .flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_color(rgb(MUTED))
                                    .text_sm()
                                    .child(format!("EXPERIMENT  /  0{}", self.scene + 1)),
                            )
                            .child(
                                button(
                                    "play",
                                    if self.scene == 4 && self.exploring {
                                        "Continue"
                                    } else if self.touring {
                                        "Pause"
                                    } else {
                                        "Play"
                                    },
                                )
                                .debug_selector(|| "play".into())
                                .bg(rgb(ACCENT))
                                .text_color(rgb(BACK))
                                .on_click(|_, w, cx| w.dispatch_action(Box::new(Play), cx)),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .pb_6()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(stage),
                    )
                    .child(div().pb_6().flex().justify_center().child(chapters)),
            )
    }
}
fn main() {
    let assets = common::SvgAsset(include_bytes!("assets/showcase.svg"));
    gpui_platform::application()
        .with_assets(assets)
        .run(|cx: &mut App| {
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.on_window_closed(|cx, _| cx.quit()).detach();
            let modifier = if cfg!(target_os = "macos") {
                "cmd"
            } else {
                "ctrl"
            };
            cx.bind_keys([
                KeyBinding::new("p", Play, None),
                KeyBinding::new("g", Flip, None),
                KeyBinding::new("right", Next, None),
                KeyBinding::new("left", Previous, None),
                KeyBinding::new("r", Restart, None),
                KeyBinding::new("m", Reduce, None),
                KeyBinding::new("tab", FocusNext, None),
                KeyBinding::new("shift-tab", FocusPrevious, None),
                KeyBinding::new(&format!("{modifier}-w"), Close, None),
                KeyBinding::new(&format!("{modifier}-q"), Quit, None),
            ]);
            let bounds = Bounds::centered(None, size(px(1000.), px(600.)), cx);
            let window = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        window_min_size: Some(size(px(800.), px(460.))),
                        titlebar: Some(TitlebarOptions {
                            title: Some("Motion studio".into()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    |w, cx| cx.new(|cx| Studio::new(w, cx)),
                )
                .expect("open studio");
            window.update(cx, |_, w, _| w.activate_window()).unwrap();
            cx.activate(true);
        });
}
#[cfg(test)]
mod tests {
    // Real tour timing; no native window or clock doubles required.
    #[test]
    fn tour_reverses_then_interrupts_and_finishes_on_last_scene() {
        assert_eq!(super::tour_position(0.), (0, false));
        assert_eq!(super::tour_position(1.5), (0, true));
        assert_eq!(super::tour_position(2.1), (0, false));
        assert_eq!(super::tour_position(2.4), (0, true));
        assert_eq!(super::tour_position(6.), (1, false));
        assert_eq!(super::tour_position(30.), (5, false));
        assert_eq!(super::tour_position(36.), (6, false));
        assert_eq!(super::tour_position(42.), (6, false));
    }

    #[test]
    fn wave_has_matching_tangents_at_the_join_in_both_states() {
        use super::PathCommand;
        for high in [false, true] {
            let shape = super::curve(high);
            let [
                _,
                PathCommand::Cubic(_, incoming, join),
                PathCommand::Cubic(outgoing, _, _),
            ] = shape.commands()
            else {
                panic!("two cubic wave segments")
            };
            for axis in 0..2 {
                assert_eq!(
                    join[axis] - incoming[axis],
                    outgoing[axis] - join[axis],
                    "discontinuous tangent for high={high}, axis={axis}"
                );
            }
        }
    }
}

#[cfg(all(test, feature = "native-tests"))]
#[path = "motion_showcase/tests.rs"]
mod native_tests;
