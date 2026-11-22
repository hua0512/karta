//! Douyin parser end to end against the fake upstream: message link →
//! requests → `ParseResult`.
//!
//! None of these scenarios reach the share-page fallback anonymously, so the
//! process-wide anonymous ttwid is never registered here; that flow has its
//! own test binary (`e2e_douyin_share_page.rs`).

use karta_core::constants::MOBILE_USER_AGENT;
use karta_core::data::{Content, ParseResult, VideoContent};
use karta_core::error::ParserError;
use karta_core::{ParserEngine, Platform};
use serde_json::{Value, json};
use test_harness::{CapturedRequest, FakeUpstream, Fixtures, Reply, Route, TempApp};

const FIXTURES: Fixtures = Fixtures(env!("CARGO_MANIFEST_DIR"));

/// The Android UA the parser signs and sends API requests with.
const DOUYIN_UA: &str = "Mozilla/5.0 (Linux; Android 5.0; SM-G900P Build/LRX21T) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/70.0.3538.25 Mobile Safari/537.36";
const COOKIE: &str = "sessionid=douyin-session";

const VIDEO_ID: &str = "7521023890996514083";
/// `play_addr.uri` of the aweme detail fixture.
const PLAY_URI: &str = "v0200fg10000test1234567890abcdef";
const ARTICLE_ID: &str = "7684141979752212594";

struct Douyin {
    fake: FakeUpstream,
    // Owns the data dir and config the engine uses.
    _app: TempApp,
    engine: ParserEngine,
}

impl Douyin {
    async fn start(overrides: Value) -> Self {
        let fake = FakeUpstream::start().await;
        let (app, engine) = TempApp::with_platforms(&fake, overrides);
        Self {
            fake,
            _app: app,
            engine,
        }
    }

    async fn anonymous() -> Self {
        Self::start(json!({ "download_media": false })).await
    }

    async fn parse(&self, text: &str) -> ParseResult {
        let result = self
            .engine
            .parse(text)
            .await
            .unwrap_or_else(|e| panic!("parse {text:?}: {e}"))
            .unwrap_or_else(|| panic!("no parser claimed {text:?}"));
        assert_eq!(result.platform, Platform::Douyin);
        (*result).clone()
    }

    /// Anonymous detail API for `id`, answering `reply`.
    fn anonymous_detail(&self, id: &str, reply: Reply) -> Route {
        self.fake
            .route(
                "www.douyin.com",
                "GET",
                &format!("/aweme/v1/web/aweme/detail/?aweme_id={id}&aid=6383"),
            )
            .reply(reply)
    }

    /// CDN ratio probes for the fixture video: 1080p answers without a size
    /// (not served), 720p advertises one.
    fn cdn_probes(&self) -> (Route, Route) {
        let route = |ratio: &str| {
            self.fake.route(
                "aweme.snssdk.com",
                "GET",
                &format!("/aweme/v1/play/?video_id={PLAY_URI}&ratio={ratio}&line=0"),
            )
        };
        let full_hd = route("1080p").reply(Reply::ok());
        let hd = route("720p").reply(
            Reply::status(206)
                .header("Content-Range", "bytes 0-1/12345")
                .body("ab"),
        );
        (full_hd, hd)
    }

    /// A short link that redirects to `location`, setting a ttwid like the
    /// real one, plus the landing page the redirect resolution fetches.
    fn short_link(&self, code: &str, location: &str, landing_host: &str, landing_path: &str) {
        self.fake
            .route("v.douyin.com", "GET", &format!("/{code}"))
            .reply(Reply::redirect(location).set_cookie("ttwid=short-link-ttwid; Path=/"));
        self.fake.route(landing_host, "GET", landing_path);
    }
}

#[track_caller]
fn only_video(result: &ParseResult) -> &VideoContent {
    match result.contents.as_slice() {
        [Content::Video(video)] => video,
        other => panic!("expected one video, got {other:?}"),
    }
}

#[track_caller]
fn extra<'a>(result: &'a ParseResult, key: &str) -> Option<&'a str> {
    result.extra.get(key).map(String::as_str)
}

