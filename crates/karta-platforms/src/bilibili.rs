use async_trait::async_trait;
use wreq::header::HeaderMap;

use karta_core::Platform;
use karta_core::config::{BiliCdnMode, BiliVideoCodec, BiliVideoQuality, Config};
use karta_core::data::ParseResult;
use karta_core::download::SharedDownloader;
use karta_core::error::{ParserError, Result};
use karta_core::http;
use karta_core::parser::{MatchResult, Parser, RegisteredPattern};

static PAGE_NUM_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"[?&]p=(\d+)").expect("valid page regex"));

mod api;
mod article;
mod bangumi;
mod cdn;
pub mod credential;
mod dynamic;
mod favorites;
mod format;
mod live;
mod opus;
mod patterns;
mod stream;
mod video;
mod wbi;
use api::ApiResponse;
use credential::SharedBiliCredential;
pub use credential::{BiliCredential, CredentialManager, QrLoginState, RefreshOutcome};
use patterns::registered_patterns;
use stream::StreamOptions;
use wbi::WbiSigner;

/// Bilibili's anti-bot edge rejects browser user agents when their advertised
/// browser does not exactly match the connection fingerprint. API requests do
/// not need a browser identity, so use the OpenAI agent identifier.
const API_USER_AGENT: &str = "ChatGPT-User/1.0";

/// Bilibili parser
pub struct BilibiliParser {
    downloader: SharedDownloader,
    cookie: Option<String>,
    credential: Option<SharedBiliCredential>,
    patterns: Vec<RegisteredPattern>,
    video_quality: BiliVideoQuality,
    video_codecs: Vec<BiliVideoCodec>,
    cdn_mode: BiliCdnMode,
    smart_resolution: bool,
    live_record_seconds: u64,
    wbi_signer: WbiSigner,
}

impl BilibiliParser {
    /// Create a new Bilibili parser
    pub fn new(downloader: SharedDownloader, config: &Config) -> Self {
        Self {
            downloader,
            cookie: config.bili_cookie.clone(),
            credential: None,
            patterns: registered_patterns(),
            video_quality: config.bili_video_quality,
            video_codecs: config.bili_video_codecs.clone(),
            cdn_mode: config.bili_cdn_mode,
            smart_resolution: config.bili_smart_resolution,
            live_record_seconds: config.bili_live_record_seconds,
            wbi_signer: WbiSigner::new(),
        }
    }

