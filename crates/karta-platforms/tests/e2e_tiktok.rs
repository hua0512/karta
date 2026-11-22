//! TikTok end to end: short links resolved to the video page, and the two
//! embedded-state formats that page may carry.

use karta_core::Platform;
use karta_core::constants::MOBILE_USER_AGENT;
use karta_core::data::Content;
use serde_json::json;
use test_harness::{FakeUpstream, Fixtures, Reply, TempApp};

const FIXTURES: Fixtures = Fixtures(env!("CARGO_MANIFEST_DIR"));

const VIDEO_PATH: &str = "/@test/video/7300000000000000001";

fn config() -> serde_json::Value {
    // A missing yt-dlp keeps `--features ytdlp` runs on the page scraper.
    json!({ "download_media": false, "ytdlp_path": "/nonexistent/yt-dlp" })
}

#[tokio::test]
async fn short_link_resolves_to_the_universal_data_page() {
    let fake = FakeUpstream::start().await;
    let short = fake
        .route("vm.tiktok.com", "GET", "/ZMabc123/")
        .reply(Reply::redirect(&format!(
            "https://www.tiktok.com{VIDEO_PATH}?_r=1&u_code=x"
        )));
    let page = fake
        .route("www.tiktok.com", "GET", VIDEO_PATH)
        .reply(FIXTURES.html("tiktok/universal_data_video.html"));
    let (_app, engine) = TempApp::with_platforms(&fake, config());

    let result = engine
        .parse("https://vm.tiktok.com/ZMabc123/")
        .await
        .unwrap()
        .expect("tiktok short link matches");

    assert_eq!(result.platform, Platform::TikTok);
    assert_eq!(
        result.url.as_deref(),
        Some("https://www.tiktok.com/@test/video/7300000000000000001?_r=1&u_code=x")
    );
    assert_eq!(
        result.title.as_deref(),
        Some("This is a test TikTok video description #fyp #viral")
    );
    assert_eq!(result.timestamp, Some(1_704_067_200));
    assert_eq!(
        result.author.as_ref().map(|a| a.name.as_str()),
        Some("TestTikTokUser")
    );
    let Some(Content::Video(video)) = result.contents.first() else {
        panic!("video content: {:?}", result.contents);
    };
    assert_eq!(
        video.url.as_deref(),
        Some("https://v16m-default.akamaized.net/test_download.mp4")
    );
    assert_eq!(video.duration, Some(15));

    assert_eq!(short.hits(), 1);
    // One hit ends the redirect walk, the next fetches the page to scrape.
    assert_eq!(page.hits(), 2);
    let request = page.last_request();
    assert_eq!(request.header("user-agent"), Some(MOBILE_USER_AGENT));
    assert_eq!(request.header("referer"), Some("https://www.tiktok.com/"));
    fake.assert_no_unmatched();
}

#[tokio::test]
async fn video_link_reads_the_sigi_state_page() {
    let fake = FakeUpstream::start().await;
    let state = json!({
        "ItemModule": {
            "7300000000000000001": {
                "desc": "sigi caption",
                "createTime": "1704067200",
                "nickname": "Sigi User",
                "video": {
                    "playAddr": "https://v16m-default.akamaized.net/sigi_play.mp4",
                    "cover": "https://p16-sign.tiktokcdn-us.com/sigi_cover.jpeg",
                    "duration": 9
                }
            }
        }
    });
    let page = fake
        .route("www.tiktok.com", "GET", VIDEO_PATH)
        .reply(Reply::ok().content_type("text/html").body(format!(
        "<html><script id=\"SIGI_STATE\" type=\"application/json\">\n{state:#}\n</script></html>"
    )));
    let (_app, engine) = TempApp::with_platforms(&fake, config());

    let result = engine
        .parse(&format!(
            "https://www.tiktok.com{VIDEO_PATH}?is_from_webapp=1"
        ))
        .await
        .unwrap()
        .expect("tiktok video link matches");

    assert_eq!(result.title.as_deref(), Some("sigi caption"));
    assert_eq!(
        result.author.as_ref().map(|a| a.name.as_str()),
        Some("Sigi User")
    );
    let Some(Content::Video(video)) = result.contents.first() else {
        panic!("video content: {:?}", result.contents);
    };
    assert_eq!(
        video.url.as_deref(),
        Some("https://v16m-default.akamaized.net/sigi_play.mp4")
    );
    assert_eq!(page.hits(), 1);
    fake.assert_no_unmatched();
}
