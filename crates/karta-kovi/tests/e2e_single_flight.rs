//! Identical links arriving while the first is still being parsed share one
//! parse (`ParseGate` single-flight), and every message still gets a reply.

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::*;
use serde_json::json;
use test_harness::onebot::group_msg;

#[tokio::test]
async fn concurrent_identical_links_parse_once_and_both_reply() {
    let parser = Arc::new(
        bili_parser()
            .returning(bili_result("shared"))
            .delayed(Duration::from_secs(1)),
    );
    let adapter = start(json!({}), vec![parser.clone()]).await;

    // With the clock paused, the parser's delay only elapses once every
    // task is idle: both messages have then been acked and are waiting on
    // the same flight, independent of scheduling.
    tokio::time::pause();
    let first = group_msg(100, 200, BILI_LINK);
    let second = group_msg(100, 201, BILI_LINK);
    let ids = [message_id(&first), message_id(&second)];
    adapter.bot.push(first);
    adapter.bot.push(second);
    for id in ids {
        adapter.wait_reaction(id, REACTION_SUCCESS).await;
    }

    assert_eq!(parser.calls(), 1, "the second message must join the flight");
    let sends = adapter.bot.calls_of("send_msg");
    assert_eq!(sends.len(), 2);
    assert!(sends.iter().all(|send| sent_text(send).contains("shared")));
}