#[track_caller]
fn assert_probe(request: &CapturedRequest) {
    assert_eq!(request.header("range"), Some("bytes=0-1"));
    assert_eq!(request.header("user-agent"), Some(MOBILE_USER_AGENT));
    assert_eq!(request.header("referer"), Some("https://www.douyin.com/"));
}

#[tokio::test]
async fn video_link_uses_the_anonymous_detail_api_and_probes_the_cdn_ratio() {
    let inputs = [
        format!("https://www.douyin.com/video/{VIDEO_ID}"),
        "看看 https://v.douyin.com/vid123/ 复制此链接".to_string(),
    ];

    for input in inputs {
        let douyin = Douyin::anonymous().await;
        douyin.short_link(
            "vid123",
            &format!("https://www.iesdouyin.com/share/video/{VIDEO_ID}/?region=CN"),
            "www.iesdouyin.com",
            &format!("/share/video/{VIDEO_ID}/"),
        );
        let detail = douyin.anonymous_detail(VIDEO_ID, FIXTURES.json("douyin_aweme_detail.json"));
        let (full_hd, hd) = douyin.cdn_probes();

        let result = douyin.parse(&input).await;

        // A single-line caption is body text, not a title.
        assert_eq!(result.title, None, "{input}");
        assert_eq!(
            result.text.as_deref(),
            Some("测试视频描述 #抖音"),
            "{input}"
        );
        assert_eq!(result.timestamp, Some(1704067200), "{input}");
        let author = result.author.as_ref().expect("author");
        assert_eq!(author.name, "测试作者");
        assert_eq!(
            author.avatar.as_deref(),
            Some("https://p3-pc.douyinpic.com/aweme/test_avatar.jpeg")
        );
        let video = only_video(&result);
        // 1080p is not served, so the URL is rewritten to the probed ratio.
        assert_eq!(
            video.url.as_deref(),
            Some(
                "https://aweme.snssdk.com/aweme/v1/play/?video_id=v0200fg10000test1234567890abcdef&ratio=720p&line=0"
            ),
            "{input}"
        );
        assert_eq!(
            video.cover.as_deref(),
            Some("https://p3-pc-sign.douyinpic.com/test_cover.jpeg")
        );
        assert_eq!(video.duration, Some(30));

        // Anonymous requests use the Open Platform contract, unsigned.
        let request = detail.last_request();
        assert_eq!(request.header("origin"), Some("https://open.douyin.com"));
        assert_eq!(request.header("referer"), Some("https://open.douyin.com/"));
        assert_eq!(request.header("user-agent"), Some(DOUYIN_UA));
        assert_eq!(request.header("cookie"), None);
        assert_eq!(request.query_param("a_bogus"), None);
        assert_eq!((full_hd.hits(), hd.hits()), (1, 1), "{input}");
        assert_probe(&full_hd.last_request());
        assert_probe(&hd.last_request());
        douyin.fake.assert_no_unmatched();
    }
}

#[tokio::test]
async fn cookie_signs_the_detail_request() {
    let douyin = Douyin::start(json!({ "download_media": false, "douyin_cookie": COOKIE })).await;
    let detail = douyin
        .fake
        .route(
            "www.douyin.com",
            "GET",
            &format!(
                "/aweme/v1/web/aweme/detail/?aweme_id={VIDEO_ID}&device_platform=webapp&aid=6383&a_bogus"
            ),
        )
        .reply(FIXTURES.json("douyin_aweme_detail.json"));
    douyin.cdn_probes();

    let result = douyin
        .parse(&format!("https://www.douyin.com/video/{VIDEO_ID}"))
        .await;

    assert_eq!(result.text.as_deref(), Some("测试视频描述 #抖音"));
    let request = detail.last_request();
    assert_eq!(request.header("cookie"), Some(COOKIE));
    assert_eq!(request.header("referer"), Some("https://www.douyin.com/"));
    assert_eq!(request.header("origin"), Some("https://open.douyin.com"));
    let a_bogus = request.query_param("a_bogus").expect("a_bogus");
    assert_eq!(a_bogus.len(), 164, "{a_bogus}");
    // The signature is the last parameter, after the web parameter set.
    assert!(request.uri().ends_with(&format!("&a_bogus={a_bogus}")));
    douyin.fake.assert_no_unmatched();
}

