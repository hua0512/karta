use async_trait::async_trait;
use wreq::header::HeaderMap;

use karta_core::Platform;
use karta_core::constants::MOBILE_USER_AGENT;
use karta_core::data::ParseResult;
use karta_core::download::SharedDownloader;
use karta_core::error::Result;
use karta_core::http;
use karta_core::parser::{MatchResult, Parser, RegisteredPattern};

mod api;
mod patterns;
mod result;

use result::{build_state_result, extract_init_state};

/// Kuaishou parser
pub struct KuaishouParser {
    downloader: SharedDownloader,
    patterns: Vec<RegisteredPattern>,
}

impl KuaishouParser {
    pub fn new(downloader: SharedDownloader) -> Self {
        Self {
            downloader,
            patterns: patterns::registered_patterns(),
        }
    }

    fn get_headers(&self) -> HeaderMap {
        http::headers_with_referer(MOBILE_USER_AGENT, "https://v.kuaishou.com/")
    }

    async fn parse_url(&self, full_match: &str) -> Result<ParseResult> {
        let url = format!("https://{}", full_match);
        let client = self.downloader.client();

        // Follow redirect to get real URL
        let resolved = http::resolve_redirect(client, &url, self.get_headers()).await?;

        // Replace long-video with photo for consistent API
        let real_url = resolved.final_url.replace("/fw/long-video/", "/fw/photo/");

        // Fetch the page
        let response = client
            .get(&real_url)
            .headers(self.get_headers())
            .send()
            .await?;

        let html = response.text().await?;

        let state = extract_init_state(&html)?;
        build_state_result(&state)
    }
}

#[async_trait]
impl Parser for KuaishouParser {
    fn platform(&self) -> Platform {
        Platform::Kuaishou
    }

    fn patterns(&self) -> &[RegisteredPattern] {
        &self.patterns
    }

    async fn parse(&self, match_result: MatchResult<'_>) -> Result<ParseResult> {
        tracing::debug!(
            pattern = match_result.pattern,
            matched = match_result.full_match,
            "Kuaishou: parsing"
        );
        self.parse_url(match_result.full_match).await
    }
}
