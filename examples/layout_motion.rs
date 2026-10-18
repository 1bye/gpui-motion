//! Automatic position-only layout animation on stock CE.
//! Run `cargo run --features demo --example layout_motion`: C compact, R reorder, A insert/remove,
//! J alignment, T tween/spring, M reduced motion, B animation on/off.
use gpui::{
    App, Bounds, Context, Div, FocusHandle, KeyBinding, Menu, MenuItem, Role, Stateful,
    TitlebarOptions, Window, WindowBounds, WindowOptions, actions, div, prelude::*, px, rgb, size,
};
use gpui_ce_motion::{Easing, LayoutGroup, Transition};
use std::time::Duration;
actions!(
    layout_demo,
    [
        /// Toggle the board width and resulting line wrapping.
        Compact,
        /// Reverse card order.
        Reorder,
        /// Insert/remove the seventh card.
        Insert,
        /// Toggle row alignment.
        Align,
        /// Switch transition timing.
        Timing,
        /// Toggle reduced motion.
        Reduced,
        /// Enable/disable positional interpolation.
        Enabled,
        /// Move keyboard focus forward.
        Next,
        /// Move keyboard focus backward.
        Previous,
        /// Close the demo window.
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
    group: LayoutGroup,
    root: FocusHandle,
    cards: [FocusHandle; 7],
    compact: bool,
    reverse: bool,
    extra: bool,
    end: bool,
    tween: bool,
    enabled: bool,
    selected: Option<usize>,
    clicks: usize,
}
impl Render for Demo {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let transition = if self.tween {
            Transition::Tween(Duration::from_millis(800), Easing::EaseInOut)
        } else {
            Transition::default()
        };

        let mut board = div()
            .id("board")
            .flex()
            .flex_wrap()
            .content_start()
            .gap_3()
            .p_4()
            .w(px(if self.compact { 340. } else { 500. }))
            .h(px(340.))
            .bg(rgb(0x222936))
            .border_1()
            .border_color(rgb(0x414757))
            .rounded_lg()
            .when(self.end, |board| board.justify_end());

        let count = if self.extra { 7 } else { 6 };

        for ix in 0..count {
            let id = if self.reverse { count - 1 - ix } else { ix };
            let label = format!("Card {}", id + 1);

            let card = div()
                .id(("card", id))
                .role(Role::Button)
                .aria_label(label.clone())
                .tab_index(0)
                .track_focus(&self.cards[id])
                .w(px(128.))
                .h(px(64.))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .rounded_md()
                .border_2()
                .border_color(if self.selected == Some(id) {
                    rgb(0xe2ecff)
                } else {
                    rgb(0x526a97)
                })
                .bg(rgb(if self.selected == Some(id) {
                    0x516d9f
                } else {
                    0x354966
                }))
                .focus_visible(|s| s.border_color(rgb(0xffd4a0)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.selected = Some(id);
                    this.clicks += 1;
                    cx.notify();
                }))
                .child(label);

            board = board.child(
                self.group
                    .position(("position", id), card)
                    .transition(transition)
                    .enabled(self.enabled),
            );
        }

        let controls = div()
            .flex()
            .gap_3()
            .child(
                button("compact", "Compact")
                    .on_click(|_, w, cx| w.dispatch_action(Box::new(Compact), cx)),
            )
            .child(
                button("reorder", "Reorder")
                    .on_click(|_, w, cx| w.dispatch_action(Box::new(Reorder), cx)),
            )
            .child(
                button("insert", "Insert / remove")
                    .on_click(|_, w, cx| w.dispatch_action(Box::new(Insert), cx)),
            )
            .child(
                button("align", "Align")
                    .on_click(|_, w, cx| w.dispatch_action(Box::new(Align), cx)),
            );

        let status = format!(
            "{} cards · {} · {} · reduced {} · {} clicks · selected {}",
            count,
            if self.enabled {
                "Animation on"
            } else {
                "Animation off"
            },
            if self.tween { "800 ms tween" } else { "Spring" },
            cx.reduce_motion(),
            self.clicks,
            self.selected
                .map_or("none".into(), |id| (id + 1).to_string())
        );

        let root = div()
            .id("layout-demo")
            .track_focus(&self.root)
            .tab_group()
            .size_full()
            .flex()
            .flex_col()
            .gap_4()
            .p_6()
            .bg(rgb(0x171b24))
            .text_color(rgb(0xe9efff))
            .on_action(cx.listener(|this, _: &Compact, _, cx| {
                this.compact = !this.compact;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Reorder, _, cx| {
                this.reverse = !this.reverse;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Insert, window, cx| {
                if this.extra && this.cards[6].is_focused(window) {
                    this.root.focus(window, cx);
                }

                this.extra = !this.extra;

                if !this.extra && this.selected == Some(6) {
                    this.selected = None;
                }

                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Align, _, cx| {
                this.end = !this.end;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Timing, _, cx| {
                this.tween = !this.tween;
                cx.notify();
            }))
            .on_action(cx.listener(|_, _: &Reduced, _, cx| {
                let value = !cx.reduce_motion();

                cx.set_reduce_motion(value);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Enabled, _, cx| {
                this.enabled = !this.enabled;
                cx.notify();
            }))
            .on_action(|_: &Next, window, cx| window.focus_next(cx))
            .on_action(|_: &Previous, window, cx| window.focus_prev(cx))
            .on_action(|_: &Close, window, _| window.remove_window());

        root.child(div().text_2xl().child("Automatic layout positions"))
            .child("C compact · R reorder · A insert/remove · J align · T timing · M reduce · B on/off")
            .child(controls)
            .child(div().id("layout-status").role(Role::Label).aria_label(status.clone()).text_sm().text_color(rgb(0xaab4c7)).child(status))
            .child(div().h(px(360.)).flex_shrink_0().child(board))
            .child("Change the layout repeatedly while cards are moving. Their positions are measured automatically.")
            .child(div().text_sm().text_color(rgb(0xaab4c7)).child("Click a moving card, or Tab to it and press Enter / Space. Input follows its displayed position."))
            .child(div().text_sm().text_color(rgb(0xaab4c7)).child("Board size and card sizes change immediately. This animates position only; new cards mount at their layout position."))
    }
}
fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        let modifier = if cfg!(target_os = "macos") {
            "cmd"
        } else {
            "ctrl"
        };

        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([
            KeyBinding::new("c", Compact, None),
            KeyBinding::new("r", Reorder, None),
            KeyBinding::new("a", Insert, None),
            KeyBinding::new("j", Align, None),
            KeyBinding::new("t", Timing, None),
            KeyBinding::new("m", Reduced, None),
            KeyBinding::new("b", Enabled, None),
            KeyBinding::new("tab", Next, None),
            KeyBinding::new("shift-tab", Previous, None),
            KeyBinding::new(&format!("{modifier}-w"), Close, None),
            KeyBinding::new(&format!("{modifier}-q"), Quit, None),
        ]);
        cx.set_menus(vec![
            Menu::new("Layout motion").items([MenuItem::action("Quit", Quit)]),
            Menu::new("File").items([MenuItem::action("Close Window", Close)]),
            Menu::new("Layout").items([
                MenuItem::action("Compact Board", Compact),
                MenuItem::action("Reverse Order", Reorder),
                MenuItem::action("Insert / Remove Card", Insert),
                MenuItem::action("Align Rows", Align),
                MenuItem::action("Switch Timing", Timing),
                MenuItem::action("Reduced Motion", Reduced),
                MenuItem::action("Enable / Disable Animation", Enabled),
            ]),
        ]);
        cx.on_window_closed(|cx, _| cx.quit()).detach();
        let bounds = Bounds::centered(None, size(px(880.), px(800.)), cx);

        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(880.), px(800.))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Layout motion".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| {
                    cx.new(|cx| {
                        let root = cx.focus_handle();

                        root.focus(window, cx);

                        Demo {
                            group: LayoutGroup::default(),
                            root,
                            cards: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
                            compact: false,
                            reverse: false,
                            extra: false,
                            end: false,
                            tween: false,
                            enabled: true,
                            selected: None,
                            clicks: 0,
                        }
                    })
                },
            )
            .expect("open layout demo");

        window
            .update(cx, |_, window, _| window.activate_window())
            .expect("layout demo remains open");
        cx.activate(true);
    });
}
