//! Bilibili parser end to end against the fake upstream: message link →
//! requests → `ParseResult`, for every link kind the parser handles.
//!
//! Every parse starts by fetching WBI keys from the nav endpoint; the fake
//! hands out fixed keys, so the `w_rid` of each signed request is checked
//! against an independent computation of the documented algorithm.

use karta_core::data::{Content, ImageContent, ParseResult, VideoContent};
use karta_core::error::ParserError;
use karta_core::{ParserEngine, Platform};
use serde_json::{Value, json};
use test_harness::{CapturedRequest, FakeUpstream, Fixtures, Reply, Route, TempApp};

const FIXTURES: Fixtures = Fixtures(env!("CARGO_MANIFEST_DIR"));

const API: &str = "api.bilibili.com";
const WWW: &str = "www.bilibili.com";

/// Keys from the bilibili-API-collect WBI example and the mixin key they give.
const IMG_KEY: &str = "7cd084941338484aae1ad9425b84077c";
const SUB_KEY: &str = "4932caff0ff746eab6f01bf08b70ac45";
const MIXIN_KEY: &str = "ea1db124af3c7062474693fa704f4ff8";
/// Mixin key for the same two keys swapped (a rotated key pair).
const ROTATED_MIXIN_KEY: &str = "4af39007a1f5828008aecf30cae44936";

const COOKIE: &str = "SESSDATA=sess; bili_jct=csrf; DedeUserID=1";

/// nav as an anonymous visitor sees it: code -101, keys still present.
fn nav_reply(img_key: &str, sub_key: &str) -> Reply {
    Reply::ok().json(&json!({
        "code": -101,
        "message": "账号未登录",
        "data": {
            "isLogin": false,
            "wbi_img": {
                "img_url": format!("https://i0.hdslb.com/bfs/wbi/{img_key}.png"),
                "sub_url": format!("https://i0.hdslb.com/bfs/wbi/{sub_key}.png"),
            }
        }
    }))
}

/// A fake upstream with the nav endpoint routed, and an engine over the real
/// parsers configured with `overrides`.
struct Bili {
    fake: FakeUpstream,
    nav: Route,
    // Owns the data dir and config the engine uses.
    _app: TempApp,
    engine: ParserEngine,
}

impl Bili {
    async fn start(overrides: Value) -> Self {
        let fake = FakeUpstream::start().await;
        let nav = fake
            .route(API, "GET", "/x/web-interface/nav")
            .reply(nav_reply(IMG_KEY, SUB_KEY));
        let (app, engine) = TempApp::with_platforms(&fake, overrides);
        Self {
            fake,
            nav,
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
        assert_eq!(result.platform, Platform::Bilibili);
        (*result).clone()
    }

    /// view for `BV1xx411c7mD` (the two-page video fixture).
    fn view(&self) -> Route {
        self.fake
            .route(API, "GET", "/x/web-interface/view?bvid=BV1xx411c7mD")
            .reply(FIXTURES.json("bilibili_video_info.json"))
    }

    /// playurl for `BV1xx411c7mD` page `cid`.
    fn playurl(&self, cid: &str, reply: Reply) -> Route {
        self.fake
            .route(
                API,
                "GET",
                &format!("/x/player/playurl?bvid=BV1xx411c7mD&cid={cid}&qn=80&fnval=4048"),
            )
            .reply(reply)
    }
}

/// Assert `request` carries `wts` and the `w_rid` the WBI algorithm gives for
/// its other query pairs under `mixin_key`.
#[track_caller]
fn assert_wbi_signed(request: &CapturedRequest, mixin_key: &str) {
    let mut pairs = request.query_pairs();
    let w_rid = pairs
        .iter()
        .position(|(key, _)| key == "w_rid")
        .map(|index| pairs.remove(index).1)
        .unwrap_or_else(|| panic!("no w_rid in {}", request.uri()));
    assert!(
        pairs
            .iter()
            .any(|(key, value)| key == "wts" && value.parse::<u64>().is_ok()),
        "no numeric wts in {}",
        request.uri()
    );
    // Every value the parser signs is already URL-safe, so the sent (form
    // encoded) and the signed (RFC 3986) spellings coincide.
    assert!(
        pairs.iter().all(|(_, value)| value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.~".contains(&b))),
        "values need encoding: {}",
        request.uri()
    );
    pairs.sort();
    let query = pairs
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    let expected = format!("{:x}", md5::compute(format!("{query}{mixin_key}")));
    assert_eq!(w_rid, expected, "w_rid of {}", request.uri());
}

#[track_caller]
fn assert_api_headers(request: &CapturedRequest, cookie: Option<&str>) {
    assert_eq!(request.header("referer"), Some("https://www.bilibili.com"));
    assert_eq!(request.header("user-agent"), Some("ChatGPT-User/1.0"));
    assert_eq!(
        request.header("cookie"),
        cookie,
        "cookie on {}",
        request.uri()
    );
}

#[track_caller]
fn only_video(result: &ParseResult) -> &VideoContent {
    match result.contents.as_slice() {
        [Content::Video(video)] => video,
        other => panic!("expected one video, got {other:?}"),
    }
}

#[track_caller]
fn only_image(result: &ParseResult) -> &ImageContent {
    match result.contents.as_slice() {
        [Content::Image(image)] => image,
        other => panic!("expected one image, got {other:?}"),
    }
}

#[track_caller]
fn extra<'a>(result: &'a ParseResult, key: &str) -> Option<&'a str> {
    result.extra.get(key).map(String::as_str)
}

