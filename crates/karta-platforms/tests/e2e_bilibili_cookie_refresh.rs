//! Bilibili web cookie refresh end to end against the fake upstream:
//! `cookie/info` → correspond page → `cookie/refresh` → `confirm/refresh`,
//! and what the credential, the shared store and `bilibili_cookies.json`
//! hold afterwards.

use karta_core::ParserError;
use karta_platforms::{BiliCredential, CredentialManager, RefreshOutcome};
use serde_json::json;
use test_harness::{CapturedRequest, FakeUpstream, Reply, Route, TempApp};

const API: &str = "api.bilibili.com";
const PASSPORT: &str = "passport.bilibili.com";
const WWW: &str = "www.bilibili.com";

const COOKIE_INFO: &str = "/x/passport-login/web/cookie/info";
const CORRESPOND: &str = "/correspond/1/";
const COOKIE_REFRESH: &str = "/x/passport-login/web/cookie/refresh";
const CONFIRM_REFRESH: &str = "/x/passport-login/web/confirm/refresh";

const OLD_COOKIE: &str =
    "SESSDATA=old-sess; bili_jct=old-csrf; buvid3=b3; DedeUserID=42; ac_time_value=old-token";

/// `data.timestamp` the fake `cookie/info` hands out (the docs' example).
const TIMESTAMP: i64 = 1684466082562;
const REFRESH_CSRF: &str = "b0cc8411ded2f9db2cff2edb3123acac";

fn nav(logged_in: bool) -> Reply {
    Reply::ok().json(&json!({
        "code": if logged_in { 0 } else { -101 },
        "data": { "isLogin": logged_in }
    }))
}

fn cookie_info(refresh: bool) -> Reply {
    Reply::ok().json(&json!({
        "code": 0,
        "message": "0",
        "ttl": 1,
        "data": { "refresh": refresh, "timestamp": TIMESTAMP }
    }))
}

/// The correspond page as the docs show it, trimmed.
fn correspond_page() -> Reply {
    Reply::ok()
        .content_type("text/html; charset=utf-8")
        .body(format!(
            r#"<!DOCTYPE html><html lang="zh-Hans"><head><title>Correspond</title></head>
<body><div id="1-name">{REFRESH_CSRF}</div><div id="token-iframe-app"></div></body></html>"#
        ))
        .gzip()
}

/// A successful `cookie/refresh`: the new cookies and refresh token.
fn refreshed() -> Reply {
    let set = |cookie: &str| format!("{cookie}; Path=/; Domain=bilibili.com");
    Reply::ok()
        .set_cookie(&set("SESSDATA=new-sess"))
        .set_cookie(&set("bili_jct=new-csrf"))
        .set_cookie(&set("DedeUserID=42"))
        .set_cookie(&set("DedeUserID__ckMd5=md5"))
        .set_cookie(&set("sid=sid"))
        .json(&json!({
            "code": 0,
            "message": "0",
            "ttl": 1,
            "data": { "status": 0, "message": "", "refresh_token": "new-token" }
        }))
}

fn api_code(code: i64) -> Reply {
    Reply::ok().json(&json!({ "code": code, "message": "error", "ttl": 1 }))
}

/// A credential manager on the fake upstream, initialised from
/// `config_cookie` (which `init` validates and saves to the file).
struct Bili {
    fake: FakeUpstream,
    app: TempApp,
    cm: CredentialManager,
    nav: Route,
}

impl Bili {
    async fn start(config_cookie: &str) -> Self {
        let fake = FakeUpstream::start().await;
        let nav = fake
            .route(API, "GET", "/x/web-interface/nav")
            .reply(nav(true));
        let app = TempApp::new(json!({}));
        let cm = CredentialManager::new(fake.client(app.config()), app.data_path());
        cm.init(Some(config_cookie)).await.expect("init");
        assert!(cm.has_credential().await);
        Self { fake, app, cm, nav }
    }

    fn info(&self, refresh: bool) -> Route {
        self.fake
            .route(PASSPORT, "GET", COOKIE_INFO)
            .reply(cookie_info(refresh))
    }

    fn correspond(&self) -> Route {
        self.fake
            .route_prefix(WWW, "GET", CORRESPOND)
            .reply(correspond_page())
    }

    fn refresh(&self, reply: Reply) -> Route {
        self.fake
            .route(PASSPORT, "POST", COOKIE_REFRESH)
            .reply(reply)
    }

    fn confirm(&self, reply: Reply) -> Route {
        self.fake
            .route(PASSPORT, "POST", CONFIRM_REFRESH)
            .reply(reply)
    }

