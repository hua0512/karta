//! V2EX end to end: topic plus its reply preview, and a topic whose replies
//! cannot be fetched.

use karta_core::Platform;
use serde_json::json;
use test_harness::{FakeUpstream, Fixtures, Reply, Route, TempApp};

const FIXTURES: Fixtures = Fixtures(env!("CARGO_MANIFEST_DIR"));

fn topic_route(fake: &FakeUpstream) -> Route {
    fake.route("www.v2ex.com", "GET", "/api/topics/show.json?id=1088888")
        .reply(FIXTURES.json("v2ex/topics_show.json"))
}

#[tokio::test]
async fn topic_text_carries_the_body_and_latest_replies() {
    let fake = FakeUpstream::start().await;
    let topic = topic_route(&fake);
    let replies = fake
        .route(
            "www.v2ex.com",
            "GET",
            "/api/replies/show.json?topic_id=1088888",
        )
        .reply(FIXTURES.json("v2ex/replies_show.json"));
    let (_app, engine) = TempApp::with_platforms(&fake, json!({ "download_media": false }));

    let result = engine
        .parse("求助 https://www.v2ex.com/t/1088888?p=2#reply5")
        .await
        .unwrap()
        .expect("v2ex link matches");

    assert_eq!(result.platform, Platform::V2ex);
    assert_eq!(result.title.as_deref(), Some("大家都用什么键盘写代码？"));
    assert_eq!(
        result.url.as_deref(),
        Some("https://www.v2ex.com/t/1088888")
    );
    assert_eq!(result.timestamp, Some(1_704_067_200));
    let author = result.author.as_ref().expect("author");
    assert_eq!(author.name, "keyboarder");
    assert_eq!(
        author.avatar.as_deref(),
        Some("https://cdn.v2ex.com/avatar/large.png")
    );
    assert_eq!(result.extra.get("节点").map(String::as_str), Some("问与答"));
    assert_eq!(result.extra.get("回复").map(String::as_str), Some("5"));
    assert_eq!(
        result.text.as_deref(),
        Some(
            "最近想换键盘\n\n预算 500 以内&求推荐\n\n\
             最近回复:\n@second: Filco 圣手\n\n@third: Cherry\n青轴\n\n@latest: Keychron"
        )
    );

    for route in [&topic, &replies] {
        assert_eq!(
            route.last_request().header("referer"),
            Some("https://www.v2ex.com/")
        );
    }
    fake.assert_no_unmatched();
}

#[tokio::test]
async fn failed_replies_request_still_returns_the_topic() {
    let fake = FakeUpstream::start().await;
    topic_route(&fake);
    let replies = fake
        .route("www.v2ex.com", "GET", "/api/replies/show.json")
        .reply(Reply::status(500).body("upstream error"));
    let (_app, engine) = TempApp::with_platforms(&fake, json!({ "download_media": false }));

    let result = engine
        .parse("https://v2ex.com/t/1088888")
        .await
        .unwrap()
        .expect("v2ex link matches");

    assert_eq!(
        result.text.as_deref(),
        Some("最近想换键盘\n\n预算 500 以内&求推荐")
    );
    assert_eq!(replies.hits(), 1);
    fake.assert_no_unmatched();
}
