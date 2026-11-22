use async_trait::async_trait;
use parking_lot::Mutex;
use wreq::header::HeaderValue;

use karta_core::Platform;
use karta_core::constants::DEFAULT_USER_AGENT;
use karta_core::data::ParseResult;
use karta_core::download::SharedDownloader;
use karta_core::error::{ParserError, Result};
use karta_core::http;
use karta_core::parser::{MatchResult, Parser, RegisteredPattern};

mod api;
mod embed;
mod patterns;
mod post;
mod result;
mod shortcode;
mod story;

use api::*;
use shortcode::generate_csrf_token;

/// Embed page base URL (append `/{shortcode}/embed/captioned/`)
pub(super) const EMBED_BASE: &str = "https://www.instagram.com";
/// GraphQL query endpoint (POST, requires cookie)
pub(super) const GRAPHQL_API: &str = "https://www.instagram.com/graphql/query/";
/// REST API base (www.instagram.com, gallery-dl approach)
pub(super) const API_BASE: &str = "https://www.instagram.com/api/v1";
/// Stories API (requires cookie)
pub(super) const STORIES_API: &str = "https://www.instagram.com/api/v1/feed/reels_media/";

// Public web-client identifiers lifted from instagram.com's browser bundle
// (gallery-dl tracks the same values). Instagram rotates them with web
// releases, so refresh them together when API calls start failing.

/// `x-ig-app-id` of the Instagram web app.
pub(super) const WEB_APP_ID: &str = "936619743392459";
/// `x-asbd-id` sent with REST API calls.
pub(super) const ASBD_ID_REST: &str = "129477";
/// `x-asbd-id` sent with GraphQL calls.
pub(super) const ASBD_ID_GRAPHQL: &str = "198387";
/// `x-instagram-ajax` build number sent with GraphQL calls.
pub(super) const AJAX_BUILD_ID: &str = "1006267176";
/// Stable `query_hash` of the post/shortcode GraphQL query.
pub(super) const POST_QUERY_HASH: &str = "9f8827793ef34641b2fb195d4d41151c";

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Instagram parser using gallery-dl's REST-first approach with GraphQL
/// and embed fallback.
pub struct InstagramParser {
    downloader: SharedDownloader,
    cookie: Option<String>,
    /// Self-generated CSRF token (gallery-dl approach — not extracted from cookie).
    csrf_token: String,
    /// Dynamic `x-ig-www-claim` value, updated from response headers.
    www_claim: Mutex<String>,
    patterns: Vec<RegisteredPattern>,
}

impl InstagramParser {
    pub fn new(downloader: SharedDownloader, cookie: Option<String>) -> Self {
        let patterns = patterns::registered_patterns(cookie.is_some());
        Self {
            downloader,
            cookie,
            csrf_token: generate_csrf_token(),
            www_claim: Mutex::new("0".to_string()),
            patterns,
        }
    }

    fn headers(&self) -> wreq::header::HeaderMap {
        http::headers_with_referer_and_cookie(
            DEFAULT_USER_AGENT,
            "https://www.instagram.com/",
            self.cookie.as_deref(),
        )
    }

    /// Build the full cookie string, injecting our generated csrftoken.
    fn full_cookie(&self) -> String {
        match &self.cookie {
            Some(c) => format!("csrftoken={}; {}", self.csrf_token, c),
            None => format!("csrftoken={}", self.csrf_token),
        }
    }

    /// Common API headers shared by REST and GraphQL.
    fn base_api_headers(&self) -> wreq::header::HeaderMap {
        let cookie = self.full_cookie();
        let mut h = http::headers_with_referer_and_cookie(
            DEFAULT_USER_AGENT,
            "https://www.instagram.com/",
            Some(&cookie),
        );
        let claim = self.www_claim.lock().clone();
        h.insert("accept", HeaderValue::from_static("*/*"));
        // Runtime values: skip silently on the (unreachable in practice) case
        // of a value with invalid header characters.
        if let Ok(v) = HeaderValue::from_str(&self.csrf_token) {
            h.insert("x-csrftoken", v);
        }
        if let Ok(v) = HeaderValue::from_str(&claim) {
            h.insert("x-ig-www-claim", v);
        }
        h.insert("x-ig-app-id", HeaderValue::from_static(WEB_APP_ID));
        h.insert(
            "x-requested-with",
            HeaderValue::from_static("XMLHttpRequest"),
        );
        h.insert("connection", HeaderValue::from_static("keep-alive"));
        h.insert("sec-fetch-dest", HeaderValue::from_static("empty"));
        h.insert("sec-fetch-mode", HeaderValue::from_static("cors"));
        h.insert("sec-fetch-site", HeaderValue::from_static("same-origin"));
        h
    }

