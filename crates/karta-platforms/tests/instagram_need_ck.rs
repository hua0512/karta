use std::path::Path;
use std::sync::Arc;

use karta_core::download::StreamDownloader;
use karta_core::{Config, ParserEngine};

// These tests require KARTA_TEST_IG_COOKIE to be set.
// Run with: KARTA_TEST_IG_COOKIE="..." cargo test --test instagram_need_ck -- --ignored

fn make_engine(cfg: Config, dp: &Path) -> ParserEngine {
    let downloader = Arc::new(StreamDownloader::new(&cfg, dp).expect("downloader"));
    let parsers = karta_platforms::create_all_parsers(&cfg, downloader.clone(), dp);
    ParserEngine::with_parsers(cfg, dp.to_path_buf(), downloader, parsers)
}

#[tokio::test]
#[ignore = "requires KARTA_TEST_IG_COOKIE env var"]
async fn instagram_post_via_graphql() {
    let cookie =
        std::env::var("KARTA_TEST_IG_COOKIE").expect("set KARTA_TEST_IG_COOKIE to run this test");

    let cfg = Config {
        instagram_cookie: Some(cookie),
        download_media: false,
        ..Default::default()
    };

    let data_dir = tempfile::tempdir().expect("temp data dir");
    let dp = data_dir.path().to_path_buf();
    let engine = make_engine(cfg, &dp);

    let url = "https://www.instagram.com/p/DWuICtXCSUU/";
    let result = engine.parse(url).await.expect("parse").expect("match");

    println!("Title: {:?}", result.title);
    println!("Text: {:?}", result.text);
    println!("Author: {:?}", result.author);
    println!("Contents: {} items", result.contents.len());
    for (i, c) in result.contents.iter().enumerate() {
        println!("  content[{}]: {:?}", i, c);
    }
    println!("Extra: {:?}", result.extra);

    assert!(!result.contents.is_empty(), "should have media content");
}
