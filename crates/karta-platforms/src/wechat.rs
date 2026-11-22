use async_trait::async_trait;
use wreq::header::HeaderMap;

use karta_core::Platform;
use karta_core::constants::DEFAULT_USER_AGENT;
use karta_core::data::ParseResult;
use karta_core::download::SharedDownloader;
use karta_core::error::Result;
use karta_core::http;
use karta_core::parser::{MatchResult, Parser, RegisteredPattern};

mod patterns;
mod result;

/// WeChat official-account article (mp.weixin.qq.com) parser.
///
/// Fetches the article page anonymously (no cookie) and extracts metadata
/// plus body text and images into an article card. Deleted articles and
/// anti-crawler verification interstitials surface clear errors.
pub struct WechatParser {
    downloader: SharedDownloader,
    patterns: Vec<RegisteredPattern>,
}

impl WechatParser {
    pub fn new(downloader: SharedDownloader) -> Self {
        Self {
            downloader,
            patterns: patterns::registered_patterns(),
        }
    }

    fn get_headers(&self) -> HeaderMap {
        http::headers_with_referer(DEFAULT_USER_AGENT, "https://mp.weixin.qq.com/")
    }

    async fn parse_article(&self, full_match: &str) -> Result<ParseResult> {
        let url = format!("https://{full_match}");
        let client = self.downloader.client();

        // Short links may 302 to the canonical article URL; wreq clients do
        // not follow redirects by default, so pin a per-request policy.
        let response = http::get_with_redirect(client, &url, self.get_headers()).await?;
        let page = http::checked_text(response).await?;

        result::build_article_result(&page, url)
    }
}

#[async_trait]
impl Parser for WechatParser {
    fn platform(&self) -> Platform {
        Platform::Wechat
    }

    fn patterns(&self) -> &[RegisteredPattern] {
        &self.patterns
    }

    async fn parse(&self, match_result: MatchResult<'_>) -> Result<ParseResult> {
        tracing::debug!(
            pattern = match_result.pattern,
            matched = match_result.full_match,
            "Wechat: parsing"
        );
        self.parse_article(match_result.full_match).await
    }
}