    /// Stream-selection knobs for this parser's configuration.
    ///
    /// The size budget is only applied when smart resolution is enabled;
    /// otherwise selection picks by quality first and codec second, letting
    /// oversized videos fail against the download size cap.
    fn stream_options(&self) -> StreamOptions<'_> {
        StreamOptions {
            cdn_mode: self.cdn_mode,
            size_budget: self.smart_resolution.then(|| self.downloader.max_size()),
            ..StreamOptions::new(self.video_quality, &self.video_codecs)
        }
    }

    /// Attach the live credential store used by QR login and refresh.
    pub fn with_shared_credential(mut self, credential: SharedBiliCredential) -> Self {
        self.credential = Some(credential);
        self
    }

    async fn cookie_string(&self) -> Option<String> {
        if let Some(credential) = &self.credential
            && let Some(credential) = credential.read().await.as_ref()
        {
            return Some(credential.to_cookie_string());
        }
        self.cookie.clone()
    }

    /// Get default headers for Bilibili API requests
    async fn get_headers(&self) -> HeaderMap {
        let cookie = self.cookie_string().await;
        http::headers_with_referer_and_cookie(
            API_USER_AGENT,
            "https://www.bilibili.com",
            cookie.as_deref(),
        )
    }

    /// Fetch WBI signing keys when they are missing or older than [`wbi::KEY_TTL`].
    async fn ensure_wbi(&self) {
        if self.wbi_signer.needs_refresh() {
            self.refresh_wbi_keys().await;
        }
    }

    /// Fetch fresh WBI signing keys from the nav API.
    ///
    /// nav returns the keys to anonymous visitors too (with code -101), and
    /// WBI endpoints check the signature either way, so no cookie is needed.
    /// On failure the previous keys, if any, stay in use. Returns whether the
    /// keys were updated.
    async fn refresh_wbi_keys(&self) -> bool {
        let client = self.downloader.client();
        let resp = match client
            .get("https://api.bilibili.com/x/web-interface/nav")
            .headers(self.get_headers().await)
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                tracing::warn!(error = %e, "WBI nav request failed; keeping current keys");
                return false;
            }
        };

        let json = match resp.json::<serde_json::Value>().await {
            Ok(json) => json,
            Err(e) => {
                tracing::warn!(error = %e, "WBI nav response decode failed; keeping current keys");
                return false;
            }
        };

        let img_url = json
            .pointer("/data/wbi_img/img_url")
            .and_then(|v| v.as_str());
        let sub_url = json
            .pointer("/data/wbi_img/sub_url")
            .and_then(|v| v.as_str());

        if let (Some(img), Some(sub)) = (img_url, sub_url) {
            self.wbi_signer.update_keys(img, sub);
            tracing::debug!("WBI signing keys updated");
            true
        } else {
            tracing::warn!(
                code = json.get("code").and_then(|v| v.as_i64()),
                "WBI nav response lacks wbi_img keys; keeping current keys"
            );
            false
        }
    }

    /// Sign query parameters with WBI if available, otherwise return original params.
    fn wbi_sign<'a>(&self, params: &[(&'a str, &'a str)]) -> Vec<(String, String)> {
        self.wbi_signer.sign_params(params).unwrap_or_else(|| {
            params
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        })
    }

    /// GET a WBI-signed API endpoint.
    ///
    /// Keys can rotate between scheduled refreshes, so when Bilibili rejects
    /// the signature the keys are fetched again and the request is retried
    /// once.
    async fn get_wbi_signed<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        params: &[(&str, &str)],
    ) -> Result<ApiResponse<T>> {
        let mut response = self.send_wbi_signed(url, params).await?;
        if wbi::is_signature_rejection(&response) && self.refresh_wbi_keys().await {
            tracing::debug!(url, "WBI signature rejected; retrying with fresh keys");
            response = self.send_wbi_signed(url, params).await?;
        }
        Ok(serde_json::from_value(response)?)
    }

    async fn send_wbi_signed(
        &self,
        url: &str,
        params: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        Ok(self
            .downloader
            .client()
            .get(url)
            .headers(self.get_headers().await)
            .query(&self.wbi_sign(params))
            .send()
            .await?
            .json()
            .await?)
    }

    /// Parse short URL (b23.tv or bili2233.cn)
    async fn parse_short_url(&self, match_result: MatchResult<'_>) -> Result<ParseResult> {
        let short_code = match_result.group(1).unwrap_or("");
        let host = if match_result.full_match.contains("bili2233") {
            "bili2233.cn"
        } else {
            "b23.tv"
        };
        let short_url = format!("https://{}/{}", host, short_code);

        // Follow redirect to get the actual URL
        let client = self.downloader.client();
        let final_url = http::resolve_redirect(client, &short_url, self.get_headers().await)
            .await?
            .final_url;

        // Try to match the final URL with other patterns (excluding short URL patterns to avoid recursion)
        for pattern in &self.patterns {
            if pattern.name == "b23" || pattern.name == "bili2233" {
                continue; // Skip short URL patterns to avoid recursion
            }

            if let Some(caps) = pattern.regex.captures(&final_url) {
                let m = MatchResult::from_captures(pattern.name, &caps);
                // Extract page number from URL if present
                let page_num = if final_url.contains("?p=") || final_url.contains("&p=") {
                    PAGE_NUM_RE
                        .captures(&final_url)
                        .and_then(|c| c.get(1))
                        .and_then(|p| p.as_str().parse().ok())
                        .unwrap_or(1)
                } else {
                    1
                };

                return match pattern.name {
                    "bv" | "bv_page" | "bv_space" => {
                        let bvid = m.group(1).unwrap_or("");
                        self.fetch_video_info(bvid, None, page_num).await
                    }
                    "av" | "av_page" | "av_space" => {
                        let aid = m.group(1).unwrap_or("");
                        self.fetch_video_info("", Some(aid), page_num).await
                    }
                    "dynamic" | "dynamic_page" | "space_dynamic" => {
                        let dynamic_id = m.group(1).unwrap_or("");
                        self.fetch_dynamic(dynamic_id).await
                    }
                    "opus" => {
                        let opus_id = m.group(1).unwrap_or("");
                        self.fetch_opus(opus_id).await
                    }
                    "live" => {
                        let room_id = m.group(1).unwrap_or("");
                        self.fetch_live(room_id).await
                    }
                    "article" => {
                        let cv_id = m.group(1).unwrap_or("");
                        self.fetch_article(cv_id).await
                    }
                    "favorites" => {
                        let fav_id = m.group(1).unwrap_or("");
                        self.fetch_favorites(fav_id).await
                    }
                    "bangumi" => {
                        let ep_id = m.group(1).unwrap_or("");
                        self.fetch_bangumi(ep_id).await
                    }
                    _ => Err(ParserError::PatternNotMatched(format!(
                        "Unknown pattern: {}",
                        pattern.name
                    ))),
                };
            }
        }

        Err(ParserError::PatternNotMatched(format!(
            "Could not parse redirected URL: {}",
            final_url
        )))
    }

    /// Dispatch to the appropriate handler based on pattern name
    async fn dispatch(&self, match_result: MatchResult<'_>) -> Result<ParseResult> {
        self.ensure_wbi().await;

        match match_result.pattern {
            "b23" | "bili2233" => self.parse_short_url(match_result).await,
            "bv" => {
                let bvid = match_result.group(1).unwrap_or("");
                self.fetch_video_info(bvid, None, 1).await
            }
            "bv_page" | "bv_space" => {
                let bvid = match_result.group(1).unwrap_or("");
                let page_num = match_result
                    .group(2)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1);
                self.fetch_video_info(bvid, None, page_num).await
            }
            "av" => {
                let aid = match_result.group(1).unwrap_or("");
                self.fetch_video_info("", Some(aid), 1).await
            }
            "av_page" | "av_space" => {
                let aid = match_result.group(1).unwrap_or("");
                let page_num = match_result
                    .group(2)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1);
                self.fetch_video_info("", Some(aid), page_num).await
            }
            "dynamic" | "dynamic_page" | "space_dynamic" => {
                let dynamic_id = match_result.group(1).unwrap_or("");
                self.fetch_dynamic(dynamic_id).await
            }
            "opus" => {
                let opus_id = match_result.group(1).unwrap_or("");
                self.fetch_opus(opus_id).await
            }
            "live" => {
                let room_id = match_result.group(1).unwrap_or("");
                self.fetch_live(room_id).await
            }
            "article" => {
                let cv_id = match_result.group(1).unwrap_or("");
                self.fetch_article(cv_id).await
            }
            "favorites" => {
                let fav_id = match_result.group(1).unwrap_or("");
                self.fetch_favorites(fav_id).await
            }
            "bangumi" => {
                let ep_id = match_result.group(1).unwrap_or("");
                self.fetch_bangumi(ep_id).await
            }
            "bm" => {
                let bvid = match_result.group(1).unwrap_or("");
                let page_num = match_result
                    .group(2)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1);
                self.parse_audio(bvid, page_num).await
            }
            _ => Err(ParserError::PatternNotMatched(format!(
                "Unknown pattern: {}",
                match_result.pattern
            ))),
        }
    }
}

#[async_trait]
impl Parser for BilibiliParser {
    fn platform(&self) -> Platform {
        Platform::Bilibili
    }

    fn patterns(&self) -> &[RegisteredPattern] {
        &self.patterns
    }

    async fn parse(&self, match_result: MatchResult<'_>) -> Result<ParseResult> {
        tracing::debug!(
            pattern = match_result.pattern,
            matched = match_result.full_match,
            "Bilibili: parsing"
        );
        self.dispatch(match_result).await
    }
}
