use std::collections::HashMap;
use std::sync::Arc;

use karta_core::message::RenderOutput;
use parking_lot::Mutex;
use tokio::sync::{Semaphore, watch};

/// Outcome of one parse+render flight, cloned to every waiting message.
#[derive(Clone)]
pub(crate) enum FlightOutcome {
    /// A pattern pre-matched but the full parse found nothing to send.
    NoResult,
    Rendered(Arc<Vec<RenderOutput>>),
    /// Parse or render failed; the display string is shared by all waiters.
    Failed(Arc<str>),
}

/// Global parse-pipeline guards: bounded concurrency plus per-URL single-flight.
pub(crate) struct ParseGate {
    /// `None` when `max_concurrent_parses == 0` (unlimited).
    semaphore: Option<Semaphore>,
    /// In-flight work keyed by `cache_key(text)`; later identical messages
    /// await the same result instead of re-running the parse+render.
    in_flight: Mutex<HashMap<String, watch::Receiver<Option<FlightOutcome>>>>,
}

impl ParseGate {
    pub(crate) fn new(max_concurrent: usize) -> Self {
        Self {
            // 0 = unlimited (documented in the README); anything larger is
            // clamped to tokio's permit ceiling so `Semaphore::new` (which
            // panics above `MAX_PERMITS`) cannot take the plugin down.
            semaphore: (max_concurrent > 0)
                .then(|| Semaphore::new(max_concurrent.min(Semaphore::MAX_PERMITS))),
            in_flight: Mutex::new(HashMap::new()),
        }
    }

    /// Run `work` under the concurrency limit, deduplicated by `key`: the
    /// first caller executes it while later callers await the same outcome.
    pub(crate) async fn run(
        &self,
        key: String,
        work: impl Future<Output = FlightOutcome>,
    ) -> FlightOutcome {
        let flight = {
            let mut map = self.in_flight.lock();
            match map.get(&key) {
                Some(rx) => Err(rx.clone()),
                None => {
                    let (tx, rx) = watch::channel(None);
                    map.insert(key.clone(), rx);
                    Ok(tx)
                }
            }
        };

        match flight {
            Ok(tx) => {
                // Remove the in-flight entry on every exit path (including
                // cancellation) so a dead flight can never trap later messages.
                struct FlightGuard<'a> {
                    gate: &'a ParseGate,
                    key: &'a str,
                }
                impl Drop for FlightGuard<'_> {
                    fn drop(&mut self) {
                        self.gate.in_flight.lock().remove(self.key);
                    }
                }
                let _guard = FlightGuard {
                    gate: self,
                    key: &key,
                };

                let _permit = match &self.semaphore {
                    Some(sem) => sem.acquire().await.ok(),
                    None => None,
                };
                let outcome = work.await;
                let _ = tx.send(Some(outcome.clone()));
                outcome
            }
            Err(mut rx) => match rx.wait_for(|slot| slot.is_some()).await {
                Ok(slot) => slot.clone().expect("wait_for guarantees Some"),
                // The leading task was cancelled before publishing a result.
                Err(_) => FlightOutcome::Failed(Arc::from("parse task cancelled")),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::pin::{Pin, pin};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::task::Poll;
    use tokio::sync::Notify;

    /// Poll `fut` exactly once; `Some` if it completed.
    async fn poll_once<F: Future + Unpin>(fut: &mut F) -> Option<F::Output> {
        std::future::poll_fn(|cx| {
            Poll::Ready(match Pin::new(&mut *fut).poll(cx) {
                Poll::Ready(output) => Some(output),
                Poll::Pending => None,
            })
        })
        .await
    }

    #[tokio::test]
    async fn cancelled_leader_fails_followers_and_frees_the_key() {
        let gate = Arc::new(ParseGate::new(8));
        let started = Arc::new(Notify::new());

        let leader = {
            let gate = gate.clone();
            let started = started.clone();
            tokio::spawn(async move {
                gate.run("key".to_string(), async {
                    started.notify_one();
                    // Never completes; the task gets aborted.
                    std::future::pending::<FlightOutcome>().await
                })
                .await
            })
        };

        started.notified().await;
        let mut follower = pin!(gate.run("key".to_string(), async { FlightOutcome::NoResult }));
        assert!(poll_once(&mut follower).await.is_none());

        leader.abort();
        assert!(matches!(follower.await, FlightOutcome::Failed(_)));

        // The key is free again: a new caller becomes a fresh leader.
        let outcome = gate
            .run("key".to_string(), async { FlightOutcome::NoResult })
            .await;
        assert!(matches!(outcome, FlightOutcome::NoResult));
    }

    #[tokio::test]
    async fn semaphore_limits_concurrent_leaders() {
        let gate = Arc::new(ParseGate::new(1));
        let first_started = Arc::new(Notify::new());
        let release_first = Arc::new(Notify::new());

        let first = {
            let gate = gate.clone();
            let first_started = first_started.clone();
            let release_first = release_first.clone();
            tokio::spawn(async move {
                gate.run("one".to_string(), async {
                    first_started.notify_one();
                    release_first.notified().await;
                    FlightOutcome::NoResult
                })
                .await
            })
        };
        first_started.notified().await;

        // The only permit is taken: the second flight's work must not start.
        let second_started = AtomicBool::new(false);
        let mut second = pin!(gate.run("two".to_string(), async {
            second_started.store(true, Ordering::SeqCst);
            FlightOutcome::NoResult
        }));
        assert!(poll_once(&mut second).await.is_none());
        assert!(
            !second_started.load(Ordering::SeqCst),
            "second flight must wait for the permit"
        );

        release_first.notify_one();
        first.await.expect("first");
        second.await;
        assert!(second_started.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn gate_capacity_edges_still_run_work() {
        // 0 means unlimited; an absurd value must clamp instead of making
        // `Semaphore::new` (which panics above MAX_PERMITS) abort startup.
        for capacity in [0, usize::MAX] {
            let gate = ParseGate::new(capacity);
            let mut run = pin!(gate.run("key".to_string(), async { FlightOutcome::NoResult }));
            assert!(
                matches!(poll_once(&mut run).await, Some(FlightOutcome::NoResult)),
                "capacity {capacity}"
            );
        }
    }
}
