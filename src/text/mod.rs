//! Unicode-aware text units and explicitly clocked, interruptible text playback.
//! Native adapters shape complete runs before preparing units. Sampling a prepared
//! layout reuses its storage; matching, shaping, and native painting have separate costs.

mod native;
mod options;
mod state;

pub use options::{
    NumberEffect, Scramble, TextClip, TextError, TextMatch, TextOptions, TextPreset, TextTarget,
    TextUnit, TextUnitInfo, text_ranges,
};
pub use state::{TextLayoutUnit, TextPlaybackState, TextPresentation};
