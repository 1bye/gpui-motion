//! Declarative transitions with retained, explicitly owned motion state.
//!
//! Keep one [`MotionState`] per logical object in its owning view. Recreate the
//! [`Motion`] description each render; changing a target interrupts from the
//! current presentation. Springs also preserve velocity. No tasks, allocations,
//! global registries, or timers are needed to sample motion. Prepared clips and
//! timelines allocate during construction; their sampling reuses retained state.
//!
//! ```
//! use gpui_ce_motion::{Motion, MotionState, Target, Transition};
//! use std::time::Duration;
//!
//! let mut state = MotionState::default();
//! let motion = Motion::new(Target::default().x(120.0).width(240.0).border_radius(12.0))
//!     .initial(Target::default().width(0.0).border_radius(0.0).opacity(0.0))
//!     .exit(Target::default().y(16.0).opacity(0.0))
//!     .transition(Transition::default());
//! assert!(state.sample(motion, Duration::ZERO, false).is_animating());
//! let frame = state.sample(motion, Duration::from_secs(5), false);
//! assert_eq!(frame.target().x_px(), 120.0);
//! assert_eq!(frame.target().width_px(), Some(240.0));
//! assert!(!frame.is_animating());
//! ```
//!
//! The optional `gpui` feature adds `MotionState::frame` and GPUI rendering
//! helpers. See the crate README for integration and publication constraints.

//!
//! Prepared playback uses the same explicit clocks and ownership:
//!
//! ```
//! use gpui_ce_motion::{Animation, AnimationState, Easing, Repeat, RepeatMode, Target};
//! use std::time::Duration;
//! # fn main() -> Result<(), gpui_ce_motion::AnimationError> {
//! let clip = Animation::to(Target::default().x(120.0), Duration::from_millis(600), Easing::Linear)
//!     .repeat(Repeat::Forever, RepeatMode::Mirror, Duration::ZERO)?;
//! let mut state = AnimationState::default();
//! state.sample(&clip, Duration::ZERO, false);
//! state.pause(Duration::from_millis(300));
//! let held = state.sample(&clip, Duration::from_millis(900), false);
//! assert_eq!(held.target().x_px(), 60.0);
//! assert!(!held.is_animating());
//! # Ok(())
//! # }
//! ```

mod animation;
mod color;
mod interaction;
mod layout;
#[cfg(feature = "gpui")]
mod layout_native;
mod motion;
mod presence;
mod svg;
mod transition;
mod value;

#[cfg(feature = "gpui")]
mod native;

pub use motion::{Frame, Motion, MotionState, Presence, Property, Target};

pub use transition::{
    Bezier, CustomEasing, Easing, InvalidEasing, InvalidSpring, Spring, Transition,
};

pub use value::{Inertia, MotionValue, ValueFrame};

pub use animation::{
    Animation, AnimationError, AnimationState, Keyframe, Playback, Repeat, RepeatMode, StaggerFrom,
    Timeline, stagger,
};

pub use color::{Color, ColorState};

pub use interaction::{
    Drag, GestureTargets, InView, Interaction, Variants, move_item, reorder_index, scroll_progress,
    transform,
};

pub use presence::{PresenceChild, PresenceGroup, PresenceMode};

pub use svg::{SvgFrame, SvgMotion, SvgMotionState, SvgTarget};

pub use layout::{LayoutPositionFrame, LayoutPositionState};
#[cfg(feature = "gpui")]
pub use layout_native::{LayoutGroup, LayoutPositionElement};

mod style_motion;

pub use style_motion::{
    ColorProperty, StyleFrame, StyleMotion, StyleMotionState, StyleProperty, StyleTarget,
};

#[cfg(feature = "gpui")]
mod measured_size;

#[cfg(feature = "gpui")]
pub use measured_size::{AutoSize, SizeAxis};

#[cfg(feature = "gpui")]
mod pop_layout;

#[cfg(feature = "gpui")]
pub use pop_layout::{PopElement, PopLayout, PopRequest};

#[cfg(feature = "gpui")]
mod gesture_native;

#[cfg(feature = "gpui")]
pub use gesture_native::{GestureElement, GestureState};

#[cfg(feature = "gpui")]
mod scroll_native;

#[cfg(feature = "gpui")]
pub use scroll_native::{Axis, ScrollMotion, native_scroll_progress};

#[cfg(feature = "gpui")]
mod shared_layout;

#[cfg(feature = "gpui")]
pub use shared_layout::{SharedElement, SharedLayout};

#[cfg(feature = "gpui")]
mod reorder_native;

#[cfg(feature = "gpui")]
pub use reorder_native::{ReorderElement, ReorderGroup};

mod vector;

pub use vector::{PathCommand, PathError, PathFrame, PathMorphState, PathShape, StrokeReveal};

#[cfg(feature = "gpui")]
mod vector_native;

#[cfg(feature = "gpui")]
pub use vector_native::{VectorElement, VectorState};
