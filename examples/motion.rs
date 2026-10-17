//! Native motion playground. Run: cargo run --features demo --example motion
//! Use the Motion menu or Space / R / W / T / M while the window is focused.

use gpui::{
    App, Bounds, Context, FocusHandle, KeyBinding, Menu, MenuItem, TitlebarOptions, Window,
    WindowBounds, WindowOptions, actions, div, prelude::*, px, rgb, size,
};
use gpui_ce_motion::{Easing, Motion, MotionState, Target, Transition};
use std::time::Duration;

actions!(
    motion_demo,
    [
        /// Toggle the animated card's presence.
        TogglePresence,
        /// Reverse the card's horizontal destination.
        Reverse,
        /// Toggle the card's width, height, and corner radius.
        Resize,
        /// Switch between spring and duration transitions.
        ToggleTransition,
        /// Toggle the application's reduced-motion setting.
        ToggleReducedMotion,
        /// Close this demo.
        Close,
        /// Quit the demo application.
        Quit,
    ]
);

// A small local palette keeps the example independent of any component framework.
struct Palette;
impl Palette {
    fn background() -> gpui::Rgba {
        rgb(0x18181b)
    }
    fn foreground() -> gpui::Rgba {
        rgb(0xf4f4f5)
    }
    fn muted() -> gpui::Rgba {
        rgb(0xa1a1aa)
    }
    fn card() -> gpui::Rgba {
        rgb(0x303a50)
    }
}

struct Playground {
    card: MotionState,
    focus: FocusHandle,
    present: bool,
    right: bool,
    tween: bool,
    expanded: bool,
}

impl Render for Playground {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let transition = if self.tween {
            Transition::Tween(Duration::from_millis(600), Easing::EaseInOut)
        } else {
            Transition::default()
        };
        let frame = self.card.frame(
            Motion::new(
                Target::default()
                    .x(if self.right { 140.0 } else { 0.0 })
                    .width(if self.expanded { 360.0 } else { 240.0 })
                    .height(if self.expanded { 200.0 } else { 160.0 })
                    .border_radius(if self.expanded { 36.0 } else { 8.0 }),
            )
            .initial(
                Target::default()
                    .width(0.0)
                    .height(0.0)
                    .border_radius(0.0)
                    .opacity(0.0),
            )
            .exit(
                Target::default()
                    .width(0.0)
                    .height(0.0)
                    .border_radius(0.0)
                    .opacity(0.0),
            )
            .transition(transition)
            .present(self.present),
            window,
            cx,
        );
        let status = format!(
            "{:?}  ·  {}  ·  Reduced motion {}",
            frame.presence(),
            if self.tween { "600 ms tween" } else { "Spring" },
            if cx.reduce_motion() { "on" } else { "off" }
        );

        div()
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &TogglePresence, _, cx| {
                this.present = !this.present;
                eprintln!("presence requested: {}", this.present);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Reverse, _, cx| {
                this.right = !this.right;
                eprintln!("right requested: {}", this.right);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Resize, _, cx| {
                this.expanded = !this.expanded;
                eprintln!("expanded requested: {}", this.expanded);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleTransition, _, cx| {
                this.tween = !this.tween;
                eprintln!("tween requested: {}", this.tween);
                cx.notify();
            }))
            .on_action(cx.listener(|_, _: &ToggleReducedMotion, _, cx| {
                let reduced = !cx.reduce_motion();
                cx.set_reduce_motion(reduced);
                eprintln!("reduced motion: {reduced}");
                cx.notify();
            }))
            .on_action(|_: &Close, window, _| window.remove_window())
            .size_full()
            .flex()
            .flex_col()
            .p_8()
            .gap_4()
            .bg(Palette::background())
            .text_color(Palette::foreground())
            .child(div().text_2xl().child("Native motion"))
            .child(
                div()
                    .text_color(Palette::muted())
                    .child("Use the Motion menu, or press these keys:"),
            )
            .child("Space: show/hide    R: move    W: resize/round    T: tween    M: reduce")
            .child(
                div()
                    .id("motion-status")
                    .role(gpui::Role::Label)
                    .aria_label(status.clone())
                    .text_color(Palette::muted())
                    .child(status),
            )
            .child(div().id("layout-status").role(gpui::Role::Label)
                .aria_label(format!("Width {:.0}, height {:.0}, radius {:.0}", frame.target().width_px().unwrap_or(0.0), frame.target().height_px().unwrap_or(0.0), frame.target().border_radius_px().unwrap_or(0.0)))
                .text_sm().text_color(Palette::muted())
                .child(format!("{:.0} × {:.0} px · radius {:.0} px", frame.target().width_px().unwrap_or(0.0), frame.target().height_px().unwrap_or(0.0), frame.target().border_radius_px().unwrap_or(0.0))))
            .child(div().h(px(240.0)).pt_8().child(frame.render(|frame| {
                frame.apply(
                    div().id("animated-card").role(gpui::Role::Group).aria_label("Animated card")
                        .overflow_hidden().flex_shrink_0().bg(Palette::card())
                        .child(div().w(px(240.0)).h(px(160.0)).p_6().flex().flex_col().gap_3()
                            .child("Interrupt me")
                            .child(div().text_sm().child("Press W to morph the card. Press it again before the spring settles."))),
                )
            })))
            .child(div().text_color(Palette::muted()).text_sm().child(
                "The card stays mounted during exit. Animation frames stop when it settles.",
            ))
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        cx.on_action(|_: &Quit, cx| cx.quit());
        let command = if cfg!(target_os = "macos") {
            "cmd"
        } else {
            "ctrl"
        };
        cx.bind_keys([
            KeyBinding::new("space", TogglePresence, None),
            KeyBinding::new("r", Reverse, None),
            KeyBinding::new("w", Resize, None),
            KeyBinding::new("t", ToggleTransition, None),
            KeyBinding::new("m", ToggleReducedMotion, None),
            KeyBinding::new(&format!("{command}-w"), Close, None),
            KeyBinding::new(&format!("{command}-q"), Quit, None),
        ]);
        cx.set_menus(vec![
            Menu::new("Motion demo").items([MenuItem::action("Quit Motion Demo", Quit)]),
            Menu::new("File").items([MenuItem::action("Close Window", Close)]),
            Menu::new("Motion").items([
                MenuItem::action("Show or Hide Card", TogglePresence),
                MenuItem::action("Reverse Destination", Reverse),
                MenuItem::action("Resize and Round Card", Resize),
                MenuItem::action("Switch Spring or Tween", ToggleTransition),
                MenuItem::action("Toggle Reduced Motion", ToggleReducedMotion),
            ]),
        ]);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(800.0), px(640.0)), cx);
        let handle = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(720.0), px(600.0))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Native motion".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| {
                    cx.new(|cx| {
                        let focus = cx.focus_handle();
                        focus.focus(window, cx);
                        Playground {
                            card: MotionState::default(),
                            focus,
                            present: true,
                            right: false,
                            tween: false,
                            expanded: false,
                        }
                    })
                },
            )
            .expect("open motion demo");
        handle
            .update(cx, |_, window, _| window.activate_window())
            .expect("demo is open");
        cx.activate(true);
    });
}
