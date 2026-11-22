//! Weibo end to end: mapp short links, mid-to-id status links, fid videos,
//! articles and risk-control responses.

use karta_core::Platform;
use karta_core::data::Content;
use karta_core::error::ParserError;
use serde_json::json;
use test_harness::{FakeUpstream, Fixtures, Reply, TempApp};

const FIXTURES: Fixtures = Fixtures(env!("CARGO_MANIFEST_DIR"));

fn engine(fake: &FakeUpstream) -> (TempApp, karta_core::ParserEngine) {
    TempApp::with_platforms(fake, json!({ "download_media": false }))
}

#[tokio::test]
async fn mapp_short_link_resolves_to_the_status_api() {
    let fake = FakeUpstream::start().await;
    let short = fake
        .route("mapp.api.weibo.cn", "GET", "/fx/8a7b6c5d4e3f.html")
        .reply(Reply::redirect("https://m.weibo.cn/status/PbQ9z1x2Y"));
    let landing = fake.route("m.weibo.cn", "GET", "/status/PbQ9z1x2Y");
    let status = fake
        .route("m.weibo.cn", "GET", "/statuses/show?id=PbQ9z1x2Y")
        .reply(FIXTURES.json("weibo_graphics.json"));
    let (_app, engine) = engine(&fake);

    let result = engine
        .parse("https://mapp.api.weibo.cn/fx/8a7b6c5d4e3f.html")
        .await
        .unwrap()
        .expect("mapp link matches");

    assert_eq!(result.platform, Platform::Weibo);
    assert_eq!(result.text.as_deref(), Some("分享几张美食照片~"));
    assert_eq!(
        result.author.as_ref().map(|a| a.name.as_str()),
        Some("美食达人")
    );
    let images: Vec<_> = result
        .images()
        .into_iter()
        .filter_map(|image| image.url.as_deref())
        .collect();
    assert_eq!(
        images,
        [
            "https://wx1.sinaimg.cn/large/food1.jpg",
            "https://wx1.sinaimg.cn/large/food2.jpg",
            "https://wx1.sinaimg.cn/large/food3.jpg",
        ]
    );

    assert_eq!(short.hits(), 1);
    assert_eq!(landing.hits(), 1);
    let request = status.last_request();
    assert!(
        request
            .query_param("_")
            .is_some_and(|ts| ts.parse::<u64>().is_ok()),
        "cache-busting timestamp: {}",
        request.uri()
    );
    assert_eq!(
        request.header("referer"),
        Some("https://m.weibo.cn/detail/PbQ9z1x2Y")
    );
    assert_eq!(request.header("mweibo-pwa"), Some("1"));
    assert_eq!(request.header("x-requested-with"), Some("XMLHttpRequest"));
    fake.assert_no_unmatched();
}

#[tokio::test]
async fn tv_link_mid_is_converted_to_the_status_id() {
    let fake = FakeUpstream::start().await;
    let status = fake
        .route("m.weibo.cn", "GET", "/statuses/show?id=Nw48JySPy")
        .reply(FIXTURES.json("weibo_status.json"));
    let (_app, engine) = engine(&fake);

    let result = engine
        .parse("https://weibo.com/tv/show/1034:4976424138313924?mid=4976424138313924")
        .await
        .unwrap()
        .expect("weibo tv link matches");

    assert_eq!(result.text.as_deref(), Some("今天的落日真美 #旅行日记#"));
    let Some(Content::Video(video)) = result.contents.first() else {
        panic!("video status: {:?}", result.contents);
    };
    assert_eq!(
        video.url.as_deref(),
        Some("https://f.video.weibocdn.com/mp4_hd.mp4")
    );
    assert_eq!(
        video.cover.as_deref(),
        Some("https://wx1.sinaimg.cn/orj480/cover.jpg")
    );
    assert_eq!(status.hits(), 1);
    fake.assert_no_unmatched();
}

#[tokio::test]
async fn fid_video_posts_the_play_info_component_form() {
    let fake = FakeUpstream::start().await;
    let component = fake
        .route("h5.video.weibo.com", "POST", "/api/component")
        .reply(FIXTURES.json("weibo_fid_video.json"));
    let (_app, engine) = engine(&fake);

    let result = engine
        .parse("https://video.weibo.com/show?fid=1034:5012345678901234")
        .await
        .unwrap()
        .expect("fid link matches");

    assert_eq!(
        result.author.as_ref().map(|a| a.name.as_str()),
        Some("FID视频作者")
    );
    let Some(Content::Video(video)) = result.contents.first() else {
        panic!("fid video: {:?}", result.contents);
    };
    assert_eq!(
        video.url.as_deref(),
        Some("https://f.video.weibocdn.com/test_fid_hd.mp4")
    );

    let request = component.last_request();
    assert_eq!(
        request.query_param("page").as_deref(),
        Some("/show/1034:5012345678901234")
    );
    assert_eq!(
        request.header("content-type"),
        Some("application/x-www-form-urlencoded")
    );
    assert_eq!(
        request.body_str(),
        r#"data={"Component_Play_Playinfo":{"oid":"1034:5012345678901234"}}"#
    );
    assert_eq!(
        request.header("referer"),
        Some("https://h5.video.weibo.com/show/1034:5012345678901234")
    );
    fake.assert_no_unmatched();
}

#[tokio::test]
async fn article_link_reads_the_card_detail_api() {
    let fake = FakeUpstream::start().await;
    let detail = fake
        .route(
            "card.weibo.com",
            "GET",
            "/article/m/aj/detail?id=2309404962180771742222",
        )
        .reply(FIXTURES.json("weibo_article.json"));
    let (_app, engine) = engine(&fake);

    let result = engine
        .parse("https://weibo.com/ttarticle/p/show?id=2309404962180771742222")
        .await
        .unwrap()
        .expect("article link matches");

    assert_eq!(result.title.as_deref(), Some("测试微博文章标题"));
    assert_eq!(
        result.url.as_deref(),
        Some("https://weibo.com/ttarticle/p/show?id=2309404962180771742222")
    );
    assert_eq!(result.contents.len(), 1);
    assert!(
        result
            .text
            .as_deref()
            .is_some_and(|text| text.contains("这是微博文章正文内容"))
    );
    assert_eq!(
        result.author.as_ref().map(|a| a.name.as_str()),
        Some("文章作者")
    );
    assert_eq!(result.timestamp, Some(1_704_067_200));
    assert!(
        result
            .text
            .as_deref()
            .is_some_and(|text| text.contains("第二段落内容")),
        "{:?}",
        result.text
    );
    let request = detail.last_request();
    assert!(
        request
            .query_param("_rid")
            .is_some_and(|rid| !rid.is_empty())
    );
    assert!(request.query_param("_t").is_some_and(|t| !t.is_empty()));
    assert_eq!(request.header("referer"), Some("https://card.weibo.com/"));
    fake.assert_no_unmatched();
}

#[tokio::test]
async fn risk_control_status_is_reported_as_blocked() {
    let fake = FakeUpstream::start().await;
    let status = fake
        .route("m.weibo.cn", "GET", "/statuses/show?id=PbQ9z1x2Y")
        .reply(Reply::status(418).body("I'm a teapot"));
    let (_app, engine) = engine(&fake);

    let err = engine
        .parse("https://m.weibo.cn/detail/PbQ9z1x2Y")
        .await
        .unwrap_err();

    assert!(
        matches!(&err, ParserError::Api { code: 418, message } if message == "被风控拦截"),
        "{err:?}"
    );
    assert_eq!(status.hits(), 1);
    fake.assert_no_unmatched();
}
