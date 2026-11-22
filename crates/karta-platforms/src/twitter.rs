use async_trait::async_trait;

use karta_core::Platform;
use karta_core::constants::DEFAULT_USER_AGENT;
use karta_core::data::ParseResult;
use karta_core::download::SharedDownloader;
use karta_core::error::{ParserError, Result};
use karta_core::http;
use karta_core::parser::{MatchResult, Parser, RegisteredPattern};

mod api;
mod patterns;
mod result;
mod syndication;

use api::VxTwitterResponse;
use syndication::syndication_token;

const SYNDICATION_API: &str = "https://cdn.syndication.twimg.com/tweet-result";

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Twitter/X parser using vxtwitter API with syndication API fallback
pub struct TwitterParser {
    downloader: SharedDownloader,
    patterns: Vec<RegisteredPattern>,
}

impl TwitterParser {
    pub fn new(downloader: SharedDownloader) -> Self {
        Self {
            downloader,
            patterns: patterns::registered_patterns(),
        }
    }

    async fn parse_tweet(&self, match_result: MatchResult<'_>) -> Result<ParseResult> {
        let status_id = match_result.group(1).unwrap_or("");
        let url = format!("https://x.com/i/status/{}", status_id);

        // Try vxtwitter first, fall back to syndication API. A tombstone from
        // the syndication API is the definitive answer for a deleted tweet;
        // any other fallback failure reports the vxtwitter error.
        match self.parse_via_vxtwitter(status_id, &url).await {
            Ok(result) => Ok(result),
            Err(vx_err) => {
                tracing::debug!("vxtwitter failed ({}), trying syndication API", vx_err);
                self.parse_via_syndication(status_id, &url)
                    .await
                    .map_err(|synd_err| {
                        tracing::debug!("syndication API also failed: {}", synd_err);
                        match synd_err {
                            ParserError::ContentNotFound(_) => synd_err,
                            _ => vx_err,
                        }
                    })
            }
        }
    }

    // -----------------------------------------------------------------------
    // Primary: vxtwitter API
    // -----------------------------------------------------------------------

    async fn parse_via_vxtwitter(&self, status_id: &str, url: &str) -> Result<ParseResult> {
        let api_url = format!("https://api.vxtwitter.com/i/status/{}", status_id);
        let client = self.downloader.client();

        let headers = http::headers_with_referer(DEFAULT_USER_AGENT, "https://x.com/");

        let response = client.get(&api_url).headers(headers).send().await?;
        http::check_status(&response)?;

        let data: VxTwitterResponse = response.json().await.map_err(|e| {
            ParserError::Parse(format!("Failed to parse vxtwitter response: {}", e))
        })?;

        Ok(result::build_vxtwitter_result(&data, url))
    }

    // -----------------------------------------------------------------------
    // Fallback: syndication API
    // -----------------------------------------------------------------------

    async fn parse_via_syndication(&self, status_id: &str, url: &str) -> Result<ParseResult> {
        let tweet_id: u64 = status_id
            .parse()
            .map_err(|_| ParserError::Parse("Invalid tweet ID".to_string()))?;
        let token = syndication_token(tweet_id);

        let api_url = format!("{}?id={}&token={}&dnt=1", SYNDICATION_API, status_id, token);
        let client = self.downloader.client();

        let headers =
            http::headers_with_referer(DEFAULT_USER_AGENT, "https://platform.twitter.com/");

        let response = client.get(&api_url).headers(headers).send().await?;
        http::check_status(&response)?;

        let data: serde_json::Value = response.json().await.map_err(|e| {
            ParserError::Parse(format!("Failed to parse syndication response: {}", e))
        })?;

        if data.get("__typename").and_then(|v| v.as_str()) == Some("TweetTombstone") {
            return Err(ParserError::ContentNotFound(
                "Tweet was deleted or is unavailable".to_string(),
            ));
        }

        result::build_syndication_result(&data, url)
    }
}

#[async_trait]
impl Parser for TwitterParser {
    fn platform(&self) -> Platform {
        Platform::Twitter
    }

    fn patterns(&self) -> &[RegisteredPattern] {
        &self.patterns
    }

    async fn parse(&self, match_result: MatchResult<'_>) -> Result<ParseResult> {
        tracing::debug!(
            pattern = match_result.pattern,
            matched = match_result.full_match,
            "Twitter: parsing"
        );
        self.parse_tweet(match_result).await
    }
}
