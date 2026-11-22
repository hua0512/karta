//! Xiaohongshu end to end: access-page redirects and stream selection
//! through the explore → discovery page fallback.

use karta_core::Platform;
use karta_core::data::Content;
use serde_json::json;
use test_harness::{CapturedRequest, FakeUpstream, Fixtures, Reply, TempApp};

const FIXTURES: Fixtures = Fixtures(env!("CARGO_MANIFEST_DIR"));

const NOTE_ID: &str = "64a1b2c3000000001e03d4f5";

/// Requests sent with the iOS Safari emulation rather than the shared
/// desktop Chrome client.
#[track_caller]
fn assert_mobile(request: &CapturedRequest) {
    let ua = request.header("user-agent").unwrap_or_default();
    assert!(ua.contains("iPhone"), "mobile user agent: {ua}");
    assert_eq!(request.header("sec-ch-ua"), None, "no desktop client hints");
}

#[tokio::test]
async fn short_link_on_access_page_recovers_the_note_from_redirect_path() {
    for access_path in ["/website-login/captcha", "/login", "/website-login"] {
        let fake = FakeUpstream::start().await;
        let access_url = format!(
            "https://www.xiaohongshu.com{access_path}?redirectPath=\
             https%3A%2F%2Fwww.xiaohongshu.com%2Fdiscovery%2Fitem%2F{NOTE_ID}\
             %3Fxsec_token%3DABtok%253D%26xsec_source%3Dapp_share&verifyUuid=u1&verifyType=102"
        );
        let short = fake
            .route("xhslink.com", "GET", "/a/AbCd1234")
            .reply(Reply::redirect(&access_url));
        let access = fake.route("www.xiaohongshu.com", "GET", access_path).reply(
            Reply::ok()
                .content_type("text/html")
                .body("<html>验证</html>"),
        );
        let explore = fake
            .route("www.xiaohongshu.com", "GET", &format!("/explore/{NOTE_ID}"))
            .reply(FIXTURES.html("xiaohongshu/explore_note.html"));
        let (_app, engine) = TempApp::with_platforms(&fake, json!({ "download_media": false }));

        let result = engine
            .parse("分享笔记 http://xhslink.com/a/AbCd1234，复制本条信息")
            .await
            .unwrap()
            .expect("xhs short link matches");

        assert_eq!(result.platform, Platform::Xiaohongshu);
        assert_eq!(result.title.as_deref(), Some("周末露营装备清单"));
        assert_eq!(
            result.text.as_deref(),
            Some("第一次露营，记录一下装备 #露营[话题]#")
        );
        assert!(
            matches!(result.contents.last(), Some(Content::Text(text)) if text == "第一次露营，记录一下装备 #露营[话题]#")
        );
        assert_eq!(
            result.author.as_ref().map(|a| a.name.as_str()),
            Some("露营小白")
        );
        assert_eq!(result.timestamp, Some(1_704_110_400));
        let images: Vec<_> = result
            .images()
            .into_iter()
            .filter_map(|image| image.url.as_deref())
            .collect();
        assert_eq!(
            images,
            [
                "https://sns-webpic-qc.xhscdn.com/large/tent.jpg",
                "https://sns-webpic-qc.xhscdn.com/default/stove.jpg",
            ]
        );
        assert_eq!(
            result.extra.get("标签").map(String::as_str),
            Some("#露营 #户外")
        );
        assert_eq!(result.extra.get("点赞").map(String::as_str), Some("1024"));
        assert_eq!(result.extra.get("IP属地").map(String::as_str), Some("浙江"));

        assert_mobile(&short.last_request());
        assert_eq!(access.hits(), 1, "{access_path}");
        let request = explore.last_request();
        assert_eq!(
            request.query.as_deref(),
            Some("xsec_token=ABtok%3D&xsec_source=app_share"),
            "xsec params recovered from redirectPath"
        );
        assert_eq!(
            request.header("referer"),
            Some("https://www.xiaohongshu.com/")
        );
        fake.assert_no_unmatched();
    }
}

