//! SVG-only native scale/rotation. Run `cargo run --features demo --example svg_motion`.
//! R rotate/reverse; S scale; F full turn; X mirror; T tween; M reduce; 0 reset.
mod common;

use gpui::{
    App, Bounds, Context, Div, FocusHandle, KeyBinding, Menu, MenuItem, Role, Stateful,
    TitlebarOptions, Window, WindowBounds, WindowOptions, actions, div, prelude::*, px, rgb, size,
    svg,
};
use gpui_ce_motion::{Easing, SvgMotion, SvgMotionState, SvgTarget, Transition};
use std::time::Duration;
actions!(
    svg_demo,
    [
        /// Reverse the rotation destination.
        Rotate,
        /// Toggle the scale destination.
        Scale,
        /// Add one full turn to the destination.
        FullTurn,
        /// Mirror the SVG horizontally.
        Mirror,
        /// Switch spring/tween timing.
        Timing,
        /// Toggle reduced motion.
        Reduced,
        /// Return to the identity destination.
        Reset,
        /// Advance keyboard focus.
        NextFocus,
        /// Reverse keyboard focus.
        PreviousFocus,
        /// Close the window.
        Close,
        /// Quit the demo.
        Quit,
    ]
);
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
struct Demo {
    focus: FocusHandle,
    state: SvgMotionState,
    angle: f64,
    large: bool,
    mirrored: bool,
    tween: bool,
}
impl Render for Demo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let scale = if self.large { 1.5 } else { 1. };
        let target = SvgTarget::default()
            .scale_xy(if self.mirrored { -scale } else { scale }, scale)
            .rotate_degrees(self.angle);
        let transition = if self.tween {
            Transition::Tween(Duration::from_millis(800), Easing::EaseInOut)
        } else {
            Transition::default()
        };
        let frame = self
            .state
            .frame(SvgMotion::new(target).transition(transition), window, cx);
        let [sx, sy] = frame.target().scale_factors();
        let status = format!(
            "{} · scale {:.2}, {:.2} · angle {:.1}° · {} · reduced {}",
            if frame.is_animating() {
                "Moving"
            } else {
                "Settled"
            },
            sx,
            sy,
            frame.target().rotation_degrees(),
            if self.tween { "800 ms tween" } else { "Spring" },
            cx.reduce_motion()
        );
        let controls = div()
            .flex()
            .gap_3()
            .child(
                button("rotate", "Reverse")
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(Rotate), cx)),
            )
            .child(
                button("scale", "Scale")
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(Scale), cx)),
            )
            .child(
                button("turn", "Full turn")
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(FullTurn), cx)),
            )
            .child(
                button("mirror", "Mirror")
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(Mirror), cx)),
            )
            .child(
                button("reset", "Reset")
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(Reset), cx)),
            );
        let stage = div()
            .id("svg-stage")
            .role(Role::Button)
            .aria_label("Rotate SVG in fixed interaction area")
            .tab_index(0)
            .w_full()
            .h(px(320.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .rounded_lg()
            .border_1()
            .border_color(rgb(0x414757))
            .bg(rgb(0x222936))
            .focus_visible(|s| s.border_color(rgb(0xb8c8ff)))
            .on_click(|_, window, cx| window.dispatch_action(Box::new(Rotate), cx))
            .child(
                div()
                    .size(px(128.))
                    .border_1()
                    .border_color(rgb(0x596275))
                    .child(
                        frame.apply(
                            svg()
                                .path("motion.svg")
                                .size(px(128.))
                                .text_color(rgb(0xb8c8ff)),
                        ),
                    ),
            );

        div().id("svg-demo").track_focus(&self.focus).tab_group()
            .size_full().flex().flex_col().gap_4().p_6()
            .bg(rgb(0x171b24)).text_color(rgb(0xe9efff))
            .on_action(cx.listener(|this, _: &Rotate, _, cx| {
                this.angle = if this.angle == 0. { 60. } else { -this.angle };
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Scale, _, cx| {
                this.large = !this.large;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &FullTurn, _, cx| {
                this.angle += 360.;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Mirror, _, cx| {
                this.mirrored = !this.mirrored;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Timing, _, cx| {
                this.tween = !this.tween;
                cx.notify();
            }))
            .on_action(cx.listener(|_, _: &Reduced, _, cx| {
                let reduced = !cx.reduce_motion();
                cx.set_reduce_motion(reduced);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Reset, _, cx| {
                this.angle = 0.; this.large = false; this.mirrored = false;
                cx.notify();
            }))
            .on_action(|_: &NextFocus, window, cx| window.focus_next(cx))
            .on_action(|_: &PreviousFocus, window, cx| window.focus_prev(cx))
            .on_action(|_: &Close, window, _| window.remove_window())
            .child(div().text_2xl().child("SVG motion"))
            .child("R reverse · S scale · F full turn · X mirror · T timing · M reduce · 0 reset")
            .child(controls)
            .child(div().id("svg-status").role(Role::Label).aria_label(status.clone())
                .text_sm().text_color(rgb(0xaab4c7)).child(status))
            .child(stage)
            .child("The SVG shapes and SVG text transform together. This native text stays still.")
            .child(div().text_sm().text_color(rgb(0xaab4c7))
                .child("The outline shows the original layout. Click the fixed stage, or Tab to it and press Enter / Space."))
            .child(div().text_sm().text_color(rgb(0xaab4c7))
                .child("Interrupt repeatedly: springs preserve momentum; tweens preserve position."))
    }
}
fn main() {
    let assets = common::SvgAsset(include_bytes!("assets/motion.svg"));
    gpui_platform::application()
        .with_assets(assets)
        .run(|cx: &mut App| {
            let modifier = if cfg!(target_os = "macos") {
                "cmd"
            } else {
                "ctrl"
            };
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.bind_keys([
                KeyBinding::new("r", Rotate, None),
                KeyBinding::new("s", Scale, None),
                KeyBinding::new("f", FullTurn, None),
                KeyBinding::new("x", Mirror, None),
                KeyBinding::new("t", Timing, None),
                KeyBinding::new("m", Reduced, None),
                KeyBinding::new("0", Reset, None),
                KeyBinding::new("tab", NextFocus, None),
                KeyBinding::new("shift-tab", PreviousFocus, None),
                KeyBinding::new(&format!("{modifier}-w"), Close, None),
                KeyBinding::new(&format!("{modifier}-q"), Quit, None),
            ]);
            cx.set_menus(vec![
                Menu::new("SVG motion").items([MenuItem::action("Quit", Quit)]),
                Menu::new("File").items([MenuItem::action("Close Window", Close)]),
                Menu::new("Motion").items([
                    MenuItem::action("Reverse", Rotate),
                    MenuItem::action("Scale", Scale),
                    MenuItem::action("Full Turn", FullTurn),
                    MenuItem::action("Mirror", Mirror),
                    MenuItem::action("Switch Spring / Tween", Timing),
                    MenuItem::action("Reduced Motion", Reduced),
                    MenuItem::action("Reset", Reset),
                ]),
            ]);
            cx.on_window_closed(|cx, _| cx.quit()).detach();
            let bounds = Bounds::centered(None, size(px(820.), px(740.)), cx);
            let handle = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        window_min_size: Some(size(px(820.), px(740.))),
                        titlebar: Some(TitlebarOptions {
                            title: Some("SVG motion".into()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    |window, cx| {
                        cx.new(|cx| {
                            let focus = cx.focus_handle();
                            focus.focus(window, cx);
                            Demo {
                                focus,
                                state: SvgMotionState::default(),
                                angle: 0.,
                                large: false,
                                mirrored: false,
                                tween: false,
                            }
                        })
                    },
                )
                .expect("open SVG demo");
            handle
                .update(cx, |_, window, _| window.activate_window())
                .expect("SVG demo remains open");
            cx.activate(true);
        });
}
