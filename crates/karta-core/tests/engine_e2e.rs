//! `ParserEngine` end to end with fake parsers: matching, the parse cache and
//! timeout, config loading, and the HTTP retry helper parsers share.

use std::sync::Arc;
use std::time::Duration;

use karta_core::config::{AccessMode, BiliVideoCodec, BiliVideoQuality, EmojiStyleConfig};
use karta_core::download::StreamDownloader;
use karta_core::{Config, ParseResult, ParserEngine, ParserError, Platform, RenderType};
use serde_json::json;
use test_harness::{FakeParser, FakeUpstream, Reply, TempApp};
use wreq::header::HeaderMap;

const LINK: &str = "https://www.bilibili.com/video/BV1xx411c7mD";

fn bilibili_parser() -> FakeParser {
    FakeParser::new(Platform::Bilibili, &[("bv", r"BV[0-9A-Za-z]{10}")])
}

fn offline_downloader(app: &TempApp) -> Arc<StreamDownloader> {
    Arc::new(StreamDownloader::new(app.config(), app.data_path()).unwrap())
}

#[tokio::test]
async fn repeated_link_is_served_from_the_parse_cache() {
    let app = TempApp::new(json!({}));
    let parser = Arc::new(bilibili_parser().returning(ParseResult::new(Platform::Bilibili)));
    let engine = app.engine(offline_downloader(&app), vec![parser.clone()]);

    let first = engine.parse(LINK).await.unwrap().unwrap();
    let second = engine
        .parse(&format!("again: {LINK} !"))
        .await
        .unwrap()
        .unwrap();
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(parser.calls(), 1);

    engine.clear_cache();
    engine.parse(LINK).await.unwrap().unwrap();
    assert_eq!(parser.calls(), 2);
}

#[tokio::test]
async fn expired_cache_entries_are_parsed_again() {
    let app = TempApp::new(json!({}));
    let parser = Arc::new(bilibili_parser().returning(ParseResult::new(Platform::Bilibili)));
    let engine = app
        .engine(offline_downloader(&app), vec![parser.clone()])
        .with_parse_cache_ttl(Duration::ZERO);

    engine.parse(LINK).await.unwrap().unwrap();
    engine.parse(LINK).await.unwrap().unwrap();
    assert_eq!(parser.calls(), 2);
}

#[tokio::test(start_paused = true)]
async fn hanging_parser_hits_parse_timeout() {
    let app = TempApp::new(json!({ "parse_timeout": 5 }));
    let parser = Arc::new(bilibili_parser().hanging());
    let engine = app.engine(offline_downloader(&app), vec![parser.clone()]);

    // The outer bound turns a missing timeout into a failure, not a hang.
    let error = tokio::time::timeout(Duration::from_secs(60), engine.parse(LINK))
        .await
        .expect("parse_timeout fires before the outer bound")
        .unwrap_err();

    assert!(
        matches!(error, ParserError::Timeout { seconds: 5 }),
        "{error:?}"
    );
    assert_eq!(parser.calls(), 1);
}

#[tokio::test]
async fn first_registered_pattern_wins_over_text_position() {
    let app = TempApp::new(json!({}));
    let bilibili = Arc::new(FakeParser::new(
        Platform::Bilibili,
        &[
            ("bv_with_page", r"BV[0-9A-Za-z]{10}\?p=\d+"),
            ("bv", r"BV[0-9A-Za-z]{10}"),
        ],
    ));
    // Also matches every BV link, but registers after `bilibili`.
    let catch_all = Arc::new(FakeParser::new(
        Platform::Douyin,
        &[("any", r"BV\w+|v\.douyin\.com/\w+")],
    ));
    let engine = app.engine(
        offline_downloader(&app),
        vec![bilibili.clone(), catch_all.clone()],
    );

    let cases = [
        ("BV1xx411c7mD?p=2", Platform::Bilibili, "BV1xx411c7mD?p=2"),
        ("BV1xx411c7mD", Platform::Bilibili, "BV1xx411c7mD"),
        // Registration order decides, not the position in the text.
        (
            "v.douyin.com/abc then BV1xx411c7mD",
            Platform::Bilibili,
            "BV1xx411c7mD",
        ),
        (
            "only v.douyin.com/abc",
            Platform::Douyin,
            "v.douyin.com/abc",
        ),
    ];
    for (text, platform, matched) in cases {
        engine.clear_cache();
        assert!(engine.would_match(text), "{text}");
        assert_eq!(engine.match_platform(text), Some(platform), "{text}");
        let result = engine.parse(text).await.unwrap().expect(text);
        assert_eq!(result.platform, platform, "{text}");
        let parser = if platform == Platform::Bilibili {
            &bilibili
        } else {
            &catch_all
        };
        assert_eq!(
            parser.inputs().last().map(String::as_str),
            Some(matched),
            "{text}"
        );
    }
    assert_eq!(bilibili.calls() + catch_all.calls(), cases.len());

    let text = "no links here";
    assert!(!engine.would_match(text));
    assert_eq!(engine.match_platform(text), None);
    assert!(engine.parse(text).await.unwrap().is_none());
    assert!(engine.parse_and_render(text).await.unwrap().is_none());
    assert_eq!(bilibili.calls() + catch_all.calls(), cases.len());
}

