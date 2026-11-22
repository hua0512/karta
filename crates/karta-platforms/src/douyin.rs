use std::future::Future;
use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;
use karta_core::Platform;
use karta_core::config::DouyinMobileConfig;
use karta_core::constants::MOBILE_USER_AGENT;
use karta_core::data::ParseResult;
use karta_core::download::SharedDownloader;
use karta_core::error::{ParserError, Result};
use karta_core::http;
use karta_core::parser::{MatchResult, Parser, RegisteredPattern};
use wreq::header::{HeaderMap, HeaderValue};

mod a_bogus;
mod api;
mod article;
mod article_ssr;
mod caption;
mod filters;
mod html;
mod live;
mod mobile;
mod patterns;
mod probe;
mod result;
mod share;
mod token;

use api::{ApiArticleInfo, ApiResponse, AwemeDetail};
use filters::DetailErrors;
use patterns::{LIVE_ID_RE, VIDEO_ID_RE, WEBCAST_ID_RE};
use result::build_aweme_result;
use share::ShareTarget;

/// Official video/note detail API.
const API_VIDEO_DETAIL: &str = "https://www.douyin.com/aweme/v1/web/aweme/detail/";

/// Standard query parameters for video detail API.
const API_VIDEO_DETAIL_PARAMS: &str = "device_platform=webapp&aid=6383&channel=channel_pc_web&pc_client_type=1&version_code=190500&version_name=19.5.0&cookie_enabled=true&screen_width=1344&screen_height=756&browser_language=zh-CN&browser_platform=Win32&browser_name=Firefox&browser_version=118.0&browser_online=true&engine_name=Gecko&engine_version=109.0&os_name=Windows&os_version=10&cpu_core_num=16&device_memory=&platform=PC";

/// Toutiao CDN for video downloads.
pub(super) const CDN_VIDEO_PLAY: &str = "https://aweme.snssdk.com/aweme/v1/play/";

/// Mobile Chrome User-Agent used for API requests and a-bogus signing.
const DOUYIN_USER_AGENT: &str = "Mozilla/5.0 (Linux; Android 5.0; SM-G900P Build/LRX21T) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/70.0.3538.25 Mobile Safari/537.36";

#[derive(Clone, Copy, PartialEq, Eq)]
enum ContentKind {
    Post,
    Note,
    Article,
}

impl ContentKind {
    fn from_url(url: &str) -> Self {
        if url.contains("/article/") {
            Self::Article
        } else if url.contains("/note/") {
            Self::Note
        } else {
            Self::Post
        }
    }
}

// ---------------------------------------------------------------------------
// DouyinParser
// ---------------------------------------------------------------------------

/// Douyin parser with official API support and HTML-scraping fallback.
pub struct DouyinParser {
    downloader: SharedDownloader,
    cookie: Option<String>,
    mobile: Option<DouyinMobileConfig>,
    mobile_registration: tokio::sync::Mutex<mobile::RegistrationState>,
    mobile_cache_path: Option<std::path::PathBuf>,
    patterns: Vec<RegisteredPattern>,
}

impl DouyinParser {
    pub fn new(downloader: SharedDownloader, cookie: Option<String>) -> Self {
        Self {
            downloader,
            cookie,
            mobile: None,
            mobile_registration: tokio::sync::Mutex::new(Default::default()),
            mobile_cache_path: None,
            patterns: patterns::registered_patterns(),
        }
    }

    /// Enable the signed mobile fallback with a registered Android identity.
    pub fn with_mobile_config(mut self, config: Option<DouyinMobileConfig>) -> Self {
        self.mobile = config;
        self.mobile_registration = tokio::sync::Mutex::new(Default::default());
        self
    }

    /// Cache automatically registered identities across process restarts.
    pub fn with_mobile_cache(mut self, path: std::path::PathBuf) -> Self {
        self.mobile_cache_path = Some(path);
        self
    }

    // -----------------------------------------------------------------------
    // Header & signing helpers
    // -----------------------------------------------------------------------

