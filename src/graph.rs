//! Explicitly owned, batched motion-value dependencies and subscriptions.
use crate::{MotionValue, Property, Target, Transition};

use std::{
    cell::Cell,
    fmt,
    rc::{Rc, Weak},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

static NEXT_GRAPH: AtomicU64 = AtomicU64::new(1);

const START: u8 = 1;
const CANCEL: u8 = 2;
const CHANGE: u8 = 4;
const COMPLETE: u8 = 8;
const RESET_VELOCITY: u8 = 16;

/// Identity of a value inside one graph. Values from different graphs cannot mix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ValueId {
    owner: u64,
    index: usize,
}

/// A graph operation referred to another owner or attempted to edit a derived value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphError {
    /// Value belongs to another graph or does not exist.
    Identity,
    /// Derived values are read-only; edit their sources instead.
    Derived,
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid motion graph operation: {self:?}")
    }
}

impl std::error::Error for GraphError {}

/// Coalesced lifecycle event delivered after a graph's entire dependency pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotionEventKind {
    /// A new trajectory started.
    Start,
    /// A previous trajectory was interrupted or explicitly jumped.
    Cancel,
    /// The sampled presentation changed.
    Change,
    /// A running trajectory settled.
    Complete,
}

/// One sampled graph event. Velocity uses value units per second.
#[derive(Clone, Copy, Debug)]
pub struct MotionEvent {
    /// Changed source or derived value.
    pub id: ValueId,
    /// Lifecycle phase.
    pub kind: MotionEventKind,
    /// Current presentation.
    pub value: f64,
    /// Current velocity.
    pub velocity: f64,
}

/// Retain this handle while listening. Dropping it disables the callback immediately.
#[derive(Debug)]
pub struct GraphSubscription(Rc<Cell<bool>>);

impl Drop for GraphSubscription {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

impl GraphSubscription {
    /// Cancels without borrowing the graph, including from inside a callback.
    pub fn cancel(&self) {
        self.0.set(false);
    }
}

struct Listener {
    id: ValueId,
    alive: Weak<Cell<bool>>,
    callback: Box<dyn FnMut(MotionEvent)>,
}

type Mapper = Box<dyn Fn(&[f64]) -> f64>;

enum Node {
    Source(MotionValue),
    Derived {
        inputs: Vec<ValueId>,
        values: Vec<f64>,
        map: Mapper,
    },
}

struct Value {
    node: Node,
    value: f64,
    velocity: f64,
    active: bool,
    events: u8,
}

/// Acyclic, insertion-ordered motion graph. Sources must precede their dependants.
/// Setup allocates; sampling reuses nodes, scratch storage and listeners. Callbacks
/// run after all derived values update. Defer edits to this graph until callbacks
/// return; they may update other application state or cancel their subscriptions.
///
/// ```
/// use gpui_ce_motion::{MotionGraph, Transition};
/// use std::time::Duration;
/// let mut graph = MotionGraph::default();
/// let x = graph.source(0.0);
/// let opacity = graph.derive(&[x], |v| (1.0 - v[0] / 200.0).clamp(0.0, 1.0)).unwrap();
/// graph.animate(x, 100.0, Transition::Instant, Duration::ZERO).unwrap();
/// graph.sample(Duration::ZERO, false);
/// assert_eq!(graph.get(opacity).unwrap(), 0.5);
/// ```
pub struct MotionGraph {
    owner: u64,
    values: Vec<Value>,
    listeners: Vec<Listener>,
    last: Duration,
    active: bool,
}

impl fmt::Debug for MotionGraph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MotionGraph")
            .field("values", &self.values.len())
            .field("active", &self.active)
            .finish()
    }
}

impl Default for MotionGraph {
    fn default() -> Self {
        Self {
            owner: NEXT_GRAPH.fetch_add(1, Ordering::Relaxed),
            values: Vec::new(),
            listeners: Vec::new(),
            last: Duration::ZERO,
            active: false,
        }
    }
}

impl MotionGraph {
    /// Adds a finite scalar source. Panics for non-finite values.
    pub fn source(&mut self, value: f64) -> ValueId {
        let node = MotionValue::new(value);

        let id = ValueId {
            owner: self.owner,
            index: self.values.len(),
        };

        self.values.push(Value {
            node: Node::Source(node),
            value,
            velocity: 0.0,
            active: false,
            events: 0,
        });

        id
    }

