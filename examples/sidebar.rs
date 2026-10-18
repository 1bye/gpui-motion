//! A native app shell with an interruptible, collapsing sidebar.
//! Run `cargo run --features demo --example sidebar`. Cmd/Ctrl+B toggles the sidebar; Escape closes it.
//! The content's inset follows sampled width, so this uses ordinary layout reflow.

use std::time::Duration;

use gpui::{
    App, Bounds, Context, Div, FocusHandle, KeyBinding, Menu, MenuItem, Role, SharedString,
    Stateful, TitlebarOptions, Window, WindowBounds, WindowOptions, actions, div, prelude::*, px,
    rgb, size,
};

use gpui_ce_motion::{Easing, Motion, MotionState, Spring, Target, Transition};

const SIDEBAR_WIDTH: f32 = 224.0;

actions!(
    sidebar_demo,
    [
        /// Show or hide the sidebar.
        ToggleSidebar,
        /// Close the sidebar and return focus to its toggle.
        HideSidebar,
        /// Switch between spring and tween transitions.
        ToggleTransition,
        /// Toggle reduced motion for the demo.
        ToggleReducedMotion,
        /// Move focus to the next control.
        FocusNext,
        /// Move focus to the previous control.
        FocusPrevious,
        /// Close the window.
        Close,
        /// Quit the example.
        Quit,
    ]
);

// Local tokens keep this direct-GPUI example independent of a component library.
struct Palette;

impl Palette {
    fn background() -> gpui::Rgba {
        rgb(0x15171b)
    }

    fn sidebar() -> gpui::Rgba {
        rgb(0x1c1f25)
    }

    fn surface() -> gpui::Rgba {
        rgb(0x22262e)
    }

    fn foreground() -> gpui::Rgba {
        rgb(0xedeef0)
    }

    fn muted() -> gpui::Rgba {
        rgb(0x9ca3af)
    }

    fn border() -> gpui::Rgba {
        rgb(0x343944)
    }

    fn accent() -> gpui::Rgba {
        rgb(0xa6baff)
    }
}

fn button(id: &'static str, label: impl Into<SharedString>, enabled: bool) -> Stateful<Div> {
    let label = label.into();

    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label.clone())
        .when(!enabled, |button| button.aria_description("Unavailable"))
        .when(enabled, |button| button.tab_index(0))
        .border_1()
        .border_color(Palette::border())
        .rounded_md()
        .px_3()
        .py_2()
        .hover(|style| style.bg(Palette::surface()))
        .focus_visible(|style| style.border_color(Palette::accent()))
        .active(|style| style.bg(Palette::border()))
        .child(label)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Overview,
    Projects,
    Archive,
}

impl Section {
    fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Projects => "Projects",
            Self::Archive => "Archive",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Overview => {
                "A little room to think. Keep your next steps close and everything else out of the way."
            }
            Self::Projects => {
                "A home for work in progress. Switch sections while the sidebar is moving."
            }
            Self::Archive => {
                "Finished work stays here. Your selection survives closing and reopening the sidebar."
            }
        }
    }

    fn notes(self) -> [(&'static str, &'static str); 3] {
        match self {
            Self::Overview => [
                ("Today", "Sketch the next idea"),
                ("This week", "Make room for focused work"),
                ("Later", "Explore something new"),
            ],
            Self::Projects => [
                ("Design system", "Refine the small details"),
                ("Native motion", "Try reversing an animation mid-flight"),
                ("Workspace", "Bring the pieces together"),
            ],
            Self::Archive => [
                ("First prototype", "A starting point worth keeping"),
                ("Spring study", "A few experiments in motion"),
                ("Early sketches", "Ideas for another day"),
            ],
        }
    }
}

struct SidebarApp {
    sidebar: MotionState,
    root_focus: FocusHandle,
    toggle_focus: FocusHandle,
    sidebar_focus: FocusHandle,
    open: bool,
    section: Section,
    tween: bool,
    spring: Spring,
}

impl SidebarApp {
    fn set_open(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !open && self.sidebar_focus.contains_focused(window, cx) {
            self.toggle_focus.focus(window, cx);
        }

        self.open = open;
        cx.notify();
    }

    fn navigation(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let mut navigation = div()
            .id("sidebar-navigation")
            .role(Role::Navigation)
            .aria_label("Workspace navigation")
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .flex()
            .flex_col()
            .p_4()
            .gap_2()
            .bg(Palette::sidebar())
            .border_r_1()
            .border_color(Palette::border())
            .when(self.open, |navigation| {
                navigation.track_focus(&self.sidebar_focus)
            })
            .when(!self.open, |navigation| {
                navigation.aria_description("Closing; unavailable")
            })
            .child(div().text_lg().mb_4().child("Motion workspace"))
            .child(
                div()
                    .text_xs()
                    .mb_2()
                    .text_color(Palette::muted())
                    .child("Workspace"),
            );

        for (id, section) in [
            ("overview", Section::Overview),
            ("projects", Section::Projects),
            ("archive", Section::Archive),
        ] {
            let selected = self.section == section;

            navigation = navigation.child(
                button(id, section.title(), self.open)
                    .aria_selected(selected)
                    .when(selected, |button| {
                        button.bg(Palette::surface()).text_color(Palette::accent())
                    })
                    .when(self.open, |button| {
                        button.on_click(cx.listener(move |this, _, _, cx| {
                            if this.open {
                                this.section = section;
                                cx.notify();
                            }
                        }))
                    }),
            );
        }

        navigation.child(div().flex_1()).child(
            div()
                .text_sm()
                .text_color(Palette::muted())
                .child("A place for your ideas."),
        )
    }
}

