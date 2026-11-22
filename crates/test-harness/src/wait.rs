//! Bounded waiting for asynchronous effects.
//!
//! Timeouts here only bound hangs: a passing test never waits for them, so
//! they are generous and never encode a correctness expectation.

use std::future::Future;
use std::time::Duration;

/// Default bound for waits in tests.
pub const WAIT: Duration = Duration::from_secs(10);

const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Poll `cond` until it returns `true`; panics after `timeout`.
#[track_caller]
pub fn eventually<F, Fut>(timeout: Duration, mut cond: F) -> impl Future<Output = ()>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = bool>,
{
    let caller = std::panic::Location::caller();
    async move {
        let poll = async {
            while !cond().await {
                tokio::time::sleep(POLL_INTERVAL).await;
            }
        };
        if tokio::time::timeout(timeout, poll).await.is_err() {
            panic!("condition not met within {timeout:?} (waiting at {caller})");
        }
    }
}