#[tokio::test]
async fn explore_page_without_note_state_falls_back_to_the_discovery_page() {
    let discovery_html = std::fs::read_to_string(FIXTURES.path("xiaohongshu/discovery_note.html"))
        .expect("read discovery fixture");
    for (case, stream, expected_url) in [
        (
            "direct URL",
            None,
            "https://sns-video-qc.xhscdn.com/stream/sunrise.mp4",
        ),
        (
            "EF5 before EF4",
            Some(json!({
                "h265": [], "h264": [], "av1": [], "h266": [],
                "EF5": [{"masterUrl": "https://example.com/ef5.mp4"}],
                "EF4": [{"masterUrl": "https://example.com/ef4.mp4"}],
            })),
            "https://example.com/ef5.mp4",
        ),
        (
            "EF4 after unusable groups",
            Some(json!({
                "h265": [], "h264": [], "av1": [], "h266": [],
                "EF5": [null, {}, {"masterUrl": ""}],
                "EF4": [{"masterUrl": "https://example.com/ef4.mp4"}],
            })),
            "https://example.com/ef4.mp4",
        ),
        (
            "unknown codec after unusable groups",
            Some(json!({
                "h265": [], "h264": [], "av1": [], "h266": [],
                "EF5": [], "EF4": [], "empty_codec": [],
                "future_codec": [null, {}, {"masterUrl": ""},
                    {"masterUrl": "https://example.com/future.mp4"}],
            })),
            "https://example.com/future.mp4",
        ),
    ] {
        let html = match stream {
            Some(stream) => discovery_html.replace(
                r#""video":{"url":"https://sns-video-qc.xhscdn.com/stream/sunrise.mp4","media":undefined}"#,
                &format!(r#""video":{}"#, json!({ "media": { "stream": stream } })),
            ),
            None => discovery_html.clone(),
        };
        let fake = FakeUpstream::start().await;
        let explore = fake
            .route("www.xiaohongshu.com", "GET", &format!("/explore/{NOTE_ID}"))
            .reply(
                Reply::ok()
                    .content_type("text/html")
                    .body("<html><title>登录后查看更多</title></html>"),
            );
        let discovery = fake
            .route(
                "www.xiaohongshu.com",
                "GET",
                &format!("/discovery/item/{NOTE_ID}"),
            )
            .reply(Reply::ok().content_type("text/html").body(html));
        let (_app, engine) = TempApp::with_platforms(
            &fake,
            json!({
                "download_media": false,
                "xhs_cookie": "web_session=abc",
            }),
        );

        let result = engine
            .parse(&format!(
                "https://www.xiaohongshu.com/explore/{NOTE_ID}?xsec_token=T1&xsec_source=pc_share"
            ))
            .await
            .unwrap()
            .expect("xhs explore link matches");

        assert_eq!(result.title.as_deref(), Some("山间日出延时"));
        assert_eq!(result.text.as_deref(), Some("凌晨四点爬山拍到的日出"));
        assert_eq!(
            result.author.as_ref().map(|a| a.name.as_str()),
            Some("山野摄影")
        );
        let Some(Content::Video(video)) = result.contents.first() else {
            panic!("discovery video note: {:?}", result.contents);
        };
        assert_eq!(video.url.as_deref(), Some(expected_url), "{case}");
        assert_eq!(
            video.cover.as_deref(),
            Some("https://sns-webpic-qc.xhscdn.com/preload/sunrise-cover.jpg")
        );

        let explore = explore.last_request();
        assert!(
            explore.header("sec-ch-ua").is_some(),
            "explore uses the desktop client"
        );
        assert_eq!(explore.header("cookie"), Some("web_session=abc"));
        let discovery = discovery.last_request();
        assert_mobile(&discovery);
        assert_eq!(
            discovery.query.as_deref(),
            Some("xsec_token=T1&xsec_source=pc_share")
        );
        assert_eq!(discovery.header("cookie"), Some("web_session=abc"));
        assert_eq!(discovery.header("x-requested-with"), Some("XMLHttpRequest"));
        fake.assert_no_unmatched();
    }
}