    /// REST API headers.
    fn rest_headers(&self) -> wreq::header::HeaderMap {
        let mut h = self.base_api_headers();
        h.insert("x-asbd-id", HeaderValue::from_static(ASBD_ID_REST));
        h
    }

    /// GraphQL API headers (distinct X-ASBD-ID, plus X-Instagram-AJAX).
    fn graphql_headers(&self) -> wreq::header::HeaderMap {
        let mut h = self.base_api_headers();
        h.insert("x-asbd-id", HeaderValue::from_static(ASBD_ID_GRAPHQL));
        h.insert("x-instagram-ajax", HeaderValue::from_static(AJAX_BUILD_ID));
        h
    }

    /// Update www_claim from a response's headers (gallery-dl approach).
    fn update_www_claim(&self, response: &wreq::Response) {
        if let Some(claim) = response.headers().get("x-ig-set-www-claim")
            && let Ok(s) = claim.to_str()
        {
            *self.www_claim.lock() = s.to_string();
        }
    }

    // -----------------------------------------------------------------------
    // Stories (requires cookie)
    // -----------------------------------------------------------------------

    async fn parse_story(&self, username: &str, story_id: &str) -> Result<ParseResult> {
        let client = self.downloader.client();
        let url = format!(
            "https://www.instagram.com/stories/{}/{}/",
            username, story_id
        );

        // First, resolve user ID from the username via the web profile page
        let user_id = self.resolve_user_id(username).await?;

        let api_url = format!("{}?reel_ids={}&media_id={}", STORIES_API, user_id, story_id);

        let response = client
            .get(&api_url)
            .headers(self.rest_headers())
            .send()
            .await?;
        self.update_www_claim(&response);
        http::check_status(&response)?;

        let data: StoriesResponse = response.json().await.map_err(|e| {
            ParserError::Parse(format!("Failed to parse Instagram stories response: {}", e))
        })?;

        let reel = data
            .reels_media
            .first()
            .ok_or_else(|| ParserError::ContentNotFound("Story not found".to_string()))?;

        // Find the specific story item
        let item = reel
            .items
            .iter()
            .find(|i| i.pk.as_deref() == Some(story_id) || i.id.as_deref() == Some(story_id))
            .or_else(|| reel.items.first())
            .ok_or_else(|| ParserError::ContentNotFound("Story item not found".to_string()))?;

        Ok(story::build_story_result(reel, item, username, &url))
    }

    /// Resolve an Instagram username to a numeric user ID.
    async fn resolve_user_id(&self, username: &str) -> Result<String> {
        let client = self.downloader.client();
        let url = format!(
            "https://www.instagram.com/api/v1/users/web_profile_info/?username={}",
            username
        );

        let response = client.get(&url).headers(self.rest_headers()).send().await?;
        self.update_www_claim(&response);
        http::check_status(&response)?;

        let data: serde_json::Value = response.json().await?;
        data.pointer("/data/user/id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| {
                ParserError::ContentNotFound(format!("Could not resolve user ID for @{}", username))
            })
    }
}

// ---------------------------------------------------------------------------
// Parser trait impl
// ---------------------------------------------------------------------------

#[async_trait]
impl Parser for InstagramParser {
    fn platform(&self) -> Platform {
        Platform::Instagram
    }

    fn patterns(&self) -> &[RegisteredPattern] {
        &self.patterns
    }

    async fn parse(&self, match_result: MatchResult<'_>) -> Result<ParseResult> {
        tracing::debug!(
            pattern = match_result.pattern,
            matched = match_result.full_match,
            "Instagram: parsing"
        );

        match match_result.pattern {
            "instagram_post" => {
                let shortcode = match_result.require_group(1)?;
                self.parse_post(shortcode).await
            }
            "instagram_story" => {
                let username = match_result.require_group(1)?;
                let story_id = match_result.require_group(2)?;
                self.parse_story(username, story_id).await
            }
            _ => Err(ParserError::PatternNotMatched(format!(
                "Unknown Instagram pattern: {}",
                match_result.pattern
            ))),
        }
    }
}