    /// Headers for official API calls.
    ///
    /// Cookie-authenticated calls keep the signed web request headers; detail
    /// requests add the Open Platform Origin at the call site. Anonymous calls
    /// use the Open Platform Origin/Referer pair.
    fn api_headers(&self) -> HeaderMap {
        Self::detail_headers(self.cookie.as_deref())
    }

    fn detail_headers(cookie: Option<&str>) -> HeaderMap {
        let referer = if cookie.is_some() {
            "https://www.douyin.com/"
        } else {
            "https://open.douyin.com/"
        };
        let mut headers = http::headers_with_referer_and_cookie(DOUYIN_USER_AGENT, referer, cookie);
        if cookie.is_none() {
            headers.insert(
                wreq::header::ORIGIN,
                HeaderValue::from_static("https://open.douyin.com"),
            );
        }
        if let Ok(v) = "zh-CN,zh;q=0.8,zh-TW;q=0.7,zh-HK;q=0.5,en-US;q=0.3,en;q=0.2".parse() {
            headers.insert("Accept-Language", v);
        }
        headers
    }

    /// Headers for short URL redirect resolution (Android UA, no Referer).
    fn redirect_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Ok(v) = DOUYIN_USER_AGENT.parse() {
            headers.insert(wreq::header::USER_AGENT, v);
        }
        headers
    }

    /// Headers for fallback HTML scraping (mobile UA).
    fn mobile_headers(&self) -> HeaderMap {
        http::headers_with_referer(MOBILE_USER_AGENT, "https://www.douyin.com/")
    }

    /// Build a signed API URL by appending the a-bogus parameter.
    fn sign_url(base_url: &str, query_params: &str) -> String {
        let a_bogus = a_bogus::generate_a_bogus(query_params, DOUYIN_USER_AGENT);
        format!("{}?{}&a_bogus={}", base_url, query_params, a_bogus)
    }

    // -----------------------------------------------------------------------
    // Official API path
    // -----------------------------------------------------------------------

    /// Parse content via the official Douyin API.
    ///
    /// Authenticated requests retain the existing signed parameter set.
    /// Anonymous requests use the Open Platform's small public request shape.
    async fn parse_via_api(
        &self,
        aweme_id: &str,
        errors: &mut DetailErrors,
        cookie: Option<&str>,
        article: bool,
    ) -> Result<ParseResult> {
        let url = if cookie.is_some() {
            let query = format!("{}&aweme_id={}", API_VIDEO_DETAIL_PARAMS, aweme_id);
            Self::sign_url(API_VIDEO_DETAIL, &query)
        } else {
            format!("{}?aweme_id={}&aid=6383", API_VIDEO_DETAIL, aweme_id)
        };

        // Include the Open Platform Origin even with a cookie to avoid 403
        // `uifid not found` responses, while keeping the web Referer.
        let mut headers = Self::detail_headers(cookie);
        headers.insert(
            wreq::header::ORIGIN,
            HeaderValue::from_static("https://open.douyin.com"),
        );
        let client = self.downloader.client();
        let response = http::get_with_retry(client, &url, headers).await?;

        tracing::debug!(
            aweme_id,
            status = %response.status(),
            content_type = ?response.headers().get(wreq::header::CONTENT_TYPE),
            content_encoding = ?response.headers().get(wreq::header::CONTENT_ENCODING),
            content_length = ?response.headers().get(wreq::header::CONTENT_LENGTH),
            "Douyin API response headers"
        );

        http::check_status(&response)?;

        let data: ApiResponse = match response.json().await {
            Ok(v) => v,
            Err(e) => {
                let mut chain = Vec::new();
                let mut cur: Option<&(dyn std::error::Error + 'static)> = Some(&e);
                while let Some(err) = cur {
                    chain.push(err.to_string());
                    cur = err.source();
                }
                tracing::warn!(aweme_id, error_chain = ?chain, "Douyin API decode failed");
                return Err(ParserError::Parse(format!(
                    "Failed to parse API response: {}",
                    e
                )));
            }
        };

        let item = detail_item(data, aweme_id, errors)?;
        if article
            && !item
                .article_info
                .as_ref()
                .is_some_and(ApiArticleInfo::has_content)
        {
            return Err(ParserError::ContentNotFound(
                "Douyin detail returned no article body".into(),
            ));
        }

        let mut result = build_aweme_result(&item);
        self.probe_cdn_ratios(&mut result).await;
        Ok(result)
    }

    // -----------------------------------------------------------------------
    // Content dispatch (API with fallback)
    // -----------------------------------------------------------------------

    /// Parse content by aweme_id: try the official API first, then fall back to
    /// HTML scraping. Both authenticated and anonymous API requests are
    /// supported; the latter uses the Open Platform request contract.
    async fn parse_content(&self, aweme_id: &str) -> Result<ParseResult> {
        self.parse_content_kind(aweme_id, ContentKind::Post).await
    }

    async fn parse_content_kind(&self, aweme_id: &str, kind: ContentKind) -> Result<ParseResult> {
        tracing::debug!(
            aweme_id,
            authenticated = self.cookie.is_some(),
            "Douyin: trying official API"
        );
        let mut errors = DetailErrors::default();
        let article = kind == ContentKind::Article;
        let cookie = if article {
            None
        } else {
            self.cookie.as_deref()
        };
        let article_recovery = kind != ContentKind::Post && self.cookie.is_some();
        // Known articles prefer the public contract even when a cookie is
        // configured. Ordinary posts retain the existing authenticated path.
        match detail_attempt(
            self.parse_via_api(aweme_id, &mut errors, cookie, article),
            if article_recovery {
                Some(8)
            } else {
                self.mobile.as_ref().map(|_| 12)
            },
        )
        .await
        {
            Ok(result) => return Ok(result),
            Err(error) => errors.record(error),
        }
        if article_recovery {
            let alternate_cookie = if article {
                self.cookie.as_deref()
            } else {
                None
            };
            match detail_attempt(
                self.parse_via_api(aweme_id, &mut errors, alternate_cookie, article),
                Some(6),
            )
            .await
            {
                Ok(result) => return Ok(result),
                Err(error) => errors.record(error),
            }
            match detail_attempt(self.parse_article_ssr(aweme_id), Some(8)).await {
                Ok(result) => return Ok(result),
                Err(error) => errors.record(error),
            }
        }
        // The mobile share-page schema has no article body. Notes can still
        // be ordinary albums, so retain that fallback for unclassified notes.
        if !article {
            match detail_attempt(
                self.parse_video_html(aweme_id, &mut errors),
                self.mobile
                    .as_ref()
                    .map(|_| if article_recovery { 8 } else { 16 }),
            )
            .await
            {
                Ok(result) => return Ok(result),
                Err(error) => errors.record(error),
            }
        }
        if self.mobile.is_some() {
            match detail_attempt(self.parse_mobile_detail(aweme_id, &mut errors), Some(28)).await {
                Ok(result)
                    if !article || result.extra.get("类型").is_some_and(|kind| kind == "文章") =>
                {
                    return Ok(result);
                }
                Ok(_) => errors.record(ParserError::ContentNotFound(
                    "Douyin mobile detail returned no article body".into(),
                )),
                Err(error) => errors.record(error),
            }
        }
        Err(errors.finish())
    }

    async fn parse_article_ssr(&self, item_id: &str) -> Result<ParseResult> {
        const MAX_PAGE_BYTES: usize = 8 * 1024 * 1024;
        const DESKTOP_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36";
        let mut headers = http::headers_with_referer_and_cookie(
            DESKTOP_USER_AGENT,
            "https://www.douyin.com/",
            self.cookie.as_deref(),
        );
        headers.insert(
            wreq::header::ACCEPT,
            HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            ),
        );
        headers.insert(
            wreq::header::ACCEPT_LANGUAGE,
            HeaderValue::from_static("zh-CN,zh;q=0.9"),
        );
        let response = self
            .downloader
            .client()
            .get(format!("https://www.douyin.com/note/{item_id}"))
            .headers(headers)
            .timeout(Duration::from_secs(8))
            .send()
            .await?;
        http::check_status(&response)?;
        if response
            .content_length()
            .is_some_and(|length| length > MAX_PAGE_BYTES as u64)
        {
            return Err(ParserError::Parse(
                "Douyin article page is too large".into(),
            ));
        }
        let mut stream = response.bytes_stream();
        let mut body = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            if body.len().saturating_add(chunk.len()) > MAX_PAGE_BYTES {
                return Err(ParserError::Parse(
                    "Douyin article page is too large".into(),
                ));
            }
            body.extend_from_slice(&chunk);
        }
        let html = std::str::from_utf8(&body)
            .map_err(|_| ParserError::Parse("Douyin article page is not UTF-8".into()))?;
        let detail = article_ssr::extract_article(html, item_id)?;
        let result = build_aweme_result(&detail);
        if result.contents.is_empty() {
            return Err(ParserError::ContentNotFound(
                "Douyin article body is empty".into(),
            ));
        }
        Ok(result)
    }

    // -----------------------------------------------------------------------
    // Short URL resolution
    // -----------------------------------------------------------------------

    async fn parse_short_url(&self, full_match: &str) -> Result<ParseResult> {
        let short_url = format!("https://{}", full_match);

        let client = self.downloader.client();
        // Hop-by-hop resolution: ttwid is set by the redirecting hop, which
        // automatic redirect following would discard.
        let resolved = http::resolve_redirect(client, &short_url, Self::redirect_headers()).await?;

        let final_url = resolved.final_url;

        // Extract ttwid from set-cookie headers (needed for live type 2)
        let ttwid = live::ttwid_from_set_cookies(&resolved.set_cookies);

        // Check for webcast.amemv.com (live type 2)
        if let Some(caps) = WEBCAST_ID_RE.captures(&final_url)
            && let Some(room_id) = caps.get(1).map(|m| m.as_str())
        {
            return self.parse_live_reflow(room_id, &ttwid).await;
        }

        // Check for live stream (type 1)
        if let Some(caps) = LIVE_ID_RE.captures(&final_url)
            && let Some(room_id) = caps.get(1).map(|m| m.as_str())
        {
            return self.parse_live(room_id).await;
        }

        if let Some(caps) = VIDEO_ID_RE.captures(&final_url) {
            let video_id = caps.get(1).map(|m| m.as_str()).unwrap_or("");

            if final_url.contains("/slides/") {
                return self.parse_slides_with_fallback(video_id).await;
            }
            return self
                .parse_content_kind(video_id, ContentKind::from_url(&final_url))
                .await;
        }

        Err(ParserError::PatternNotMatched(format!(
            "Could not extract video ID from: {}",
            final_url
        )))
    }

    // -----------------------------------------------------------------------
    // Share 口令
    // -----------------------------------------------------------------------

    /// Resolve an in-app share token to its item and parse that item.
    ///
    /// Carrier schemas distinguish posts, articles and live rooms.
    async fn parse_share_token(&self, token: &str, matched_text: &str) -> Result<ParseResult> {
        match self.resolve_share_target(token, matched_text).await? {
            ShareTarget::Item { id, article, note } => {
                let kind = if article {
                    ContentKind::Article
                } else if note {
                    ContentKind::Note
                } else {
                    ContentKind::Post
                };
                self.parse_content_kind(&id, kind).await
            }
            ShareTarget::Live {
                web_rid: Some(id), ..
            } => self.parse_live(&id).await,
            ShareTarget::Live {
                room_id: Some(id), ..
            } => self.parse_live_reflow(&id, "").await,
            ShareTarget::Live { .. } => Err(ParserError::ContentNotFound(
                "Douyin share has no live room ID".into(),
            )),
        }
    }

    // -----------------------------------------------------------------------
    // Top-level dispatch
    // -----------------------------------------------------------------------

    async fn dispatch(&self, match_result: MatchResult<'_>) -> Result<ParseResult> {
        match match_result.pattern {
            "v.douyin" | "jx.douyin" => self.parse_short_url(match_result.full_match).await,
            "webcast_amemv" => {
                // Extract room_id from the webcast URL
                let url = format!("https://{}", match_result.full_match);
                let room_id = WEBCAST_ID_RE
                    .captures(&url)
                    .and_then(|c| c.get(1))
                    .map(|m| m.as_str())
                    .unwrap_or("");
                if room_id.is_empty() {
                    return Err(ParserError::PatternNotMatched(
                        "Could not extract room ID from webcast URL".into(),
                    ));
                }
                // No ttwid available from direct URL match; use empty string
                self.parse_live_reflow(room_id, "").await
            }
            "douyin_live" | "douyin_live_path" => {
                let room_id = match_result.group(1).unwrap_or("");
                self.parse_live(room_id).await
            }
            "douyin_share_token"
            | "douyin_share_phrase"
            | "douyin_share_command"
            | "douyin_activity" => {
                let token = match_result.require_group(1)?;
                self.parse_share_token(token, match_result.full_match).await
            }
            "douyin" | "iesdouyin" | "m.douyin" | "jingxuan.douyin" | "douyin_slides"
            | "douyin_modal" => {
                let id = match_result.group(1).unwrap_or("");
                // Fallback slides path when no cookie (legacy pattern detection)
                if self.cookie.is_none() && match_result.full_match.contains("/slides/") {
                    return self.parse_slides_with_fallback(id).await;
                }
                self.parse_content_kind(id, ContentKind::from_url(match_result.full_match))
                    .await
            }
            _ => Err(ParserError::PatternNotMatched(format!(
                "Unknown pattern: {}",
                match_result.pattern
            ))),
        }
    }
}

