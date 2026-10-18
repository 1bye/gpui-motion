//! Native Motion-like feature gallery. Run `cargo run --features demo --example motion_features`.
//! Space pause; R restart; S seek; V variant; P keyed replacement; I reorder;
//! M reduced motion; arrows move the focused drag card; Escape cancels dragging.
use gpui::{
    App, Bounds, Context, Div, FocusHandle, MouseButton, Role, ScrollHandle, SharedString,
    Stateful, Subscription, TitlebarOptions, Window, WindowBounds, WindowOptions, div, prelude::*,
    px, rgb, size,
};
use gpui_ce_motion::{
    Animation, AnimationState, Color, ColorState, Drag, Easing, GestureTargets, InView,
    Interaction, Keyframe, Motion, MotionState, PresenceGroup, PresenceMode, Repeat, RepeatMode,
    StaggerFrom, Target, Timeline, Transition, Variants, move_item, scroll_progress, stagger,
};
use std::time::{Duration, Instant};
gpui::actions!(
    motion_features,
    [
        /// Quit the gallery.
        Quit,
        /// Close the gallery window.
        Close,
    ]
);

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
fn button(id: &'static str, label: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .tab_index(0)
        .px_3()
        .py_2()
        .rounded_md()
        .border_1()
        .border_color(rgb(0x414757))
        .bg(rgb(0x272d39))
        .focus_visible(|s| s.border_color(rgb(0xb8c8ff)))
        .child(label)
}
fn label(text: impl Into<SharedString>) -> Div {
    div().text_sm().text_color(rgb(0xaab4c7)).child(text.into())
}
fn status(id: &'static str, text: impl Into<SharedString>) -> Stateful<Div> {
    let text = text.into();

    div()
        .id(id)
        .role(Role::Label)
        .aria_label(text.clone())
        .child(label(text))
}
#[derive(Clone, Copy, PartialEq)]
enum Variant {
    Compact,
    Wide,
}
struct Gallery {
    started: Instant,
    focus: FocusHandle,
    drag_focus: FocusHandle,
    gesture_focus: FocusHandle,
    timeline: Timeline,
    paused: bool,
    variant: Variant,
    variants: Variants<Variant>,
    interaction: Interaction,
    gesture: MotionState,
    color: ColorState,
    drag: Drag,
    scroll: ScrollHandle,
    view: InView,
    view_motion: MotionState,
    group: PresenceGroup<usize>,
    children: [MotionState; 2],
    replacement: usize,
    order: [usize; 3],
    reorder: [MotionState; 3],
    clip: Animation,
    clip_state: AnimationState,
    _activation: Subscription,
}
impl Gallery {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();

        focus.focus(window, cx);
        let mut timeline = Timeline::default();

        for i in 0..3 {
            timeline.at(
                stagger(ms(160), i, 3, StaggerFrom::Start).unwrap(),
                Animation::keyframes(
                    ms(1600),
                    [
                        Keyframe::new(
                            0.,
                            Target::default().width(70.).height(40.).border_radius(8.),
                        ),
                        Keyframe::new(
                            0.4,
                            Target::default().width(160.).height(60.).border_radius(24.),
                        )
                        .easing(Easing::EaseOut),
                        Keyframe::new(
                            1.,
                            Target::default().width(70.).height(40.).border_radius(8.),
                        )
                        .easing(Easing::EaseInOut),
                    ],
                )
                .unwrap()
                .repeat(Repeat::Forever, RepeatMode::Mirror, ms(250))
                .unwrap(),
            );
        }

        timeline.restart(Duration::ZERO);
        let mut group = PresenceGroup::new(PresenceMode::Wait);

