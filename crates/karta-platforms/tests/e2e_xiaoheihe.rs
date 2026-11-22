//! Xiaoheihe end to end: signed BBS and game API requests, HTML post bodies
//! and API-level failures.

use karta_core::Platform;
use karta_core::data::Content;
use karta_core::error::ParserError;
use serde_json::json;
use test_harness::{CapturedRequest, FakeUpstream, Fixtures, Reply, TempApp};

const FIXTURES: Fixtures = Fixtures(env!("CARGO_MANIFEST_DIR"));

/// Every API call carries the web signing parameters and the configured cookie.
#[track_caller]
fn assert_signed(request: &CapturedRequest) {
    let param = |key| request.query_param(key).unwrap_or_default();
    assert_eq!(param("os_type"), "web", "{}", request.uri());
    assert_eq!(param("hkey").len(), 7, "{}", request.uri());
    assert!(param("_time").parse::<u64>().is_ok(), "{}", request.uri());
    let nonce = param("nonce");
    assert!(
        nonce.len() == 32
            && nonce
                .chars()
                .all(|c| c.is_ascii_digit() || c.is_ascii_uppercase()),
        "nonce: {nonce}"
    );
    assert_eq!(request.header("cookie"), Some("x_xhh_tokenid=abc"));
    assert_eq!(request.header("referer"), Some("https://xiaoheihe.cn/"));
}

fn config() -> serde_json::Value {
    json!({ "download_media": false, "xhh_cookie": "x_xhh_tokenid=abc" })
}

#[tokio::test]
async fn bbs_post_with_html_body_keeps_text_media_order() {
    let fake = FakeUpstream::start().await;
    let tree = fake
        .route(
            "api.xiaoheihe.cn",
            "GET",
            "/bbs/app/link/tree?link_id=170000001",
        )
        .reply(FIXTURES.json("xiaoheihe/bbs_html_post.json"));
    let (_app, engine) = TempApp::with_platforms(&fake, config());

    let result = engine
        .parse("https://www.xiaoheihe.cn/bbs/link/170000001")
        .await
        .unwrap()
        .expect("xiaoheihe bbs link matches");

    assert_eq!(result.platform, Platform::Xiaoheihe);
    assert_eq!(result.title.as_deref(), Some("新版本攻略合集"));
    assert_eq!(
        result.url.as_deref(),
        Some("https://xiaoheihe.cn/bbs/link/170000001")
    );
    assert_eq!(
        result.text.as_deref(),
        Some(
            "开头一段\n中间一段\n(https://player.bilibili.com/player.html?bvid=BV1xx411c7mD)\n结尾一段"
        )
    );
    // The thumb and the first inline image are one picture on two CDN
    // hosts; only the thumb is kept.
    let contents: Vec<String> = result
        .contents
        .iter()
        .map(|content| match content {
            Content::Image(image) => format!("img {}", image.url.as_deref().unwrap_or("")),
            Content::Text(text) => format!("text {text}"),
            other => panic!("unexpected content: {other:?}"),
        })
        .collect();
    assert_eq!(
        contents,
        [
            "img https://imgheybox.max-c.com/web/bbs/2026/01/02/cover.png",
            "text 开头一段",
            "text 中间一段",
            "img https://imgheybox.max-c.com/web/bbs/2026/01/02/detail.jpg",
            "text (https://player.bilibili.com/player.html?bvid=BV1xx411c7mD)",
            "text 结尾一段",
        ]
    );

    assert_signed(&tree.last_request());
    fake.assert_no_unmatched();
}

#[tokio::test]
async fn game_links_query_the_endpoint_for_their_platform() {
    let cases = [
        (
            "https://www.xiaoheihe.cn/game/pc/730",
            "/game/get_game_detail",
            "steam_appid",
            "730",
        ),
        (
            "https://api.xiaoheihe.cn/game/share_game_detail?appid=5001&game_type=console",
            "/game/console/get_game_detail",
            "appid",
            "5001",
        ),
        (
            "https://www.xiaoheihe.cn/game/mobile/9001",
            "/game/mobile/get_game_detail",
            "appid",
            "9001",
        ),
    ];

    for (input, path, id_param, id) in cases {
        let fake = FakeUpstream::start().await;
        let detail = fake
            .route(
                "api.xiaoheihe.cn",
                "GET",
                &format!("{path}?{id_param}={id}"),
            )
            .reply(FIXTURES.json("xiaoheihe_game_info.json"));
        let (_app, engine) = TempApp::with_platforms(&fake, config());

        let result = engine
            .parse(input)
            .await
            .unwrap_or_else(|e| panic!("{input}: {e:?}"))
            .expect("xiaoheihe game link matches");

        assert_eq!(
            result.title.as_deref(),
            Some("测试游戏 (Test Game)"),
            "{input}"
        );
        assert_eq!(detail.hits(), 1, "{input}");
        assert_signed(&detail.last_request());
        fake.assert_no_unmatched();
    }
}

#[tokio::test]
async fn non_ok_api_status_is_an_api_error() {
    let fake = FakeUpstream::start().await;
    fake.route("api.xiaoheihe.cn", "GET", "/bbs/app/link/tree")
        .reply(Reply::ok().json(&json!({ "status": "failed", "msg": "hkey 不能为空" })));
    let (_app, engine) = TempApp::with_platforms(&fake, config());

    let err = engine
        .parse("https://www.xiaoheihe.cn/bbs/link/170000001")
        .await
        .unwrap_err();

    assert!(
        matches!(&err, ParserError::Api { code: -1, message } if message == "hkey 不能为空"),
        "{err:?}"
    );
    fake.assert_no_unmatched();
}
