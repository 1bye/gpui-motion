//! Kit adapter/layout/prepaint/paint preparation with synthetic fonts, display, and GPU.
use gpui_kit as ui;
use gpui_motion_kit as motion;

#[path = "../../../benches/common/text_native.rs"]
mod fixture;

fn main() {
    fixture::benchmark();
}