fn live_room(title: &str, user_count: u64) -> Value {
    json!({
        "title": title,
        "cover": { "url_list": ["https://p3-webcast.douyinpic.com/live_cover.jpeg"] },
        "user_count": user_count,
    })
}

#[track_caller]
fn assert_live(result: &ParseResult, title: &str, viewers: &str) {
    assert_eq!(result.title.as_deref(), Some(title));
    assert_eq!(extra(result, "在线观看"), Some(viewers));
    assert_eq!(extra(result, "类型"), Some("直播"));
    let [Content::Image(cover)] = result.contents.as_slice() else {
        panic!("expected the live cover: {:?}", result.contents);
    };
    assert_eq!(
        cover.url.as_deref(),
        Some("https://p3-webcast.douyinpic.com/live_cover.jpeg")
    );
}

#[tokio::test]
async fn live_links_use_the_signed_enter_api() {
    for input in [
        "https://live.douyin.com/23585383",
        "https://www.douyin.com/live/23585383",
        "https://v.douyin.com/live123/",
    ] {
        let douyin = Douyin::anonymous().await;
        douyin.short_link(
            "live123",
            "https://live.douyin.com/23585383?from=share",
            "live.douyin.com",
            "/23585383",
        );
        let enter = douyin
            .fake
            .route(
                "live.douyin.com",
                "GET",
                "/webcast/room/web/enter/?web_rid=23585383&room_id_str=23585383&a_bogus",
            )
            .reply(
                Reply::ok().json(&json!({ "data": { "data": [live_room("测试直播间", 1234)] } })),
            );

        let result = douyin.parse(input).await;

        assert_live(&result, "测试直播间", "1234人");
        assert_eq!(enter.hits(), 1, "{input}");
        assert_eq!(
            enter.last_request().header("referer"),
            Some("https://open.douyin.com/")
        );
        douyin.fake.assert_no_unmatched();
    }
}

#[tokio::test]
async fn webcast_links_use_the_reflow_api_with_the_short_link_ttwid() {
    // (input, msToken sent): only a short link hands out a ttwid.
    let cases = [
        (
            "https://webcast.amemv.com/douyin/webcast/reflow/7300000000000000000",
            "",
        ),
        ("https://v.douyin.com/reflow1/", "short-link-ttwid"),
    ];

    for (input, ms_token) in cases {
        let douyin = Douyin::anonymous().await;
        douyin.short_link(
            "reflow1",
            "https://webcast.amemv.com/douyin/webcast/reflow/7300000000000000000?u_code=1",
            "webcast.amemv.com",
            "/douyin/webcast/reflow/7300000000000000000",
        );
        let reflow = douyin
            .fake
            .route(
                "webcast.amemv.com",
                "GET",
                "/webcast/room/reflow/info/?room_id=7300000000000000000&app_id=1128",
            )
            .reply(Reply::ok().json(&json!({ "data": { "room": live_room("回流直播间", 56) } })));

        let result = douyin.parse(input).await;

        assert_live(&result, "回流直播间", "56人");
        let request = reflow.last_request();
        assert_eq!(
            request.query_param("msToken").as_deref(),
            Some(ms_token),
            "{input}"
        );
        let verify_fp = request.query_param("verifyFp").expect("verifyFp");
        assert!(verify_fp.starts_with("verify_"), "{verify_fp}");
        douyin.fake.assert_no_unmatched();
    }
}

