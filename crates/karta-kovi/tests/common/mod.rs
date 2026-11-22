//! Shared setup for the adapter's end-to-end tests: the real plugin
//! (`register_plugin_with`) on the harness's fake OneBot driver, with
//! [`FakeParser`]s instead of the platform parsers.
#![allow(dead_code)]

use std::sync::Arc;

use karta_core::parser::Parser;
use karta_core::{ParseResult, Platform};
use karta_kovi::PluginOptions;
use serde_json::{Value, json};
use test_harness::onebot::{ADMIN_ID, ApiCall, FakeBot};
use test_harness::{FakeParser, TempApp, WAIT};

/// A bilibili link the [`bili_parser`] patterns match.
pub const BILI_LINK: &str = "https://www.bilibili.com/video/BV1xx411c7mD";
/// A second, different bilibili link.
pub const OTHER_BILI_LINK: &str = "https://www.bilibili.com/video/BV1yy411c7mE";

pub const REACTION_ACK: &str = "424";
pub const REACTION_SUCCESS: &str = "144";
pub const REACTION_FAILURE: &str = "10060";

/// A bilibili parser matching `BV` ids.
pub fn bili_parser() -> FakeParser {
    FakeParser::new(Platform::Bilibili, &[("bv", r"BV[0-9A-Za-z]{10}")])
}

pub fn bili_result(title: &str) -> ParseResult {
    ParseResult::new(Platform::Bilibili).with_title(title)
}

/// The plugin running on a fake bot, over a temp data dir.
pub struct Adapter {
    pub bot: FakeBot,
    pub app: TempApp,
}

/// Start the plugin with `overrides` on top of a text-only, no-download
/// config, and `parsers` as the only parsers.
pub async fn start(overrides: Value, parsers: Vec<Arc<dyn Parser>>) -> Adapter {
    let mut config = json!({ "render_type": "text", "download_media": false });
    config
        .as_object_mut()
        .unwrap()
        .extend(overrides.as_object().expect("overrides object").clone());
    let app = TempApp::new(config);
    let bot = start_on(&app, parsers).await;
    Adapter { bot, app }
}

/// Start the plugin on an existing data dir (e.g. to restart it).
pub async fn start_on(app: &TempApp, parsers: Vec<Arc<dyn Parser>>) -> FakeBot {
    let options = PluginOptions {
        data_path: Some(app.data_path().to_path_buf()),
        parsers: Some(parsers),
        ..PluginOptions::default()
    };
    FakeBot::start(move || {
        let options = options.clone();
        async move { karta_kovi::register_plugin_with(options).await }
    })
    .await
}

pub fn message_id(event: &Value) -> i64 {
    event["message_id"].as_i64().expect("event message_id")
}

/// Whether `call` is a `set_msg_emoji_like` on `message_id` with `emoji`.
pub fn is_reaction(call: &ApiCall, message_id: i64, emoji: &str) -> bool {
    call.action == "set_msg_emoji_like"
        && call.params["message_id"] == message_id
        && call.params["emoji_id"] == emoji
}

/// The emoji ids reacted on `message_id` (either reaction API), in order.
pub fn reactions_for(calls: &[ApiCall], message_id: i64) -> Vec<String> {
    calls
        .iter()
        .filter(|call| {
            matches!(
                call.action.as_str(),
                "set_msg_emoji_like" | "set_group_reaction"
            ) && call.params["message_id"] == message_id
        })
        .map(|call| {
            let emoji = &call.params["emoji_id"];
            let emoji = if emoji.is_null() {
                &call.params["code"]
            } else {
                emoji
            };
            emoji.as_str().unwrap_or_default().to_string()
        })
        .collect()
}

/// Concatenated `text` segments of a `send_msg` call.
pub fn sent_text(call: &ApiCall) -> String {
    call.params["message"]
        .as_array()
        .expect("message array")
        .iter()
        .filter(|segment| segment["type"] == "text")
        .filter_map(|segment| segment["data"]["text"].as_str())
        .collect()
}

impl Adapter {
    /// Push `event` and wait until its final reaction (`emoji`) arrives.
    pub async fn push_and_wait_reaction(&self, event: Value, emoji: &str) {
        let id = message_id(&event);
        self.bot.push(event);
        self.wait_reaction(id, emoji).await;
    }

    pub async fn wait_reaction(&self, message_id: i64, emoji: &str) {
        self.bot
            .wait_until(WAIT, |calls| {
                calls
                    .iter()
                    .any(|call| is_reaction(call, message_id, emoji))
                    .then_some(())
            })
            .await;
    }

    /// Assert that `ignored` makes no API call: push it, then `probe` (a
    /// link that is parsed and answered), and once the probe's success
    /// reaction is in, check that the only new reply is the probe's and
    /// nothing reacted on `ignored`. Events are handled in arrival order and
    /// the ignore paths never await, so the probe finishing bounds them.
    pub async fn assert_ignored(&self, ignored: Value, probe: Value) {
        let ignored_id = message_id(&ignored);
        let sends_before = self.bot.calls_of("send_msg").len();
        self.bot.push(ignored);
        self.push_and_wait_reaction(probe, REACTION_SUCCESS).await;

        let calls = self.bot.calls();
        assert_eq!(
            reactions_for(&calls, ignored_id),
            Vec::<String>::new(),
            "ignored message got reactions"
        );
        assert_eq!(
            calls
                .iter()
                .filter(|call| call.action == "send_msg")
                .count(),
            sends_before + 1,
            "only the probe may be answered; calls: {calls:#?}"
        );
    }

    /// Send an admin command and wait for the plugin's text reply.
    pub async fn admin_command(&self, event: Value) -> String {
        let before = self.bot.calls_of("send_msg").len();
        self.bot.push(event);
        let reply = self.bot.wait_for_call("send_msg", before + 1, WAIT).await;
        sent_text(&reply)
    }
}

/// An admin text message in `group_id` (or private when `None`).
pub fn admin_msg(group_id: Option<i64>, text: &str) -> Value {
    match group_id {
        Some(group_id) => test_harness::onebot::group_msg(group_id, ADMIN_ID, text),
        None => test_harness::onebot::private_msg(ADMIN_ID, text),
    }
}
