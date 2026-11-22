//! Parser for WeChat video-channel (微信视频号) share links.
//!
//! Share links look like `https://weixin.qq.com/sph/AbCd123` and are resolved
//! in two steps (see `api`): a Tencent Yuanbao API call yields one-shot
//! feed-preview credentials, which the channels feed API exchanges for the
//! video URL, author, and interaction stats.
//!
//! Auth is a Tencent Yuanbao web cookie only (`wechat_channel_cookie` in
//! config.json); no WeChat login is involved. Step 2 needs no cookie, and the
//! returned CDN video URL carries its own token, so downloads need no special
//! headers either.

use async_trait::async_trait;

use karta_core::Platform;
use karta_core::data::ParseResult;
use karta_core::download::SharedDownloader;
use karta_core::error::{ParserError, Result};
use karta_core::parser::{MatchResult, Parser, RegisteredPattern};

mod api;
mod patterns;
mod result;

/// Actionable hint shown when no Yuanbao cookie is configured.
const MISSING_COOKIE_HINT: &str = "未配置腾讯元宝 Cookie，无法解析视频号：请在 config.json 中设置 \
    wechat_channel_cookie（浏览器登录 https://yuanbao.tencent.com 后，从任意请求的 Request \
    Headers 复制 Cookie，无需微信登录）";

/// WeChat video-channel (视频号) share-link parser.
///
/// Requires a Tencent Yuanbao web cookie (`wechat_channel_cookie`).
pub struct WechatChannelParser {
    downloader: SharedDownloader,
    cookie: Option<String>,
    patterns: Vec<RegisteredPattern>,
}

impl WechatChannelParser {
    pub fn new(downloader: SharedDownloader, cookie: Option<String>) -> Self {
        Self {
            downloader,
            cookie,
            patterns: patterns::registered_patterns(),
        }
    }

    /// The configured Yuanbao cookie, or an actionable error for the operator.
    fn require_cookie(&self) -> Result<&str> {
        self.cookie
            .as_deref()
            .map(str::trim)
            .filter(|cookie| !cookie.is_empty())
            .ok_or_else(|| ParserError::Parse(MISSING_COOKIE_HINT.to_string()))
    }

    async fn parse_share(&self, full_match: &str) -> Result<ParseResult> {
        let cookie = self.require_cookie()?;
        // The registered pattern is scheme-less; the Yuanbao API wants a full URL.
        let share_url = format!("https://{full_match}");
        let client = self.downloader.client();

        // Step 1: share link -> playable_url carrying the preview credentials.
        let parse_data = api::parse_share_url(client, &share_url, cookie).await?;
        tracing::debug!(
            export_id = parse_data.wx_export_id.as_deref().unwrap_or_default(),
            "WechatChannel: step 1/2 done"
        );

        let (general_token, export_id) =
            api::extract_playable_params(parse_data.playable_url.as_deref().unwrap_or_default())?;

        // Step 2: credentials -> feed detail (video URL, author, stats).
        let feed = api::get_feed_info(client, &export_id, &general_token).await?;
        tracing::debug!("WechatChannel: step 2/2 done");

        result::build_result(&parse_data, &feed, &share_url)
    }
}

#[async_trait]
impl Parser for WechatChannelParser {
    fn platform(&self) -> Platform {
        Platform::WechatChannel
    }

    fn patterns(&self) -> &[RegisteredPattern] {
        &self.patterns
    }

    async fn parse(&self, match_result: MatchResult<'_>) -> Result<ParseResult> {
        tracing::debug!(
            pattern = match_result.pattern,
            matched = match_result.full_match,
            "WechatChannel: parsing"
        );
        self.parse_share(match_result.full_match).await
    }
}
