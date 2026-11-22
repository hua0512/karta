//! YouTube end to end without yt-dlp: video metadata from the oEmbed API.

use karta_core::Platform;
use karta_core::data::Content;
use serde_json::json;
use test_harness::{FakeUpstream, Fixtures, TempApp};

const FIXTURES: Fixtures = Fixtures(env!("CARGO_MANIFEST_DIR"));

#[tokio::test]
async fn short_link_reads_oembed_metadata_and_max_resolution_thumbnail() {
    let fake = FakeUpstream::start().await;
    let oembed = fake
        .route(
            "www.youtube.com",
            "GET",
            "/oembed?url=https://www.youtube.com/watch?v=EKkzbbLYPuI&format=json",
        )
        .reply(FIXTURES.json("youtube_oembed.json"));
    // A missing yt-dlp keeps `--features ytdlp` runs on the oEmbed path.
    let (_app, engine) = TempApp::with_platforms(
        &fake,
        json!({
            "download_media": false,
            "ytb_cookie": "VISITOR_INFO1_LIVE=abc",
            "ytdlp_path": "/nonexistent/yt-dlp",
        }),
    );

    let result = engine
        .parse("https://youtu.be/EKkzbbLYPuI?si=share")
        .await
        .unwrap()
        .expect("youtube link matches");

    assert_eq!(result.platform, Platform::YouTube);
    assert_eq!(result.title.as_deref(), Some("Test YouTube Video Title"));
    assert_eq!(
        result.url.as_deref(),
        Some("https://www.youtube.com/watch?v=EKkzbbLYPuI")
    );
    let author = result.author.as_ref().expect("author");
    assert_eq!(author.name, "TestYouTubeChannel");
    assert_eq!(
        author.url.as_deref(),
        Some("https://www.youtube.com/@TestChannel")
    );
    let Some(Content::Image(thumbnail)) = result.contents.first() else {
        panic!("thumbnail image: {:?}", result.contents);
    };
    assert_eq!(
        thumbnail.url.as_deref(),
        Some("https://i.ytimg.com/vi/EKkzbbLYPuI/maxresdefault.jpg")
    );

    let request = oembed.last_request();
    assert_eq!(request.header("cookie"), Some("VISITOR_INFO1_LIVE=abc"));
    assert_eq!(request.header("referer"), Some("https://www.youtube.com/"));
    fake.assert_no_unmatched();
}