    fn saved(&self) -> String {
        std::fs::read_to_string(self.app.data_path().join("bilibili_cookies.json"))
            .expect("read bilibili_cookies.json")
    }

    async fn cookie(&self) -> String {
        self.cm.get_cookie_string().await.expect("credential")
    }

    /// Paths requested after `init` (which only calls nav).
    fn paths_after_init(&self) -> Vec<String> {
        self.fake
            .requests()
            .into_iter()
            .filter(|r| r.host != API)
            .map(|r| r.path)
            .collect()
    }
}

fn saved_credential(json: &str) -> BiliCredential {
    serde_json::from_str(json).expect("saved credential parses")
}

/// Form fields of a urlencoded request body.
fn form(request: &CapturedRequest) -> Vec<(String, String)> {
    url::form_urlencoded::parse(&request.body)
        .into_owned()
        .collect()
}

fn pairs(fields: &[(&str, &str)]) -> Vec<(String, String)> {
    fields
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// Value of cookie `name` in the request's `Cookie` header.
fn cookie_value(request: &CapturedRequest, name: &str) -> Option<String> {
    request
        .header("cookie")?
        .split(';')
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value.to_string())
}

#[tokio::test]
async fn requested_refresh_swaps_cookies_everywhere_and_confirms_with_the_old_token() {
    let bili = Bili::start(OLD_COOKIE).await;
    let info = bili.info(true);
    let correspond = bili.correspond();
    let refresh = bili.refresh(refreshed());
    let confirm = bili.confirm(Reply::ok().json(&json!({ "code": 0, "message": "0", "ttl": 1 })));
    let shared = bili.cm.shared_credential();

    let outcome = bili.cm.refresh_if_requested().await.expect("refresh");
    assert_eq!(outcome, RefreshOutcome::Refreshed);
    bili.fake.assert_no_unmatched();
    assert_eq!(
        bili.paths_after_init(),
        [
            COOKIE_INFO.to_string(),
            correspond.last_request().path,
            COOKIE_REFRESH.to_string(),
            CONFIRM_REFRESH.to_string(),
        ]
    );

    // 1. cookie/info: the csrf of the current cookie.
    let info = info.last_request();
    assert_eq!(info.query_param("csrf").as_deref(), Some("old-csrf"));
    assert_eq!(cookie_value(&info, "SESSDATA").as_deref(), Some("old-sess"));

    // 2. correspond page: an RSA-1024 ciphertext as lowercase hex.
    let page = correspond.last_request();
    let path = page.path.strip_prefix(CORRESPOND).unwrap();
    assert_eq!(path.len(), 256, "{path}");
    assert!(
        path.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "{path}"
    );
    assert_eq!(cookie_value(&page, "SESSDATA").as_deref(), Some("old-sess"));

    // 3. cookie/refresh: old cookie, old token, the page's refresh_csrf.
    let refresh = refresh.last_request();
    assert_eq!(
        form(&refresh),
        pairs(&[
            ("csrf", "old-csrf"),
            ("refresh_csrf", REFRESH_CSRF),
            ("source", "main_web"),
            ("refresh_token", "old-token"),
        ])
    );
    assert_eq!(
        cookie_value(&refresh, "SESSDATA").as_deref(),
        Some("old-sess")
    );

    // 4. confirm/refresh: new cookie and its csrf, but the OLD token.
    let confirm = confirm.last_request();
    assert_eq!(
        form(&confirm),
        pairs(&[("csrf", "new-csrf"), ("refresh_token", "old-token")])
    );
    assert_eq!(
        cookie_value(&confirm, "SESSDATA").as_deref(),
        Some("new-sess")
    );
    assert_eq!(
        cookie_value(&confirm, "bili_jct").as_deref(),
        Some("new-csrf")
    );

    // The new credential is live, shared with parsers, and saved.
    let expected =
        "SESSDATA=new-sess; bili_jct=new-csrf; buvid3=b3; DedeUserID=42; ac_time_value=new-token";
    assert_eq!(bili.cookie().await, expected);
    let live = shared.read().await.clone().expect("shared credential");
    assert_eq!(live.to_cookie_string(), expected);
    assert_eq!(live.refresh_token.as_deref(), Some("new-token"));
    let saved = saved_credential(&bili.saved());
    assert_eq!(saved.to_cookie_string(), expected);
    assert_eq!(saved.ac_time_value.as_deref(), Some("new-token"));
}