/// The DASH pair the stream fixture yields at the default 1080p.
#[track_caller]
fn assert_fixture_dash(video: &VideoContent) {
    assert_eq!(
        video.video_url.as_deref(),
        Some("https://upos-sz-mirrorcos.bilivideo.com/video_1080p.m4s")
    );
    assert_eq!(
        video.audio_url.as_deref(),
        Some("https://upos-sz-mirrorcos.bilivideo.com/audio_192k.m4s")
    );
    assert_eq!(video.url, None);
    assert_eq!(video.quality.as_deref(), Some("1080p avc1"));
    assert_eq!((video.width, video.height), (Some(1920), Some(1080)));
    let headers = video.download_headers.as_deref().expect("download headers");
    assert!(
        headers
            .iter()
            .any(|(name, value)| name.eq_ignore_ascii_case("range") && value == "bytes=0-"),
        "expected Range: bytes=0- in {headers:?}"
    );
    assert!(
        headers
            .iter()
            .any(|(name, value)| name.eq_ignore_ascii_case("referer")
                && value == "https://www.bilibili.com"),
        "expected Bilibili Referer in {headers:?}"
    );
}

#[tokio::test]
async fn video_link_builds_dash_video_from_signed_requests() {
    let bili = Bili::anonymous().await;
    let view = bili.view();
    let playurl = bili.playurl("99999001", FIXTURES.json("bilibili_stream_info.json"));

    let result = bili
        .parse("看看 https://www.bilibili.com/video/BV1xx411c7mD 这个")
        .await;

    assert_eq!(
        result.title.as_deref(),
        Some("【测试】这是一个测试视频标题 | 分集 - P1 第一集")
    );
    assert_eq!(
        result.url.as_deref(),
        Some("https://www.bilibili.com/video/BV1xx411c7mD")
    );
    assert_eq!(result.text.as_deref(), Some("视频描述信息"));
    assert_eq!(result.timestamp, Some(1704067200));
    let author = result.author.as_ref().expect("author");
    assert_eq!(author.name, "测试UP主");
    assert_eq!(author.uid.as_deref(), Some("12345678"));
    assert_eq!(
        author.avatar.as_deref(),
        Some("https://i1.hdslb.com/bfs/face/test_avatar.jpg")
    );
    assert_eq!(
        author.url.as_deref(),
        Some("https://space.bilibili.com/12345678")
    );
    assert_eq!(
        extra(&result, "stats"),
        Some("👍 5.0万 🪙 1.5万 ⭐ 3.0万 ↩️ 8000 💬 2000 👀 100.0万 💭 5000")
    );
    // No cookie: no AI summary request.
    assert_eq!(extra(&result, "ai_summary"), None);
    let video = only_video(&result);
    assert_fixture_dash(video);
    assert_eq!(video.duration, Some(90));
    assert_eq!(
        video.cover.as_deref(),
        Some("https://i0.hdslb.com/bfs/archive/test_cover.jpg")
    );

    // nav hands out keys to anonymous visitors; both API calls are signed.
    assert_eq!(bili.nav.hits(), 1);
    assert_api_headers(&bili.nav.last_request(), None);
    for route in [&view, &playurl] {
        assert_eq!(route.hits(), 1);
        let request = route.last_request();
        assert!(request.tls);
        assert_api_headers(&request, None);
        assert_wbi_signed(&request, MIXIN_KEY);
    }
    assert_eq!(
        playurl.last_request().query_param("fourk").as_deref(),
        Some("1")
    );
    bili.fake.assert_no_unmatched();
}

