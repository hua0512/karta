//! In-process fake OneBot implementation for driving a kovi bot.
//!
//! [`FakeBot::start`] builds a real `kovi::Bot` on a [`kovi::driver::Driver`]
//! that takes events from the test and records every API call, answering
//! with scripted replies (default: success). No sockets, no `kovi.conf.toml`.
//!
//! ```ignore
//! let bot = FakeBot::start(move || {
//!     let options = options.clone();
//!     async move { karta_kovi::register_plugin_with(options).await }
//! })
//! .await;
//! bot.push(group_msg(100, 200, "https://example.com/v/1"));
//! let sent = bot.wait_for_call("send_msg", 1, WAIT).await;
//! ```
//!
//! # Process-global state
//!
//! Each scenario gets a fresh bot, but some state lives in statics:
//! the adapter's reaction-support and forward-variant latches are set by the
//! first answer they see, and kovi's exit-signal watcher is bound to the
//! first tokio runtime that starts a bot. Script those actions the same way
//! in every test of one binary; a scenario that needs a fresh latch (e.g.
//! `set_msg_emoji_like` answering 1404) goes in its own test binary.
//!
//! The bot is never shut down through kovi: kovi's exit path writes
//! `kovi.plugin.toml` / `kovi.conf.toml` into the working directory. Dropping
//! [`FakeBot`] aborts it instead.
//!
//! Both leave kovi tasks behind that panic once their channel closes: a
//! dropped bot's plugin tasks (e.g. a cron job: "The enabled channel
//! closed"), and, after the first runtime's test ends, the exit-signal
//! watcher of every later bot ("The exit signal wait failed"). The panics
//! stay inside those tasks and only show up in a failing test's output.
//!
//! Neither the bot nor kovi creates files: kovi's plugin data dir is only
//! computed (`<cwd>/data/<plugin>`). Code under test that writes there
//! should get a data dir override (or a temp working directory).

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::Stream;
use kovi::ApiReturn;
use kovi::bot::SendApi;
use kovi::config::kovi_conf::KoviConf;
use kovi::driver::{AnyError, ApiHandlerResult, Driver, DriverEvent, MessageEventRegister};
use kovi::event::id::ID;
use serde_json::{Value, json};
use tokio::sync::{Notify, mpsc, oneshot};

/// The bot's own user id (`self_id` in events).
pub const SELF_ID: i64 = 10_000;
/// The main admin configured for the bot.
pub const ADMIN_ID: i64 = 10_001;

/// Plugin name the plugin is mounted under.
pub const PLUGIN_NAME: &str = "karta";

/// One API call the bot made.
#[derive(Clone, Debug)]
pub struct ApiCall {
    pub action: String,
    pub params: Value,
}

/// Scripted answer to an API action.
#[derive(Clone, Debug)]
pub enum ApiReply {
    /// `status: ok`, `retcode: 0`, with `data`.
    Ok(Value),
    /// `status: failed` with `retcode`.
    Failed(i32),
}

impl ApiReply {
    /// The OneBot "action not found" answer (`retcode` 1404).
    pub fn unsupported() -> Self {
        Self::Failed(1404)
    }

    fn to_return(&self) -> Result<ApiReturn, ApiReturn> {
        match self {
            Self::Ok(data) => Ok(ApiReturn {
                status: "ok".to_string(),
                retcode: 0,
                message: None,
                data: data.clone(),
            }),
            Self::Failed(retcode) => Err(ApiReturn {
                status: "failed".to_string(),
                retcode: *retcode,
                message: Some("scripted failure".to_string()),
                data: Value::Null,
            }),
        }
    }
}

#[derive(Default)]
struct Shared {
    calls: Mutex<Vec<ApiCall>>,
    replies: Mutex<HashMap<String, ApiReply>>,
    activity: Notify,
}

struct FakeDriver {
    shared: Arc<Shared>,
    // Held so the event stream never ends: a closed stream makes kovi run
    // its exit path, which writes files into the working directory.
    _events_tx: mpsc::UnboundedSender<DriverEvent>,
    events_rx: Mutex<Option<mpsc::UnboundedReceiver<DriverEvent>>>,
}

