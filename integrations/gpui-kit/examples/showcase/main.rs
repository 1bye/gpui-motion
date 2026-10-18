//! Six interactive product flows built with GPUI Kit and gpui-ce-motion.
mod app;

use gpui_kit::assets::Assets;
use gpui_kit::{AppContext, Bounds, WindowBounds, WindowOptions, point, px, size};

fn main() {
    let scene = std::env::args().nth(1).unwrap_or_else(|| "inbox".into());

    gpui_kit::application().with_assets(Assets).run(move |cx| {
        gpui_kit::init(cx);
        app::bind_keys(cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                point(px(80.), px(80.)),
                size(px(1180.), px(780.)),
            ))),
            titlebar: Some(gpui_kit::TitlebarOptions {
                title: Some("Motion / Kit".into()),
                ..Default::default()
            }),
            ..Default::default()
        };

        gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| app::Showcase::new(&scene, window, cx))
        })
        .expect("open Motion / Kit showcase");
        cx.activate(true);
    });
}