#[tokio::test]
async fn slides_use_the_slides_api() {
    for input in [
        "https://www.douyin.com/share/slides/7500000000000000001",
        "https://v.douyin.com/slide12/",
    ] {
        let douyin = Douyin::anonymous().await;
        douyin.short_link(
            "slide12",
            "https://www.iesdouyin.com/share/slides/7500000000000000001/",
            "www.iesdouyin.com",
            "/share/slides/7500000000000000001/",
        );
        let slides = douyin
            .fake
            .route(
                "www.iesdouyin.com",
                "GET",
                "/web/api/v2/aweme/slidesinfo/?request_source=200",
            )
            .reply(Reply::ok().json(&json!({
                "aweme_details": [{
                    "desc": "图集标题\n图集正文",
                    "create_time": 1704067300,
                    "author": { "nickname": "图集作者" },
                    "images": [
                        {
                            "url_list": ["https://p3-sign.douyinpic.com/slide_1.jpeg"],
                            "video": { "play_addr": { "url_list": ["https://v26.douyinvod.com/slide_1.mp4"] } }
                        },
                        { "url_list": ["https://p3-sign.douyinpic.com/slide_2.jpeg"] }
                    ]
                }]
            })));

        let result = douyin.parse(input).await;

        assert_eq!(result.title.as_deref(), Some("图集标题"), "{input}");
        assert_eq!(result.text.as_deref(), Some("图集正文"));
        assert_eq!(result.timestamp, Some(1704067300));
        let [Content::Video(motion), Content::Image(still)] = result.contents.as_slice() else {
            panic!(
                "expected a video slide then an image: {:?}",
                result.contents
            );
        };
        assert_eq!(
            motion.url.as_deref(),
            Some("https://v26.douyinvod.com/slide_1.mp4")
        );
        assert_eq!(
            motion.cover.as_deref(),
            Some("https://p3-sign.douyinpic.com/slide_1.jpeg")
        );
        assert_eq!(
            still.url.as_deref(),
            Some("https://p3-sign.douyinpic.com/slide_2.jpeg")
        );
        assert_eq!(
            slides.last_request().query_param("aweme_ids").as_deref(),
            Some("%5B7500000000000000001%5D")
        );
        douyin.fake.assert_no_unmatched();
    }
}

#[tokio::test]
async fn article_share_token_resolves_through_the_carrier() {
    let douyin = Douyin::anonymous().await;
    let carrier_path =
        "/aweme/share/carrier/parse/?carrier_type=3&code_info=5w0ncBiCzhF99&aid=1128";
    // The first app host fails; the next one answers.
    let primary = douyin
        .fake
        .route("api5-normal-c-hl.amemv.com", "GET", carrier_path)
        .reply(Reply::status(503));
    let secondary = douyin
        .fake
        .route("aweme.snssdk.com", "GET", carrier_path)
        .reply(Reply::ok().json(&json!({
            "status_code": 0,
            "data": {
                "schema": format!("aweme://article/detail?item_id={ARTICLE_ID}&enter_from=share"),
                "share_scene": "long_article"
            }
        })));
    let detail = douyin.anonymous_detail(ARTICLE_ID, FIXTURES.json("douyin_article_detail.json"));

    let result = douyin
        .parse("4-M:/. 09/19 【当了三年朋友，她突然不干了】长按复制打开抖音，即可阅读文章 ˇˇ5w0ncBiCzhF99➝➝")
        .await;

    assert_eq!(result.title.as_deref(), Some("当了三年朋友，她突然不干了"));
    assert_eq!(result.timestamp, Some(1_789_103_723));
    // Query-stripped share URL.
    assert_eq!(
        result.url.as_deref(),
        Some("https://www.iesdouyin.com/share/video/7684141979752212594/")
    );
    let author = result.author.as_ref().expect("author");
    assert_eq!(author.name, "小岛失眠中");
    assert_eq!(author.uid.as_deref(), Some("867969968655273"));
    assert_eq!(author.description.as_deref(), Some("还好你来了."));
    assert_eq!(extra(&result, "类型"), Some("文章"));
    // The placeholder video of an article envelope never leaks through.
    assert!(
        !result
            .contents
            .iter()
            .any(|content| matches!(content, Content::Video(_))),
        "{:?}",
        result.contents
    );
    assert_eq!((primary.hits(), secondary.hits(), detail.hits()), (1, 1, 1));
    assert_eq!(
        secondary.last_request().header("user-agent"),
        Some(DOUYIN_UA)
    );
    douyin.fake.assert_no_unmatched();
}

