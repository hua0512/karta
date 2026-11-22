//! End-to-end message pipeline: OneBot event in, OneBot API calls out.
//!
//! Every API action answers with success unless a test scripts otherwise,
//! so the process-global reaction and forward-variant latches settle on
//! their first variant here; scenarios that need them unsettled live in
//! their own test files.

mod common;

use std::sync::Arc;

use common::*;
use karta_core::ParserError;
use karta_core::data::{Content, ImageContent, VideoContent};
use serde_json::json;
use test_harness::onebot::{ApiReply, FakeBot, SELF_ID, group_msg, private_msg};
use test_harness::{FakeUpstream, Reply, TempApp};

#[tokio::test]
async fn group_link_acks_replies_then_reacts_success() {
    let parser = Arc::new(bili_parser().returning(bili_result("hello title")));
    let adapter = start(json!({}), vec![parser.clone()]).await;

    let event = group_msg(100, 200, &format!("look {BILI_LINK}"));
    let id = message_id(&event);
    adapter
        .push_and_wait_reaction(event, REACTION_SUCCESS)
        .await;

    let calls = adapter.bot.calls();
    let actions: Vec<&str> = calls.iter().map(|call| call.action.as_str()).collect();
    assert_eq!(
        actions,
        ["set_msg_emoji_like", "send_msg", "set_msg_emoji_like"]
    );
    assert_eq!(reactions_for(&calls, id), [REACTION_ACK, REACTION_SUCCESS]);

    let send = &calls[1].params;
    assert_eq!(send["message_type"], "group");
    assert_eq!(send["group_id"], 100);
    // OneBot wire shape: segments carry `type`, never kovi's `kind`.
    assert_eq!(send["message"][0]["type"], "text");
    assert!(send["message"][0].get("kind").is_none());
    assert!(sent_text(&calls[1]).contains("hello title"));

    assert_eq!(parser.inputs(), ["BV1xx411c7mD"]);
}

#[tokio::test]
async fn private_link_replies_to_the_sender() {
    let parser = Arc::new(bili_parser().returning(bili_result("private title")));
    let adapter = start(json!({}), vec![parser]).await;

    let event = private_msg(300, BILI_LINK);
    let id = message_id(&event);
    adapter
        .push_and_wait_reaction(event, REACTION_SUCCESS)
        .await;

    let calls = adapter.bot.calls();
    let sends = adapter.bot.calls_of("send_msg");
    assert_eq!(sends.len(), 1);
    assert_eq!(sends[0].params["message_type"], "private");
    assert_eq!(sends[0].params["user_id"], 300);
    assert!(sends[0].params.get("group_id").is_none());
    assert!(sent_text(&sends[0]).contains("private title"));
    assert_eq!(reactions_for(&calls, id), [REACTION_ACK, REACTION_SUCCESS]);
}

#[tokio::test]
async fn rejected_send_reacts_failure() {
    let parser = Arc::new(bili_parser().returning(bili_result("t")));
    let adapter = start(json!({}), vec![parser]).await;
    adapter.bot.script("send_msg", ApiReply::Failed(100));

    let event = group_msg(100, 200, BILI_LINK);
    let id = message_id(&event);
    adapter
        .push_and_wait_reaction(event, REACTION_FAILURE)
        .await;

    let calls = adapter.bot.calls();
    assert_eq!(adapter.bot.calls_of("send_msg").len(), 1);
    assert_eq!(reactions_for(&calls, id), [REACTION_ACK, REACTION_FAILURE]);
}

