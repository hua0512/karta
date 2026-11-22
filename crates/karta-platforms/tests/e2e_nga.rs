//! NGA thread end to end: the anti-bot `guestJs` interstitial, the retry that
//! carries its cookie, and a thread page served as GBK.

use encoding_rs::GBK;
use karta_core::Platform;
use karta_core::data::Content;
use serde_json::json;
use test_harness::{FakeUpstream, Reply, TempApp};

const INTERSTITIAL: &str = "<html><script>document.cookie = 'guestJs=1748350000;path=/';\
                            window.location.reload();</script></html>";

#[tokio::test]
async fn guestjs_interstitial_is_retried_with_its_cookie_and_gbk_page_decoded() {
    let fake = FakeUpstream::start().await;
    let (thread_gbk, _, unmappable) = GBK.encode(include_str!("fixtures/nga_thread.html"));
    assert!(!unmappable, "fixture must be representable in GBK");
    let thread = fake
        .route("nga.178.com", "GET", "/read.php?tid=45263995")
        .reply(
            Reply::status(403)
                .content_type("text/html")
                .body(INTERSTITIAL),
        )
        .reply(
            Reply::ok()
                .content_type("text/html; charset=GBK")
                .body(thread_gbk.into_owned()),
        );
    let (_app, engine) = TempApp::with_platforms(&fake, json!({ "download_media": false }));

    let result = engine
        .parse("看看 https://bbs.nga.cn/read.php?tid=45263995&page=2")
        .await
        .unwrap()
        .expect("nga link matches");

    assert_eq!(result.platform, Platform::Nga);
    assert_eq!(
        result.url.as_deref(),
        Some("https://nga.178.com/read.php?tid=45263995")
    );
    assert_eq!(result.title.as_deref(), Some("测试主题标题"));
    assert_eq!(
        result.author.as_ref().map(|a| a.name.as_str()),
        Some("泥潭老哥")
    );
    // 2026-05-01 12:34 Beijing time.
    assert_eq!(result.timestamp, Some(1_777_610_040));
    let text = result.text.as_deref().expect("thread text");
    assert!(text.contains("首楼正文"), "{text}");
    assert!(text.contains("收工"), "{text}");
    assert!(!text.contains('['), "{text}");
    assert!(!text.contains("引用的楼层内容"), "{text}");
    let images: Vec<_> = result
        .contents
        .iter()
        .map(|content| match content {
            Content::Image(image) => image.url.as_deref().expect("image URL"),
            other => panic!("unexpected content: {other:?}"),
        })
        .collect();
    assert_eq!(
        images,
        [
            "https://img.nga.178.com/attachments/mon_202605/01/abc123.jpg",
            "https://img.nga.178.com/attachments/mon_202605/01/def456.jpg",
        ]
    );

    let requests = thread.requests();
    assert_eq!(requests.len(), 2, "one interstitial, one retry");
    assert_eq!(requests[0].header("cookie"), None);
    assert_eq!(requests[0].query_param("rand"), None);
    let retry = &requests[1];
    assert_eq!(retry.header("cookie"), Some("guestJs=1748350000"));
    assert!(
        retry
            .query_param("rand")
            .is_some_and(|rand| rand.parse::<u32>().is_ok()),
        "retry carries a numeric rand: {}",
        retry.uri()
    );
    assert_eq!(retry.header("referer"), Some("https://nga.178.com/"));
    fake.assert_no_unmatched();
}

#[tokio::test]
async fn forbidden_page_without_guest_cookie_is_an_api_error() {
    let fake = FakeUpstream::start().await;
    let thread = fake.route("nga.178.com", "GET", "/read.php?tid=1").reply(
        Reply::status(403)
            .content_type("text/html")
            .body("<script>setCookie('guestJs', 1)</script>"),
    );
    let (_app, engine) = TempApp::with_platforms(&fake, json!({ "download_media": false }));

    let err = engine
        .parse("https://nga.178.com/read.php?tid=1")
        .await
        .unwrap_err();

    assert!(
        matches!(err, karta_core::error::ParserError::Api { code: 403, .. }),
        "{err:?}"
    );
    assert_eq!(thread.hits(), 1, "no retry without a cookie");
    fake.assert_no_unmatched();
}
