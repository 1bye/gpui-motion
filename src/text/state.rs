use super::{TextMatch, TextOptions, TextTarget};
use crate::{AnimationState, MotionValue, Playback, PresenceMode, stagger};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::Duration,
};

/// A renderer-prepared text unit. Geometry is relative to the complete text element.
#[derive(Clone, Debug)]
pub struct TextLayoutUnit {
    /// Logical content, shaped before this unit was separated for painting.
    pub text: Arc<str>,
    /// Stable slot, normally logical index; numbers count slots from the right.
    pub slot: usize,
    /// Natural top-left position in logical pixels.
    pub origin: [f64; 2],
    /// Stationary clipping reservation in logical pixels.
    pub size: [f64; 2],
    /// Resolved font size for text-relative offsets.
    pub em: f64,
    /// Optional per-unit entrance override, prepared by `enter_with`.
    pub enter: Option<TextTarget>,
}

impl TextLayoutUnit {
    /// Prepares finite geometry. Panics for negative sizes or nonpositive font size.
    pub fn new(
        text: impl Into<Arc<str>>,
        slot: usize,
        origin: [f64; 2],
        size: [f64; 2],
        em: f64,
    ) -> Self {
        assert!(origin.iter().all(|value| value.is_finite()));
        assert!(size.iter().all(|value| value.is_finite() && *value >= 0.));
        assert!(em.is_finite() && em > 0.);

        Self {
            text: text.into(),
            slot,
            origin,
            size,
            em,
            enter: None,
        }
    }
}

/// A sampled unit. Geometry and opacity are drawing values; retained spring state
/// remains unclamped so reversals preserve its momentum.
#[derive(Clone, Copy, Debug)]
pub struct TextPresentation {
    /// Stable identity assigned by the text state.
    pub id: u64,
    /// Translated ink origin, in logical pixels.
    pub origin: [f64; 2],
    /// Stationary mask origin, including natural-position reflow.
    pub clip_origin: [f64; 2],
    /// Unit reservation.
    pub size: [f64; 2],
    /// Drawable opacity.
    pub opacity: f64,
    /// Drawable horizontal reveal fraction.
    pub reveal: f64,
    /// A prepared scramble alphabet index, if substituting this unit.
    pub substitution: Option<usize>,
    /// Outgoing units remain paintable until their transition finishes.
    pub exiting: bool,
    /// False for units waiting for outgoing content or a staggered clip start.
    pub visible: bool,
}

struct Unit {
    layout: TextLayoutUnit,
    id: u64,
    position: [MotionValue; 2],
    pose: [MotionValue; 4],
    last_pose: [f64; 4],
    enter: TextTarget,
    delay: Duration,
    sequence_index: usize,
    born: Duration,
    exiting: bool,
    waiting: bool,
    finished: bool,
    animation: AnimationState,
    declared_clip: Option<crate::Animation>,
    clip_resume: Option<(Duration, Duration)>,
    frame: TextPresentation,
    scramble: Option<super::Scramble>,
}

impl Unit {
    fn reset_pose(&mut self, now: Duration, options: &TextOptions, count: usize, animate: bool) {
        self.delay = stagger(
            options.stagger,
            self.sequence_index,
            count,
            options.stagger_from,
        )
        .unwrap_or(Duration::ZERO);
        self.born = now;
        self.finished = false;
        let from = if animate {
            self.enter
        } else {
            TextTarget::default()
        };

        let values = from.resolved_values(self.layout.em, self.layout.size[1]);

        for (value, initial) in self.pose.iter_mut().zip(values) {
            value.jump(initial, now);
        }

        self.animation = AnimationState::default();
        self.declared_clip = None;
        self.clip_resume = None;
        self.last_pose = values;
        self.target_pose(now, options, self.delay);
    }

    fn entrance_progress(&self, progress: f64, total: f64, options: &TextOptions) -> f64 {
        let duration = options.entrance_duration(1).as_secs_f64();
        let start = self.delay.as_secs_f64();

        if duration == 0. {
            if progress * total >= start { 1. } else { 0. }
        } else {
            ((progress * total - start) / duration).clamp(0., 1.)
        }
    }

    fn target_pose(&mut self, now: Duration, options: &TextOptions, delay: Duration) {
        let target = if self.exiting {
            options.exit
        } else {
            TextTarget::default()
        };

        let values = target.resolved_values(self.layout.em, self.layout.size[1]);

        for (index, (value, target)) in self.pose.iter_mut().zip(values).enumerate() {
            value.animate_delayed(target, options.pose_transition(index), delay, now);
        }
    }

