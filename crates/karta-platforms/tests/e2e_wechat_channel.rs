//! WeChat video-channel share links end to end: the two-step Yuanbao parse →
//! channels feed-detail request shape.

use std::time::{SystemTime, UNIX_EPOCH};

use karta_core::Platform;
use karta_core::data::Content;
use serde_json::{Value, json};
use test_harness::{FakeUpstream, Fixtures, TempApp};

const FIXTURES: Fixtures = Fixtures(env!("CARGO_MANIFEST_DIR"));

#[tokio::test]
async fn share_link_is_resolved_in_two_steps() {
    let fake = FakeUpstream::start().await;
    let parse = fake
        .route(
            "yuanbao.tencent.com",
            "POST",
            "/api/weixin/get_parse_result",
        )
        .reply(FIXTURES.json("wechat_channel/parse_result.json"));
    let feed = fake
        .route(
            "channels.weixin.qq.com",
            "POST",
            "/finder-preview/api/feed/get_feed_info",
        )
        .reply(FIXTURES.json("wechat_channel/feed_info.json"));
    let (_app, engine) = TempApp::with_platforms(
        &fake,
        json!({
            "download_media": false,
            "wechat_channel_cookie": " yuanbao_session=abc ",
        }),
    );

    let result = engine
        .parse("看看这个 https://weixin.qq.com/sph/AbCd123 视频")
        .await
        .unwrap()
        .expect("video-channel link matches");

    assert_eq!(result.platform, Platform::WechatChannel);
    assert_eq!(
        result.url.as_deref(),
        Some("https://weixin.qq.com/sph/AbCd123")
    );
    assert_eq!(result.text.as_deref(), Some("这是一个测试视频号视频"));
    assert_eq!(result.timestamp, Some(1_700_000_000));
    assert_eq!(
        result.author.as_ref().map(|a| a.name.as_str()),
        Some("测试视频号作者")
    );
    assert_eq!(
        result.author.as_ref().and_then(|a| a.avatar.as_deref()),
        Some("https://wx.qlogo.cn/finderhead/head.jpg")
    );
    let [Content::Video(video)] = result.contents.as_slice() else {
        panic!("video content: {:?}", result.contents);
    };
    assert_eq!(
        video.url.as_deref(),
        Some(
            "https://finder.video.qq.com/251/20302/stodownload?encfilekey=ENC123&token=CDNTOKEN&bizid=1023"
        )
    );

    assert_eq!(
        video.cover.as_deref(),
        Some("https://finder.video.qq.com/cover.jpg")
    );
    assert_eq!(
        result.extra.get("stats").map(String::as_str),
        Some("❤️1.2万 👍8656 🔄3456 💬999")
    );

    // Step 1: the trimmed Yuanbao cookie and a JSON body naming the share URL.
    let step1 = parse.last_request();
    assert_eq!(step1.header("cookie"), Some("yuanbao_session=abc"));
    assert_eq!(step1.header("content-type"), Some("application/json"));
    assert_eq!(step1.header("origin"), Some("https://yuanbao.tencent.com"));
    assert!(step1.header("x-device-id").is_some_and(|id| !id.is_empty()));
    assert_eq!(
        serde_json::from_slice::<Value>(&step1.body).expect("step 1 JSON body"),
        json!({ "type": "video_channel_url", "url": "https://weixin.qq.com/sph/AbCd123", "scene": 1 })
    );

    // Step 2: `_rid` + `_pageUrl` query, the playable_url's token/eid in the
    // body and the Referer, and no cookie.
    let step2 = feed.last_request();
    let rid = step2.query_param("_rid").expect("_rid");
    let (secs, suffix) = rid.split_once('-').expect("rid has a dash");
    let secs = u64::from_str_radix(secs, 16).expect("hex unix seconds");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!(secs <= now && now - secs < 3600, "rid time: {rid}");
    assert!(
        suffix.len() == 8
            && suffix
                .chars()
                .all(|c| c.is_ascii_digit() || matches!(c, 'a'..='f')),
        "rid suffix: {rid}"
    );
    assert_eq!(
        step2.query_param("_pageUrl").as_deref(),
        Some("https%3A%2F%2Fchannels.weixin.qq.com%2Ffinder-preview%2Fpages%2Ffeed")
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&step2.body).expect("step 2 JSON body"),
        json!({ "baseReq": { "generalToken": "G+T/1=" }, "exportId": "export=id" })
    );
    assert_eq!(
        step2.header("referer"),
        Some(
            "https://channels.weixin.qq.com/finder-preview/pages/feed\
             ?entry_card_type=48&comment_scene=39&appid=0\
             &token=G%2BT%2F1%3D&entry_scene=0&eid=export%3Did"
        )
    );
    assert_eq!(step2.header("cookie"), None, "step 2 sends no cookie");
    assert_eq!(
        step2.header("origin"),
        Some("https://channels.weixin.qq.com")
    );
    fake.assert_no_unmatched();
}
