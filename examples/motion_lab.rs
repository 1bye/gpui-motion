//! Expanded stock-CE API gallery: `cargo run --features demo --example motion_lab`.
pub(crate) use gpui as ui;
pub(crate) use gpui_ce_motion as motion;

#[path = "common/mod.rs"]
mod common;
#[path = "motion_lab/view.rs"]
pub(crate) mod view;

use ui::{App, AppContext, ClickEvent, ElementId, IntoElement, Window, div, prelude::*, px, rgb};

pub(crate) fn native_color(color: gpui_ce_motion::Color) -> ui::Hsla {
    ui::rgb_to_hsla(color.to_rgba())
}

pub(crate) fn configure_drag(state: motion::GestureState) -> motion::GestureState {
    state
}

pub(crate) fn native_svg() -> ui::Svg {
    if cfg!(feature = "native-tests") {
        ui::svg().external_path(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/examples/motion_lab/arrow.svg"
        ))
    } else {
        ui::svg().path("motion.svg")
    }
}

pub(crate) fn button(
    id: impl Into<ElementId>,
    label: String,
    selected: bool,
    callback: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> ui::AnyElement {
    let callback = std::rc::Rc::new(callback);
    let click = callback.clone();

    let selector = if label == "Play / reverse" {
        "play"
    } else {
        "control"
    };

    div()
        .id(id)
        .debug_selector(move || selector.into())
        .role(ui::Role::Button)
        .aria_label(label.clone())
        .tab_index(0)
        .px_3()
        .py_2()
        .rounded_lg()
        .border_1()
        .border_color(rgb(0x3b4736))
        .bg(rgb(if selected { 0xd8f58b } else { 0x202a20 }))
        .text_color(rgb(if selected { 0x182018 } else { 0xf0f4e8 }))
        .focus_visible(|style| style.border_color(rgb(0xf7ac8b)))
        .child(label)
        .on_click(move |event, window, cx| click(event, window, cx))
        .on_key_down(move |event, window, cx| {
            if event.keystroke.key == "enter" || event.keystroke.key == "space" {
                callback(&ClickEvent::default(), window, cx);
                cx.stop_propagation();
            }
        })
        .into_any_element()
}

fn main() {
    let assets = common::SvgAsset(br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path fill="black" d="M2 10h12V4l8 8-8 8v-6H2z"/></svg>"#);

    gpui_platform::application().with_assets(assets).run(|cx| {
        cx.on_window_closed(|cx, _| cx.quit()).detach();

        let options = ui::WindowOptions {
            window_bounds: Some(ui::WindowBounds::Windowed(ui::Bounds::centered(
                None,
                ui::size(px(960.), px(640.)),
                cx,
            ))),
            titlebar: Some(ui::TitlebarOptions {
                title: Some("Motion lab / CE".into()),
                ..Default::default()
            }),
            ..Default::default()
        };

        cx.open_window(options, |_, cx| cx.new(|_| view::Lab::new()))
            .expect("open Motion lab");
        cx.activate(true);
    });
}

#[cfg(all(test, feature = "native-tests"))]
fn init_test(cx: &mut ui::TestAppContext) {
    let _ = cx;
}