#[tokio::test]
async fn cookie_is_forwarded_and_adds_the_ai_summary() {
    let bili = Bili::start(json!({ "download_media": false, "bili_cookie": COOKIE })).await;
    let view = bili.view();
    let playurl = bili.playurl("99999001", FIXTURES.json("bilibili_stream_info.json"));
    let conclusion = bili
        .fake
        .route(
            API,
            "GET",
            "/x/web-interface/view/conclusion/get?bvid=BV1xx411c7mD&cid=99999001&up_mid=0",
        )
        .reply(Reply::ok().json(&json!({
            "code": 0,
            "message": "0",
            "data": { "model_result": { "summary": "视频讲了测试。" } }
        })));

    let result = bili.parse("BV1xx411c7mD").await;

    assert_eq!(extra(&result, "ai_summary"), Some("AI总结: 视频讲了测试。"));
    assert_fixture_dash(only_video(&result));
    assert_api_headers(&bili.nav.last_request(), Some(COOKIE));
    for route in [&view, &conclusion, &playurl] {
        assert_eq!(route.hits(), 1);
        let request = route.last_request();
        assert_api_headers(&request, Some(COOKIE));
        assert_wbi_signed(&request, MIXIN_KEY);
    }
    bili.fake.assert_no_unmatched();
}

#[tokio::test]
async fn dash_without_audio_is_sent_as_one_video_url() {
    let bili = Bili::anonymous().await;
    bili.view();
    let mut stream = FIXTURES.read_json("bilibili_stream_info.json");
    stream["data"]["dash"]["audio"] = Value::Null;
    bili.playurl("99999001", Reply::ok().json(&stream));

    let result = bili
        .parse("https://www.bilibili.com/video/BV1xx411c7mD")
        .await;

    let video = only_video(&result);
    assert_eq!(
        video.url.as_deref(),
        Some("https://upos-sz-mirrorcos.bilivideo.com/video_1080p.m4s")
    );
    assert_eq!(video.video_url, None);
    assert_eq!(video.audio_url, None);
    bili.fake.assert_no_unmatched();
}

#[tokio::test]
async fn page_two_is_selected_from_a_page_link_or_a_short_link() {
    let cases = [
        "https://www.bilibili.com/video/BV1xx411c7mD?p=2",
        "BV1xx411c7mD 2",
        "https://b23.tv/abc123",
        "bili2233.cn/xyz789",
    ];

    for input in cases {
        let bili = Bili::anonymous().await;
        let short = [("b23.tv", "/abc123"), ("bili2233.cn", "/xyz789")].map(|(host, path)| {
            bili.fake.route(host, "GET", path).reply(Reply::redirect(
                "https://www.bilibili.com/video/BV1xx411c7mD?p=2&share_source=copy",
            ))
        });
        let landing = bili.fake.route(WWW, "GET", "/video/BV1xx411c7mD");
        bili.view();
        let playurl = bili.playurl("99999002", FIXTURES.json("bilibili_stream_info.json"));

        let result = bili.parse(input).await;

        assert_eq!(
            result.title.as_deref(),
            Some("【测试】这是一个测试视频标题 | 分集 - P2 第二集"),
            "{input}"
        );
        assert_eq!(
            result.url.as_deref(),
            Some("https://www.bilibili.com/video/BV1xx411c7mD?p=2"),
            "{input}"
        );
        assert_fixture_dash(only_video(&result));
        assert_eq!(playurl.hits(), 1, "{input}");
        let short_hits: usize = short.iter().map(Route::hits).sum();
        let is_short = input.contains("b23") || input.contains("bili2233");
        assert_eq!(short_hits, usize::from(is_short), "{input}");
        assert_eq!(landing.hits(), usize::from(is_short), "{input}");
        bili.fake.assert_no_unmatched();
    }
}