    fn resume_pose(&mut self, now: Duration, options: &TextOptions) {
        for (value, held) in self.pose.iter_mut().zip(self.last_pose) {
            value.jump(held, now);
        }

        self.target_pose(now, options, Duration::ZERO);
    }

    fn sample(
        &mut self,
        now: Duration,
        options: &TextOptions,
        reduced: bool,
        progress: Option<f64>,
    ) -> bool {
        if self.waiting {
            self.frame.visible = false;

            return false;
        }

        let mut active = false;
        let mut origin = [0.; 2];

        for (axis, value) in self.position.iter_mut().enumerate() {
            let frame = value.sample(now, reduced);

            origin[axis] = frame.value();
            active |= frame.is_animating();
        }

        let mut pose = [0.; 4];

        if let Some(progress) = progress.filter(|_| !self.exiting && !reduced) {
            let elapsed = options.entrance_duration(1).mul_f64(progress);

            for (index, ((output, initial), target)) in pose
                .iter_mut()
                .zip(
                    self.enter
                        .resolved_values(self.layout.em, self.layout.size[1]),
                )
                .zip(TextTarget::default().values())
                .enumerate()
            {
                let mut value = MotionValue::new(initial);

                value.animate(target, options.pose_transition(index), Duration::ZERO);
                *output = if progress >= 1. {
                    target
                } else if progress <= 0. {
                    initial
                } else {
                    value.sample(elapsed, false).value()
                };
            }
        } else {
            for (output, value) in pose.iter_mut().zip(&mut self.pose) {
                let frame = value.sample(now, reduced);

                *output = frame.value();
                active |= frame.is_animating();
            }
        }

        let mut clip = [0., 0., 1.];

        if let Some(animation) = &options.animation {
            if self.declared_clip.as_ref() != Some(animation) {
                if self.declared_clip.is_some() {
                    self.clip_resume = Some((now, Duration::ZERO));
                }

                self.animation.sample(animation, now, reduced);
                self.declared_clip = Some(animation.clone());
            }

            if let Some(progress) = progress.filter(|_| !self.exiting)
                && let Some(duration) = animation.duration()
            {
                self.clip_resume = Some((now, duration.mul_f64(progress)));
            }

            let elapsed = self.clip_resume.map_or_else(
                || now.saturating_sub(self.born).saturating_sub(self.delay),
                |(anchor, cursor)| cursor.saturating_add(now.saturating_sub(anchor)),
            );

            let held = self.clip_resume.is_none() && now < self.born.saturating_add(self.delay);

            self.animation.seek(elapsed, now);
            let frame = self.animation.sample(animation, now, reduced);

            clip = [
                frame.target().x_px().into(),
                frame.target().y_px().into(),
                frame.target().alpha().into(),
            ];
            active |= !self.exiting && progress.is_none() && (held || frame.is_animating());
        }

        let mut substitution = if self.exiting {
            self.frame.substitution
        } else {
            None
        };

        if let Some(scramble) = &self.scramble {
            let elapsed = now.saturating_sub(self.born).saturating_sub(self.delay);
            let in_scramble = progress.map_or(elapsed < scramble.duration, |value| value < 1.);

            if in_scramble && !self.exiting && !reduced && !scramble.duration.is_zero() {
                let tick = progress.map_or(elapsed.as_millis() as u64 / 35, |value| {
                    (value * 30.) as u64
                });

                let mut bits = scramble.seed
                    ^ self.id.wrapping_mul(0x9e3779b97f4a7c15)
                    ^ tick.wrapping_mul(0xbf58476d1ce4e5b9);

                bits = (bits ^ (bits >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
                bits = (bits ^ (bits >> 27)).wrapping_mul(0x94d049bb133111eb);
                substitution = Some((bits ^ (bits >> 31)) as usize % scramble.alphabet.len());
                active |= progress.is_none();
            }
        }

        self.last_pose = pose;
        self.frame = TextPresentation {
            id: self.id,
            clip_origin: origin,
            origin: [
                origin[0] + pose[0] * self.layout.em + clip[0],
                origin[1] + pose[1] * self.layout.em + clip[1],
            ],
            size: self.layout.size,
            opacity: (pose[2] * clip[2]).clamp(0., 1.),
            reveal: pose[3].clamp(0., 1.),
            substitution,
            exiting: self.exiting,
            visible: !self.waiting,
        };
        self.finished = self.exiting && !active;

        active
    }
}

/// Renderer-independent text lifecycle and playback. Retain it beside prepared
/// glyph data and reconcile only after content, configuration, or layout changes.
/// No allocations occur in `sample`; reconciliation allocates membership storage.
#[derive(Default)]
pub struct TextPlaybackState {
    units: Vec<Unit>,
    content: String,
    options: TextOptions,
    next_id: u64,
    clock: Playback,
    manual_progress: Option<f64>,
    last_progress: Option<f64>,
    initialized: bool,
    active: bool,
    count: usize,
}

impl TextPlaybackState {
    /// Reconciles a prepared destination and returns identities in layout order.
    /// Equal content can reclaim an outgoing unit before its exit completes.
    pub fn reconcile(
        &mut self,
        text: &str,
        layout: &[TextLayoutUnit],
        options: &TextOptions,
        now: Duration,
        reduced: bool,
    ) -> Vec<u64> {
        self.sample(now, reduced, self.last_progress);
        let time = self.clock.time(now);

        let transition_changed = self.options.transition != options.transition
            || self.options.opacity_transition != options.opacity_transition;

        if self.last_progress.is_some() {
            for unit in &mut self.units {
                unit.resume_pose(time, &self.options);
            }
        }

        let changed = self.content != text;
        let first = !self.initialized;

        self.initialized = true;
        let mut candidates: HashMap<(Arc<str>, Option<usize>), VecDeque<usize>> = HashMap::new();

        for (index, unit) in self.units.iter().enumerate() {
            if !unit.waiting && (!changed || options.matching != TextMatch::Replace) {
                let slot = if !changed || options.matching == TextMatch::Position {
                    Some(unit.layout.slot)
                } else {
                    None
                };

                candidates
                    .entry((unit.layout.text.clone(), slot))
                    .or_default()
                    .push_back(index);
            }
        }

        let mut used = vec![false; self.units.len()];
        let mut assigned = Vec::with_capacity(layout.len());

        for unit_layout in layout {
            let slot = if !changed || options.matching == TextMatch::Position {
                Some(unit_layout.slot)
            } else {
                None
            };

            let existing = candidates
                .get_mut(&(unit_layout.text.clone(), slot))
                .and_then(VecDeque::pop_front);

            assigned.push(existing);

            if let Some(index) = existing {
                used[index] = true;
            }
        }

        self.options = options.clone();

        if options
            .animation
            .as_ref()
            .is_some_and(|clip| clip.duration().is_none())
        {
            self.manual_progress = None;
            self.last_progress = None;
        }

        self.count = layout.len();

        for (index, unit) in self.units.iter_mut().enumerate() {
            if !used[index] {
                if unit.waiting {
                    unit.finished = true;
                    continue;
                }

                if !unit.exiting {
                    unit.exiting = true;
                    unit.target_pose(time, options, Duration::ZERO);
                }
            }
        }

        let waiting = options.presence_mode == PresenceMode::Wait
            && self.units.iter().any(|unit| unit.exiting && !unit.finished);

        let mut ids = Vec::with_capacity(layout.len());

        for (index, (layout, existing)) in layout.iter().zip(assigned).enumerate() {
            if let Some(existing) = existing {
                let unit = &mut self.units[existing];
                let returning = unit.exiting;

                unit.layout = layout.clone();
                unit.sequence_index = index;
                unit.enter = layout.enter.unwrap_or(options.enter);
                unit.scramble = options.scramble.clone();
                unit.exiting = false;
                unit.waiting = false;
                unit.finished = false;

                for (value, target) in unit.position.iter_mut().zip(layout.origin) {
                    if let Some(transition) = options.reflow {
                        value.animate(target, transition, time);
                    } else {
                        value.jump(target, time);
                    }
                }

                if returning || transition_changed {
                    unit.target_pose(time, options, Duration::ZERO);
                }

                ids.push(unit.id);
            } else {
                let id = self.next_id;

                self.next_id = self
                    .next_id
                    .checked_add(1)
                    .expect("text identity exhausted");
                let mut unit = Unit {
                    layout: layout.clone(),
                    id,
                    position: layout.origin.map(MotionValue::new),
                    pose: TextTarget::default().values().map(MotionValue::new),
                    last_pose: TextTarget::default().values(),
                    enter: layout.enter.unwrap_or(options.enter),
                    delay: Duration::ZERO,
                    sequence_index: index,
                    born: time,
                    exiting: false,
                    waiting,
                    finished: false,
                    animation: AnimationState::default(),
                    declared_clip: None,
                    clip_resume: None,
                    frame: TextPresentation {
                        id,
                        origin: layout.origin,
                        clip_origin: layout.origin,
                        size: layout.size,
                        opacity: 0.,
                        reveal: 1.,
                        substitution: None,
                        exiting: false,
                        visible: false,
                    },
                    scramble: options.scramble.clone(),
                };

                unit.reset_pose(time, options, self.count, !first || options.animate_initial);
                self.units.push(unit);
                ids.push(id);
            }
        }

        self.content.clear();
        self.content.push_str(text);
        self.sample(now, reduced, self.last_progress);

        ids
    }

    /// Samples the presentation with an optional normalized finite entrance cursor.
    /// Scrubbing holds entrance playback; layout reflow still follows its own target.
    /// Panics for non-finite progress or values outside zero through one.
    pub fn sample(&mut self, now: Duration, reduced: bool, progress: Option<f64>) -> bool {
        let progress = progress.or(self.manual_progress);

        if let Some(progress) = progress {
            assert!((0. ..=1.).contains(&progress));
        }

        if progress.is_some() {
            assert!(
                self.options
                    .animation
                    .as_ref()
                    .is_none_or(|animation| animation.duration().is_some()),
                "cannot scrub an infinite text clip"
            );
        }

        let time = self.clock.time(now);

        if progress.is_none() && self.last_progress.is_some() {
            for unit in &mut self.units {
                unit.resume_pose(time, &self.options);
            }
        }

        self.last_progress = progress;
        self.active = false;
        let total = self.options.entrance_duration(self.count).as_secs_f64();

        for unit in &mut self.units {
            let unit_progress =
                progress.map(|progress| unit.entrance_progress(progress, total, &self.options));

            self.active |= unit.sample(time, &self.options, reduced, unit_progress);
        }

        self.units.retain(|unit| !unit.finished);

        if !self.units.iter().any(|unit| unit.exiting) {
            for unit in &mut self.units {
                if unit.waiting {
                    unit.waiting = false;
                    unit.reset_pose(time, &self.options, self.count, !reduced);
                    let unit_progress = progress
                        .map(|progress| unit.entrance_progress(progress, total, &self.options));

                    self.active |= unit.sample(time, &self.options, reduced, unit_progress);
                }
            }
        }

        self.active &= self.clock.is_playing() && !reduced;

        self.active
    }

    /// Moves the coordinate reference without changing velocity or playback time.
    /// Useful when an unchanged text element follows its parent layout. Panics for
    /// non-finite offsets. Content-induced reflow should use `reconcile` instead.
    pub fn translate(&mut self, offset: [f64; 2]) {
        assert!(offset.iter().all(|value| value.is_finite()));

        for unit in &mut self.units {
            for (axis, delta) in offset.into_iter().enumerate() {
                unit.position[axis].translate(delta);
                unit.layout.origin[axis] += delta;
                unit.frame.origin[axis] += delta;
                unit.frame.clip_origin[axis] += delta;
            }
        }
    }

    /// Current prepared presentation, without allocation or advancing time.
    pub fn presentation(&self) -> impl Iterator<Item = &TextPresentation> {
        self.units.iter().map(|unit| &unit.frame)
    }

    /// Semantic destination; outgoing visual content is never included.
    pub fn text(&self) -> &str {
        &self.content
    }

    /// Whether the previous sample requires another frame.
    pub fn is_animating(&self) -> bool {
        self.active
    }

    /// Freezes the animation clock at its current presentation.
    pub fn pause(&mut self, now: Duration) {
        self.sample(now, false, self.last_progress);
        self.clock.pause(now);
        self.active = false;
    }

    /// Resumes after a pause or scrub, retargeting from the held pose after a scrub.
    pub fn play(&mut self, now: Duration) {
        let time = self.clock.time(now);

        if self.manual_progress.take().is_some() || self.last_progress.is_some() {
            for unit in &mut self.units {
                unit.resume_pose(time, &self.options);
            }
        }

        self.last_progress = None;

        self.clock.play(now);
    }

    /// Holds a normalized entrance cursor. Infinite clips reject normalized seeking.
    /// Returns false for an infinite clip; otherwise panics for invalid progress.
    pub fn seek(&mut self, progress: f64, now: Duration) -> bool {
        assert!((0. ..=1.).contains(&progress));

        if self
            .options
            .animation
            .as_ref()
            .is_some_and(|animation| animation.duration().is_none())
        {
            return false;
        }

        self.manual_progress = Some(progress);
        self.clock.pause(now);
        self.sample(now, false, None);

        true
    }

    /// Replays the current incoming units, preserving their stable identities.
    pub fn restart(&mut self, now: Duration) {
        self.manual_progress = None;
        self.last_progress = None;
        self.clock.play(now);
        let time = self.clock.time(now);

        for unit in &mut self.units {
            if !unit.exiting {
                unit.reset_pose(time, &self.options, self.count, true);
            }
        }
    }
}
