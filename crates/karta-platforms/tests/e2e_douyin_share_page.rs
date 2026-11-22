//! Douyin share-page fallback end to end.
//!
//! Kept in its own test binary: the anonymous ttwid it registers is cached
//! process-wide, so this must be the only test that triggers registration.

use std::path::PathBuf;

use karta_core::Platform;
use karta_core::constants::MOBILE_USER_AGENT;
use karta_core::data::Content;
use serde_json::{Value, json};
use test_harness::{FakeUpstream, Reply, TempApp};

fn share_page(item_id: Option<&str>) -> Reply {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/douyin_router_data.json");
    let mut data: Value =
        serde_json::from_slice(&std::fs::read(path).expect("read fixture")).expect("fixture JSON");
    let item = &mut data["loaderData"]["video_(id)/page"]["videoInfoRes"]["item_list"][0];
    // Share pages hand out the watermarked play URL.
    item["video"]["play_addr"]["url_list"] = json!([
        "https://aweme.snssdk.com/aweme/v1/playwm/?video_id=v0200fg10000router&ratio=720p&line=0"
    ]);
    if let Some(item_id) = item_id {
        item["aweme_id"] = json!(item_id);
    }
    Reply::ok().content_type("text/html").body(format!(
        "<html><body><script>window._ROUTER_DATA = {data};</script></body></html>"
    ))
}

#[tokio::test]
async fn rejected_api_falls_back_to_the_share_page_with_an_anonymous_ttwid() {
    let fake = FakeUpstream::start().await;
    let register = fake
        .route("ttwid.bytedance.com", "POST", "/ttwid/union/register/")
        .reply(Reply::ok().set_cookie("ttwid=registered-ttwid; Path=/; HttpOnly"));
    let mut pages = Vec::new();
    for id in ["7521023890996514083", "7521023890996514084"] {
        fake.route(
            "www.douyin.com",
            "GET",
            &format!("/aweme/v1/web/aweme/detail/?aweme_id={id}"),
        )
        .reply(Reply::status(403));
        // Matching the requested item (the first) or the lone item without an
        // id (the second) both count.
        let item_id = (id == "7521023890996514083").then_some(id);
        pages.push(
            fake.route("m.douyin.com", "GET", &format!("/share/video/{id}"))
                .reply(share_page(item_id)),
        );
    }
    let (_app, engine) = TempApp::with_platforms(&fake, json!({ "download_media": false }));

    for id in ["7521023890996514083", "7521023890996514084"] {
        let result = engine
            .parse(&format!("https://www.douyin.com/video/{id}"))
            .await
            .expect("share page parses")
            .expect("douyin link matches");

        assert_eq!(result.platform, Platform::Douyin);
        assert_eq!(
            result.text.as_deref(),
            Some("这是一个测试抖音视频 #测试 #搞笑")
        );
        assert_eq!(
            result.author.as_ref().map(|a| a.name.as_str()),
            Some("抖音测试用户")
        );
        let [Content::Video(video)] = result.contents.as_slice() else {
            panic!("expected one video: {:?}", result.contents);
        };
        // The watermarked play URL is swapped for the clean one.
        assert_eq!(
            video.url.as_deref(),
            Some(
                "https://aweme.snssdk.com/aweme/v1/play/?video_id=v0200fg10000router&ratio=720p&line=0"
            )
        );
        assert_eq!(video.duration, Some(15));
    }

    // One registration serves every later share-page request.
    assert_eq!(register.hits(), 1);
    let registration = register.last_request();
    assert_eq!(
        registration.header("content-type"),
        Some("application/json")
    );
    let body: Value = serde_json::from_slice(&registration.body).expect("JSON body");
    assert_eq!(body["aid"], 1768);
    assert_eq!(body["service"], "www.ixigua.com");
    for page in &pages {
        assert_eq!(page.hits(), 1);
        let request = page.last_request();
        assert_eq!(request.header("cookie"), Some("ttwid=registered-ttwid"));
        assert_eq!(request.header("user-agent"), Some(MOBILE_USER_AGENT));
    }
    fake.assert_no_unmatched();
}
