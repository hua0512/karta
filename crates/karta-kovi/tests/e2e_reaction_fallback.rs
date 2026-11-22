//! Servers without `set_msg_emoji_like` get group reactions instead.
//!
//! Own test binary: the reaction-support latches are process-global, and
//! this scenario needs them unprobed.

mod common;

use std::sync::Arc;

use common::*;
use serde_json::json;
use test_harness::onebot::{ApiReply, group_msg};

#[tokio::test]
async fn unsupported_emoji_like_falls_back_to_group_reaction() {
    let parser = Arc::new(bili_parser().returning(bili_result("t")));
    let adapter = start(json!({}), vec![parser]).await;
    adapter
        .bot
        .script("set_msg_emoji_like", ApiReply::unsupported());

    let event = group_msg(100, 200, BILI_LINK);
    let id = message_id(&event);
    adapter.bot.push(event);
    adapter
        .bot
        .wait_until(test_harness::WAIT, |calls| {
            calls
                .iter()
                .any(|call| {
                    call.action == "set_group_reaction" && call.params["code"] == REACTION_SUCCESS
                })
                .then_some(())
        })
        .await;

    // Probed once (1404 latches "unsupported"); every later reaction goes
    // straight to the group API.
    let calls = adapter.bot.calls();
    let actions: Vec<&str> = calls.iter().map(|call| call.action.as_str()).collect();
    assert_eq!(
        actions,
        [
            "set_msg_emoji_like",
            "set_group_reaction",
            "send_msg",
            "set_group_reaction",
        ]
    );
    assert_eq!(calls[1].params["group_id"], 100);
    assert_eq!(calls[1].params["message_id"], id);
    assert_eq!(calls[1].params["code"], REACTION_ACK);
}