        group.update([0]).unwrap();
        let activation = cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() {
                this.drag.cancel(this.started.elapsed());
                this.interaction.pressed = false;
                this.interaction.hovered = false;
                cx.notify();
            }
        });

        Self {
            started: Instant::now(),
            focus,
            drag_focus: cx.focus_handle().tab_stop(true),
            gesture_focus: cx.focus_handle().tab_stop(true),
            timeline,
            paused: false,
            variant: Variant::Compact,
            variants: Variants::new([
                (
                    Variant::Compact,
                    Motion::new(Target::default().width(140.).height(62.).border_radius(12.)),
                ),
                (
                    Variant::Wide,
                    Motion::new(Target::default().width(240.).height(62.).border_radius(24.)),
                ),
            ])
            .unwrap(),
            interaction: Interaction::default(),
            gesture: MotionState::default(),
            color: ColorState::new(Color::rgb(0x4e6397)),
            drag: Drag::new([0., 0.], [0., 0.], [620., 42.]).snap_grid(20.),
            scroll: ScrollHandle::new(),
            view: InView::new(0.5, false),
            view_motion: MotionState::default(),
            group,
            children: std::array::from_fn(|_| MotionState::default()),
            replacement: 0,
            order: [0, 1, 2],
            reorder: std::array::from_fn(|_| MotionState::default()),
            clip: Animation::to(Target::default().x(60.), ms(800), Easing::EaseInOut)
                .repeat(Repeat::Forever, RepeatMode::Reverse, ms(100))
                .unwrap(),
            clip_state: AnimationState::default(),
            _activation: activation,
        }
    }
    fn toggle_pause(&mut self) {
        let now = self.started.elapsed();

        self.paused = !self.paused;

        if self.paused {
            self.timeline.pause(now);
            self.clip_state.pause(now);
        } else {
            self.timeline.play(now);
            self.clip_state.play(now);
        }
    }
    fn restart(&mut self) {
        let now = self.started.elapsed();

        self.timeline.restart(now);
        self.clip_state.restart(now);
        self.paused = false;
    }
    fn variant(&mut self) {
        self.variant = if self.variant == Variant::Compact {
            Variant::Wide
        } else {
            Variant::Compact
        };
    }
    fn replace(&mut self) {
        self.replacement = 1 - self.replacement;
        self.group.update([self.replacement]).unwrap();
    }
    fn reorder(&mut self) {
        move_item(&mut self.order, 0, 2).unwrap();
    }
}
impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = self.started.elapsed();
        let reduced = cx.reduce_motion();

        let frames = {
            let f = self.timeline.sample(now, reduced);

            [f[0], f[1], f[2]]
        };

        let clip = self.clip_state.sample(&self.clip, now, reduced);

        self.interaction.focused = self.gesture_focus.is_focused(window);
        let base = self.variants.get(&self.variant).unwrap();

        let target = GestureTargets::default()
            .while_hover(Target::default().border_radius(28.))
            .while_press(Target::default().y(6.).opacity(0.6))
            .while_focus(Target::default().border_radius(4.))
            .resolve(base.target(), self.interaction);

        // The base declaration supplies policy; gesture targets replace only explicitly named properties.
        let gesture = self.gesture.sample(base.with_target(target), now, reduced);

        let (color, color_active) = self.color.sample(
            Color::rgb(if self.interaction.pressed {
                0xf0b87c
            } else if self.interaction.hovered {
                0x8bafff
            } else {
                0x4e6397
            }),
            Transition::default(),
            now,
            reduced,
        );

        let drag = self.drag.sample(now, reduced);
        let offset = -f64::from(f32::from(self.scroll.offset().y));
        let progress = scroll_progress(offset, 640., 128.);

        self.view
            .update([0., 0., 700., 128.], [0., 320. - offset, 700., 32.]);
        let view = self.view_motion.sample(
            Motion::new(Target::default().opacity(if self.view.is_visible() { 1. } else { 0.35 }))
                .initial(Target::default()),
            now,
            reduced,
        );

        let mut presence = div().flex().gap_3().h(px(48.));
        let mut finished = None;
        let mut presence_active = false;

        for child in self.group.children() {
            let key = *child.key();

            let f = self.children[key].sample(
                Motion::new(Target::default().opacity(1.))
                    .initial(Target::default().opacity(0.).x(20.))
                    .exit(Target::default().opacity(0.).x(-20.))
                    .present(child.is_present()),
                now,
                reduced,
            );

            presence_active |= f.is_animating();

            if !f.is_mounted() {
                finished = Some(key);
            }

            presence = presence.child(f.render(|f| {
                f.apply(
                    div()
                        .id(("replacement", key))
                        .px_4()
                        .py_2()
                        .rounded_md()
                        .bg(rgb(if key == 0 { 0x425987 } else { 0x866245 }))
                        .child(format!("Key {key}")),
                )
            }));
        }

        if let Some(key) = finished {
            self.group.finish_exit(&key);
            window.request_animation_frame();
        }

        let mut order = div().relative().h(px(44.));
        let mut reorder_active = false;

        for id in 0..3 {
            let ix = self.order.iter().position(|v| *v == id).unwrap();

            let f = self.reorder[id].sample(
                Motion::new(Target::default().x(ix as f32 * 110.))
                    .initial(Target::default().x(id as f32 * 110.)),
                now,
                reduced,
            );

            reorder_active |= f.is_animating();
            order = order.child(
                div().absolute().child(
                    f.apply(
                        div()
                            .id(("reorder", id))
                            .w(px(100.))
                            .h(px(36.))
                            .rounded_md()
                            .bg(rgb(0x344456))
                            .p_2()
                            .child(format!("Item {id}")),
                    ),
                ),
            );
        }

        if self.timeline.is_animating()
            || clip.is_animating()
            || color_active
            || gesture.is_animating()
            || drag.is_animating()
            || view.is_animating()
            || presence_active
            || reorder_active
        {
            window.request_animation_frame();
        }

        let move_view = cx.entity().downgrade();
        let up_view = cx.entity().downgrade();

        div()
            .id("gallery")
            .track_focus(&self.focus)
            .tab_group()
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_6()
            .bg(rgb(0x171b24))
            .text_color(rgb(0xe9efff))
            .child(
                gpui::canvas(
                    |_, _, _| (),
                    move |_, _, window, _| {
                        window.on_mouse_event(move |event: &gpui::MouseMoveEvent, phase, _, cx| {
                            if phase.bubble() {
                                let _ = move_view.update(cx, |this, cx| {
                                    if this.drag.is_dragging() {
                                        this.drag.move_to(
                                            [
                                                f64::from(f32::from(event.position.x)),
                                                f64::from(f32::from(event.position.y)),
                                            ],
                                            this.started.elapsed(),
                                        );
                                        cx.notify();
                                    }
                                });
                            }
                        });
                    },
                )
                .absolute(),
            )
            .on_mouse_up_all(move |event, phase, _, _, cx| {
                if phase.bubble() && event.button == MouseButton::Left {
                    let _ = up_view.update(cx, |this, cx| {
                        this.interaction.pressed = false;

                        if this.drag.is_dragging() {
                            this.drag.end(this.started.elapsed());
                        }

                        cx.notify();
                    });
                }
            })
            .on_action(|_: &Close, window, _| window.remove_window())
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                let key = event.keystroke.key.as_str();
                let now = this.started.elapsed();

                match key {
                    "tab" => {
                        this.interaction.pressed = false;

                        if event.keystroke.modifiers.shift {
                            window.focus_prev(cx);
                        } else {
                            window.focus_next(cx);
                        }
                    }
                    "space" if this.gesture_focus.is_focused(window) => {
                        this.interaction.pressed = true
                    }
                    "space"
                        if this.focus.is_focused(window) || this.drag_focus.is_focused(window) =>
                    {
                        this.toggle_pause()
                    }
                    "r" => this.restart(),
                    "s" => {
                        this.timeline.seek(ms(700), now);
                        this.clip_state.seek(ms(400), now);
                    }
                    "v" => this.variant(),
                    "p" => this.replace(),
                    "i" => this.reorder(),
                    "m" => {
                        let reduced = !cx.reduce_motion();

                        cx.set_reduce_motion(reduced);
                    }
                    "escape" => {
                        this.drag.cancel(now);
                        this.interaction.pressed = false;
                    }
                    "left" | "right" | "up" | "down" if this.drag_focus.is_focused(window) => {
                        this.drag.move_by(
                            match key {
                                "left" => [-20., 0.],
                                "right" => [20., 0.],
                                "up" => [0., -20.],
                                _ => [0., 20.],
                            },
                            now,
                        )
                    }
                    "enter" if this.gesture_focus.is_focused(window) => {
                        this.interaction.pressed = true
                    }
                    _ => return,
                }

                cx.notify();
            }))
            .on_key_up(cx.listener(|this, event: &gpui::KeyUpEvent, _, cx| {
                if event.keystroke.key == "enter" || event.keystroke.key == "space" {
                    this.interaction.pressed = false;
                    cx.notify();
                }
            }))
            .child(div().text_2xl().child("Motion, in native Rust"))
            .child(label(
                "Space pause · R restart · S scrub · V variant · P replace · I reorder · M reduce",
            ))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(button("pause", "Pause / resume").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.toggle_pause();
                            cx.notify();
                        },
                    )))
                    .child(
                        button("restart", "Restart").on_click(cx.listener(|this, _, _, cx| {
                            this.restart();
                            cx.notify();
                        })),
                    )
                    .child(
                        button("variant", "Variant").on_click(cx.listener(|this, _, _, cx| {
                            this.variant();
                            cx.notify();
                        })),
                    )
                    .child(button("replace", "Replace key").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.replace();
                            cx.notify();
                        },
                    )))
                    .child(
                        button("reorder", "Reorder").on_click(cx.listener(|this, _, _, cx| {
                            this.reorder();
                            cx.notify();
                        })),
                    ),
            )
            .child(status(
                "gallery-status",
                format!(
                    "{} · reduced motion {} · drag {:.0}, {:.0}",
                    if self.paused { "Paused" } else { "Playing" },
                    reduced,
                    drag.target().x_px(),
                    drag.target().y_px()
                ),
            ))
            .child(label(
                "Keyframes · nonuniform times · staggered timeline · mirrored repeats",
            ))
            .child(
                div()
                    .flex()
                    .gap_4()
                    .h(px(70.))
                    .children(frames.into_iter().enumerate().map(|(i, f)| {
                        div().w(px(180.)).child(
                            f.apply(
                                div()
                                    .id(("keyframe", i))
                                    .bg(rgb(0x455d88))
                                    .p_2()
                                    .child(format!("Track {}", i + 1)),
                            ),
                        )
                    })),
            )
            .child(label(
                "Variants + hover / press / keyboard focus + color springs",
            ))
            .child(
                div().h(px(72.)).child(
                    gesture.apply(
                        div()
                            .id("gesture")
                            .role(Role::Button)
                            .aria_label("Interactive color card")
                            .tab_index(0)
                            .track_focus(&self.gesture_focus)
                            .bg(color.to_rgba())
                            .p_4()
                            .focus_visible(|s| s.border_2().border_color(rgb(0xd9e5ff)))
                            .on_hover(cx.listener(|this, hover, _, cx| {
                                this.interaction.hovered = *hover;
                                cx.notify();
                            }))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.gesture_focus.focus(window, cx);
                                    this.interaction.pressed = true;
                                    cx.notify();
                                }),
                            )
                            .child("Hover / Enter"),
                    ),
                ),
            )
            .child(label(
                "Drag · momentum · bounds · snap grid (Tab to card, then arrow keys; Esc cancels)",
            ))
            .child(
                div()
                    .h(px(88.))
                    .w(px(760.))
                    .bg(rgb(0x222936))
                    .rounded_md()
                    .child(
                        drag.apply(
                            div()
                                .id("drag-card")
                                .role(Role::Button)
                                .aria_label("Drag card")
                                .tab_index(0)
                                .track_focus(&self.drag_focus)
                                .w(px(120.))
                                .h(px(42.))
                                .rounded_md()
                                .bg(rgb(0x796044))
                                .p_2()
                                .focus_visible(|s| s.border_2().border_color(rgb(0xffd9ac)))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(
                                        |this, event: &gpui::MouseDownEvent, window, cx| {
                                            this.drag_focus.focus(window, cx);
                                            this.drag.begin(
                                                [
                                                    f64::from(f32::from(event.position.x)),
                                                    f64::from(f32::from(event.position.y)),
                                                ],
                                                this.started.elapsed(),
                                            );
                                            cx.notify();
                                        },
                                    ),
                                )
                                .child("Drag me"),
                        ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_6()
                    .child(
                        div()
                            .w(px(340.))
                            .child(label("Keyed exits · wait mode (P)"))
                            .child(presence),
                    )
                    .child(
                        div()
                            .child(label("Stable identity · reorder springs (I)"))
                            .child(order),
                    ),
            )
            .child(status(
                "scroll-status",
                format!(
                    "Scroll-linked width {:.0}% · row 10 in view: {}",
                    progress * 100.,
                    self.view.is_visible()
                ),
            ))
            .child(
                div().w_full().h(px(4.)).bg(rgb(0x303849)).child(
                    div()
                        .w(px(progress as f32 * 760.))
                        .h_full()
                        .bg(rgb(0x9bbaff)),
                ),
            )
            .child(
                div()
                    .flex()
                    .gap_4()
                    .child(
                        div()
                            .id("scroll-panel")
                            .role(Role::Group)
                            .aria_label("Scroll to reveal row ten")
                            .w(px(500.))
                            .h(px(128.))
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .on_scroll_wheel(cx.listener(|_, _, _, cx| cx.notify()))
                            .children((0..20).map(|i| {
                                div()
                                    .h(px(32.))
                                    .flex_shrink_0()
                                    .px_3()
                                    .bg(rgb(if i == 10 { 0x456038 } else { 0x222936 }))
                                    .child(format!("Row {i}"))
                            })),
                    )
                    .child(
                        view.apply(
                            div()
                                .w(px(220.))
                                .rounded_md()
                                .bg(rgb(0x456038))
                                .p_3()
                                .child("Row 10 visibility target"),
                        ),
                    ),
            )
            .child(clip.apply(label("Prepared clip · reverse playback")))
    }
}
fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        cx.on_action(|_: &Quit, cx| cx.quit());
        let modifier = if cfg!(target_os = "macos") {
            "cmd"
        } else {
            "ctrl"
        };

        cx.bind_keys([
            gpui::KeyBinding::new(&format!("{modifier}-q"), Quit, None),
            gpui::KeyBinding::new(&format!("{modifier}-w"), Close, None),
        ]);
        cx.set_menus(vec![gpui::Menu {
            name: "Motion".into(),
            disabled: false,
            items: vec![
                gpui::MenuItem::action("Close window", Close),
                gpui::MenuItem::action("Quit", Quit),
            ],
        }]);
        cx.on_window_closed(|cx, _| cx.quit()).detach();
        let bounds = Bounds::centered(None, size(px(880.), px(900.)), cx);

        let handle = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(880.), px(900.))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Native motion features".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| Gallery::new(window, cx)),
            )
            .expect("open motion gallery");

        handle
            .update(cx, |_, window, _| window.activate_window())
            .expect("gallery remains open");
        cx.activate(true);
    });
}
