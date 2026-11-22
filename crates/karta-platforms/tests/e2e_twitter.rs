//! Twitter/X parser end to end against the fake upstream.

use karta_core::Platform;
use karta_core::constants::DEFAULT_USER_AGENT;
use karta_core::data::Content;
use karta_core::error::ParserError;
use serde_json::json;
use test_harness::{FakeUpstream, Fixtures, Reply, TempApp};

const FIXTURES: Fixtures = Fixtures(env!("CARGO_MANIFEST_DIR"));

#[tokio::test]
async fn vxtwitter_failure_falls_back_to_the_syndication_api() {
    let fake = FakeUpstream::start().await;
    let vx = fake
        .route("api.vxtwitter.com", "GET", "/i/status/1234567890")
        .reply(Reply::status(500));
    let syndication = fake
        .route(
            "cdn.syndication.twimg.com",
            "GET",
            "/tweet-result?id=1234567890&dnt=1",
        )
        .reply(FIXTURES.json("twitter_syndication.json"));
    let (_app, engine) = TempApp::with_platforms(&fake, json!({ "download_media": false }));

    let result = engine
        .parse("https://x.com/testuser/status/1234567890")
        .await
        .unwrap()
        .expect("twitter link matches");

    assert_eq!(result.platform, Platform::Twitter);
    assert_eq!(
        result.url.as_deref(),
        Some("https://x.com/i/status/1234567890")
    );
    assert_eq!(
        result.text.as_deref(),
        Some("This is a test tweet with media 🎉 https://t.co/abc123")
    );
    let author = result.author.as_ref().expect("author");
    assert_eq!(author.name, "Test Twitter User");
    assert_eq!(
        author.avatar.as_deref(),
        Some("https://pbs.twimg.com/profile_images/test/photo_normal.jpg")
    );
    assert_eq!(author.url.as_deref(), Some("https://x.com/testuser"));
    assert_eq!(result.title, None);
    assert_eq!(result.timestamp, Some(1_704_110_400));
    assert_eq!(result.contents.len(), 2);
    let Content::Image(image) = &result.contents[0] else {
        panic!("first media is the photo: {:?}", result.contents);
    };
    assert_eq!(
        image.url.as_deref(),
        Some("https://pbs.twimg.com/media/test_photo1.jpg?format=jpg&name=orig")
    );
    let Some(Content::Video(video)) = result.contents.get(1) else {
        panic!("second media is the video: {:?}", result.contents);
    };
    assert_eq!(
        video.url.as_deref(),
        Some("https://video.twimg.com/ext_tw_video/test/pu/vid/1080x1920/test_1080.mp4")
    );

    assert_eq!(vx.hits(), 1);
    let request = vx.last_request();
    assert_eq!(request.header("referer"), Some("https://x.com/"));
    assert_eq!(request.header("user-agent"), Some(DEFAULT_USER_AGENT));
    let request = syndication.last_request();
    assert!(request.tls);
    assert!(request.query_param("token").is_some_and(|t| !t.is_empty()));
    assert_eq!(
        request.header("referer"),
        Some("https://platform.twitter.com/")
    );
    assert_eq!(request.header("user-agent"), Some(DEFAULT_USER_AGENT));
    fake.assert_no_unmatched();
}

#[tokio::test]
async fn tombstoned_tweet_is_reported_as_unavailable_after_trying_both_apis() {
    let fake = FakeUpstream::start().await;
    let vx = fake
        .route("api.vxtwitter.com", "GET", "/i/status/1234567890")
        .reply(Reply::status(500));
    let syndication = fake
        .route(
            "cdn.syndication.twimg.com",
            "GET",
            "/tweet-result?id=1234567890",
        )
        .reply(Reply::ok().json(&json!({
            "__typename": "TweetTombstone",
            "tombstone": { "text": { "text": "This Post was deleted by the Post author." } }
        })));
    let (_app, engine) = TempApp::with_platforms(&fake, json!({ "download_media": false }));

    let outcome = engine
        .parse("https://twitter.com/testuser/status/1234567890")
        .await
        .map(|result| assert!(result.is_some(), "twitter link matches"));
    fake.assert_no_unmatched();
    assert_eq!((vx.hits(), syndication.hits()), (1, 1));
    assert!(
        matches!(outcome, Err(ParserError::ContentNotFound(_))),
        "{outcome:?}"
    );
}
