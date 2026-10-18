use gpui_kit::base::StyledExt as _;

#[path = "assets.rs"]
mod assets;
#[path = "automation.rs"]
mod automation;
#[path = "downloads.rs"]
mod downloads;
#[path = "inbox.rs"]
mod inbox;
#[path = "kanban.rs"]
mod kanban;
#[path = "palette.rs"]
mod palette;

use gpui_ce_motion::{Spring, Transition};

use gpui_kit::{
    AnyView, App, AppContext, Context, FocusHandle, KeyBinding, Render, Styled, Window, actions,
    component::{
        ActiveTheme,
        tab::{Tab, TabBar},
    },
    div,
    prelude::*,
};

use std::time::Duration;

actions!(kit_showcase, [Quit]);

pub const SCENES: [&str; 6] = [
    "inbox",
    "assets",
    "downloads",
    "palette",
    "kanban",
    "automation",
];

const TITLES: [&str; 6] = [
    "Inbox",
    "Assets",
    "Downloads",
    "Commands",
    "Planner",
    "Automation",
];

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("ctrl-q", Quit, None),
    ]);
    cx.on_action(|_: &Quit, cx| cx.quit());
    inbox::bind_keys(cx);
}

pub fn spring() -> Transition {
    Transition::Spring(
        Spring::visual(Duration::from_millis(240), 0.08).expect("valid visual spring"),
    )
}

pub fn heading(title: &'static str, subtitle: &'static str, cx: &App) -> gpui_kit::Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .mb_6()
        .child(div().text_2xl().font_semibold().child(title))
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(subtitle),
        )
}

pub fn surface(cx: &App) -> gpui_kit::Div {
    div()
        .bg(cx.theme().background)
        .border_1()
        .border_color(cx.theme().border)
        .rounded_lg()
}

pub struct Showcase {
    active: usize,
    views: Vec<AnyView>,
    focus: FocusHandle,
}

impl Showcase {
    pub fn new(scene: &str, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();

        window.focus(&focus, cx);
        let views = vec![
            cx.new(|cx| inbox::Inbox::new(window, cx)).into(),
            cx.new(|cx| assets::Browser::new(window, cx)).into(),
            cx.new(|cx| downloads::Downloads::new(window, cx)).into(),
            cx.new(|cx| palette::Palette::new(window, cx)).into(),
            cx.new(|cx| kanban::Planner::new(window, cx)).into(),
            cx.new(|cx| automation::Runner::new(window, cx)).into(),
        ];

        Self {
            active: SCENES.iter().position(|name| *name == scene).unwrap_or(0),
            views,
            focus,
        }
    }
}

impl Render for Showcase {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("showcase")
            .key_context("MotionKit")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().muted)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_8()
                    .py_4()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().background)
                    .child(
                        div()
                            .font_semibold()
                            .child(format!("{:02} / 06", self.active + 1)),
                    )
                    .child(
                        TabBar::new("showcase-tabs")
                            .pill()
                            .selected_index(self.active)
                            .children(TITLES.map(|title| Tab::new().label(title)))
                            .on_click(cx.listener(|this, index: &usize, _, cx| {
                                this.active = *index;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .p_8()
                    .child(self.views[self.active].clone()),
            )
    }
}

#[cfg(all(test, feature = "native-tests"))]
#[path = "tests.rs"]
mod tests;

// Base supplies progress semantics; this example owns its single animation channel.
pub fn progress(
    id: impl Into<gpui_kit::ElementId>,
    label: impl Into<gpui_kit::SharedString>,
    reported: f32,
    displayed: f32,
    cx: &App,
) -> impl IntoElement {
    gpui_kit::base::Progress::new(id)
        .value(reported)
        .accessibility_label(label)
        .relative()
        .w_full()
        .h_2()
        .rounded_full()
        .child(
            gpui_kit::base::ProgressTrack::new()
                .absolute()
                .size_full()
                .rounded_full()
                .bg(cx.theme().muted),
        )
        .child(
            gpui_kit::base::ProgressIndicator::new()
                .absolute()
                .left_0()
                .top_0()
                .h_full()
                .w(gpui_kit::relative((displayed / 100.).clamp(0., 1.)))
                .rounded_full()
                .bg(cx.theme().primary),
        )
}
