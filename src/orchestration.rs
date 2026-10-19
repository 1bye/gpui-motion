//! Retained variant inheritance and frame-boundary parent/child sequencing.
use crate::{
    Completion, CompletionStatus, Frame, Motion, MotionState, Target, Transition, Variants,
};

use std::{fmt, time::Duration};

/// Timing relationship between an element and its direct children.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChildrenOrder {
    /// Parent and children begin together.
    #[default]
    Together,
    /// Children begin after the parent's animation completes.
    BeforeChildren,
    /// Parent begins after all child subtrees complete.
    AfterChildren,
}

/// Explicit application-owned defaults. Pass this value through render helpers;
/// it does not install a global provider or override the OS reduced-motion policy.
#[derive(Clone, Copy, Debug, Default)]
pub struct MotionConfig {
    transition: Transition,
    stagger: Duration,
}

impl MotionConfig {
    /// Chooses the transition for descriptions created with `motion`.
    pub fn transition(mut self, transition: Transition) -> Self {
        self.transition = transition;

        self
    }

    /// Chooses the interval between eligible direct children.
    pub fn stagger(mut self, interval: Duration) -> Self {
        self.stagger = interval;

        self
    }

    /// Creates a declaration using these defaults. Per-value overrides still apply.
    pub fn motion(self, target: Target) -> Motion {
        Motion::new(target).transition(self.transition)
    }
}

/// Invalid tree identity or a variant missing from an affected node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeError {
    /// Duplicate key, missing parent, or unknown requested node.
    Identity,
    /// A participating node does not define the requested variant.
    Variant,
    /// Nesting exceeds the bounded 256-level preparation limit.
    Depth,
}

impl fmt::Display for TreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid motion tree operation: {self:?}")
    }
}

impl std::error::Error for TreeError {}

/// One prepared node. Add parents before children, making cycles impossible.
#[derive(Debug)]
pub struct MotionNode<K, V> {
    key: K,
    parent: Option<K>,
    initial: Target,
    variants: Variants<V>,
    order: ChildrenOrder,
    exit_order: Option<ChildrenOrder>,
    inherit: bool,
}

impl<K, V> MotionNode<K, V> {
    /// Creates a node with an explicit initial presentation and variant table.
    pub fn new(key: K, initial: Target, variants: Variants<V>) -> Self {
        Self {
            key,
            parent: None,
            initial,
            variants,
            order: ChildrenOrder::Together,
            exit_order: None,
            inherit: true,
        }
    }

    /// Associates a previously added parent.
    pub fn parent(mut self, parent: K) -> Self {
        self.parent = Some(parent);

        self
    }

    /// Sequences this node relative to its children.
    pub fn when(mut self, order: ChildrenOrder) -> Self {
        self.order = order;

        self
    }

    /// Uses a separate parent/child timing policy when this node exits.
    /// For example, enter before children and leave after their exits finish.
    pub fn exit_when(mut self, order: ChildrenOrder) -> Self {
        self.exit_order = Some(order);

        self
    }

    /// Controls ancestor variant propagation. Explicit `set_node_variant` still works.
    pub fn inherit(mut self, inherit: bool) -> Self {
        self.inherit = inherit;

        self
    }
}

#[derive(Debug)]
struct Node<K, V> {
    declaration: MotionNode<K, V>,
    parent: Option<usize>,
    children: Vec<usize>,
    rank: usize,
    state: MotionState,
    current: Motion,
    desired: Motion,
    pending: Option<Motion>,
    clock: Duration,
    eligible: Option<Duration>,
    running: bool,
    done: bool,
    frame: Frame,
}

/// An explicitly retained tree of independently interruptible motion states.
/// Variant changes validate before mutation and capture the old presentation at
/// the change timestamp. Gated nodes hold their local clock and velocity until
/// eligible. Sequencing starts at the next sampled frame boundary, not through a
/// timer. Preparation allocates; normal sampling does not. Host owns focus policy.
#[derive(Debug)]
pub struct MotionTree<K, V> {
    nodes: Vec<Node<K, V>>,
    config: MotionConfig,
    last: Duration,
    active: bool,
    observers: crate::completion::CompletionSource,
}

