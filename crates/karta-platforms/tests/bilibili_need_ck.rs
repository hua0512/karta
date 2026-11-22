use std::path::Path;
use std::sync::Arc;

use karta_core::download::StreamDownloader;
use karta_core::{Config, ParserEngine};

// These tests require KARTA_TEST_BILI_COOKIE to be set.
// Run with: KARTA_TEST_BILI_COOKIE="..." cargo test --test bilibili_need_ck -- --ignored

fn make_engine(cfg: Config, dp: &Path) -> ParserEngine {
    let downloader = Arc::new(StreamDownloader::new(&cfg, dp).expect("downloader"));
    let parsers = karta_platforms::create_all_parsers(&cfg, downloader.clone(), dp);
    ParserEngine::with_parsers(cfg, dp.to_path_buf(), downloader, parsers)
}

#[tokio::test]
#[ignore = "requires KARTA_TEST_BILI_COOKIE env var"]
async fn bilibili_favlist_requires_cookie_and_downloads() {
    let cookie = std::env::var("KARTA_TEST_BILI_COOKIE")
        .expect("set KARTA_TEST_BILI_COOKIE to run this test");

    let cfg = Config {
        bili_cookie: Some(cookie),
        download_media: true,
        render_type: karta_core::RenderType::Text,
        ..Default::default()
    };

    let data_dir = tempfile::tempdir().expect("temp data dir");
    let dp = data_dir.path().to_path_buf();
    let engine = make_engine(cfg, &dp);

    let url = "https://space.bilibili.com/396886341/favlist?fid=311147541&ftype=create";
    let result = engine.parse(url).await.expect("parse").expect("match");

    // Should have author/title and at least one graphics item.
    assert!(result.title.is_some());
    assert!(result.author.is_some());
    assert!(!result.contents.is_empty());

    // Render should attempt downloads; this test is mainly to catch auth failures.
    engine.render(&result).await.expect("render");

    // Clean up best-effort.
}
