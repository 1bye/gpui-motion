//! Embedded SVG assets for the standalone native demos.
use std::borrow::Cow;

pub struct SvgAsset(pub &'static [u8]);

impl gpui::AssetSource for SvgAsset {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        Ok((path == "motion.svg").then_some(Cow::Borrowed(self.0)))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<gpui::SharedString>> {
        Ok(if "motion.svg".starts_with(path) {
            vec!["motion.svg".into()]
        } else {
            Vec::new()
        })
    }
}