/// Limit a fallback stage without changing the overall engine deadline.
async fn detail_attempt<T>(
    future: impl Future<Output = Result<T>>,
    seconds: Option<u64>,
) -> Result<T> {
    match seconds {
        Some(seconds) => tokio::time::timeout(Duration::from_secs(seconds), future)
            .await
            .map_err(|_| ParserError::Timeout { seconds })?,
        None => future.await,
    }
}

/// Successful items take precedence over filter metadata in the same envelope.
fn detail_item(data: ApiResponse, item_id: &str, errors: &mut DetailErrors) -> Result<AwemeDetail> {
    if data.status_code.unwrap_or(0) != 0 {
        errors.observe(item_id, data.filter_list.as_deref());
        return Err(ParserError::Api {
            code: data.status_code.unwrap_or(0),
            message: data
                .status_msg
                .unwrap_or_else(|| "Douyin detail request rejected".into()),
        });
    }
    if let Some(item) = data.aweme_detail {
        if item.aweme_id.as_deref().is_some_and(|id| id != item_id) {
            return Err(ParserError::Parse(
                "Douyin detail returned a different item".into(),
            ));
        }
        if item
            .article_info
            .as_ref()
            .is_some_and(|info| info.has_content() && !article::has_body(info))
        {
            errors.observe(item_id, data.filter_list.as_deref());
            return Err(ParserError::ContentNotFound(
                "Douyin detail returned an incomplete article body".into(),
            ));
        }
        if build_aweme_result(&item).contents.is_empty() {
            errors.observe(item_id, data.filter_list.as_deref());
            return Err(ParserError::ContentNotFound(
                "Douyin detail returned no content".into(),
            ));
        }
        return Ok(item);
    }
    errors.observe(item_id, data.filter_list.as_deref());
    Err(ParserError::ContentNotFound(
        "aweme_detail not found".into(),
    ))
}

#[async_trait]
impl Parser for DouyinParser {
    fn platform(&self) -> Platform {
        Platform::Douyin
    }

    fn patterns(&self) -> &[RegisteredPattern] {
        &self.patterns
    }

    async fn parse(&self, match_result: MatchResult<'_>) -> Result<ParseResult> {
        tracing::debug!(
            pattern = match_result.pattern,
            matched = match_result.full_match,
            "Douyin: parsing"
        );
        self.dispatch(match_result).await
    }
}