#[tokio::test]
async fn oversized_video_sends_a_notice_and_failure_reaction_after_the_card() {
    let fake = FakeUpstream::start().await;
    let video = fake
        .route("cdn.example", "GET", "/video.mp4")
        .reply(Reply::ok().body(vec![0; 1024 * 1024 + 1]));
    let app = TempApp::new(json!({
        "render_type": "image", "emoji_enabled": false, "max_size": 1,
        "forward_contents": true,
    }));
    let result = bili_result("oversized video").with_content(Content::Video(
        VideoContent::new().with_url("https://cdn.example/video.mp4"),
    ));
    let options = karta_kovi::PluginOptions {
        data_path: Some(app.data_path().to_path_buf()),
        parsers: Some(vec![Arc::new(bili_parser().returning(result))]),
        downloader: Some(fake.downloader(app.config(), app.data_path())),
    };
    let bot = FakeBot::start(move || {
        let options = options.clone();
        async move { karta_kovi::register_plugin_with(options).await }
    })
    .await;
    let adapter = Adapter { bot, app };
    let event = group_msg(100, 200, BILI_LINK);
    let id = message_id(&event);
    adapter
        .push_and_wait_reaction(event, REACTION_FAILURE)
        .await;

    let sends = adapter.bot.calls_of("send_msg");
    assert_eq!(sends.len(), 2);
    assert_eq!(sends[0].params["message"][0]["type"], "image");
    assert!(sent_text(&sends[1]).contains("上限 1 MiB"));
    assert!(adapter.bot.calls_of("send_group_forward_msg").is_empty());
    assert!(adapter.bot.calls_of("upload_group_file").is_empty());
    assert_eq!(
        reactions_for(&adapter.bot.calls(), id),
        [REACTION_ACK, REACTION_FAILURE]
    );
    assert_eq!(video.hits(), 1);
    fake.assert_no_unmatched();
}

#[tokio::test]
async fn failed_file_upload_is_not_hidden_by_a_delivered_header_or_notice() {
    let result = bili_result("video").with_content(Content::Video(
        VideoContent::new().with_path("/tmp/video.mp4".into()),
    ));
    let adapter = start(
        json!({ "need_upload": true }),
        vec![Arc::new(bili_parser().returning(result))],
    )
    .await;
    adapter
        .bot
        .script("upload_group_file", ApiReply::Failed(1200));
    let event = group_msg(100, 200, BILI_LINK);
    let id = message_id(&event);
    adapter
        .push_and_wait_reaction(event, REACTION_FAILURE)
        .await;

    let uploads = adapter.bot.calls_of("upload_group_file");
    assert_eq!(uploads.len(), 1);
    assert_eq!(uploads[0].params["name"], "video.mp4");
    let sends = adapter.bot.calls_of("send_msg");
    assert_eq!(sends.len(), 2);
    assert!(sent_text(&sends[1]).contains("视频发送失败或未获确认"));
    assert_eq!(
        reactions_for(&adapter.bot.calls(), id),
        [REACTION_ACK, REACTION_FAILURE]
    );
}

#[tokio::test]
async fn parse_error_reacts_failure_without_replying() {
    let parser = Arc::new(bili_parser().failing(|| ParserError::Parse("boom".into())));
    let adapter = start(json!({}), vec![parser.clone()]).await;

    let event = group_msg(100, 200, BILI_LINK);
    let id = message_id(&event);
    adapter
        .push_and_wait_reaction(event, REACTION_FAILURE)
        .await;

    let calls = adapter.bot.calls();
    assert!(adapter.bot.calls_of("send_msg").is_empty(), "{calls:#?}");
    assert_eq!(reactions_for(&calls, id), [REACTION_ACK, REACTION_FAILURE]);
    assert_eq!(parser.calls(), 1);
}

#[tokio::test]
async fn parse_time_media_limit_sends_a_safe_notice() {
    let parser = bili_parser().failing(|| {
        ParserError::Download(karta_core::error::DownloadError::DurationTooLong {
            duration: 600,
            limit: 480,
        })
        .with_context(
            karta_core::Platform::Bilibili,
            "https://cdn.example/?token=private",
        )
    });
    let adapter = start(json!({}), vec![Arc::new(parser)]).await;
    adapter
        .push_and_wait_reaction(group_msg(100, 200, BILI_LINK), REACTION_FAILURE)
        .await;

    let sends = adapter.bot.calls_of("send_msg");
    assert_eq!(sends.len(), 1);
    let notice = sent_text(&sends[0]);
    assert!(notice.contains("上限 480 秒"));
    assert!(!notice.contains("token=private"));
}

