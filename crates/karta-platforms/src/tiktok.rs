use async_trait::async_trait;
use karta_core::Platform;
use karta_core::constants::MOBILE_USER_AGENT;
use karta_core::data::ParseResult;
use karta_core::download::SharedDownloader;
use karta_core::error::{ParserError, Result};
use karta_core::extract;
use karta_core::http;
use karta_core::parser::{MatchResult, Parser, RegisteredPattern};
use wreq::header::HeaderMap;

#[cfg(feature = "ytdlp")]
use karta_core::data::{Author, Content, ImageContent, VideoContent};
#[cfg(feature = "ytdlp")]
use karta_core::download::YtdlpDownloader;

mod patterns;
mod result;

use patterns::VIDEO_ID_RE;
use result::{build_from_sigi_state, build_from_universal_data};

/// Markers anchoring the embedded-state `<script>` tags; the balanced-brace
/// scanner picks up the JSON object that follows the opening tag.
const SIGI_STATE_MARKER: &str = r#"<script id="SIGI_STATE""#;
const UNIVERSAL_DATA_MARKER: &str = r#"<script id="__UNIVERSAL_DATA_FOR_REHYDRATION__""#;

/// TikTok parser
pub struct TikTokParser {
    downloader: SharedDownloader,
    patterns: Vec<RegisteredPattern>,
    #[allow(dead_code)]
    download_media: bool,
    #[cfg(feature = "ytdlp")]
    ytdlp: Option<YtdlpDownloader>,
}

impl TikTokParser {
    pub fn new(downloader: SharedDownloader, download_media: bool) -> Self {
        Self {
            downloader,
            patterns: patterns::registered_patterns(),
            download_media,
            #[cfg(feature = "ytdlp")]
            ytdlp: None,
        }
    }

    /// Configure yt-dlp for TikTok downloads.
    #[cfg(feature = "ytdlp")]
    pub fn with_ytdlp(mut self, ytdlp: YtdlpDownloader) -> Self {
        self.ytdlp = Some(ytdlp);
        self
    }

    fn get_headers(&self) -> HeaderMap {
        http::headers_with_referer(MOBILE_USER_AGENT, "https://www.tiktok.com/")
    }

    async fn parse_short_url(&self, full_match: &str) -> Result<ParseResult> {
        let url = format!("https://{}", full_match);
        let client = self.downloader.client();

        let final_url = http::resolve_redirect(client, &url, self.get_headers())
            .await?
            .final_url;

        // Try to extract video ID from final URL
        if let Some(caps) = VIDEO_ID_RE.captures(&final_url) {
            let video_id = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            return self.parse_video(video_id, &final_url).await;
        }

        Err(ParserError::PatternNotMatched(format!(
            "Could not extract video ID from: {}",
            final_url
        )))
    }

    async fn parse_video(&self, video_id: &str, url: &str) -> Result<ParseResult> {
        // Prefer yt-dlp when available.
        #[cfg(feature = "ytdlp")]
        if let Some(ytdlp) = &self.ytdlp
            && ytdlp.is_available().await
        {
            match self.parse_with_ytdlp(ytdlp, url).await {
                Ok(result) => return Ok(result),
                Err(e) => tracing::warn!(
                    url,
                    error = %e,
                    "TikTok yt-dlp extraction failed; falling back to HTML scraping"
                ),
            }
        }

        let client = self.downloader.client();

        // Fetch the page to extract SIGI_STATE
        let response = client.get(url).headers(self.get_headers()).send().await?;

        let html = response.text().await?;

        // Extract SIGI_STATE from HTML
        if let Some(json_str) = extract::extract_json_after(&html, SIGI_STATE_MARKER) {
            let data: serde_json::Value = serde_json::from_str(json_str)
                .map_err(|e| ParserError::Parse(format!("Failed to parse TikTok data: {}", e)))?;

            return build_from_sigi_state(&data, video_id, url);
        }

        // Fallback: try __UNIVERSAL_DATA_FOR_REHYDRATION__ (newer TikTok pages)
        if let Some(data) = extract::extract_json_value(&html, UNIVERSAL_DATA_MARKER)
            && let Ok(result) = build_from_universal_data(&data, url)
        {
            return Ok(result);
        }

        // Both embedded-state formats are absent: surface the failure instead
        // of masking it behind an empty placeholder result.
        Err(ParserError::ContentNotFound(format!(
            "Could not extract TikTok video data from page: {}",
            url
        )))
    }

    #[cfg(feature = "ytdlp")]
    async fn parse_with_ytdlp(&self, ytdlp: &YtdlpDownloader, url: &str) -> Result<ParseResult> {
        let info = ytdlp
            .extract_info(url)
            .await
            .map_err(|e| ParserError::Parse(format!("yt-dlp failed: {}", e)))?;

        let mut author = Author::new(info.author_name());
        if let Some(thumb) = &info.thumbnail {
            // Some renderers use author.avatar; keep it optional.
            author = author.with_avatar(thumb);
        }

        let mut result = ParseResult::new(Platform::TikTok)
            .with_title(&info.title)
            .with_author(author)
            .with_url(url);

        if let Some(ts) = info.timestamp {
            result = result.with_timestamp(ts);
        }

        // If we're not downloading, at least include the thumbnail for a usable output.
        if !self.download_media {
            if let Some(thumb) = &info.thumbnail {
                result.add_content(Content::Image(ImageContent::new().with_url(thumb)));
            }
            return Ok(result);
        }

        // Download video to a local file.
        match ytdlp.download_video(url).await {
            Ok(path) => {
                let mut video = VideoContent::new().with_path(path);
                if let Some(thumb) = &info.thumbnail {
                    video = video.with_cover(thumb);
                }
                if let Some(dur) = info.duration_secs() {
                    video = video.with_duration(dur);
                }
                result.add_content(Content::Video(video));
            }
            Err(e) => {
                tracing::warn!("Failed to download TikTok video via yt-dlp: {}", e);
                if let Some(thumb) = &info.thumbnail {
                    result.add_content(Content::Image(ImageContent::new().with_url(thumb)));
                }
            }
        }

        Ok(result)
    }

    async fn dispatch(&self, match_result: MatchResult<'_>) -> Result<ParseResult> {
        match match_result.pattern {
            "tiktok_short" => self.parse_short_url(match_result.full_match).await,
            "tiktok" => {
                let video_id = match_result.group(1).unwrap_or("");
                let url = format!("https://{}", match_result.full_match);
                self.parse_video(video_id, &url).await
            }
            _ => Err(ParserError::PatternNotMatched(format!(
                "Unknown pattern: {}",
                match_result.pattern
            ))),
        }
    }
}

#[async_trait]
impl Parser for TikTokParser {
    fn platform(&self) -> Platform {
        Platform::TikTok
    }

    fn patterns(&self) -> &[RegisteredPattern] {
        &self.patterns
    }

    async fn parse(&self, match_result: MatchResult<'_>) -> Result<ParseResult> {
        tracing::debug!(
            pattern = match_result.pattern,
            matched = match_result.full_match,
            "TikTok: parsing"
        );
        self.dispatch(match_result).await
    }
}