#[tokio::test]
async fn no_refresh_requested_means_one_check_and_no_change() {
    let bili = Bili::start(OLD_COOKIE).await;
    let info = bili.info(false);
    let before = bili.saved();

    let outcome = bili.cm.refresh_if_requested().await.expect("check");
    assert_eq!(outcome, RefreshOutcome::NotNeeded);
    assert_eq!(info.hits(), 1);
    assert_eq!(bili.paths_after_init(), [COOKIE_INFO]);
    assert_eq!(bili.cookie().await, OLD_COOKIE);
    assert_eq!(bili.saved(), before);
}

#[tokio::test]
async fn rejected_refresh_keeps_the_old_credential_and_file() {
    let bili = Bili::start(OLD_COOKIE).await;
    bili.info(true);
    bili.correspond();
    let refresh = bili.refresh(api_code(86095));
    let confirm = bili.confirm(api_code(0));
    let before = bili.saved();

    let error = bili.cm.refresh_if_requested().await.unwrap_err();
    let ParserError::Context { source, url, .. } = &error else {
        panic!("unexpected error: {error}");
    };
    assert!(
        matches!(**source, ParserError::Api { code: 86095, .. }),
        "{error}"
    );
    assert!(url.ends_with(COOKIE_REFRESH), "{url}");
    assert!(!error.to_string().contains("old-csrf"), "{error}");

    assert_eq!(refresh.hits(), 1);
    assert_eq!(confirm.hits(), 0);
    assert_eq!(bili.cookie().await, OLD_COOKIE);
    assert_eq!(bili.saved(), before);
}

#[tokio::test]
async fn failed_confirm_keeps_the_new_credential() {
    let bili = Bili::start(OLD_COOKIE).await;
    bili.info(true);
    bili.correspond();
    bili.refresh(refreshed());
    let confirm = bili.confirm(api_code(-111));

    let error = bili.cm.refresh_if_requested().await.unwrap_err();
    assert!(error.to_string().contains("-111"), "{error}");
    assert_eq!(confirm.hits(), 1);

    // cookie/refresh spent the old token: only the new credential can be
    // refreshed again.
    let expected =
        "SESSDATA=new-sess; bili_jct=new-csrf; buvid3=b3; DedeUserID=42; ac_time_value=new-token";
    assert_eq!(bili.cookie().await, expected);
    assert_eq!(saved_credential(&bili.saved()).to_cookie_string(), expected);
}

#[tokio::test]
async fn cookie_without_ac_time_value_cannot_be_refreshed() {
    let bili = Bili::start("SESSDATA=old-sess; bili_jct=old-csrf; DedeUserID=42").await;
    bili.info(true);

    let outcome = bili.cm.refresh_if_requested().await.expect("check");
    assert_eq!(outcome, RefreshOutcome::MissingRefreshToken);
    assert!(bili.paths_after_init().is_empty());
}

#[tokio::test]
async fn restart_after_refresh_uses_the_saved_cookie_until_config_has_a_valid_one() {
    let bili = Bili::start(OLD_COOKIE).await;
    bili.info(true);
    bili.correspond();
    bili.refresh(refreshed());
    bili.confirm(api_code(0));
    bili.cm.refresh_if_requested().await.expect("refresh");
    let refreshed_file = bili.saved();

    // Restart with the now retired cookie still in config: nav rejects it
    // and accepts the saved one.
    let nav_route = bili.nav.clone().reply(nav(false)).reply(nav(true));
    let restarted =
        CredentialManager::new(bili.fake.client(bili.app.config()), bili.app.data_path());
    restarted.init(Some(OLD_COOKIE)).await.expect("init");
    assert_eq!(nav_route.hits(), 3);
    assert_eq!(
        restarted.get_cookie_string().await.as_deref(),
        Some(
            "SESSDATA=new-sess; bili_jct=new-csrf; buvid3=b3; DedeUserID=42; ac_time_value=new-token"
        )
    );
    assert_eq!(bili.saved(), refreshed_file);

    // A cookie newly pasted into config, which nav accepts, wins.
    let fresh = "SESSDATA=fresh-sess; bili_jct=fresh-csrf; ac_time_value=fresh-token";
    let nav_route = nav_route.reply(nav(true));
    let restarted =
        CredentialManager::new(bili.fake.client(bili.app.config()), bili.app.data_path());
    restarted.init(Some(fresh)).await.expect("init");
    assert_eq!(nav_route.hits(), 4);
    assert_eq!(restarted.get_cookie_string().await.as_deref(), Some(fresh));
    assert_eq!(saved_credential(&bili.saved()).to_cookie_string(), fresh);
}
