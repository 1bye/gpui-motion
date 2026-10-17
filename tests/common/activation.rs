//! Activation-name compatibility for the two explicitly verified CE sources.
//! Registry 0.2.2 calls this activate_window; the official pin calls it activate.
//! Each source's inherent method wins over the trait fallback below. Recheck
//! this adapter when changing the CE version; it is not a runtime feature.
use gpui::Window;
#[allow(dead_code)]
trait ActivationCompat {
    fn activate(&self);
    fn activate_window(&self);
}
impl ActivationCompat for Window {
    fn activate(&self) {
        let window: &Window = self;
        window.activate_window();
    }
    fn activate_window(&self) {
        let window: &Window = self;
        window.activate();
    }
}
pub fn activate(window: &Window) {
    window.activate();
}
