use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::OwnedMutexGuard;

/// Keyed async lock so concurrent requests for the same cache entry perform a
/// single download: the first caller holds the key while it works, later
/// callers wait on the same key and then re-check the cache.
#[derive(Default)]
pub(super) struct SingleFlight {
    locks: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

impl SingleFlight {
    /// Acquire the lock for `key`, waiting for any in-flight holder.
    ///
    /// The map entry is removed when the last interested guard drops, so the
    /// map only grows with concurrently active keys. (A future cancelled while
    /// still waiting inside `acquire` may leave an unlocked entry behind; it is
    /// tiny and reused verbatim by the next request for the same key.)
    pub(super) async fn acquire(&self, key: &str) -> SingleFlightGuard<'_> {
        let lock = self
            .locks
            .lock()
            .entry(key.to_string())
            .or_default()
            .clone();
        let guard = lock.clone().lock_owned().await;
        SingleFlightGuard {
            flights: self,
            key: key.to_string(),
            lock,
            guard: Some(guard),
        }
    }
}

pub(super) struct SingleFlightGuard<'a> {
    flights: &'a SingleFlight,
    key: String,
    lock: Arc<tokio::sync::Mutex<()>>,
    guard: Option<OwnedMutexGuard<()>>,
}

impl Drop for SingleFlightGuard<'_> {
    fn drop(&mut self) {
        let mut locks = self.flights.locks.lock();
        // Release while holding the map lock so no new waiter can clone the
        // Arc between the count check and the removal.
        self.guard = None;
        // 2 = the map entry + `self.lock`; anything more means another task
        // holds a clone (is waiting) and the entry must stay.
        if Arc::strong_count(&self.lock) <= 2 {
            locks.remove(&self.key);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use futures::FutureExt;

    use super::*;

    #[tokio::test]
    async fn same_key_serializes_and_cleans_up() {
        let flights = Arc::new(SingleFlight::default());
        let concurrent = Arc::new(AtomicUsize::new(0));
        let max_concurrent = Arc::new(AtomicUsize::new(0));

        let tasks: Vec<_> = (0..4)
            .map(|_| {
                let flights = Arc::clone(&flights);
                let concurrent = Arc::clone(&concurrent);
                let max_concurrent = Arc::clone(&max_concurrent);
                tokio::spawn(async move {
                    let _guard = flights.acquire("key").await;
                    let now = concurrent.fetch_add(1, Ordering::SeqCst) + 1;
                    max_concurrent.fetch_max(now, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    concurrent.fetch_sub(1, Ordering::SeqCst);
                })
            })
            .collect();

        for task in tasks {
            task.await.unwrap();
        }

        assert_eq!(max_concurrent.load(Ordering::SeqCst), 1);
        assert!(flights.locks.lock().is_empty(), "map entry must be removed");

        // Distinct keys are independent: "b" is free while "a" is held.
        let first = flights.acquire("a").await;
        let second = flights
            .acquire("b")
            .now_or_never()
            .expect("acquiring a distinct key must not wait");
        drop(first);
        drop(second);
        assert!(flights.locks.lock().is_empty());
    }
}
