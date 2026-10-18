//! Interactive stock-Kit helper gallery: `--features demo --example helpers`.
use gpui_kit::TestSupportExt as _;
use gpui_kit::base::StyledExt as _;

use gpui_kit::{
    AnyElement, App, AppContext, Bounds, Context, FocusHandle, IntoElement, KeyBinding, Render,
    Role, ScrollHandle, Window, WindowBounds, WindowOptions, actions,
    assets::Assets,
    component::{
        ActiveTheme,
        button::{Button, ButtonVariants},
        checkbox::Checkbox,
        tab::{Tab, TabBar},
    },
    div, point,
    prelude::*,
    px, size,
};

use gpui_motion_kit::core::{
    Animation, Color, ColorProperty, Drag, Easing, GestureTargets, Keyframe, Motion, Spring,
    StyleMotion, StyleTarget, SvgMotion, SvgTarget, Target, Transition,
};

use gpui_motion_kit::{
    AnimationState, AutoSize, Axis, GestureState, MotionState, PopLayout, ReorderGroup,
    ScrollMotion, SharedLayout, StyleMotionState, SvgMotionState, Timeline,
};

use std::time::Duration;

actions!(
    kit_helpers,
    [
        /// Close the helper gallery.
        Quit
    ]
);

const SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path fill="black" d="M2 10h12V4l8 8-8 8v-6H2z"/></svg>"#;

const SCENES: [&str; 8] = [
    "Styles", "Input", "Scroll", "Content", "Lists", "Shared", "Playback", "SVG",
];

fn spring() -> Transition {
    Transition::Spring(Spring::visual(Duration::from_millis(260), 0.06).expect("valid spring"))
}

fn surface(cx: &App) -> gpui_kit::Div {
    div()
        .bg(cx.theme().background)
        .border_1()
        .border_color(cx.theme().border)
        .rounded_xl()
        .p_6()
}

struct Gallery {
    scene: usize,
    expanded: bool,
    removed: bool,
    styles: StyleMotionState,
    gesture: GestureState,
    visibility: GestureState,
    scroll: ScrollHandle,
    scroll_spring: bool,
    pop: PopLayout,
    rows: [MotionState; 3],
    reorder: ReorderGroup<usize>,
    shared: SharedLayout<usize>,
    player: AnimationState,
    clip: Animation,
    timeline: Timeline,
    svg: SvgMotionState,
    focus: FocusHandle,
}

impl Gallery {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let clip = Animation::keyframes(
            Duration::from_millis(1000),
            [
                Keyframe::current(0.),
                Keyframe::new(0.5, Target::default().x(260.)).easing(Easing::EaseInOut),
                Keyframe::new(1., Target::default().x(0.)).easing(Easing::EaseInOut),
            ],
        )
        .expect("valid keyframes");

        let mut timeline = Timeline::default();

        for index in 0..3 {
            timeline.at(Duration::from_millis(index * 100), clip.clone());
        }

        let focus = cx.focus_handle();

        window.focus(&focus, cx);

