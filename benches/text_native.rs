//! CE adapter/layout/prepaint/paint preparation with synthetic fonts, display, and GPU.
use gpui as ui;
use gpui_ce_motion as motion;

#[path = "common/text_native.rs"]
mod fixture;

fn main() {
    fixture::benchmark();
}
