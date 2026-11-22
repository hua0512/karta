use std::path::Path;
use std::sync::Arc;

use karta_core::data::Content;
use karta_core::download::StreamDownloader;
use karta_core::{Config, ParserEngine};

// These tests require KARTA_TEST_DOUYIN_COOKIE to be set.
// Run with: KARTA_TEST_DOUYIN_COOKIE="..." cargo test --test douyin_need_ck -- --ignored

fn make_engine(cfg: Config, dp: &Path) -> ParserEngine {
    let downloader = Arc::new(StreamDownloader::new(&cfg, dp).expect("downloader"));
    let parsers = karta_platforms::create_all_parsers(&cfg, downloader.clone(), dp);
    ParserEngine::with_parsers(cfg, dp.to_path_buf(), downloader, parsers)
}

/// Strict acceptance for the user-provided share; failures must not become skips.
#[tokio::test]
#[ignore = "requires network, ffprobe, and KARTA_TEST_DOUYIN_MOBILE JSON identity"]
async fn douyin_teteyang_share_end_to_end() {
    use karta_core::message::SegmentType;

    let identity = serde_json::from_str(
        &std::env::var("KARTA_TEST_DOUYIN_MOBILE")
            .expect("set KARTA_TEST_DOUYIN_MOBILE to the registered Android identity"),
    )
    .unwrap();
    let config = Config {
        douyin_mobile: Some(identity),
        render_type: karta_core::RenderType::Text,
        download_media: true,
        ..Default::default()
    };
    let dir = tempfile::tempdir().unwrap();
    let engine = make_engine(config, dir.path());
    let share = "8.23 复制打开抖音，看看【特特羊的作品】  https://v.douyin.com/CxPdeP4iqG4/ FHv:/ :9pm q@E.hB 12/22.";
    let result = engine
        .parse(share)
        .await
        .expect("share must parse")
        .expect("share must match");
    assert_eq!(result.platform, karta_core::Platform::Douyin);
    assert_eq!(result.author.as_ref().unwrap().name, "特特羊");
    assert_eq!(
        result.author.as_ref().unwrap().uid.as_deref(),
        Some("3787178018350446")
    );
    assert!(
        result
            .url
            .as_deref()
            .is_some_and(|u| u.contains("7688376340010315432"))
    );
    assert!(
        result
            .contents
            .iter()
            .any(|c| matches!(c, Content::Video(v) if v.duration == Some(18)))
    );
    let rendered = engine
        .render(&result)
        .await
        .expect("render with media download");
    let messages: Vec<_> = rendered.iter().flat_map(|out| out.messages()).collect();
    assert!(messages.iter().any(|m| m.plain_text().contains("特特羊")));
    let video = messages
        .iter()
        .flat_map(|m| &m.segments)
        .find(|s| s.segment_type == SegmentType::Video)
        .expect("rendered video segment");
    let path = video.file_path.as_ref().expect("downloaded video file");
    assert!(std::fs::metadata(path).unwrap().len() > 1024);
    let probe = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_type,codec_name,width,height:format=duration",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .expect("ffprobe installed");
    assert!(
        probe.status.success(),
        "{}",
        String::from_utf8_lossy(&probe.stderr)
    );
    let info: serde_json::Value = serde_json::from_slice(&probe.stdout).unwrap();
    assert!(
        info["streams"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["codec_type"] == "video" && s["width"].as_u64().unwrap_or(0) > 0)
    );
    let duration: f64 = info["format"]["duration"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!((17.0..20.0).contains(&duration));
    eprintln!(
        "Verified item=7688376340010315432 author=特特羊 bytes={} media={info}",
        std::fs::metadata(path).unwrap().len()
    );
}

/// Parse real Douyin links (video, image note, short link) with a cookie
/// through the official API.
#[tokio::test]
#[ignore = "requires KARTA_TEST_DOUYIN_COOKIE env var"]
async fn douyin_links_via_api() {
    let cookie = std::env::var("KARTA_TEST_DOUYIN_COOKIE")
        .expect("set KARTA_TEST_DOUYIN_COOKIE to run this test");

    let cfg = Config {
        douyin_cookie: Some(cookie),
        download_media: false,
        ..Default::default()
    };

    let data_dir = tempfile::tempdir().expect("temp data dir");
    let dp = data_dir.path().to_path_buf();
    let engine = make_engine(cfg, &dp);

    for url in [
        "https://www.douyin.com/video/7521023890996514083",
        "https://www.douyin.com/note/7450744229229235491",
        "https://v.douyin.com/iP6Uu1Kh",
    ] {
        let result = engine
            .parse(url)
            .await
            .unwrap_or_else(|e| panic!("{url}: {e}"))
            .expect("match");

        assert!(result.text.is_some(), "{url}: should have caption text");
        assert!(result.author.is_some(), "{url}: should have an author");
        assert!(
            result
                .contents
                .iter()
                .any(|c| matches!(c, Content::Video(_) | Content::Image(_))),
            "{url}: should have video or image content"
        );

        eprintln!(
            "{url}: title={:?} author={:?} contents={:?}",
            result.title,
            result.author.as_ref().map(|a| &a.name),
            result.contents
        );
    }
}

/// Resolve a long-article share 口令 anonymously: token -> share-carrier
/// endpoint -> article id -> anonymous detail API -> article card.
///
/// Tokens expire server-side; when this one does, replace it with a fresh
/// share text copied from the app ("长按复制打开抖音，即可阅读文章 …").
#[tokio::test]
#[ignore = "requires network access"]
async fn douyin_article_share_token_anonymous() {
    let cfg = Config {
        douyin_cookie: None,
        download_media: false,
        ..Default::default()
    };

    let data_dir = tempfile::tempdir().expect("temp data dir");
    let dp = data_dir.path().to_path_buf();
    let engine = make_engine(cfg, &dp);

    let share_text = "4-M:/. 09/19 :0pm e@B.TL 【当了三年朋友，她突然不干了】长按复制打开抖音，即可阅读文章 ˇˇ5w0ncBiCzhF99➝➝";
    let result = engine
        .parse(share_text)
        .await
        .expect("article must parse; replace the share token if it has expired")
        .expect("share text should match the douyin_share_token pattern");

    assert_eq!(result.extra.get("类型").map(String::as_str), Some("文章"));
    assert!(result.title.is_some(), "article should have a title");
    assert!(result.author.is_some(), "article should have an author");
    assert!(
        result
            .contents
            .iter()
            .any(|c| matches!(c, Content::Text(text) if text.chars().count() > 200)),
        "article body should be attached as ordered text"
    );
    assert!(
        result
            .contents
            .iter()
            .any(|c| matches!(c, Content::Image(_))),
        "article should carry its header poster"
    );
    assert!(
        !result
            .contents
            .iter()
            .any(|c| matches!(c, Content::Video(_))),
        "placeholder video must not be attached"
    );

    eprintln!("Title: {:?}", result.title);
    eprintln!("Author: {:?}", result.author.as_ref().map(|a| &a.name));
    eprintln!("Extra: {:?}", result.extra);
    eprintln!("Contents: {} items", result.contents.len());
}