#[tokio::test]
async fn draw_dynamic_builds_an_image_gallery() {
    let bili = Bili::anonymous().await;
    let detail = bili
        .fake
        .route(
            API,
            "GET",
            "/x/polymer/web-dynamic/v1/detail?id=998440765151510535",
        )
        .reply(FIXTURES.json("bilibili_dynamic_info.json"));

    let result = bili
        .parse("https://t.bilibili.com/998440765151510535")
        .await;

    assert_eq!(
        result.url.as_deref(),
        Some("https://t.bilibili.com/998440765151510535")
    );
    assert_eq!(result.text.as_deref(), Some("这是一条测试动态文字内容"));
    assert_eq!(result.timestamp, Some(1704067200));
    let author = result.author.as_ref().expect("author");
    assert_eq!(author.name, "测试动态作者");
    assert_eq!(author.uid.as_deref(), Some("12345678"));
    let [Content::Graphics(gallery)] = result.contents.as_slice() else {
        panic!("expected one gallery: {:?}", result.contents);
    };
    let images: Vec<_> = gallery
        .images
        .iter()
        .map(|image| (image.url.as_deref(), image.width, image.height))
        .collect();
    assert_eq!(
        images,
        [
            (
                Some("https://i0.hdslb.com/bfs/new_dyn/test1.jpg"),
                Some(1080),
                Some(1920)
            ),
            (
                Some("https://i0.hdslb.com/bfs/new_dyn/test2.jpg"),
                Some(1920),
                Some(1080)
            ),
        ]
    );
    assert_eq!(detail.hits(), 1);
    assert_api_headers(&detail.last_request(), None);
    bili.fake.assert_no_unmatched();
}

fn dynamic_author(name: &str, mid: i64, pub_ts: Value) -> Value {
    json!({
        "name": name,
        "mid": mid,
        "face": format!("https://i1.hdslb.com/bfs/face/{mid}.jpg"),
        "pub_ts": pub_ts,
    })
}

#[tokio::test]
async fn forwarded_video_dynamic_resolves_the_repost_and_fetches_its_video() {
    let bili = Bili::anonymous().await;
    bili.fake
        .route(
            API,
            "GET",
            "/x/polymer/web-dynamic/v1/detail?id=900000000000000001",
        )
        .reply(Reply::ok().json(&json!({
            "code": 0,
            "message": "0",
            "data": { "item": {
                "id_str": "900000000000000001",
                "type": "DYNAMIC_TYPE_FORWARD",
                "modules": {
                    "module_author": dynamic_author("转发者", 1001, json!("1704067300")),
                    "module_dynamic": { "desc": { "text": "转发理由" }, "major": null }
                },
                "orig": {
                    "id_str": "900000000000000002",
                    "type": "DYNAMIC_TYPE_AV",
                    "modules": {
                        "module_author": dynamic_author("原作者", 1002, json!(1704067200)),
                        "module_dynamic": {
                            "desc": { "text": "看这个视频" },
                            "major": {
                                "type": "MAJOR_TYPE_ARCHIVE",
                                "archive": {
                                    "bvid": "BV1xx411c7mD",
                                    "title": "动态里的视频标题",
                                    "cover": "https://i0.hdslb.com/bfs/archive/dynamic_cover.jpg",
                                    "desc": "简介",
                                    "duration_text": "03:00"
                                }
                            }
                        }
                    },
                    "orig": null
                }
            } }
        })));
    let view = bili.view();
    bili.playurl("99999001", FIXTURES.json("bilibili_stream_info.json"));

    let result = bili
        .parse("https://t.bilibili.com/900000000000000001")
        .await;

    assert_eq!(result.text.as_deref(), Some("转发理由"));
    assert_eq!(result.timestamp, Some(1704067300));
    assert!(result.contents.is_empty(), "{:?}", result.contents);
    let repost = result.repost.as_deref().expect("repost");
    // The shared video is fetched like a direct link, captioned with what the
    // poster wrote.
    assert_eq!(
        repost.title.as_deref(),
        Some("【测试】这是一个测试视频标题 | 分集 - P1 第一集")
    );
    assert_eq!(repost.text.as_deref(), Some("看这个视频"));
    assert_eq!(extra(repost, "content_type"), Some("动态"));
    assert_fixture_dash(only_video(repost));
    assert_eq!(view.hits(), 1);
    bili.fake.assert_no_unmatched();
}