#[async_trait::async_trait]
impl Driver for FakeDriver {
    async fn event_channel(
        &self,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<DriverEvent, AnyError>> + Send>>, AnyError> {
        let rx = self
            .events_rx
            .lock()
            .unwrap()
            .take()
            .ok_or("event channel already taken")?;
        Ok(Box::pin(futures::stream::unfold(rx, |mut rx| async move {
            rx.recv().await.map(|event| (Ok(event), rx))
        })))
    }

    fn api_handler(&self, api: SendApi) -> ApiHandlerResult {
        let reply = self
            .shared
            .replies
            .lock()
            .unwrap()
            .get(&api.action)
            .cloned()
            .unwrap_or_else(|| default_reply(&api.action));
        self.shared.calls.lock().unwrap().push(ApiCall {
            action: api.action,
            params: api.params,
        });
        self.shared.activity.notify_waiters();
        Box::pin(async move { Ok(reply.to_return()) })
    }

    fn message_event_register(&self) -> MessageEventRegister {
        MessageEventRegister::register::<kovi_onebot::event::MsgEvent>()
    }
}

static NEXT_MESSAGE_ID: AtomicI32 = AtomicI32::new(1);

fn default_reply(action: &str) -> ApiReply {
    match action {
        "send_msg" | "send_group_msg" | "send_private_msg" => ApiReply::Ok(json!({
            "message_id": NEXT_MESSAGE_ID.fetch_add(1, Ordering::Relaxed)
        })),
        _ => ApiReply::Ok(Value::Null),
    }
}

/// A running kovi bot with one plugin, on a fake OneBot driver.
pub struct FakeBot {
    shared: Arc<Shared>,
    events: mpsc::UnboundedSender<DriverEvent>,
    run: tokio::task::JoinHandle<()>,
}

impl FakeBot {
    /// Build the bot, mount `main` as plugin [`PLUGIN_NAME`], run it, and
    /// return once `main` has finished (its listeners are registered).
    pub async fn start<F, Fut>(main: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let shared = Arc::new(Shared::default());
        let (events, events_rx) = mpsc::unbounded_channel();
        let driver = FakeDriver {
            shared: Arc::clone(&shared),
            _events_tx: events.clone(),
            events_rx: Mutex::new(Some(events_rx)),
        };

        let (ready_tx, ready_rx) = oneshot::channel::<()>();
        let ready_tx = Arc::new(Mutex::new(Some(ready_tx)));
        let main = Arc::new(main);
        let plugin_main: Arc<dyn Fn() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync> =
            Arc::new(move || {
                let main = Arc::clone(&main);
                let ready_tx = Arc::clone(&ready_tx);
                Box::pin(async move {
                    main().await;
                    if let Some(tx) = ready_tx.lock().unwrap().take() {
                        let _ = tx.send(());
                    }
                })
            });

        let mut bot = kovi::Bot::build(KoviConf::new(ID::new(ADMIN_ID), None, false), driver);
        bot.mount_plugin(kovi::plugin::Plugin::new(PLUGIN_NAME, "0", plugin_main));
        let run = tokio::spawn(async move {
            bot.run().await;
        });

        tokio::time::timeout(crate::WAIT, ready_rx)
            .await
            .expect("plugin main did not finish in time")
            .expect("plugin main dropped before finishing");

        Self {
            shared,
            events,
            run,
        }
    }

    /// Deliver an event (OneBot JSON) to the bot.
    pub fn push(&self, event: Value) {
        self.events
            .send(DriverEvent::Normal(event))
            .expect("bot event channel closed");
    }

    /// Answer `action` with `reply` from now on.
    pub fn script(&self, action: &str, reply: ApiReply) {
        self.shared
            .replies
            .lock()
            .unwrap()
            .insert(action.to_string(), reply);
    }

    /// Every API call so far, in order.
    pub fn calls(&self) -> Vec<ApiCall> {
        self.shared.calls.lock().unwrap().clone()
    }

    /// Calls of `action`, in order.
    pub fn calls_of(&self, action: &str) -> Vec<ApiCall> {
        self.calls()
            .into_iter()
            .filter(|call| call.action == action)
            .collect()
    }

    /// Wait until `action` was called `n` times; returns the n-th call.
    /// Panics after `timeout`, which only bounds hangs.
    pub async fn wait_for_call(&self, action: &str, n: usize, timeout: Duration) -> ApiCall {
        self.wait_until(timeout, |calls| {
            calls
                .iter()
                .filter(|call| call.action == action)
                .nth(n - 1)
                .cloned()
        })
        .await
    }

    /// Wait until `check` returns `Some` for the API calls so far (in
    /// order); re-checked after every call. Panics after `timeout`, which
    /// only bounds hangs.
    pub async fn wait_until<T>(
        &self,
        timeout: Duration,
        mut check: impl FnMut(&[ApiCall]) -> Option<T>,
    ) -> T {
        let wait = async {
            loop {
                let notified = self.shared.activity.notified();
                if let Some(found) = check(&self.calls()) {
                    return found;
                }
                notified.await;
            }
        };
        match tokio::time::timeout(timeout, wait).await {
            Ok(found) => found,
            Err(_) => panic!(
                "condition not met within {timeout:?}; calls: {:#?}",
                self.calls()
            ),
        }
    }
}

impl Drop for FakeBot {
    fn drop(&mut self) {
        self.run.abort();
    }
}

static NEXT_EVENT_MESSAGE_ID: AtomicI32 = AtomicI32::new(1_000);

fn message_event(message_type: &str, user_id: i64, text: &str) -> Value {
    json!({
        "time": 1_700_000_000,
        "self_id": SELF_ID,
        "post_type": "message",
        "message_type": message_type,
        "sub_type": if message_type == "group" { "normal" } else { "friend" },
        "message_id": NEXT_EVENT_MESSAGE_ID.fetch_add(1, Ordering::Relaxed),
        "user_id": user_id,
        "message": [{ "type": "text", "data": { "text": text } }],
        "raw_message": text,
        "font": 0,
        "sender": { "user_id": user_id, "nickname": format!("user{user_id}") },
    })
}

/// A group text message event (fresh `message_id`).
pub fn group_msg(group_id: i64, user_id: i64, text: &str) -> Value {
    let mut event = message_event("group", user_id, text);
    event["group_id"] = json!(group_id);
    event
}

/// A private text message event (fresh `message_id`).
pub fn private_msg(user_id: i64, text: &str) -> Value {
    message_event("private", user_id, text)
}