#[tokio::test]
async fn legacy_config_file_loads_and_builds_the_engine() {
    let app = TempApp::new(json!({}));
    let fixture = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/legacy_config.json"
    ))
    .unwrap();
    std::fs::write(app.data_path().join("config.json"), &fixture).unwrap();

    let config = Config::load(app.data_path());

    assert!(
        !app.data_path().join("config.json.bad").exists(),
        "the legacy file must parse"
    );
    assert_eq!(
        std::fs::read(app.data_path().join("config.json")).unwrap(),
        fixture,
        "loading must not rewrite the user's file"
    );
    assert_eq!(config.render_type, RenderType::Image);
    assert_eq!(config.bili_video_quality, BiliVideoQuality::Q1080P60);
    assert_eq!(
        config.bili_video_codecs,
        vec![BiliVideoCodec::HEVC, BiliVideoCodec::AV1]
    );
    assert_eq!(config.emoji_style, EmojiStyleConfig::Google);
    assert_eq!(config.disabled_platforms, vec![Platform::Weibo]);
    assert!(config.access_control.enabled);
    assert_eq!(config.access_control.mode, AccessMode::Whitelist);
    assert_eq!(config.access_control.users, vec![10001]);
    assert_eq!(config.access_control.groups, vec![20002]);
    assert_eq!(config.max_size_bytes(), 50 * 1024 * 1024);
    assert_eq!(config.bili_cookie.as_deref(), Some("SESSDATA=legacy"));

    let engine = ParserEngine::new(config, app.data_path().to_path_buf())
        .expect("the engine builds from the legacy config");
    assert_eq!(engine.config().cache_size, 32);
    assert_eq!(engine.config().parse_timeout, 30);
}

// `get_with_retry` sleeps between attempts; paused time skips the back-off.
// The client has no timeouts that auto-advancing time could trip.
#[tokio::test(start_paused = true)]
async fn get_with_retry_retries_transient_statuses_with_backoff() {
    let fake = FakeUpstream::start().await;
    let client = fake.intercept(wreq::Client::builder()).build().unwrap();
    let get = |path: &str| {
        let url = format!("https://api.example.com{path}");
        let client = client.clone();
        async move {
            karta_core::http::get_with_retry(&client, &url, HeaderMap::new())
                .await
                .expect("a response")
        }
    };

    let flaky = fake
        .route("api.example.com", "GET", "/flaky")
        .reply(Reply::status(503))
        .reply(Reply::status(503))
        .reply(Reply::ok().body("ok"));
    let response = get("/flaky").await;
    assert_eq!(response.status(), 200);
    assert_eq!(response.text().await.unwrap(), "ok");
    assert_eq!(flaky.hits(), 3);

    // After the last attempt the retryable status itself is returned.
    let down = fake
        .route("api.example.com", "GET", "/down")
        .reply(Reply::status(503));
    assert_eq!(get("/down").await.status(), 503);
    assert_eq!(down.hits(), 3);

    for (status, retried) in [
        (429, true),
        (500, true),
        (502, true),
        (504, true),
        (403, false),
        (404, false),
    ] {
        let path = format!("/status/{status}");
        let route = fake
            .route("api.example.com", "GET", &path)
            .reply(Reply::status(status))
            .reply(Reply::ok());
        let expected = if retried { (200, 2) } else { (status, 1) };
        let response = get(&path).await;
        assert_eq!(
            (response.status().as_u16(), route.hits()),
            expected,
            "status {status}"
        );
    }
    fake.assert_no_unmatched();
}

async fn redirect_fixture() -> FakeUpstream {
    let fake = FakeUpstream::start().await;
    fake.route("example.com", "GET", "/short")
        .reply(Reply::redirect("/final").set_cookie("ttwid=abc123; Path=/"));
    fake.route("example.com", "GET", "/final")
        .reply(Reply::ok().body("ok"));
    fake
}

#[tokio::test]
async fn resolve_redirect_collects_hop_cookies() {
    let fake = redirect_fixture().await;
    let client = fake.intercept(wreq::Client::builder()).build().unwrap();
    let resolved =
        karta_core::http::resolve_redirect(&client, "https://example.com/short", HeaderMap::new())
            .await
            .unwrap();

    assert_eq!(resolved.final_url, "https://example.com/final");
    assert_eq!(resolved.set_cookies, ["ttwid=abc123; Path=/"]);
    assert_eq!(fake.requests().len(), 2);
    fake.assert_no_unmatched();
}

#[tokio::test]
async fn get_with_redirect_follows_despite_client_policy() {
    let fake = redirect_fixture().await;
    // The builder does not follow redirects by default; the helper must.
    let client = fake.intercept(wreq::Client::builder()).build().unwrap();
    let response =
        karta_core::http::get_with_redirect(&client, "https://example.com/short", HeaderMap::new())
            .await
            .unwrap();

    assert_eq!(response.status(), 200);
    assert_eq!(response.uri().to_string(), "https://example.com/final");
    assert_eq!(fake.requests().len(), 2);
    fake.assert_no_unmatched();
}
