//! Hongguo parser end to end against the fake upstream: SSR pages for
//! episodes, series cards and motion comics, plus the authorized
//! motion-comic video API and its fallback to the page gallery.

use std::path::PathBuf;

use karta_core::data::{Content, ParseResult};
use karta_core::{ParserEngine, Platform};
use serde_json::{Value, json};
use test_harness::{FakeUpstream, Reply, Route, TempApp};

const HOST: &str = "hongguoduanju.com";
const SERIES_ID: &str = "7546798920437877785";
const BOOK_ID: &str = "7111943588446997534";
/// The comic fixture's `next_vid`.
const COMIC_VID: &str = "7111943588446997600";
const API_HOST: &str = "reading.example.test";
const API_PATH: &str = "/novel/player/multi_video_model/";
/// A `spade_a` using the `app_v2` transform, wrapping this CENC key.
const SPADE_A: &str = "pPJgUllZw/NgVF0d/fmX19+f+voVQmcX29NKUgUVysheBlE2JycIIWU=";
const CENC_KEY: &str = "00112233445566778899aabbccddeeff";
const APP_USER_AGENT: &str = "com.dragon.read/72732 (Linux; U; Android 14; zh_CN; Pixel 7; Build/UP1A.231105.003; Cronet/TTNetVersion:5f3d03c5)";

/// An SSR page embedding the router fixture `name`.
fn ssr_page(name: &str) -> Reply {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    let data = std::fs::read_to_string(path).expect("read fixture");
    Reply::ok().content_type("text/html").body(format!(
        "<html><head></head><body><script>window._ROUTER_DATA = {data};</script></body></html>"
    ))
}

struct Hongguo {
    fake: FakeUpstream,
    // Owns the data dir and config the engine uses.
    _app: TempApp,
    engine: ParserEngine,
}

impl Hongguo {
    async fn start(overrides: Value) -> Self {
        let fake = FakeUpstream::start().await;
        let (app, engine) = TempApp::with_platforms(&fake, overrides);
        Self {
            fake,
            _app: app,
            engine,
        }
    }

    /// With an authorized motion-comic session pointing at the fake API.
    async fn authorized() -> Self {
        Self::start(json!({
            "download_media": false,
            "hongguo_api_auth": {
                "url": format!("https://{API_HOST}{API_PATH}"),
                "params": { "device_id": "1234567890" },
                "headers": { "Cookie": "sessionid=hongguo-session" },
                "body": { "biz_param": { "caller_scene": "play" } }
            }
        }))
        .await
    }

    async fn parse(&self, text: &str) -> ParseResult {
        let result = self
            .engine
            .parse(text)
            .await
            .unwrap_or_else(|e| panic!("parse {text:?}: {e}"))
            .unwrap_or_else(|| panic!("no parser claimed {text:?}"));
        assert_eq!(result.platform, Platform::Hongguo);
        (*result).clone()
    }

    fn comic_page(&self) -> Route {
        self.fake
            .route(HOST, "GET", &format!("/comic/{BOOK_ID}"))
            .reply(ssr_page("hongguo_comic_router_data.json"))
    }

    fn motion_comic_api(&self, reply: Reply) -> Route {
        self.fake
            .route(
                API_HOST,
                "POST",
                &format!("{API_PATH}?device_id=1234567890&iid=1234567890&aid=8662"),
            )
            .reply(reply)
    }
}

#[track_caller]
fn extra<'a>(result: &'a ParseResult, key: &str) -> Option<&'a str> {
    result.extra.get(key).map(String::as_str)
}

