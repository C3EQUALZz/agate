//! Background-task supervision: one cancellation token plus one task tracker,
//! so every long-running task the server spawns (the audit outbox, the
//! checkpoint scheduler, the CEL hot-reload handler) is started, signalled to
//! stop, and awaited through a single, uniform handle instead of a per-task mix
//! of `Notify` signals and detached `JoinHandle`s.
//!
//! On shutdown the caller [`trigger`](Supervisor::trigger)s the token — each
//! task selects on [`token`](Supervisor::token)`.cancelled()` and returns at its
//! next boundary — then [`wait`](Supervisor::wait)s for them all to finish.

use std::future::Future;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

/// Supervises the server's background tasks: a shared cancellation token they
/// watch for shutdown, and a tracker that lets the caller await them as a group.
///
/// The token is a *cooperative* stop signal — a task observes `cancelled()` at a
/// boundary and returns, so it never dies mid-operation (e.g. abandoning a
/// half-open audit scope). A task whose own completion is driven by something
/// else (the outbox drains until its channel closes) is still spawned here so it
/// is awaited on shutdown; it simply ignores the token.
#[derive(Clone, Default)]
pub struct Supervisor {
    cancel: CancellationToken,
    tasks: TaskTracker,
}

impl Supervisor {
    /// A fresh supervisor with an un-cancelled token and no tracked tasks.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A clone of the shutdown token. Long-running tasks select on
    /// [`CancellationToken::cancelled`] to return promptly when shutdown begins.
    #[must_use]
    pub fn token(&self) -> CancellationToken {
        self.cancel.clone()
    }

    /// Spawn a supervised background task. It is tracked, so [`wait`](Self::wait)
    /// will not return until it finishes. The handle is rarely needed (shutdown
    /// goes through the token and `wait`), so callers usually drop it.
    pub fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.tasks.spawn(future)
    }

    /// Trip the cancellation token, asking every supervised task to stop. Returns
    /// immediately; pair with [`wait`](Self::wait) to block until they have.
    pub fn trigger(&self) {
        self.cancel.cancel();
    }

    /// Stop accepting new tasks and wait for every supervised task to finish.
    /// Call after the listener has stopped (so the outbox channel has closed and
    /// its task can drain the remaining records) and after [`trigger`](Self::trigger).
    pub async fn wait(&self) {
        self.tasks.close();
        self.tasks.wait().await;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::Supervisor;

    #[tokio::test]
    async fn trigger_stops_a_token_watching_task_and_wait_joins_it() {
        let supervisor = Supervisor::new();
        let stopped = Arc::new(AtomicBool::new(false));
        let token = supervisor.token();
        let flag = stopped.clone();
        supervisor.spawn(async move {
            token.cancelled().await;
            flag.store(true, Ordering::SeqCst);
        });

        supervisor.trigger();
        // Returns only once the task observed the cancellation and exited; a hang
        // here would fail the test, proving the cooperative stop joins cleanly.
        supervisor.wait().await;
        assert!(stopped.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn wait_awaits_a_task_that_finishes_on_its_own() {
        // A task driven to completion by something other than the token (like the
        // outbox draining its channel) is still awaited by `wait`.
        let supervisor = Supervisor::new();
        let done = Arc::new(AtomicBool::new(false));
        let flag = done.clone();
        supervisor.spawn(async move {
            flag.store(true, Ordering::SeqCst);
        });

        supervisor.wait().await;
        assert!(done.load(Ordering::SeqCst));
    }
}
