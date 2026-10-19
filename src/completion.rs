//! Optional completion futures. Only callers requesting a future allocate.
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, Weak},
    task::{Context, Poll, Waker},
};

/// Result of a trajectory observed by a completion future.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionStatus {
    /// The trajectory reached its endpoint, including reduced-motion completion.
    Completed,
    /// It was replaced, explicitly cancelled, restarted, or its owner was dropped.
    Cancelled,
}

#[derive(Debug, Default)]
struct ResultState {
    result: Option<CompletionStatus>,
    waker: Option<Waker>,
}

/// A single observer of playback completion. Polling never starts a timer or
/// executor; the retained player must continue being sampled by its host.
#[derive(Debug)]
pub struct Completion(Arc<Mutex<ResultState>>);

impl Future for Completion {
    type Output = CompletionStatus;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());

        if let Some(result) = state.result {
            return Poll::Ready(result);
        }

        if !state
            .waker
            .as_ref()
            .is_some_and(|waker| waker.will_wake(cx.waker()))
        {
            state.waker = Some(cx.waker().clone());
        }

        Poll::Pending
    }
}

#[derive(Debug, Default)]
pub(crate) struct CompletionSource {
    observers: Vec<Weak<Mutex<ResultState>>>,
}

impl CompletionSource {
    pub(crate) fn subscribe(&mut self, complete: bool) -> Completion {
        self.observers
            .retain(|observer| observer.strong_count() > 0);
        let result = complete.then_some(CompletionStatus::Completed);

        let state = Arc::new(Mutex::new(ResultState {
            result,
            waker: None,
        }));

        if !complete {
            self.observers.push(Arc::downgrade(&state));
        }

        Completion(state)
    }

    pub(crate) fn resolve(&mut self, result: CompletionStatus) {
        for observer in self.observers.drain(..) {
            if let Some(observer) = observer.upgrade() {
                let waker = {
                    let mut state = observer.lock().unwrap_or_else(|error| error.into_inner());

                    state.result = Some(result);

                    state.waker.take()
                };

                if let Some(waker) = waker {
                    waker.wake();
                }
            }
        }
    }
}

impl Drop for CompletionSource {
    fn drop(&mut self) {
        self.resolve(CompletionStatus::Cancelled);
    }
}
