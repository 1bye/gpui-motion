//! Keyed ownership of independent transitions, using the ordinary motion sampler.
use crate::{Frame, Motion, MotionState};
use std::{collections::HashMap, hash::Hash, time::Duration};

#[derive(Debug, Default)]
struct Entry {
    state: MotionState,
    batch: u64,
}

/// Component-owned transitions keyed by stable part or item IDs.
///
/// Each key retains an independent [`MotionState`]. Batch results follow input
/// order; reordering declarations does not change identity. Existing entries
/// sample without allocating. Each new key allocates its state and may grow the
/// hash table. Key construction/destruction may also allocate depending on `K`.
///
/// Omitted keys are neither sampled nor removed. Their elapsed time still passes:
/// submitting them again samples their old trajectory at the new time. Continue
/// submitting `present(false)` through an exit, then call [`Self::remove`] after
/// its frame is no longer mounted. Dropping the collection cancels all its motion.
///
/// ```
/// use gpui_ce_motion::{Motion, Motions, Target};
/// use std::time::Duration;
///
/// let mut motions = Motions::with_capacity(2);
/// let panel = Motion::new(Target::default().height(240.0))
///     .initial(Target::default().height(80.0));
/// let title = Motion::new(Target::default()).initial(Target::default().opacity(0.0));
///
/// let [panel, title] = motions.sample(
///     [("panel", panel), ("title", title)],
///     Duration::ZERO,
///     false,
/// );
///
/// assert_eq!(panel.target().height_px(), Some(80.0));
/// assert_eq!(title.target().alpha(), 0.0);
/// assert_eq!(motions.len(), 2);
/// ```
#[derive(Debug)]
pub struct Motions<K> {
    // Large trajectories do not occupy unused hash buckets.
    entries: HashMap<K, Box<Entry>>,
    batch: u64,
    last: Duration,
    #[cfg(feature = "gpui")]
    pub(crate) epoch: Option<std::time::Instant>,
}

impl<K> Default for Motions<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K> Motions<K> {
    /// Creates an empty collection without allocating.
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    /// Reserves space for at least `capacity` keys, without creating trajectories.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(capacity),
            batch: 0,
            last: Duration::ZERO,
            #[cfg(feature = "gpui")]
            epoch: None,
        }
    }

    /// Number of retained keys, including settled or absent objects.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no keys are retained.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Number of keys that fit before the hash table must grow.
    pub fn capacity(&self) -> usize {
        self.entries.capacity()
    }

    /// Cancels all trajectories and drops their keys, keeping table capacity.
    /// The collection's clock continues. Call [`Self::shrink_to_fit`] to release
    /// unused table storage, or drop the collection to release everything.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl<K: Eq + Hash> Motions<K> {
    /// Samples a batch at one absolute monotonic timestamp, returning owned frames.
    ///
    /// Springs preserve velocity when retargeted. Reduced motion snaps each
    /// submitted entry; backward timestamps clamp to the collection's last sample.
    /// An empty batch is valid and leaves entries retained. Expected cost is O(N)
    /// hash lookups plus sampling, independent of the number of omitted entries.
    /// Do not mix explicit timestamps with native `frame` calls on this collection.
    ///
    /// # Panics
    /// Panics if a key occurs twice in this batch. This is a declaration error,
    /// not a transaction: entries preceding the duplicate may already be sampled.
    pub fn sample<const N: usize>(
        &mut self,
        declarations: [(K, Motion); N],
        now: Duration,
        reduced_motion: bool,
    ) -> [Frame; N] {
        self.last = self.last.max(now);
        self.batch = self.batch.wrapping_add(1);

        if self.batch == 0 {
            for entry in self.entries.values_mut() {
                entry.batch = 0;
            }

            self.batch = 1;
        }

        declarations.map(|(key, motion)| {
            let entry = self.entries.entry(key).or_default();

            assert_ne!(entry.batch, self.batch, "duplicate key in Motions batch");
            entry.batch = self.batch;

            entry.state.sample(motion, self.last, reduced_motion)
        })
    }

    /// Samples one key, creating its state on first use. Repeated calls for the
    /// same key are allowed; each declaration retargets the same trajectory.
    /// Uses the same clock and interruption contract as [`Self::sample`].
    pub fn sample_one(
        &mut self,
        key: K,
        motion: Motion,
        now: Duration,
        reduced_motion: bool,
    ) -> Frame {
        self.last = self.last.max(now);

        self.entries
            .entry(key)
            .or_default()
            .state
            .sample(motion, self.last, reduced_motion)
    }

    /// Whether this key has retained state, including a completed exit.
    pub fn contains_key(&self, key: &K) -> bool {
        self.entries.contains_key(key)
    }

    /// Immediately cancels and removes a key; returns whether it existed.
    /// Reusing a removed key starts fresh, including its `initial` declaration.
    /// For animated removal, wait until its sampled frame is no longer mounted.
    /// Hash table capacity is preserved.
    pub fn remove(&mut self, key: &K) -> bool {
        self.entries.remove(key).is_some()
    }

    /// Immediately removes keys for which `keep` returns false, preserving capacity.
    /// This does not wait for exits; keep exiting keys until their final sample.
    /// Iteration order is unspecified; visits the table in O(capacity) time.
    pub fn retain(&mut self, mut keep: impl FnMut(&K) -> bool) {
        self.entries.retain(|key, _| keep(key));
    }

    /// Reserves space for at least `additional` new keys beyond the current length.
    pub fn reserve(&mut self, additional: usize) {
        self.entries.reserve(additional);
    }

    /// Releases unused table capacity as far as the standard hash table allows.
    /// Retained trajectories are unchanged. Do this after cleanup, outside a hot
    /// sampling loop; it can allocate and move entries.
    pub fn shrink_to_fit(&mut self) {
        self.entries.shrink_to_fit();
    }
}
