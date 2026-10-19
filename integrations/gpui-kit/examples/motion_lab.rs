//! Expanded stock-Kit API gallery: `--features demo --example motion_lab`.
pub(crate) use gpui_kit as ui;
pub(crate) use gpui_motion_kit as motion;

#[path = "motion_lab/view.rs"]
pub(crate) mod view;

use ui::component::button::{Button, ButtonVariants};
use ui::{App, ClickEvent, ElementId, Window, px};

pub(crate) fn native_color(color: motion::core::Color) -> ui::Hsla {
    motion::color_to_rgba(color).into()
}

pub(crate) fn configure_drag(state: motion::GestureState) -> motion::GestureState {
    state.touch_drag(true)
}

pub(crate) fn native_svg() -> ui::Svg {
    ui::svg().data(br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path fill="black" d="M2 10h12V4l8 8-8 8v-6H2z"/></svg>"#)
}

pub(crate) fn button(
    id: impl Into<ElementId>,
    label: String,
    selected: bool,
    callback: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> ui::AnyElement {
    let id = id.into();

    let selector = if label == "Play / reverse" {
        "play"
    } else {
        "control"
    };

    let button = Button::new(id.clone()).label(label).on_click(callback);

    use ui::prelude::*;
    let button = if selected {
        button.primary().into_any_element()
    } else {
        button.into_any_element()
    };

    ui::div()
        .id(id)
        .debug_selector(move || selector.into())
        .child(button)
        .into_any_element()
}

fn main() {
    use ui::AppContext as _;
    ui::application().with_assets(ui::assets::Assets).run(|cx| {
        init_app(cx);
        ui::open_window(
            ui::WindowOptions {
                window_bounds: Some(ui::WindowBounds::Windowed(ui::Bounds::centered(
                    None,
                    ui::size(px(960.), px(640.)),
                    cx,
                ))),
                titlebar: Some(ui::TitlebarOptions {
                    title: Some("Motion lab / Kit".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            cx,
            |_, cx| cx.new(|_| view::Lab::new()),
        )
        .unwrap();
        cx.activate(true);
    });
}

#[cfg(all(test, feature = "native-tests"))]
fn init_test(cx: &mut ui::TestAppContext) {
    cx.update(init_app);
}

pub(crate) fn init_app(cx: &mut ui::App) {
    ui::init(cx);
    ui::component::Theme::change(ui::component::ThemeMode::Dark, None, cx);
}