impl<K, V> Default for MotionTree<K, V> {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            config: MotionConfig::default(),
            last: Duration::ZERO,
            active: false,
            observers: Default::default(),
        }
    }
}

impl<K: PartialEq, V: PartialEq> MotionTree<K, V> {
    /// Applies explicit defaults to this tree. Each node's `Motion` keeps its own transition.
    pub fn config(mut self, config: MotionConfig) -> Self {
        self.config = config;

        self
    }

    /// Adds a unique node. Parents must already exist; invalid additions are transactional.
    pub fn add(&mut self, declaration: MotionNode<K, V>) -> Result<(), TreeError> {
        if self
            .nodes
            .iter()
            .any(|node| node.declaration.key == declaration.key)
        {
            return Err(TreeError::Identity);
        }

        let parent = declaration
            .parent
            .as_ref()
            .map(|key| self.index(key))
            .transpose()?;

        let mut ancestor = parent;
        let mut depth = 0;

        while let Some(index) = ancestor {
            depth += 1;
            ancestor = self.nodes[index].parent;
        }

        if depth >= 256 {
            return Err(TreeError::Depth);
        }

        let rank = parent.map_or(0, |index| self.nodes[index].children.len());
        let current = Motion::new(declaration.initial).transition(Transition::Instant);
        let mut state = MotionState::default();
        let frame = state.sample(current, Duration::ZERO, false);
        let index = self.nodes.len();

        self.nodes.push(Node {
            declaration,
            parent,
            rank,
            children: Vec::new(),
            state,
            current,
            desired: current,
            pending: None,
            clock: Duration::ZERO,
            eligible: None,
            running: false,
            done: true,
            frame,
        });

        if let Some(parent) = parent {
            self.nodes[parent].children.push(index);
        }

        Ok(())
    }

    fn index(&self, key: &K) -> Result<usize, TreeError> {
        self.nodes
            .iter()
            .position(|node| node.declaration.key == *key)
            .ok_or(TreeError::Identity)
    }

    fn inherits(&self, mut index: usize, root: Option<usize>) -> bool {
        loop {
            if Some(index) == root {
                return true;
            }

            if !self.nodes[index].declaration.inherit {
                return false;
            }

            match self.nodes[index].parent {
                Some(parent) => index = parent,
                None => return root.is_none(),
            }
        }
    }

    /// Propagates a variant through inheriting roots/descendants. A non-inheriting
    /// node blocks propagation through its subtree. New variants interrupt smoothly.
    pub fn set_variant(
        &mut self,
        variant: &V,
        now: Duration,
        reduced: bool,
    ) -> Result<(), TreeError> {
        self.change(None, variant, now, reduced)
    }

    /// Explicitly changes one subtree, including its root even when inheritance is disabled.
    pub fn set_node_variant(
        &mut self,
        key: &K,
        variant: &V,
        now: Duration,
        reduced: bool,
    ) -> Result<(), TreeError> {
        let root = self.index(key)?;

        self.change(Some(root), variant, now, reduced)
    }

    /// Retargets one node from application-computed data while retaining its
    /// parent/child timing policy. Other nodes keep their current declarations.
    /// Returns `Identity` without mutation for an unknown key.
    pub fn set_motion(
        &mut self,
        key: &K,
        motion: Motion,
        now: Duration,
        reduced: bool,
    ) -> Result<(), TreeError> {
        let index = self.index(key)?;

        if self.nodes[index].desired == motion {
            return Ok(());
        }

        self.sample(now, reduced);
        self.observers.resolve(CompletionStatus::Cancelled);
        let node = &mut self.nodes[index];

        node.desired = motion;
        node.pending = Some(motion);
        node.running = false;
        node.done = false;
        node.eligible = None;
        self.active = true;

        Ok(())
    }