#[tokio::test]
async fn article_dynamics_are_read_from_the_opus_endpoint() {
    let shapes = [
        ("DYNAMIC_TYPE_ARTICLE", Value::Null),
        ("DYNAMIC_TYPE_DRAW", json!({ "type": "MAJOR_TYPE_OPUS" })),
    ];

    for (dynamic_type, major) in shapes {
        let bili = Bili::anonymous().await;
        bili.fake
            .route(
                API,
                "GET",
                "/x/polymer/web-dynamic/v1/detail?id=998440765151510535",
            )
            .reply(Reply::ok().json(&json!({
                "code": 0,
                "message": "0",
                "data": { "item": {
                    "id_str": "998440765151510535",
                    "type": dynamic_type,
                    "modules": {
                        "module_author": dynamic_author("测试专栏作者", 23456789, json!(1704153600)),
                        "module_dynamic": { "desc": { "text": "被压平的摘要" }, "major": major }
                    },
                    "orig": null
                } }
            })));
        let opus = bili
            .fake
            .route(
                API,
                "GET",
                "/x/polymer/web-dynamic/v1/opus/detail?id=998440765151510535&timezone_offset=-480",
            )
            .reply(FIXTURES.json("bilibili_opus_detail.json"));

        let result = bili
            .parse("https://t.bilibili.com/998440765151510535")
            .await;

        assert_eq!(opus.hits(), 1, "{dynamic_type}");
        assert_opus_fixture(&result);
        assert_eq!(
            result.url.as_deref(),
            Some("https://www.bilibili.com/opus/998440765151510535"),
            "{dynamic_type}"
        );
        bili.fake.assert_no_unmatched();
    }
}

/// The opus fixture's body, in document order, with the caption as alt text.
#[track_caller]
fn assert_opus_fixture(result: &ParseResult) {
    assert_eq!(result.title.as_deref(), Some("【测试】图文专栏标题"));
    assert_eq!(result.timestamp, Some(1704153600));
    let author = result.author.as_ref().expect("author");
    assert_eq!(author.name, "测试专栏作者");
    assert_eq!(author.uid.as_deref(), Some("23456789"));
    let [
        Content::Text(first),
        Content::Image(image),
        Content::Text(second),
    ] = result.contents.as_slice()
    else {
        panic!("expected text, image, text: {:?}", result.contents);
    };
    assert_eq!(first, "第一段正文");
    assert_eq!(
        image.url.as_deref(),
        Some("https://i0.hdslb.com/bfs/new_dyn/opus_1.jpg")
    );
    assert_eq!(image.alt.as_deref(), Some("图片说明"));
    assert_eq!((image.width, image.height), (Some(1280), Some(720)));
    assert_eq!(second, "第二段正文");
}