impl Render for SidebarApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let hidden = Target::default().width(0.0).opacity(0.0);

        let transition = if self.tween {
            Transition::Tween(Duration::from_millis(300), Easing::EaseInOut)
        } else {
            Transition::Spring(self.spring)
        };

        // Keep declaring the sidebar while closed: render() removes it after exit.
        let frame = self.sidebar.frame(
            Motion::new(Target::default().width(SIDEBAR_WIDTH))
                .initial(hidden)
                .exit(hidden)
                .transition(transition)
                .present(self.open),
            window,
            cx,
        );

        let status = format!(
            "Sidebar {:?} · {} · Reduced motion {}",
            frame.presence(),
            if self.tween { "Tween" } else { "Spring" },
            if cx.reduce_motion() { "on" } else { "off" }
        );

        let section = self.section;
        let mut notes = div().flex().flex_col().gap_3();

        for (title, description) in section.notes() {
            notes = notes.child(
                div()
                    .p_4()
                    .rounded_lg()
                    .bg(Palette::surface())
                    .border_1()
                    .border_color(Palette::border())
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(title)
                    .child(
                        div()
                            .text_sm()
                            .text_color(Palette::muted())
                            .child(description),
                    ),
            );
        }

        div().id("sidebar-app").track_focus(&self.root_focus).tab_group()
            .on_action(cx.listener(|this, _: &ToggleSidebar, window, cx| this.set_open(!this.open, window, cx)))
            .on_action(cx.listener(|this, _: &HideSidebar, window, cx| this.set_open(false, window, cx)))
            .on_action(cx.listener(|this, _: &ToggleTransition, _, cx| { this.tween = !this.tween; cx.notify(); }))
            .on_action(cx.listener(|_, _: &ToggleReducedMotion, _, cx| {
                let reduced = !cx.reduce_motion();

                cx.set_reduce_motion(reduced);
                cx.notify();
            }))
            .on_action(|_: &FocusNext, window, cx| window.focus_next(cx))
            .on_action(|_: &FocusPrevious, window, cx| window.focus_prev(cx))
            .on_action(|_: &Close, window, _| window.remove_window())
            .size_full().flex().flex_col().bg(Palette::background()).text_color(Palette::foreground())
            .child(div().flex().items_center().gap_4().px_4().py_3().border_b_1().border_color(Palette::border())
                .child(button("toggle-sidebar", if self.open { "Hide sidebar" } else { "Show sidebar" }, true)
                    .track_focus(&self.toggle_focus).aria_expanded(self.open)
                    .on_click(cx.listener(|this, _, window, cx| this.set_open(!this.open, window, cx))))
                .child("Your workspace").child(div().flex_1())
                .child(div().text_sm().text_color(Palette::muted()).child(if cfg!(target_os = "macos") { "⌘B  Toggle sidebar" } else { "Ctrl+B  Toggle sidebar" })))
            .child(div().flex().flex_1().min_h_0()
                .child(frame.render(|frame| frame.apply(div().h_full().flex_shrink_0().overflow_hidden())
                    .child(self.navigation(cx))))
                .child(div().id("main-content").role(Role::Main).aria_label(section.title())
                    .flex_1().min_w_0().h_full().overflow_y_scroll().p_8()
                    .child(div().flex().flex_col().gap_6().max_w(px(680.0))
                        .child(div().id("section-title").role(Role::Heading).aria_label(section.title())
                            .text_2xl().child(section.title()))
                        .child(div().text_color(Palette::muted()).child(section.description()))
                        .child(notes)
                        .child(div().text_sm().text_color(Palette::muted()).child("Try closing and reopening the sidebar before it settles. The content follows along.")))))
            .child(div().id("motion-status").role(Role::Status).aria_label(status.clone())
                .px_4().py_2().border_t_1().border_color(Palette::border()).text_xs()
                .text_color(Palette::muted()).child(status))
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
            KeyBinding::new(&format!("{command}-b"), ToggleSidebar, None),
            KeyBinding::new("escape", HideSidebar, None),
            KeyBinding::new(&format!("{command}-shift-t"), ToggleTransition, None),
            KeyBinding::new(&format!("{command}-shift-m"), ToggleReducedMotion, None),
            KeyBinding::new("tab", FocusNext, None),
            KeyBinding::new("shift-tab", FocusPrevious, None),
            KeyBinding::new(&format!("{command}-w"), Close, None),
            KeyBinding::new(&format!("{command}-q"), Quit, None),
        ]);
        cx.set_menus(vec![
            Menu::new("Sidebar demo").items([MenuItem::action("Quit Sidebar Demo", Quit)]),
            Menu::new("File").items([MenuItem::action("Close Window", Close)]),
            Menu::new("View").items([
                MenuItem::action("Toggle Sidebar", ToggleSidebar),
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
        let bounds = Bounds::centered(None, size(px(1040.0), px(720.0)), cx);

        let handle = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(640.0), px(520.0))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("1keep · Sidebar motion".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| {
                    cx.new(|cx| {
                        let root_focus = cx.focus_handle();
                        let toggle_focus = cx.focus_handle().tab_stop(true);

                        toggle_focus.focus(window, cx);

                        SidebarApp {
                            sidebar: MotionState::default(),
                            root_focus,
                            toggle_focus,
                            sidebar_focus: cx.focus_handle(),
                            open: true,
                            section: Section::Overview,
                            tween: false,
                            spring: Spring::new(256.0, 32.0, 1.0)
                                .expect("valid critically damped spring"),
                        }
                    })
                },
            )
            .expect("open sidebar example");

        handle
            .update(cx, |_, window, _| window.activate_window())
            .expect("sidebar window is open");
        cx.activate(true);
    });
}
