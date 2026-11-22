//! A bundle whose forward cannot be sent falls back to one plain-text
//! message.
//!
//! Own test binary: no forward send succeeds here, so the process-global
//! forward-variant latch stays unknown for every scenario.

mod common;

use std::sync::Arc;

use common::*;
use karta_core::data::{Content, ImageContent};
use serde_json::json;
use test_harness::onebot::{ApiReply, group_msg};

const GROUP_FORWARD_ACTIONS: [&str; 2] = ["send_group_forward_msg", "send_forward_msg_to_group"];

async fn start_bundling() -> Adapter {
    let result = bili_result("bundle title")
        .with_text("body text")
        .with_content(Content::Image(
            ImageContent::new().with_url("https://img.example/a.jpg"),
        ));
    let parser = Arc::new(bili_parser().returning(result));
    start(json!({ "forward_contents": true }), vec![parser]).await
}

fn forward_calls(adapter: &Adapter) -> Vec<String> {
    adapter
        .bot
        .calls()
        .into_iter()
        .filter(|call| GROUP_FORWARD_ACTIONS.contains(&call.action.as_str()))
        .map(|call| {
            let key = if call.params.get("messages").is_some() {
                "messages"
            } else {
                "message"
            };
            format!("{} {key}", call.action)
        })
        .collect()
}

#[tokio::test]
async fn unsupported_forward_shapes_fall_back_to_plain_text() {
    let adapter = start_bundling().await;
    for action in GROUP_FORWARD_ACTIONS {
        adapter.bot.script(action, ApiReply::unsupported());
    }

    let event = group_msg(100, 200, BILI_LINK);
    let id = message_id(&event);
    adapter
        .push_and_wait_reaction(event, REACTION_SUCCESS)
        .await;

    // Every shape is probed once before giving up on the forward.
    assert_eq!(
        forward_calls(&adapter),
        [
            "send_group_forward_msg messages",
            "send_group_forward_msg message",
            "send_forward_msg_to_group message",
        ]
    );
    let sends = adapter.bot.calls_of("send_msg");
    assert_eq!(sends.len(), 1);
    assert_eq!(sends[0].params["group_id"], 100);
    let text = sent_text(&sends[0]);
    assert!(
        text.contains("bundle title") && text.ends_with("\nbody text"),
        "{text:?}"
    );
    assert_eq!(
        reactions_for(&adapter.bot.calls(), id),
        [REACTION_ACK, REACTION_SUCCESS]
    );
}

#[tokio::test]
async fn forward_failure_that_may_have_delivered_is_not_retried_in_another_shape() {
    let adapter = start_bundling().await;
    // Not a shape rejection: the server may have delivered the forward.
    adapter
        .bot
        .script("send_group_forward_msg", ApiReply::Failed(-500));

    let event = group_msg(100, 200, BILI_LINK);
    adapter
        .push_and_wait_reaction(event, REACTION_SUCCESS)
        .await;

    assert_eq!(forward_calls(&adapter), ["send_group_forward_msg messages"]);
    assert_eq!(adapter.bot.calls_of("send_msg").len(), 1);
}

#[tokio::test]
async fn rejected_plain_text_fallback_reacts_failure() {
    let adapter = start_bundling().await;
    for action in GROUP_FORWARD_ACTIONS {
        adapter.bot.script(action, ApiReply::unsupported());
    }
    adapter.bot.script("send_msg", ApiReply::Failed(100));

    let event = group_msg(100, 200, BILI_LINK);
    let id = message_id(&event);
    adapter
        .push_and_wait_reaction(event, REACTION_FAILURE)
        .await;

    assert_eq!(adapter.bot.calls_of("send_msg").len(), 1);
    assert_eq!(
        reactions_for(&adapter.bot.calls(), id),
        [REACTION_ACK, REACTION_FAILURE]
    );
}