        Self {
            scene: 0,
            expanded: false,
            removed: false,
            styles: StyleMotionState::default(),
            gesture: GestureState::default()
                .with_drag(Drag::new([0., 0.], [0., 0.], [280., 100.]).snap_grid(20.)),
            visibility: GestureState::default().in_view(0.5, false),
            scroll: ScrollHandle::new(),
            scroll_spring: true,
            pop: PopLayout::default(),
            rows: std::array::from_fn(|_| MotionState::default()),
            reorder: ReorderGroup::new(vec![1, 2, 3], Axis::Y).expect("unique row keys"),
            shared: SharedLayout::default(),
            player: AnimationState::default(),
            clip,
            timeline,
            svg: SvgMotionState::default(),
            focus,
        }
    }

    fn stage(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        match self.scene {
            0 => {
                let accent = if self.expanded { 0x0e7490 } else { 0x4338ca };

                let target = StyleTarget::default()
                    .padding([if self.expanded { 32. } else { 16. }; 4])
                    .radii([if self.expanded { 28. } else { 12. }; 4])
                    .border_widths([if self.expanded { 3. } else { 1. }; 4])
                    .color(ColorProperty::Background, Color::rgb(accent))
                    .color(ColorProperty::Border, Color::rgb(0x818cf8))
                    .shadow(
                        [0., 8.],
                        if self.expanded { 24. } else { 4. },
                        0.,
                        Color::rgba(0., 0., 0., 0.18),
                    );

                let frame =
                    self.styles
                        .frame(StyleMotion::new(target).transition(spring()), window, cx);

                frame.apply(div().id("styled-card").test_support().w(px(340.)).text_color(gpui_kit::rgb(0xffffff))
                    .flex().flex_col().gap_3()
                    .child(div().text_xl().font_semibold().child("Project settings"))
                    .child("Colors, spacing, corners and shadow share one retained style sampler."))
                    .into_any_element()
            }
            1 => surface(cx)
                .w(px(440.))
                .h(px(260.))
                .child(
                    self.gesture.bind(
                        "drag-motion",
                        div()
                            .id("drag-card")
                            .test_support()
                            .role(Role::Button)
                            .aria_label("Drag card; arrow keys move it")
                            .w(px(110.))
                            .h(px(72.))
                            .rounded_lg()
                            .p_3()
                            .bg(cx.theme().primary)
                            .text_color(cx.theme().primary_foreground)
                            .child("Drag me"),
                        Motion::new(Target::default()).transition(spring()),
                        GestureTargets::default()
                            .while_hover(Target::default().opacity(0.85))
                            .while_press(Target::default().opacity(0.65))
                            .while_focus(Target::default().opacity(0.9)),
                    ),
                )
                .into_any_element(),
            2 => div()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .w(px(420.))
                        .h(px(8.))
                        .rounded_full()
                        .bg(cx.theme().muted)
                        .child(
                            ScrollMotion::new(
                                "scroll-fill",
                                self.scroll.clone(),
                                Axis::Y,
                                Target::default().width(0.),
                                Target::default().width(420.),
                                div()
                                    .id("scroll-progress")
                                    .test_support()
                                    .h(px(8.))
                                    .rounded_full()
                                    .bg(cx.theme().primary),
                            )
                            .transition(if self.scroll_spring {
                                spring()
                            } else {
                                Transition::Instant
                            }),
                        ),
                )
                .child(
                    div()
                        .id("manual-scroll")
                        .test_support()
                        .w(px(420.))
                        .h(px(240.))
                        .overflow_y_scroll()
                        .track_scroll(&self.scroll)
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_3()
                                .pb_4()
                                .child(
                                    surface(cx)
                                        .h(px(170.))
                                        .child("Scroll down to reveal the card."),
                                )
                                .child(
                                    self.visibility.bind(
                                        "visible-motion",
                                        div()
                                            .id("visible-card")
                                            .test_support()
                                            .w_full()
                                            .h(px(120.))
                                            .p_6()
                                            .rounded_xl()
                                            .bg(cx.theme().primary)
                                            .text_color(cx.theme().primary_foreground)
                                            .child("Now in view"),
                                        Motion::new(Target::default().opacity(0.2))
                                            .transition(spring()),
                                        GestureTargets::default()
                                            .while_in_view(Target::default().opacity(1.)),
                                    ),
                                )
                                .child(
                                    surface(cx)
                                        .h(px(200.))
                                        .child("The indicator follows measured scroll extents."),
                                ),
                        ),
                )
                .into_any_element(),
            3 => {
                let content = surface(cx)
                    .w(px(340.))
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(div().font_semibold().child("Release details"))
                    .child("Content height is measured, not guessed.")
                    .when(self.expanded, |content| {
                        content
                            .child("Native builds are ready.")
                            .child("Interaction tests passed.")
                            .child("Documentation is up to date.")
                            .child("Ready for review.")
                    });

                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        AutoSize::new("content-size", content)
                            .width(340.)
                            .transition(spring()),
                    )
                    .child(
                        div()
                            .id("content-sibling")
                            .test_support()
                            .child("Following content moves with the measured height."),
                    )
                    .into_any_element()
            }
            4 => {
                let mut popped = div().flex().flex_col().gap_2().w(px(280.));

                for id in 0..3usize {
                    let present = id != 0 || !self.removed;

                    let frame = self.rows[id].frame(
                        Motion::new(Target::default())
                            .exit(Target::default().opacity(0.))
                            .present(present)
                            .transition(spring()),
                        window,
                        cx,
                    );

                    let row = frame.render(|frame| {
                        frame.apply(
                            surface(cx)
                                .id(("pop-card", id))
                                .test_support()
                                .w(px(280.))
                                .h(px(64.))
                                .p_4()
                                .child(format!("Notification {}", id + 1)),
                        )
                    });

                    popped = popped.child(
                        self.pop
                            .item(("pop-row", id), present, row)
                            .transition(spring()),
                    );
                }

                let order = self.reorder.order().to_vec();

                let reordered =
                    div()
                        .w(px(280.))
                        .flex()
                        .flex_col()
                        .gap_2()
                        .children(order.into_iter().map(|id| {
                            self.reorder
                                .item(
                                    ("sort-motion", id),
                                    id,
                                    surface(cx)
                                        .id(("sort-row", id))
                                        .test_support()
                                        .role(Role::ListItem)
                                        .aria_label(format!(
                                            "Task {id}; drag or use Option arrow keys to reorder"
                                        ))
                                        .w(px(280.))
                                        .h(px(52. + id as f32 * 6.))
                                        .p_4()
                                        .child(format!("Task {id}  ·  drag to reorder")),
                                )
                                .transition(spring())
                        }));

                div()
                    .flex()
                    .gap_8()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child("Retained exits")
                            .child(self.pop.container("popped-list", popped)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child("Measured reorder · Option ↑/↓")
                            .child(reordered),
                    )
                    .into_any_element()
            }
            5 => {
                let first = surface(cx)
                    .id("shared-old")
                    .test_support()
                    .absolute()
                    .left(px(0.))
                    .top(px(32.))
                    .w(px(140.))
                    .h(px(100.))
                    .bg(cx.theme().primary);

                let second = surface(cx)
                    .id("shared-new")
                    .test_support()
                    .absolute()
                    .left(px(360.))
                    .top(px(100.))
                    .w(px(200.))
                    .h(px(120.))
                    .bg(cx.theme().secondary);

                div()
                    .id("shared-overlay")
                    .relative()
                    .w(px(600.))
                    .h(px(260.))
                    .role(Role::Image)
                    .aria_label("Selection moves between list and detail positions")
                    .child(
                        self.shared
                            .member(1, "shared-source", !self.expanded, first)
                            .transition(spring()),
                    )
                    .child(
                        self.shared
                            .member(1, "shared-destination", self.expanded, second)
                            .transition(spring()),
                    )
                    .into_any_element()
            }
            6 => {
                let frame = self.player.frame(&self.clip, window, cx);

                let player = frame.apply(
                    div()
                        .id("player-card")
                        .test_support()
                        .w(px(120.))
                        .h(px(40.))
                        .rounded_lg()
                        .bg(cx.theme().primary)
                        .text_color(cx.theme().primary_foreground)
                        .p_2()
                        .child("Prepared clip"),
                );

                let frames = self.timeline.frame(window, cx);

                div()
                    .flex()
                    .flex_col()
                    .gap_6()
                    .w(px(440.))
                    .child(player)
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child("Three tracks, one timeline"),
                    )
                    .children(frames.iter().copied().enumerate().map(|(id, frame)| {
                        frame.apply(
                            div()
                                .id(("timeline-track", id))
                                .test_support()
                                .w(px(120.))
                                .h(px(32.))
                                .rounded_lg()
                                .bg(cx.theme().secondary)
                                .p_2()
                                .child(format!("Track {}", id + 1)),
                        )
                    }))
                    .into_any_element()
            }
            7 => {
                let target = SvgTarget::default()
                    .scale(if self.expanded { 1.6 } else { 0.8 })
                    .rotate_degrees(if self.expanded { 270. } else { -45. });

                let frame = self
                    .svg
                    .frame(SvgMotion::new(target).transition(spring()), window, cx);

                surface(cx)
                    .id("svg-stage")
                    .test_support()
                    .w(px(420.))
                    .h(px(260.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        frame.apply(
                            gpui_kit::svg()
                                .data(SVG)
                                .size(px(80.))
                                .text_color(cx.theme().primary),
                        ),
                    )
                    .into_any_element()
            }
            _ => unreachable!(),
        }
    }
}

impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let stage = self.stage(window, cx);

        div().id("helpers-gallery").key_context("KitHelpers").track_focus(&self.focus)
            .on_action(|_: &Quit, _, cx| cx.quit())
            .size_full().flex().flex_col().gap_6().p_8().bg(cx.theme().muted).text_color(cx.theme().foreground)
            .child(div().flex().justify_between().items_center()
                .child(div().text_2xl().font_semibold().child("Motion / Kit helpers"))
                .child(Checkbox::new("reduced-motion").label("Reduced motion").checked(cx.reduce_motion())
                    .on_change(cx.listener(|_, checked, _, cx| {
                        cx.set_reduce_motion(*checked);
                        cx.notify();
                    }))))
            .child(TabBar::new("helper-scenes").pill().selected_index(self.scene)
                .children(SCENES.iter().map(|title| Tab::new().label(*title)))
                .on_click(cx.listener(|this, scene: &usize, _, cx| {
                    this.scene = *scene;
                    cx.notify();
                })))
            .child(div().flex().gap_3()
                .child(Button::new("change-state").label("Change state").on_click(cx.listener(|this, _, _, cx| {
                    this.expanded = !this.expanded;
                    cx.notify();
                })))
                .when(self.scene == 2, |row| row.child(Button::new("scroll-smoothing").ghost().label(if self.scroll_spring { "Smoothing: spring" } else { "Smoothing: instant" })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.scroll_spring = !this.scroll_spring;
                        cx.notify();
                    }))))
                .when(self.scene == 4, |row| row.child(Button::new("remove-row").ghost().label(if self.removed { "Restore first" } else { "Remove first" })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.removed = !this.removed;
                        cx.notify();
                    }))))
                .when(self.scene == 6, |row| row
                    .child(Button::new("replay").ghost().label("Replay").on_click(cx.listener(|this, _, _, cx| {
                        let now = this.player.now(cx);
                        let timeline_now = this.timeline.now(cx);

                        this.player.restart(now);
                        this.timeline.restart(timeline_now);
                        cx.notify();
                    })))
                    .child(Button::new("pause").ghost().label("Pause").on_click(cx.listener(|this, _, _, cx| {
                        let now = this.player.now(cx);
                        let timeline_now = this.timeline.now(cx);

                        this.player.pause(now);
                        this.timeline.pause(timeline_now);
                        cx.notify();
                    })))
                    .child(Button::new("play").ghost().label("Resume").on_click(cx.listener(|this, _, _, cx| {
                        let now = this.player.now(cx);
                        let timeline_now = this.timeline.now(cx);

                        this.player.play(now);
                        this.timeline.play(timeline_now);
                        cx.notify();
                    })))
                    .child(Button::new("seek").ghost().label("Scrub to 25%").on_click(cx.listener(|this, _, _, cx| {
                        let now = this.player.now(cx);
                        let timeline_now = this.timeline.now(cx);

                        this.player.pause(now);
                        this.player.seek(Duration::from_millis(250), now);
                        this.timeline.pause(timeline_now);
                        this.timeline.seek(Duration::from_millis(250), timeline_now);
                        cx.notify();
                    }))))
            )
            .child(div().id("playground").flex_1().min_h_0().pt_4().child(stage))
            .child(div().text_sm().text_color(cx.theme().muted_foreground).child(match self.scene {
                0 => "Click Change state repeatedly to interrupt colors, spacing and shadows.",
                1 => "Drag the card, release outside, grab it mid-flight, or use arrow keys. Escape cancels.",
                2 => "Scroll the panel yourself. Progress follows real extents; the card observes its clipped viewport.",
                3 => "The content is measured at a fixed available width. Rapid toggles preserve the displayed height.",
                4 => "Remove and restore during the exit. Drag variable-height tasks, or use Option/Alt + arrows.",
                5 => "Both appearances share one key in a common overlay. Position and opacity animate; sizes change immediately.",
                6 => "Prepared clips and staggered tracks share the engine. Pause, seek, resume and replay use Kit's executor clock.",
                7 => "Only the SVG drawing transforms. Its padded parent remains the fixed interactive/layout reservation.",
                _ => unreachable!(),
            }))
    }
}

fn main() {
    gpui_kit::application().with_assets(Assets).run(|cx| {
        gpui_kit::init(cx);
        cx.bind_keys([
            KeyBinding::new("cmd-q", Quit, Some("KitHelpers")),
            KeyBinding::new("ctrl-q", Quit, Some("KitHelpers")),
        ]);

        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(80.), px(80.)),
                    size(px(1020.), px(760.)),
                ))),
                titlebar: Some(gpui_kit::TitlebarOptions {
                    title: Some("Motion / Kit helpers".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| Gallery::new(window, cx)),
        )
        .expect("open helper gallery");

        cx.activate(true);
    });
}

#[cfg(all(test, feature = "native-tests"))]
#[path = "helpers/tests.rs"]
mod tests;
