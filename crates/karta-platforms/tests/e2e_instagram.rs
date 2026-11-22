//! Instagram posts end to end: the REST → GraphQL → embed fallback tiers and
//! the CSRF / `x-ig-www-claim` header contract across them.

use karta_core::Platform;
use karta_core::data::Content;
use serde_json::json;
use test_harness::{CapturedRequest, FakeUpstream, Fixtures, Reply, TempApp};

const FIXTURES: Fixtures = Fixtures(env!("CARGO_MANIFEST_DIR"));

/// `BQ0eAlwhDrw` decodes to this media pk.
const SHORTCODE: &str = "BQ0eAlwhDrw";
const PK: &str = "1455920561485265648";

/// The `csrftoken` cookie value, which `x-csrftoken` must repeat.
#[track_caller]
fn csrf_cookie(request: &CapturedRequest) -> String {
    let cookie = request.header("cookie").expect("cookie header");
    cookie
        .split("; ")
        .find_map(|pair| pair.strip_prefix("csrftoken="))
        .unwrap_or_else(|| panic!("no csrftoken in {cookie}"))
        .to_string()
}

#[track_caller]
fn assert_api_headers(request: &CapturedRequest, asbd_id: &str, www_claim: &str) {
    let token = csrf_cookie(request);
    assert_eq!(token.len(), 32, "{token}");
    assert_eq!(request.header("x-csrftoken"), Some(token.as_str()));
    assert_eq!(request.header("x-ig-www-claim"), Some(www_claim));
    assert_eq!(request.header("x-asbd-id"), Some(asbd_id));
    assert_eq!(request.header("x-ig-app-id"), Some("936619743392459"));
}

#[tokio::test]
async fn rest_and_graphql_failures_fall_back_to_the_embed_page() {
    let fake = FakeUpstream::start().await;
    let rest = fake
        .route(
            "www.instagram.com",
            "GET",
            &format!("/api/v1/media/{PK}/info/"),
        )
        .reply(
            Reply::status(401)
                .header("x-ig-set-www-claim", "hmac.AR3-claim")
                .json(&json!({ "message": "login_required", "status": "fail" })),
        );
    let graphql = fake
        .route("www.instagram.com", "GET", "/graphql/query/")
        .reply(Reply::ok().json(&json!({ "data": { "shortcode_media": null }, "status": "ok" })));
    let embed = fake
        .route(
            "www.instagram.com",
            "GET",
            &format!("/p/{SHORTCODE}/embed/captioned/"),
        )
        .reply(FIXTURES.html("instagram/embed_captioned_image.html"));
    let (_app, engine) = TempApp::with_platforms(
        &fake,
        json!({
            "download_media": false,
            "instagram_cookie": "sessionid=SESS; ds_user_id=1",
        }),
    );

    let result = engine
        .parse(&format!(
            "https://www.instagram.com/p/{SHORTCODE}/?igsh=abc"
        ))
        .await
        .unwrap()
        .expect("instagram link matches");

    assert_eq!(result.platform, Platform::Instagram);
    assert_eq!(
        result.url.as_deref(),
        Some("https://www.instagram.com/p/BQ0eAlwhDrw/")
    );
    assert_eq!(result.text.as_deref(), Some("Hello Instagram!"));
    assert_eq!(
        result.author.as_ref().map(|a| a.name.as_str()),
        Some("Test User")
    );
    assert!(matches!(
        result.contents.first(),
        Some(Content::Image(image)) if image.url.as_deref() == Some("https://example.com/photo_hd.jpg")
    ));

    let rest = rest.last_request();
    assert_api_headers(&rest, "129477", "0");
    assert_eq!(
        rest.header("cookie"),
        Some(
            format!(
                "csrftoken={}; sessionid=SESS; ds_user_id=1",
                csrf_cookie(&rest)
            )
            .as_str()
        )
    );
    let graphql = graphql.last_request();
    // The claim the REST response set is sent with the next call.
    assert_api_headers(&graphql, "198387", "hmac.AR3-claim");
    assert_eq!(csrf_cookie(&graphql), csrf_cookie(&rest));
    assert_eq!(graphql.header("x-instagram-ajax"), Some("1006267176"));
    assert_eq!(
        graphql.query_param("query_hash").as_deref(),
        Some("9f8827793ef34641b2fb195d4d41151c")
    );
    assert!(
        graphql
            .query_param("variables")
            .is_some_and(|v| v.contains(SHORTCODE)),
        "{}",
        graphql.uri()
    );
    // The embed page is fetched like a browser: the user's cookie only.
    let embed = embed.last_request();
    assert_eq!(embed.header("cookie"), Some("sessionid=SESS; ds_user_id=1"));
    assert_eq!(embed.header("x-csrftoken"), None);
    fake.assert_no_unmatched();
}

#[tokio::test]
async fn without_cookie_graphql_is_tried_first() {
    let fake = FakeUpstream::start().await;
    let graphql = fake
        .route("www.instagram.com", "GET", "/graphql/query/")
        .reply(Reply::ok().json(&json!({
            "data": { "xdt_shortcode_media": {
                "__typename": "XDTGraphVideo",
                "display_url": "https://example.com/reel_cover.jpg",
                "is_video": true,
                "video_url": "https://example.com/reel.mp4",
                "video_duration": 12.4,
                "taken_at_timestamp": 1704110400,
                "owner": { "username": "reeler", "full_name": "Reel Maker" },
                "edge_media_to_caption": { "edges": [{ "node": { "text": "New reel" } }] }
            } },
            "status": "ok"
        })));
    let (_app, engine) = TempApp::with_platforms(&fake, json!({ "download_media": false }));

    let result = engine
        .parse(&format!("https://www.instagram.com/reel/{SHORTCODE}/"))
        .await
        .unwrap()
        .expect("instagram reel link matches");

    assert_eq!(result.text.as_deref(), Some("New reel"));
    let Some(Content::Video(video)) = result.contents.first() else {
        panic!("reel video: {:?}", result.contents);
    };
    assert_eq!(video.url.as_deref(), Some("https://example.com/reel.mp4"));
    assert_eq!(video.duration, Some(12));

    let request = graphql.last_request();
    assert_api_headers(&request, "198387", "0");
    assert_eq!(
        request.header("cookie"),
        Some(format!("csrftoken={}", csrf_cookie(&request)).as_str())
    );
    fake.assert_no_unmatched();
}