#[tokio::test]
async fn player_page_builds_the_episode_video() {
    let hongguo = Hongguo::start(json!({ "download_media": false })).await;
    let page = hongguo
        .fake
        .route(HOST, "GET", &format!("/player/{SERIES_ID}/1234567890"))
        .reply(ssr_page("hongguo_router_data.json"));

    let result = hongguo
        .parse(&format!(
            "https://hongguoduanju.com/player/{SERIES_ID}/1234567890"
        ))
        .await;

    assert_eq!(result.title.as_deref(), Some("测试短剧"));
    assert_eq!(result.text.as_deref(), Some("这是一个测试简介"));
    assert_eq!(
        result.url.as_deref(),
        Some("https://hongguoduanju.com/player/7546798920437877785/1234567890")
    );
    assert_eq!(extra(&result, "series_id"), Some(SERIES_ID));
    assert_eq!(extra(&result, "vid"), Some("1234567890"));
    let [Content::Video(video)] = result.contents.as_slice() else {
        panic!("expected one video: {:?}", result.contents);
    };
    assert_eq!(
        video.url.as_deref(),
        Some("https://cdn.example.com/v.mp4?dy_q=1700000000")
    );
    // The episode poster wins over the series cover.
    assert_eq!(
        video.cover.as_deref(),
        Some("https://example.com/poster.jpg")
    );
    assert_eq!(video.duration, Some(120));
    assert_eq!((video.width, video.height), (Some(720), Some(1280)));
    assert_eq!(video.quality.as_deref(), Some("720p"));
    assert_eq!(
        page.last_request().header("referer"),
        Some("https://hongguoduanju.com/")
    );
    hongguo.fake.assert_no_unmatched();
}

#[tokio::test]
async fn detail_page_builds_the_series_card() {
    let hongguo = Hongguo::start(json!({ "download_media": false })).await;
    hongguo
        .fake
        .route(HOST, "GET", &format!("/detail?series_id={SERIES_ID}"))
        .reply(ssr_page("hongguo_router_data.json"));

    let result = hongguo
        .parse(&format!(
            "https://hongguoduanju.com/detail?series_id={SERIES_ID}"
        ))
        .await;

    assert_eq!(result.title.as_deref(), Some("测试短剧"));
    assert_eq!(
        result.text.as_deref(),
        Some("这是一个测试简介\n\n标签: 都市 / 逆袭")
    );
    assert_eq!(extra(&result, "episode_count"), Some("80"));
    assert_eq!(extra(&result, "accessible_episode_count"), Some("12"));
    let [Content::Image(cover)] = result.contents.as_slice() else {
        panic!("expected the series cover: {:?}", result.contents);
    };
    assert_eq!(cover.url.as_deref(), Some("https://example.com/cover.jpg"));
    hongguo.fake.assert_no_unmatched();
}

/// The comic fixture's readable chapter, as a gallery.
#[track_caller]
fn assert_comic_gallery(result: &ParseResult) {
    assert_eq!(result.title.as_deref(), Some("测试漫剧"));
    assert_eq!(result.text.as_deref(), Some("漫剧 · 共 3 页"));
    assert_eq!(extra(result, "book_id"), Some(BOOK_ID));
    assert_eq!(extra(result, "vid"), Some(COMIC_VID));
    assert_eq!(extra(result, "readable_episode_count"), Some("3"));
    let [Content::Graphics(gallery)] = result.contents.as_slice() else {
        panic!("expected one gallery: {:?}", result.contents);
    };
    let pages: Vec<_> = gallery
        .images
        .iter()
        .map(|image| (image.url.as_deref(), image.width, image.height))
        .collect();
    assert_eq!(
        pages,
        [
            (
                Some("https://p1.example.com/comic/page1.png~tplv-x.image"),
                Some(1080),
                Some(1920)
            ),
            (
                Some("https://p1.example.com/comic/page2.png~tplv-x.image"),
                Some(1080),
                Some(1920)
            ),
            // An empty `pic_url` falls back to `low_pic_url`; zero sizes are dropped.
            (
                Some("https://p1.example.com/comic/page3_low.png"),
                None,
                None
            ),
        ]
    );
}

