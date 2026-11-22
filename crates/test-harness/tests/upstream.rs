//! The fake upstream intercepts the production client for any host.

use karta_core::Config;
use test_harness::upstream::UNMATCHED_STATUS;
use test_harness::{FakeUpstream, Reply};

#[tokio::test]
async fn cross_host_https_redirect_stays_on_fake_and_keeps_real_hosts() {
    let fake = FakeUpstream::start().await;
    let a = fake
        .route("a.example", "GET", "/")
        .reply(Reply::redirect("https://b.example/x?k=v"));
    let b = fake
        .route("b.example", "GET", "/x?k=v")
        .reply(Reply::ok().body("landed"));

    let client = fake.client(&Config::default());
    let resp = client.get("https://a.example/").send().await.unwrap();

    assert_eq!(resp.status(), 200);
    assert_eq!(resp.uri().to_string(), "https://b.example/x?k=v");
    assert_eq!(resp.version(), wreq::Version::HTTP_11);
    assert_eq!(resp.text().await.unwrap(), "landed");

    let (a, b) = (a.last_request(), b.last_request());
    assert!(a.tls && b.tls);
    assert_eq!(a.header("host"), Some("a.example"));
    assert_eq!(b.header("host"), Some("b.example"));
    assert_eq!(b.sni.as_deref(), Some("b.example"));
    fake.assert_no_unmatched();
}

#[tokio::test]
async fn plain_http_and_unrouted_hosts() {
    let fake = FakeUpstream::start().await;
    fake.route("c.example", "POST", "/api")
        .reply(Reply::ok().json(&serde_json::json!({ "ok": true })));
    let client = fake.client(&Config::default());

    let resp = client
        .post("http://c.example/api")
        .body("payload")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let req = &fake.requests()[0];
    assert!(!req.tls);
    assert_eq!(req.body_str(), "payload");

    let resp = client
        .get("https://nowhere.example/z")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), UNMATCHED_STATUS);
    assert_eq!(fake.unmatched().len(), 1);
}

#[tokio::test]
async fn certificate_verification_is_what_the_interception_disables() {
    let fake = FakeUpstream::start().await;
    fake.route("a.example", "GET", "/").reply(Reply::ok());
    let verifying = fake
        .intercept(
            karta_core::download::StreamDownloader::production_client_builder(&Config::default())
                .unwrap(),
        )
        .tls_cert_verification(true)
        .build()
        .unwrap();

    let err = verifying
        .get("https://a.example/")
        .send()
        .await
        .unwrap_err();
    assert!(err.is_connect(), "{err:?}");
    assert!(fake.requests().is_empty());
}

#[tokio::test]
async fn scripted_sequence_then_last_reply_repeats() {
    let fake = FakeUpstream::start().await;
    let route = fake
        .route("api.example", "GET", "/v")
        .reply(Reply::status(503))
        .reply(Reply::ok().set_cookie("sid=1; Path=/").body("ready"));
    let client = fake.client(&Config::default());

    let mut statuses = Vec::new();
    for _ in 0..3 {
        let resp = client.get("https://api.example/v").send().await.unwrap();
        statuses.push(resp.status().as_u16());
        if resp.status() == 200 {
            assert_eq!(resp.headers()["set-cookie"], "sid=1; Path=/");
        }
    }
    assert_eq!(statuses, [503, 200, 200]);
    fake.wait_for_hits(&route, 3, test_harness::WAIT).await;
}

#[tokio::test]
async fn body_framings_reach_the_client_intact() {
    let fake = FakeUpstream::start().await;
    let body = "x".repeat(10_000);
    fake.route("cdn.example", "GET", "/gz")
        .reply(Reply::ok().body(body.clone()).gzip());
    fake.route("cdn.example", "GET", "/chunked")
        .reply(Reply::ok().body(body.clone()).chunked());
    fake.route("cdn.example", "GET", "/close")
        .reply(Reply::ok().body(body.clone()).no_content_length());
    fake.route("cdn.example", "GET", "/empty")
        .reply(Reply::ok());
    let client = fake.client(&Config::default());

    for path in ["/gz", "/chunked", "/close"] {
        let resp = client
            .get(format!("https://cdn.example{path}"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.text().await.unwrap(), body, "{path}");
    }
    let resp = client
        .get("https://cdn.example/empty")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert!(resp.bytes().await.unwrap().is_empty());
    fake.assert_no_unmatched();
}

#[tokio::test]
async fn truncated_and_hanging_replies_fail_the_client() {
    let fake = FakeUpstream::start().await;
    fake.route("cdn.example", "GET", "/cut")
        .reply(Reply::ok().body("short").truncated(1_000));
    let hang = fake
        .route("cdn.example", "GET", "/hang")
        .reply(Reply::hang());
    let builder =
        karta_core::download::StreamDownloader::production_client_builder(&Config::default())
            .unwrap()
            .timeout(std::time::Duration::from_millis(300));
    let client = fake.intercept(builder).build().unwrap();

    let resp = client.get("https://cdn.example/cut").send().await.unwrap();
    assert!(resp.bytes().await.is_err());

    let err = client
        .get("https://cdn.example/hang")
        .send()
        .await
        .unwrap_err();
    assert!(err.is_timeout(), "{err:?}");
    assert_eq!(hang.hits(), 1);
}

#[tokio::test]
async fn query_pairs_in_a_route_must_all_match() {
    let fake = FakeUpstream::start().await;
    let route = fake.route("api.example", "GET", "/item?id=1&full");
    let client = fake.client(&Config::default());

    for (query, routed) in [
        ("id=1&full=0&x=y", true),
        ("id=2&full", false),
        ("id=1", false),
    ] {
        let resp = client
            .get(format!("https://api.example/item?{query}"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status() == 200, routed, "{query}");
    }
    assert_eq!(route.hits(), 1);
    assert_eq!(route.last_request().query_param("x").as_deref(), Some("y"));
    assert_eq!(fake.unmatched().len(), 2);
}