#[tokio::test]
async fn live_room_uses_the_keyframe_and_the_anchor() {
    let bili = Bili::anonymous().await;
    let room = bili
        .fake
        .route(
            "api.live.bilibili.com",
            "GET",
            "/room/v1/Room/get_info?room_id=23585383",
        )
        .reply(FIXTURES.json("bilibili_live_info.json"));
    let anchor = bili
        .fake
        .route(
            "api.live.bilibili.com",
            "GET",
            "/live_user/v1/UserInfo/get_anchor_in_room?room_id=23585383",
        )
        .reply(Reply::ok().json(&json!({
            "code": 0,
            "message": "0",
            "data": { "info": {
                "uid": 12345678,
                "uname": "测试主播",
                "face": "https://i1.hdslb.com/bfs/face/anchor.jpg"
            } }
        })));

    let result = bili.parse("https://live.bilibili.com/23585383").await;

    assert_eq!(result.title.as_deref(), Some("直播 - 【测试】直播间标题"));
    assert_eq!(result.text.as_deref(), Some("直播间描述"));
    assert_eq!(
        result.url.as_deref(),
        Some("https://live.bilibili.com/23585383")
    );
    let author = result.author.as_ref().expect("author");
    assert_eq!(author.name, "测试主播");
    assert_eq!(author.uid.as_deref(), Some("12345678"));
    assert_eq!(extra(&result, "status"), Some("直播中"));
    assert_eq!(extra(&result, "area"), Some("虚拟主播 / 虚拟主播"));
    assert_eq!(extra(&result, "tags"), Some("虚拟,直播"));
    assert_eq!(
        only_image(&result).url.as_deref(),
        Some("https://i0.hdslb.com/bfs/live/test_keyframe.jpg")
    );
    assert_eq!((room.hits(), anchor.hits()), (1, 1));
    bili.fake.assert_no_unmatched();
}

#[tokio::test]
async fn article_redirecting_to_opus_keeps_the_shared_link() {
    let bili = Bili::anonymous().await;
    let page = bili
        .fake
        .route(WWW, "GET", "/read/cv523868")
        .reply(Reply::redirect("/opus/998440765151510535"));
    let landing = bili.fake.route(WWW, "GET", "/opus/998440765151510535");
    let opus = bili
        .fake
        .route(
            API,
            "GET",
            "/x/polymer/web-dynamic/v1/opus/detail?id=998440765151510535",
        )
        .reply(FIXTURES.json("bilibili_opus_detail.json"));
    let article_api = bili.fake.route(API, "GET", "/x/article/view");

    let result = bili.parse("https://www.bilibili.com/read/cv523868").await;

    assert_opus_fixture(&result);
    assert_eq!(
        result.url.as_deref(),
        Some("https://www.bilibili.com/read/cv523868")
    );
    assert_eq!((page.hits(), landing.hits(), opus.hits()), (1, 1, 1));
    assert_eq!(article_api.hits(), 0);
    bili.fake.assert_no_unmatched();
}

#[tokio::test]
async fn article_without_opus_redirect_uses_the_article_api() {
    let bili = Bili::anonymous().await;
    let page = bili
        .fake
        .route(WWW, "GET", "/read/cv523868")
        .reply(Reply::ok().content_type("text/html").body("<html></html>"));
    let article_api = bili
        .fake
        .route(API, "GET", "/x/article/view?id=523868")
        .reply(FIXTURES.json("bilibili_article_view.json"));

    let result = bili.parse("https://www.bilibili.com/read/cv523868").await;

    assert_eq!(result.title.as_deref(), Some("【测试】专栏标题"));
    assert_eq!(result.text.as_deref(), Some("专栏摘要"));
    assert_eq!(
        result.url.as_deref(),
        Some("https://www.bilibili.com/read/cv523868")
    );
    assert_eq!(result.timestamp, Some(1704067200));
    assert_eq!(
        extra(&result, "stats"),
        Some("👍 1.0万 🪙 3 ⭐ 500 ↩️ 7 💬 23 👀 1.2万")
    );
    assert_eq!(
        only_image(&result).url.as_deref(),
        Some("https://i0.hdslb.com/bfs/article/article_1.jpg")
    );
    assert_eq!((page.hits(), article_api.hits()), (1, 1));
    bili.fake.assert_no_unmatched();
}