#[tokio::test]
async fn bundle_is_sent_as_a_group_forward() {
    let result = bili_result("bundle title")
        .with_text("body text")
        .with_content(Content::Image(
            ImageContent::new().with_url("https://img.example/a.jpg"),
        ));
    let parser = Arc::new(bili_parser().returning(result));
    let adapter = start(json!({ "forward_contents": true }), vec![parser]).await;

    let event = group_msg(100, 200, BILI_LINK);
    let id = message_id(&event);
    adapter
        .push_and_wait_reaction(event, REACTION_SUCCESS)
        .await;

    let calls = adapter.bot.calls();
    assert!(adapter.bot.calls_of("send_msg").is_empty(), "{calls:#?}");
    let forwards = adapter.bot.calls_of("send_group_forward_msg");
    assert_eq!(forwards.len(), 1);
    let params = &forwards[0].params;
    assert_eq!(params["group_id"], 100);

    // Header, body text, image: one node each, with typed OneBot content.
    let nodes = params["messages"]
        .as_array()
        .expect("nodes under `messages`");
    assert_eq!(nodes.len(), 3, "{nodes:#?}");
    assert!(nodes.iter().all(|node| node["type"] == "node"));
    let content = |i: usize| &nodes[i]["data"]["content"][0];
    assert_eq!(content(0)["type"], "text");
    assert!(
        content(0)["data"]["text"]
            .as_str()
            .unwrap()
            .contains("bundle title")
    );
    assert_eq!(content(1)["type"], "text");
    assert_eq!(content(1)["data"]["text"], "body text");
    assert_eq!(content(2)["type"], "image");
    assert_eq!(content(2)["data"]["file"], "https://img.example/a.jpg");
    assert!(content(2).get("kind").is_none());

    assert_eq!(reactions_for(&calls, id), [REACTION_ACK, REACTION_SUCCESS]);
}

#[tokio::test]
async fn own_and_sent_messages_make_no_api_calls() {
    let parser = Arc::new(bili_parser().returning(bili_result("t")));
    let adapter = start(json!({}), vec![parser.clone()]).await;

    // The bot's own message, as some implementations echo it back.
    adapter
        .assert_ignored(
            group_msg(100, SELF_ID, BILI_LINK),
            group_msg(100, 200, BILI_LINK),
        )
        .await;

    // NapCat's `message_sent` report of an outgoing message.
    let mut sent = group_msg(100, 201, OTHER_BILI_LINK);
    sent["post_type"] = json!("message_sent");
    adapter
        .assert_ignored(sent, private_msg(300, BILI_LINK))
        .await;

    // Only the probe was parsed; the second probe hit the parse cache.
    assert_eq!(parser.inputs(), ["BV1xx411c7mD"]);
}

#[tokio::test]
async fn json_card_url_is_parsed() {
    let parser = Arc::new(bili_parser().returning(bili_result("card title")));
    let adapter = start(json!({}), vec![parser.clone()]).await;

    let card = json!({ "meta": { "detail_1": {
        "title": "哔哩哔哩",
        "qqdocurl": BILI_LINK,
    } } });
    let mut event = group_msg(100, 200, "");
    event["message"] = json!([{ "type": "json", "data": { "data": card.to_string() } }]);
    event["raw_message"] = json!("[CQ:json]");
    adapter
        .push_and_wait_reaction(event, REACTION_SUCCESS)
        .await;

    assert_eq!(parser.inputs(), ["BV1xx411c7mD"]);
    let sends = adapter.bot.calls_of("send_msg");
    assert_eq!(sends.len(), 1);
    assert!(sent_text(&sends[0]).contains("card title"));
}
