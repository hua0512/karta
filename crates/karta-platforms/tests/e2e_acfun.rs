//! AcFun end to end: the `ajaxpipe=1` video page and its JSON envelope.

use karta_core::Platform;
use karta_core::data::Content;
use serde_json::json;
use test_harness::{FakeUpstream, Fixtures, TempApp};

const FIXTURES: Fixtures = Fixtures(env!("CARGO_MANIFEST_DIR"));

#[tokio::test]
async fn ajaxpipe_page_builds_the_video_result() {
    let fake = FakeUpstream::start().await;
    let page = fake
        .route(
            "www.acfun.cn",
            "GET",
            "/v/ac12345678?quickViewId=videoInfo_new&ajaxpipe=1",
        )
        .reply(FIXTURES.html("acfun/ajaxpipe_video_info.txt"));
    let (_app, engine) = TempApp::with_platforms(&fake, json!({ "download_media": false }));

    let result = engine
        .parse("看 https://m.acfun.cn/v/?ac=12345678 这个")
        .await
        .unwrap()
        .expect("acfun link matches");

    assert_eq!(result.platform, Platform::AcFun);
    assert_eq!(
        result.url.as_deref(),
        Some("https://www.acfun.cn/v/ac12345678")
    );
    assert_eq!(result.title.as_deref(), Some(r#"测试"AcFun"视频 {第1集}"#));
    assert_eq!(result.text.as_deref(), Some("这是AcFun测试视频描述"));
    assert_eq!(result.timestamp, Some(1_704_067_200));
    assert_eq!(
        result.author.as_ref().map(|a| a.name.as_str()),
        Some("AC测试用户")
    );
    assert_eq!(
        result.author.as_ref().and_then(|a| a.avatar.as_deref()),
        Some("https://imgs.aixifan.com/test_avatar.jpg")
    );
    let Some(Content::Video(video)) = result.contents.first() else {
        panic!("video content: {:?}", result.contents);
    };
    assert_eq!(
        video.url.as_deref(),
        Some("https://tx-safety-video.acfun.cn/test_video.mp4")
    );
    assert_eq!(video.duration, Some(180));
    assert_eq!(
        video.cover.as_deref(),
        Some("https://imgs.aixifan.com/test_cover.jpg")
    );

    assert_eq!(
        page.last_request().header("referer"),
        Some("https://www.acfun.cn/")
    );
    fake.assert_no_unmatched();
}