    /// Adds an automatically updated, multi-input mapping. An empty input list is
    /// a constant. The mapper must return finite values and must not edit this graph.
    ///
    /// # Errors
    /// Returns `Identity` for inputs from another graph.
    /// # Panics
    /// Panics if the mapping returns a non-finite value, during setup or sampling.
    pub fn derive(
        &mut self,
        inputs: &[ValueId],
        map: impl Fn(&[f64]) -> f64 + 'static,
    ) -> Result<ValueId, GraphError> {
        let values = inputs
            .iter()
            .map(|&id| self.get(id))
            .collect::<Result<Vec<_>, _>>()?;

        let value = map(&values);

        assert!(
            value.is_finite(),
            "graph mapping must return a finite value"
        );
        let id = ValueId {
            owner: self.owner,
            index: self.values.len(),
        };

        self.values.push(Value {
            node: Node::Derived {
                inputs: inputs.to_vec(),
                values,
                map: Box::new(map),
            },
            value,
            velocity: 0.0,
            active: false,
            events: 0,
        });

        Ok(id)
    }

    fn index(&self, id: ValueId) -> Result<usize, GraphError> {
        if id.owner != self.owner || id.index >= self.values.len() {
            return Err(GraphError::Identity);
        }

        Ok(id.index)
    }

    /// Last sampled presentation. Does not advance the graph.
    pub fn get(&self, id: ValueId) -> Result<f64, GraphError> {
        Ok(self.values[self.index(id)?].value)
    }

    /// Last sampled velocity, including finite-difference derived velocities.
    pub fn velocity(&self, id: ValueId) -> Result<f64, GraphError> {
        Ok(self.values[self.index(id)?].velocity)
    }

    /// Retargets a source with the shared spring/tween solver. Identical declarations
    /// do not produce new start/cancel events. Panics for non-finite destinations.
    pub fn animate(
        &mut self,
        id: ValueId,
        target: f64,
        transition: Transition,
        now: Duration,
    ) -> Result<(), GraphError> {
        let i = self.index(id)?;
        let value = &mut self.values[i];

        let Node::Source(source) = &mut value.node else {
            return Err(GraphError::Derived);
        };

        let changed = !source.matches(target, transition);
        let was_active = source.active();

        source.animate(target, transition, now.max(self.last));

        if changed && was_active {
            value.events |= CANCEL;
        }

        if changed && source.active() {
            value.events |= START;
        }

        Ok(())
    }

    /// Directly edits a source, with optional retained pointer velocity. This is
    /// batched: dependants and listeners update on the next `sample`.
    pub fn set(&mut self, id: ValueId, value: f64, now: Duration) -> Result<(), GraphError> {
        self.edit(id, value, now, false)
    }

    /// Cancels a source trajectory and clears momentum on the next sample.
    pub fn jump(&mut self, id: ValueId, value: f64, now: Duration) -> Result<(), GraphError> {
        self.edit(id, value, now, true)
    }

    fn edit(
        &mut self,
        id: ValueId,
        target: f64,
        now: Duration,
        jump: bool,
    ) -> Result<(), GraphError> {
        let i = self.index(id)?;
        let value = &mut self.values[i];

        let Node::Source(source) = &mut value.node else {
            return Err(GraphError::Derived);
        };

        if source.active() {
            value.events |= CANCEL;
        }

        if jump {
            value.events = (value.events & !START) | RESET_VELOCITY;
            source.jump(target, now.max(self.last));
        } else {
            source.set(target, now.max(self.last));
        }

        Ok(())
    }

    /// Listens to sampled changes and lifecycle events. Store the returned handle.
    pub fn subscribe(
        &mut self,
        id: ValueId,
        callback: impl FnMut(MotionEvent) + 'static,
    ) -> Result<GraphSubscription, GraphError> {
        self.index(id)?;
        let alive = Rc::new(Cell::new(true));

        self.listeners.push(Listener {
            id,
            alive: Rc::downgrade(&alive),
            callback: Box::new(callback),
        });

        Ok(GraphSubscription(alive))
    }