    fn change(
        &mut self,
        root: Option<usize>,
        variant: &V,
        now: Duration,
        reduced: bool,
    ) -> Result<(), TreeError> {
        for (index, node) in self.nodes.iter().enumerate() {
            if self.inherits(index, root) && node.declaration.variants.get(variant).is_none() {
                return Err(TreeError::Variant);
            }
        }

        if !self.nodes.iter().enumerate().any(|(index, node)| {
            self.inherits(index, root)
                && node.declaration.variants.get(variant) != Some(node.desired)
        }) {
            return Ok(());
        }

        self.sample(now, reduced);
        self.observers.resolve(CompletionStatus::Cancelled);

        for index in 0..self.nodes.len() {
            if self.inherits(index, root)
                && self.nodes[index].declaration.variants.get(variant)
                    != Some(self.nodes[index].desired)
            {
                let node = &mut self.nodes[index];

                node.pending = node.declaration.variants.get(variant);
                node.desired = node.pending.expect("validated variant");
                node.running = false;
                node.done = false;
                node.eligible = None;
            }
        }

        self.active = self.nodes.iter().any(|node| !node.done);

        Ok(())
    }

    fn own(&mut self, index: usize, allowed: bool, delta: Duration, now: Duration, reduced: bool) {
        let delay = self
            .config
            .stagger
            .saturating_mul(self.nodes[index].rank.min(u32::MAX as usize) as u32);

        let node = &mut self.nodes[index];

        if node.done {
            return;
        }

        if !allowed && !reduced {
            return;
        }

        let due = *node.eligible.get_or_insert(now.saturating_add(delay));

        if now < due && !reduced {
            return;
        }

        if let Some(motion) = node.pending.take() {
            node.current = motion;
        } else if node.running {
            node.clock = node.clock.saturating_add(delta);
        }

        node.frame = node.state.sample(node.current, node.clock, reduced);
        node.running = true;
        node.done = !node.frame.is_animating();
    }

    fn subtree(
        &mut self,
        index: usize,
        allowed: bool,
        delta: Duration,
        now: Duration,
        reduced: bool,
    ) -> bool {
        let node = &self.nodes[index];

        let order = if node.desired.is_present() {
            node.declaration.order
        } else {
            node.declaration
                .exit_order
                .unwrap_or(node.declaration.order)
        };

        if order != ChildrenOrder::AfterChildren {
            self.own(index, allowed, delta, now, reduced);
        }

        let children_allowed =
            allowed && (order != ChildrenOrder::BeforeChildren || self.nodes[index].done);

        let mut children_done = true;

        for i in 0..self.nodes[index].children.len() {
            let child = self.nodes[index].children[i];

            children_done &= self.subtree(child, children_allowed || reduced, delta, now, reduced);
        }

        if order == ChildrenOrder::AfterChildren {
            self.own(index, allowed && children_done, delta, now, reduced);
        }

        self.nodes[index].done && children_done
    }

    /// Advances one monotonic frame. Reduced motion bypasses gates and delays.
    pub fn sample(&mut self, now: Duration, reduced: bool) {
        let now = now.max(self.last);
        let delta = now.saturating_sub(self.last);
        let mut done = true;

        for index in 0..self.nodes.len() {
            if self.nodes[index].parent.is_none() {
                done &= self.subtree(index, true, delta, now, reduced);
            }
        }

        if done && self.active {
            self.observers.resolve(CompletionStatus::Completed);
        }

        self.active = !done;
        self.last = now;
    }

    /// Current node presentation, for ordinary native `Frame` application.
    pub fn get(&self, key: &K) -> Option<Frame> {
        self.index(key).ok().map(|index| self.nodes[index].frame)
    }

    /// Keeps an exiting root mounted until its entire nested subtree exits.
    /// Once false, the host may release that view/entity and restore focus.
    pub fn is_mounted(&self, key: &K) -> bool {
        let Ok(root) = self.index(key) else {
            return false;
        };

        self.nodes.iter().enumerate().any(|(mut index, node)| {
            if !node.frame.is_mounted() {
                return false;
            }

            loop {
                if index == root {
                    return true;
                }

                match self.nodes[index].parent {
                    Some(parent) => index = parent,
                    None => return false,
                }
            }
        })
    }

    /// Whether a trajectory, dependent child, or stagger delay is pending.
    pub fn is_animating(&self) -> bool {
        self.active
    }

    /// Observes the current change; replacing it or dropping the tree cancels.
    pub fn completion(&mut self) -> Completion {
        self.observers.subscribe(!self.active)
    }
}
