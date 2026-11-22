//! Kuaishou end to end: a short link's cross-host redirect, the page the
//! parser rewrites the target to, and a page without the embedded state.

use karta_core::error::ParserError;
use karta_core::{Platform, data::Content};
use serde_json::json;
use test_harness::{FakeUpstream, Reply, TempApp};

#[tokio::test]
async fn short_link_redirects_cross_host_and_long_video_is_read_as_photo() {
    let fake = FakeUpstream::start().await;
    let short = fake
        .route("v.kuaishou.com", "GET", "/2yAnzeZ")
        .reply(Reply::redirect(
            "https://v.m.chenzhongtech.com/fw/long-video/3xabc?fid=1",
        ));
    let landing = fake.route("v.m.chenzhongtech.com", "GET", "/fw/long-video/3xabc");
    let state = include_str!("fixtures/kuaishou_init_state.json");
    let page = fake
        .route("v.m.chenzhongtech.com", "GET", "/fw/photo/3xabc?fid=1")
        .reply(Reply::ok().content_type("text/html").body(format!(
            "<html><script>window.INIT_STATE = {state};(function() {{}})()</script></html>"
        )));
    let (_app, engine) = TempApp::with_platforms(&fake, json!({ "download_media": false }));

    let result = engine
        .parse("快手 https://v.kuaishou.com/2yAnzeZ 看看")
        .await
        .unwrap()
        .expect("kuaishou short link matches");

    assert_eq!(result.title.as_deref(), Some("快手测试视频标题"));
    assert_eq!(result.platform, Platform::Kuaishou);
    assert_eq!(result.timestamp, Some(1_704_067_200));
    assert_eq!(
        result.author.as_ref().map(|a| a.name.as_str()),
        Some("快手测试用户")
    );
    assert_eq!(
        result.author.as_ref().and_then(|a| a.avatar.as_deref()),
        Some("https://tx2.a.yximgs.com/test_avatar.jpg")
    );
    let [Content::Video(video)] = result.contents.as_slice() else {
        panic!("expected one video: {:?}", result.contents);
    };
    assert_eq!(
        video.url.as_deref(),
        Some("https://txmov2.a.yximgs.com/test_video.mp4")
    );
    assert_eq!(
        video.cover.as_deref(),
        Some("https://p3.a.yximgs.com/test_cover.jpg")
    );
    assert_eq!(video.duration, Some(15));
    assert_eq!(short.hits(), 1);
    assert_eq!(landing.hits(), 1);
    assert_eq!(page.hits(), 1);
    assert_eq!(
        page.last_request().header("referer"),
        Some("https://v.kuaishou.com/")
    );
    fake.assert_no_unmatched();
}

#[tokio::test]
async fn web_link_page_without_init_state_is_a_parse_error() {
    let fake = FakeUpstream::start().await;
    let page =
        fake.route("www.kuaishou.com", "GET", "/short-video/3xhjgcmir24m4nm")
            .reply(Reply::ok().content_type("text/html").body(
                "<html><title>验证中</title><script>window.__APOLLO_STATE__={}</script></html>",
            ));
    let (_app, engine) = TempApp::with_platforms(&fake, json!({ "download_media": false }));

    let err = engine
        .parse("https://www.kuaishou.com/short-video/3xhjgcmir24m4nm")
        .await
        .unwrap_err();

    assert!(
        matches!(&err, ParserError::Parse(message) if message.contains("INIT_STATE")),
        "{err:?}"
    );
    // One probe for redirects, one fetch of the page itself.
    assert_eq!(page.hits(), 2);
    fake.assert_no_unmatched();
}