#[tokio::test]
async fn bangumi_episode_builds_a_pgc_video() {
    let bili = Bili::anonymous().await;
    let season = bili
        .fake
        .route(API, "GET", "/pgc/view/web/season?ep_id=199612")
        .reply(FIXTURES.json("bilibili_bangumi_season.json"));
    // PGC wraps the same stream payload in `result` instead of `data`.
    let stream = FIXTURES.read_json("bilibili_stream_info.json")["data"].clone();
    let playurl = bili
        .fake
        .route(
            API,
            "GET",
            "/pgc/player/web/playurl?ep_id=199612&cid=88880002&qn=80",
        )
        .reply(Reply::ok().json(&json!({ "code": 0, "message": "success", "result": stream })));

    let result = bili
        .parse("https://www.bilibili.com/bangumi/play/ep199612")
        .await;

    assert_eq!(
        result.title.as_deref(),
        Some("【测试】番剧标题 - 第2话 继续")
    );
    assert_eq!(result.text.as_deref(), Some("番剧简介"));
    assert_eq!(
        result.url.as_deref(),
        Some("https://www.bilibili.com/bangumi/play/ep199612")
    );
    assert_eq!(result.timestamp, Some(1704672000));
    let video = only_video(&result);
    assert_fixture_dash(video);
    // Episode durations are in milliseconds.
    assert_eq!(video.duration, Some(90));
    assert_eq!(
        video.cover.as_deref(),
        Some("https://i0.hdslb.com/bfs/bangumi/ep_2.jpg")
    );
    assert_eq!((season.hits(), playurl.hits()), (1, 1));
    bili.fake.assert_no_unmatched();
}

#[tokio::test]
async fn favorites_list_builds_one_card_per_media() {
    let bili = Bili::anonymous().await;
    let list = bili
        .fake
        .route(
            API,
            "GET",
            "/x/v3/fav/resource/list?media_id=311147541&ps=20&pn=1",
        )
        .reply(FIXTURES.json("bilibili_favorites.json"));

    let result = bili
        .parse("https://space.bilibili.com/396886341/favlist?fid=311147541&ftype=create")
        .await;

    assert_eq!(result.title.as_deref(), Some("收藏夹 - 测试收藏夹"));
    assert_eq!(result.text.as_deref(), Some("收藏夹简介"));
    assert_eq!(
        result.url.as_deref(),
        Some("https://space.bilibili.com/396886341/favlist?fid=311147541")
    );
    assert_eq!(
        result.author.as_ref().map(|a| a.name.as_str()),
        Some("测试收藏者")
    );
    assert_eq!(extra(&result, "media_count"), Some("2"));
    let cards: Vec<_> = result
        .contents
        .iter()
        .map(|content| match content {
            Content::Graphics(card) => (
                card.images.first().and_then(|image| image.url.as_deref()),
                card.description.as_deref(),
            ),
            other => panic!("expected a card, got {other:?}"),
        })
        .collect();
    assert_eq!(
        cards,
        [
            (
                Some("https://i0.hdslb.com/bfs/archive/fav_1.jpg"),
                Some(
                    "标题: 收藏视频一\n简介: 简介一\n链接: https://www.bilibili.com/video/BV1xx411c7mD"
                )
            ),
            (
                Some("https://i0.hdslb.com/bfs/archive/fav_2.jpg"),
                Some(
                    "标题: 收藏视频二\n简介: 简介二\n链接: https://www.bilibili.com/video/av170002"
                )
            ),
        ]
    );
    assert_eq!(list.hits(), 1);
    bili.fake.assert_no_unmatched();
}

#[tokio::test]
async fn api_error_surfaces_code_and_message() {
    let bili = Bili::anonymous().await;
    let view = bili
        .fake
        .route(API, "GET", "/x/web-interface/view?bvid=BV1xx411c7mD")
        .reply(Reply::ok().json(&json!({ "code": -404, "message": "啥都木有", "data": null })));

    let error = bili
        .engine
        .parse("https://www.bilibili.com/video/BV1xx411c7mD")
        .await
        .expect_err("a -404 view is an error");

    match error {
        ParserError::Api { code, message } => {
            assert_eq!((code, message.as_str()), (-404, "啥都木有"));
        }
        other => panic!("expected an API error, got {other:?}"),
    }
    // Not a signature rejection: no key refresh, no retry.
    assert_eq!((bili.nav.hits(), view.hits()), (1, 1));
    bili.fake.assert_no_unmatched();
}