#[tokio::test]
async fn long_article_body_renders_in_order_across_chunks() {
    let douyin = Douyin::anonymous().await;
    let long = "长文🙂".repeat(2000);
    let ending = "后续段落：故事到这里才结束。";
    let markdown = format!("{long}\n\n![](https://img.example/1.jpg)\n\n{ending}");
    douyin.anonymous_detail(
        "7600000000000000001",
        Reply::ok().json(&json!({
            "status_code": 0,
            "aweme_detail": {
                "aweme_id": "7600000000000000001",
                "aweme_type": 0,
                "desc": "长文",
                "create_time": 5,
                "author": { "nickname": "作者" },
                "video": { "duration": 0, "cover": { "url_list": ["https://img.example/feed-cover.jpg"] } },
                "article_info": {
                    "article_id": "1",
                    "article_title": "长文标题",
                    "article_content": json!({ "markdown": markdown }).to_string(),
                    "fe_data": "{}",
                    "read_time": 0
                }
            }
        })),
    );

    let result = douyin
        .parse("https://www.douyin.com/note/7600000000000000001")
        .await;
    let rendered = douyin.engine.render(&result).await.expect("render");

    // The first message is metadata; the rest carry the body in order.
    let messages: Vec<_> = rendered.iter().flat_map(|o| o.messages()).collect();
    let segments: Vec<_> = messages.iter().skip(1).flat_map(|m| &m.segments).collect();
    let texts: Vec<_> = segments.iter().filter_map(|s| s.text.as_deref()).collect();
    assert!(texts.iter().all(|t| t.chars().count() <= 800));
    assert_eq!(texts.concat(), format!("{long}{ending}"));
    let image_index = segments
        .iter()
        .position(|s| s.url.as_deref() == Some("https://img.example/1.jpg"))
        .expect("body image is rendered");
    assert!(segments[..image_index].iter().any(|s| s.text.is_some()));
    assert!(
        segments[image_index + 1..]
            .iter()
            .any(|s| s.text.as_deref() == Some(ending))
    );
    douyin.fake.assert_no_unmatched();
}

#[tokio::test]
async fn filtered_item_reports_the_server_reason_after_every_fallback_fails() {
    let douyin = Douyin::start(json!({ "download_media": false, "douyin_cookie": COOKIE })).await;
    let detail = douyin
        .fake
        .route(
            "www.douyin.com",
            "GET",
            &format!("/aweme/v1/web/aweme/detail/?aweme_id={VIDEO_ID}"),
        )
        .reply(Reply::ok().json(&json!({
            "status_code": 0,
            "filter_list": [{ "aweme_id": VIDEO_ID, "filter_reason": "author_secret" }],
            "aweme_detail": null
        })));
    let share_pages: Vec<Route> = [
        ("m.douyin.com", "video"),
        ("m.douyin.com", "note"),
        ("www.iesdouyin.com", "video"),
        ("www.iesdouyin.com", "note"),
    ]
    .into_iter()
    .map(|(host, kind)| {
        douyin
            .fake
            .route(host, "GET", &format!("/share/{kind}/{VIDEO_ID}"))
            .reply(Reply::ok().content_type("text/html").body("<html></html>"))
    })
    .collect();

    let error = douyin
        .engine
        .parse(&format!("https://www.douyin.com/video/{VIDEO_ID}"))
        .await
        .expect_err("a filtered item has no content");

    match error {
        ParserError::ContentNotFound(message) => {
            assert_eq!(message, "该作品为私密或受可见范围限制 (author_secret)");
        }
        other => panic!("expected ContentNotFound, got {other:?}"),
    }
    assert_eq!(detail.hits(), 1);
    for page in &share_pages {
        assert_eq!(page.hits(), 1);
        // With a configured cookie no anonymous ttwid is attached.
        assert_eq!(page.last_request().header("cookie"), None);
    }
    douyin.fake.assert_no_unmatched();
}
