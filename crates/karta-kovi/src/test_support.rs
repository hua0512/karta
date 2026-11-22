//! Helpers shared by the unit tests.

use std::sync::{Arc, Mutex};

use kovi::ApiReturn;
use kovi::bot::SendApi;
use kovi::bot::runtimebot::CanSendApi;
use kovi::types::ApiAndOptOneshot;
use tokio::sync::mpsc;

/// One request a [`MockApi`] received.
#[derive(Clone, Debug)]
pub(crate) struct ApiRequest {
    pub action: String,
    pub params: serde_json::Value,
    /// The caller waits for the answer (`send_api_return`), as opposed to
    /// fire-and-forget (`send_api`).
    pub awaited: bool,
}

pub(crate) type RequestLog = Arc<Mutex<Vec<ApiRequest>>>;

/// A [`CanSendApi`] whose requests are answered by a closure.
pub(crate) struct MockApi {
    tx: mpsc::Sender<ApiAndOptOneshot>,
}

impl CanSendApi for MockApi {
    fn __get_api_tx(&self) -> &mpsc::Sender<ApiAndOptOneshot> {
        &self.tx
    }
}

/// Record every request and answer awaited ones with `respond`: `Ok(())`
/// is success, `Err(retcode)` a failure with that retcode.
///
/// Requests are recorded on a spawned task; fire-and-forget ones show up in
/// the log after the caller yields once.
pub(crate) fn mock_api(
    respond: impl Fn(&SendApi) -> Result<(), i32> + Send + 'static,
) -> (MockApi, RequestLog) {
    let (tx, mut rx) = mpsc::channel::<ApiAndOptOneshot>(64);
    let log = RequestLog::default();
    let log_bg = log.clone();
    tokio::spawn(async move {
        while let Some((api, reply)) = rx.recv().await {
            log_bg.lock().unwrap().push(ApiRequest {
                action: api.action.clone(),
                params: api.params.clone(),
                awaited: reply.is_some(),
            });
            if let Some(reply) = reply {
                let result = respond(&api);
                let ret = ApiReturn {
                    status: if result.is_ok() { "ok" } else { "failed" }.to_string(),
                    retcode: result.err().unwrap_or(0),
                    message: None,
                    data: serde_json::Value::Null,
                };
                let _ = reply.send(if result.is_ok() { Ok(ret) } else { Err(ret) });
            }
        }
    });
    (MockApi { tx }, log)
}

/// The actions in `log`, in order.
pub(crate) fn actions(log: &RequestLog) -> Vec<String> {
    log.lock()
        .unwrap()
        .iter()
        .map(|request| request.action.clone())
        .collect()
}
