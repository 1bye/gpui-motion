//! Keyed mount ownership; callers keep each child's animation state alongside its key.
use crate::AnimationError;

/// How new keys enter while removed children finish exiting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PresenceMode {
    /// Enter new children immediately while removed children exit.
    #[default]
    Sync,
    /// Wait for every removed child to exit before mounting new keys.
    Wait,
}

/// A retained key and its requested presence. Exiting keys stay mounted.
#[derive(Clone, Debug)]
pub struct PresenceChild<K> {
    key: K,
    present: bool,
}

impl<K> PresenceChild<K> {
    /// Stable identity used to retrieve the caller's retained child state.
    pub fn key(&self) -> &K {
        &self.key
    }

    /// Feed this into `Motion::present` for the keyed child.
    pub fn is_present(&self) -> bool {
        self.present
    }
}

/// Keyed enter/exit coordination. Allocates only on membership changes, never on sampling.
/// Call `finish_exit` after a child's `Frame::is_mounted` becomes false.
#[derive(Debug)]
pub struct PresenceGroup<K> {
    children: Vec<PresenceChild<K>>,
    pending: Vec<K>,
    mode: PresenceMode,
}

impl<K: Clone + PartialEq> PresenceGroup<K> {
    /// Creates an empty group with explicit overlap/wait behavior.
    pub fn new(mode: PresenceMode) -> Self {
        Self {
            children: Vec::new(),
            pending: Vec::new(),
            mode,
        }
    }

    /// Reconciles desired keys, rejecting duplicate keys without changing the group.
    /// Reintroducing an exiting key cancels its removal and preserves its identity.
    pub fn update(&mut self, keys: impl IntoIterator<Item = K>) -> Result<(), AnimationError> {
        let keys: Vec<_> = keys.into_iter().collect();

        if keys.iter().enumerate().any(|(i, k)| keys[..i].contains(k)) {
            return Err(AnimationError::Index);
        }

        for child in &mut self.children {
            child.present = keys.contains(&child.key);
        }

        let waiting = self.mode == PresenceMode::Wait && self.children.iter().any(|c| !c.present);

        self.pending.clear();

        for key in keys {
            if self.children.iter().any(|c| c.key == key) {
                continue;
            }

            if waiting {
                self.pending.push(key);
            } else {
                self.children.push(PresenceChild { key, present: true });
            }
        }

        Ok(())
    }

    /// Mounted children, including exits; keys retain their original mount order.
    pub fn children(&self) -> &[PresenceChild<K>] {
        &self.children
    }

    /// Unmounts an absent key after its exit. Returns false for unknown or present keys.
    /// The last exit in wait mode mounts the currently requested pending keys.
    pub fn finish_exit(&mut self, key: &K) -> bool {
        let Some(ix) = self
            .children
            .iter()
            .position(|c| &c.key == key && !c.present)
        else {
            return false;
        };

        self.children.remove(ix);

        if self.children.iter().all(|c| c.present) {
            self.children.extend(
                self.pending
                    .drain(..)
                    .map(|key| PresenceChild { key, present: true }),
            );
        }

        true
    }
}