#[tokio::test]
async fn rejected_signature_refreshes_the_keys_and_retries_once() {
    let fake = FakeUpstream::start().await;
    // The first keys are stale by the time the video is requested.
    let nav = fake
        .route(API, "GET", "/x/web-interface/nav")
        .reply(nav_reply(IMG_KEY, SUB_KEY))
        .reply(nav_reply(SUB_KEY, IMG_KEY));
    let view = fake
        .route(API, "GET", "/x/web-interface/view?bvid=BV1xx411c7mD")
        .reply(Reply::ok().json(&json!({ "code": -352, "message": "风控校验失败" })))
        .reply(FIXTURES.json("bilibili_video_info.json"));
    let playurl = fake
        .route(
            API,
            "GET",
            "/x/player/playurl?bvid=BV1xx411c7mD&cid=99999001",
        )
        .reply(FIXTURES.json("bilibili_stream_info.json"));
    let (_app, engine) = TempApp::with_platforms(&fake, json!({ "download_media": false }));

    let result = engine
        .parse("https://www.bilibili.com/video/BV1xx411c7mD")
        .await
        .expect("parse after retry")
        .expect("bilibili link matches");

    assert_eq!(
        result.title.as_deref(),
        Some("【测试】这是一个测试视频标题 | 分集 - P1 第一集")
    );
    assert_eq!(nav.hits(), 2);
    let views = view.requests();
    assert_eq!(views.len(), 2);
    assert_wbi_signed(&views[0], MIXIN_KEY);
    assert_wbi_signed(&views[1], ROTATED_MIXIN_KEY);
    // Later requests keep the refreshed keys.
    assert_wbi_signed(&playurl.last_request(), ROTATED_MIXIN_KEY);
    fake.assert_no_unmatched();
}

#[tokio::test]
async fn wbi_keys_are_reused_across_parses() {
    let bili = Bili::anonymous().await;
    bili.view();
    bili.playurl("99999001", FIXTURES.json("bilibili_stream_info.json"));
    let by_aid = bili
        .fake
        .route(API, "GET", "/x/web-interface/view?aid=170001")
        .reply(FIXTURES.json("bilibili_video_info.json"));

    let first = bili.parse("BV1xx411c7mD").await;
    let second = bili.parse("av170001").await;

    assert_eq!(first.title, second.title);
    assert_eq!(bili.nav.hits(), 1);
    assert_wbi_signed(&by_aid.last_request(), MIXIN_KEY);
    bili.fake.assert_no_unmatched();
}

#[tokio::test]
async fn bm_audio_sends_range_omits_cookie_and_falls_back_on_567() {
    let bili = Bili::start(json!({ "bili_cookie": COOKIE })).await;
    bili.view();
    let mut stream = FIXTURES.read_json("bilibili_stream_info.json");
    stream["data"]["dash"]["audio"][0]["baseUrl"] =
        json!("https://upos-sz-mirrorcosov.bilivideo.com/audio_192k.m4s");
    stream["data"]["dash"]["audio"][0]["backup_url"] =
        json!(["https://upos-sz-mirrorhwo1.bilivideo.com/audio_192k.m4s"]);
    bili.playurl("99999001", Reply::ok().json(&stream));

    let primary_cdn = bili
        .fake
        .route(
            "upos-sz-mirrorcosov.bilivideo.com",
            "GET",
            "/audio_192k.m4s",
        )
        .reply(Reply::status(567).body("blocked"));
    let backup_cdn = bili
        .fake
        .route("upos-sz-mirrorhwo1.bilivideo.com", "GET", "/audio_192k.m4s")
        .reply(
            Reply::status(206)
                .header("content-range", "bytes 0-10/11")
                .body("audio-bytes"),
        );

    let result = bili.parse("bm BV1xx411c7mD").await;

    let [Content::Audio(audio)] = result.contents.as_slice() else {
        panic!("expected one audio item, got {:?}", result.contents);
    };
    let path = audio.path.as_ref().expect("downloaded audio path");
    assert_eq!(std::fs::read(path).unwrap(), b"audio-bytes");
    assert_eq!((primary_cdn.hits(), backup_cdn.hits()), (1, 1));

    for route in [&primary_cdn, &backup_cdn] {
        let req = route.last_request();
        assert_eq!(req.header("range"), Some("bytes=0-"));
        assert_eq!(req.header("referer"), Some("https://www.bilibili.com"));
        assert_eq!(req.header("cookie"), None);
    }
    bili.fake.assert_no_unmatched();
}