#[tokio::test]
async fn comic_page_builds_the_page_gallery_without_auth() {
    let hongguo = Hongguo::start(json!({ "download_media": false })).await;
    hongguo.comic_page();

    let result = hongguo
        .parse(&format!("https://hongguoduanju.com/comic/{BOOK_ID}"))
        .await;

    assert_comic_gallery(&result);
    // No session: the motion-comic API is never called.
    hongguo.fake.assert_no_unmatched();
}

#[tokio::test]
async fn authorized_comic_resolves_the_encrypted_motion_comic_video() {
    let hongguo = Hongguo::authorized().await;
    hongguo.comic_page();
    let video_model = json!({
        "video_duration": 93.4,
        "video_list": [
            {
                "main_url": "https://v3-novel.example.com/low.mp4",
                "video_meta": { "vwidth": 720, "vheight": 1280, "definition": "720p", "size": 1000 }
            },
            {
                "main_url": "https://v3-novel.example.com/high.mp4",
                "video_meta": { "vwidth": 1080, "vheight": 1920, "definition": "1080p", "size": 2000 },
                "encrypt_info": {
                    "encrypt": true,
                    "encryption_method": "cenc-aes-ctr",
                    "spade_a": SPADE_A
                }
            }
        ]
    });
    let api = hongguo.motion_comic_api(Reply::ok().json(&json!({
        "code": 0,
        "data": { COMIC_VID: { "video_model": video_model.to_string() } }
    })));

    let result = hongguo
        .parse(&format!("https://hongguoduanju.com/comic/{BOOK_ID}"))
        .await;

    assert_eq!(result.title.as_deref(), Some("测试漫剧"));
    assert_eq!(extra(&result, "book_id"), Some(BOOK_ID));
    assert_eq!(extra(&result, "vid"), Some(COMIC_VID));
    let [Content::Video(video)] = result.contents.as_slice() else {
        panic!("expected one video: {:?}", result.contents);
    };
    assert_eq!(
        video.url.as_deref(),
        Some("https://v3-novel.example.com/high.mp4")
    );
    assert_eq!(video.decryption_key.as_deref(), Some(CENC_KEY));
    assert_eq!(video.duration, Some(93));
    assert_eq!((video.width, video.height), (Some(1080), Some(1920)));
    assert_eq!(video.size, Some(2000));
    assert_eq!(video.quality.as_deref(), Some("1080p"));
    assert_eq!(
        video.download_headers,
        Some(vec![("User-Agent".to_string(), APP_USER_AGENT.to_string())])
    );

    let request = api.last_request();
    // Configured headers override the app defaults; the app UA stays.
    assert_eq!(request.header("cookie"), Some("sessionid=hongguo-session"));
    assert_eq!(request.header("user-agent"), Some(APP_USER_AGENT));
    assert_eq!(
        request.query_param("version_code").as_deref(),
        Some("72732")
    );
    let body: Value = serde_json::from_slice(&request.body).expect("JSON body");
    assert_eq!(body["mixed_video_id_map"]["1004"], json!([COMIC_VID]));
    // The configured body overlays the default one, keeping its siblings.
    assert_eq!(body["biz_param"]["caller_scene"], "play");
    assert_eq!(body["biz_param"]["video_platform"], 1024);
    hongguo.fake.assert_no_unmatched();
}

#[tokio::test]
async fn rejected_motion_comic_api_falls_back_to_the_page_gallery() {
    let hongguo = Hongguo::authorized().await;
    hongguo.comic_page();
    let api = hongguo.motion_comic_api(
        Reply::ok().json(&json!({ "code": 101000, "message": "服务异常", "data": {} })),
    );

    let result = hongguo
        .parse(&format!("https://hongguoduanju.com/comic/{BOOK_ID}"))
        .await;

    assert_comic_gallery(&result);
    assert_eq!(api.hits(), 1);
    hongguo.fake.assert_no_unmatched();
}
