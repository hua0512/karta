use async_trait::async_trait;

use wreq::header::HeaderMap;

use karta_core::Platform;
use karta_core::constants::DEFAULT_USER_AGENT;
use karta_core::data::ParseResult;
use karta_core::download::SharedDownloader;
use karta_core::error::Result;
use karta_core::http;
use karta_core::parser::{MatchResult, Parser, RegisteredPattern};

mod api;
mod patterns;
mod result;

/// AcFun parser
pub struct AcfunParser {
    downloader: SharedDownloader,
    patterns: Vec<RegisteredPattern>,
}

impl AcfunParser {
    pub fn new(downloader: SharedDownloader) -> Self {
        Self {
            downloader,
            patterns: patterns::registered_patterns(),
        }
    }

    fn get_headers(&self) -> HeaderMap {
        http::headers_with_referer(DEFAULT_USER_AGENT, "https://www.acfun.cn/")
    }

    async fn parse_video(&self, acid: &str) -> Result<ParseResult> {
        let url = format!(
            "https://www.acfun.cn/v/ac{}?quickViewId=videoInfo_new&ajaxpipe=1",
            acid
        );
        let client = self.downloader.client();

        let response = client.get(&url).headers(self.get_headers()).send().await?;

        let body = response.text().await?;

        let video_info = api::extract_video_info(&body)?;

        Ok(result::build_video_result(acid, &video_info))
    }
}

#[async_trait]
impl Parser for AcfunParser {
    fn platform(&self) -> Platform {
        Platform::AcFun
    }

    fn patterns(&self) -> &[RegisteredPattern] {
        &self.patterns
    }

    async fn parse(&self, match_result: MatchResult<'_>) -> Result<ParseResult> {
        tracing::debug!(
            pattern = match_result.pattern,
            matched = match_result.full_match,
            "AcFun: parsing"
        );
        let acid = match_result.group(1).unwrap_or("");
        self.parse_video(acid).await
    }
}