    /// Advances sources, then derived dependencies, then dispatches listeners.
    /// Multiple writes coalesce to one change event per value and sample. Backward
    /// timestamps hold. Reduced motion settles sources using the shared policy.
    pub fn sample(&mut self, now: Duration, reduced_motion: bool) {
        let now = now.max(self.last);
        let dt = now.saturating_sub(self.last).as_secs_f64();

        self.active = false;

        for index in 0..self.values.len() {
            let (before, rest) = self.values.split_at_mut(index);
            let value = &mut rest[0];

            let (next, velocity, active, completed) = match &mut value.node {
                Node::Source(source) => {
                    let frame = source.sample(now, reduced_motion);

                    (
                        frame.value(),
                        frame.velocity(),
                        frame.is_animating(),
                        frame.just_completed(),
                    )
                }
                Node::Derived {
                    inputs,
                    values,
                    map,
                } => {
                    let active = inputs.iter().any(|id| before[id.index].active);

                    let propagated = inputs.iter().fold(0, |events, id| {
                        events | (before[id.index].events & (START | CANCEL | RESET_VELOCITY))
                    });

                    value.events |= propagated;

                    for (id, slot) in inputs.iter().zip(values.iter_mut()) {
                        *slot = before[id.index].value;
                    }

                    let next = map(values);

                    assert!(next.is_finite(), "graph mapping must return a finite value");
                    let velocity = if propagated & RESET_VELOCITY != 0 {
                        0.0
                    } else if dt > 0.0 {
                        crate::value::finite((next - value.value) / dt)
                    } else {
                        value.velocity
                    };

                    (
                        next,
                        velocity,
                        active,
                        !active
                            && inputs
                                .iter()
                                .any(|id| before[id.index].events & COMPLETE != 0),
                    )
                }
            };

            if !value.active && active {
                value.events |= START;
            }

            if value.value != next {
                value.events |= CHANGE;
            }

            if completed {
                value.events |= COMPLETE;
            }

            value.value = next;
            value.velocity = velocity;
            value.active = active;
            self.active |= active;
        }

        self.last = now;
        self.listeners.retain_mut(|listener| {
            let Some(alive) = listener.alive.upgrade().filter(|alive| alive.get()) else {
                return false;
            };

            let value = &self.values[listener.id.index];

            for (flag, kind) in [
                (CANCEL, MotionEventKind::Cancel),
                (START, MotionEventKind::Start),
                (CHANGE, MotionEventKind::Change),
                (COMPLETE, MotionEventKind::Complete),
            ] {
                if value.events & flag != 0 && alive.get() {
                    (listener.callback)(MotionEvent {
                        id: listener.id,
                        kind,
                        value: value.value,
                        velocity: value.velocity,
                    });
                }
            }

            alive.get()
        });

        for value in &mut self.values {
            value.events = 0;
        }
    }

    /// Whether any source or dependant requires another frame.
    pub fn is_animating(&self) -> bool {
        self.active
    }

    /// Binds sampled values to declared target properties. Values are saturated
    /// to drawable bounds; opacity is clamped and dimensions are non-negative.
    pub fn target(
        &self,
        base: Target,
        bindings: &[(Property, ValueId)],
    ) -> Result<Target, GraphError> {
        let mut values = base.values();
        let mut mask = base.mask();

        for &(property, id) in bindings {
            let i = property as usize;
            let value = self.get(id)?;

            values[i] = if i == 2 {
                value.clamp(0.0, 1.0)
            } else if i >= 3 {
                value.clamp(0.0, f32::MAX.into())
            } else {
                value.clamp(f32::MIN.into(), f32::MAX.into())
            };
            mask |= 1 << i;
        }

        Ok(Target::from_values(values, mask))
    }
}

impl MotionGraph {
    /// Adds the numeric channels of a typed value, allocating only during setup.
    /// Panics for invalid channels as `source` does.
    pub fn typed_source<T: crate::MotionData<N>, const N: usize>(
        &mut self,
        value: T,
    ) -> [ValueId; N] {
        let values = value.channels();

        assert!(
            N > 0 && values.iter().all(|v| v.is_finite()),
            "invalid typed source"
        );

        std::array::from_fn(|i| self.source(values[i]))
    }

    /// Reconstructs a typed value from automatically sampled/derived channels.
    pub fn typed_value<T: crate::MotionData<N>, const N: usize>(
        &self,
        template: T,
        ids: [ValueId; N],
    ) -> Result<T, GraphError> {
        for id in ids {
            self.index(id)?;
        }

        Ok(template.with_channels(std::array::from_fn(|i| self.values[ids[i].index].value)))
    }

    /// Retargets typed source channels transactionally. Invalid or derived IDs
    /// leave all sources unchanged; values/velocities use the shared solver.
    pub fn animate_typed<T: crate::MotionData<N>, const N: usize>(
        &mut self,
        ids: [ValueId; N],
        target: T,
        transition: Transition,
        now: Duration,
    ) -> Result<(), GraphError> {
        let values = target.channels();

        assert!(
            values.iter().all(|v| v.is_finite()),
            "invalid typed destination"
        );

        for id in ids {
            if !matches!(self.values[self.index(id)?].node, Node::Source(_)) {
                return Err(GraphError::Derived);
            }
        }

        for i in 0..N {
            self.animate(ids[i], values[i], transition, now)?;
        }

        Ok(())
    }
}
