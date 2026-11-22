use async_trait::async_trait;
use serde::Deserialize;
use wreq::header::HeaderMap;

use karta_core::Platform;
use karta_core::constants::DEFAULT_USER_AGENT;
use karta_core::data::{Author, Content, ImageContent, ParseResult};

#[cfg(feature = "ytdlp")]
use karta_core::data::VideoContent;
use karta_core::download::SharedDownloader;
use karta_core::error::{ParserError, Result};
use karta_core::http;
use karta_core::parser::{MatchResult, Parser, RegisteredPattern};
use karta_core::patterns;

#[cfg(feature = "ytdlp")]
use karta_core::download::YtdlpDownloader;

/// YouTube oEmbed response
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OEmbedResponse {
    title: Option<String>,
    author_name: Option<String>,
    author_url: Option<String>,
    thumbnail_url: Option<String>,
}

/// YouTube parser
pub struct YouTubeParser {
    downloader: SharedDownloader,
    cookie: Option<String>,
    patterns: Vec<RegisteredPattern>,
    #[allow(dead_code)]
    download_media: bool,
    #[allow(dead_code)]
    duration_maximum: u64,
    #[cfg(feature = "ytdlp")]
    ytdlp: Option<YtdlpDownloader>,
}

impl YouTubeParser {
    pub fn new(
        downloader: SharedDownloader,
        cookie: Option<String>,
        download_media: bool,
        duration_maximum: u64,
    ) -> Self {
        // The registry picks the first matching pattern in list order. The
        // video patterns also match the URL inside "ym <url>", so the audio
        // pattern must come first.
        let patterns = patterns![
            "youtube_audio" => r"\bym\s+(?:https?://)?(?:www\.)?(?:youtube\.com/(?:watch\?v=|shorts/)|youtu\.be/)([a-zA-Z0-9_-]{11})",
            "youtu_be"      => r"youtu\.be/([a-zA-Z0-9_-]{11})",
            "youtube_watch" => r"youtube\.com/watch\?v=([a-zA-Z0-9_-]{11})",
            "youtube_shorts" => r"youtube\.com/shorts/([a-zA-Z0-9_-]{11})",
        ];

        Self {
            downloader,
            cookie,
            patterns,
            download_media,
            duration_maximum,
            #[cfg(feature = "ytdlp")]
            ytdlp: None,
        }
    }

    /// Configure yt-dlp for video downloads
    #[cfg(feature = "ytdlp")]
    pub fn with_ytdlp(mut self, ytdlp: YtdlpDownloader) -> Self {
        self.ytdlp = Some(ytdlp);
        self
    }

    fn get_headers(&self) -> HeaderMap {
        http::headers_with_referer_and_cookie(
            DEFAULT_USER_AGENT,
            "https://www.youtube.com/",
            self.cookie.as_deref(),
        )
    }

    async fn parse_video(&self, video_id: &str) -> Result<ParseResult> {
        let _video_url = format!("https://www.youtube.com/watch?v={}", video_id);

        // Try yt-dlp first if available
        #[cfg(feature = "ytdlp")]
        if let Some(ytdlp) = &self.ytdlp
            && ytdlp.is_available().await
        {
            return self.parse_with_ytdlp(ytdlp, &_video_url, video_id).await;
        }

        // Fallback to oEmbed API
        self.parse_with_oembed(video_id).await
    }

    /// Parse and return audio content.
    ///
    /// Delegates to yt-dlp when the feature is enabled; returns an error otherwise.
    async fn parse_audio(&self, video_id: &str) -> Result<ParseResult> {
        let video_url = format!("https://www.youtube.com/watch?v={}", video_id);

        #[cfg(feature = "ytdlp")]
        {
            return self.parse_audio_with_ytdlp(&video_url).await;
        }

        #[cfg(not(feature = "ytdlp"))]
        {
            let _ = video_url;
            Err(ParserError::Parse(
                "audio parsing requires ytdlp feature".to_string(),
            ))
        }
    }

    /// Parse and return audio content for the `ym` audio flow.
    #[cfg(feature = "ytdlp")]
    async fn parse_audio_with_ytdlp(&self, video_url: &str) -> Result<ParseResult> {
        let Some(ytdlp) = &self.ytdlp else {
            return Err(ParserError::Parse("yt-dlp not configured".to_string()));
        };
        if !ytdlp.is_available().await {
            return Err(ParserError::Parse("yt-dlp not available".to_string()));
        }

        let info = ytdlp
            .extract_info(video_url)
            .await
            .map_err(|e| ParserError::Parse(format!("yt-dlp failed: {}", e)))?;

        let mut author = Author::new(info.author_name());
        if let Some(channel) = &info.channel {
            author = author.with_url(format!("https://www.youtube.com/@{}", channel));
        }

        let mut result = ParseResult::new(Platform::YouTube)
            .with_title(&info.title)
            .with_author(author)
            .with_url(video_url);

        if let Some(ts) = info.timestamp {
            result = result.with_timestamp(ts);
        }

        if let Some(thumb) = &info.thumbnail {
            result.add_content(Content::Image(ImageContent::new().with_url(thumb)));
        }

        // Only download audio when allowed and within duration limit.
        if self.download_media {
            if let Some(duration) = info.duration_secs()
                && self.duration_maximum > 0
                && duration > self.duration_maximum
            {
                return Ok(result);
            }

            match ytdlp.download_audio(video_url).await {
                Ok(path) => {
                    let mut audio = karta_core::data::AudioContent::new().with_path(path);
                    if let Some(d) = info.duration_secs() {
                        audio = audio.with_duration(d);
                    }
                    result.add_content(Content::Audio(audio));
                }
                Err(e) => {
                    tracing::warn!("Failed to download YouTube audio: {}", e);
                }
            }
        }

        Ok(result)
    }

    /// Parse using yt-dlp for full video info and download
    #[cfg(feature = "ytdlp")]
    async fn parse_with_ytdlp(
        &self,
        ytdlp: &YtdlpDownloader,
        video_url: &str,
        _video_id: &str,
    ) -> Result<ParseResult> {
        let info = ytdlp
            .extract_info(video_url)
            .await
            .map_err(|e| ParserError::Parse(format!("yt-dlp failed: {}", e)))?;

        let mut author = Author::new(info.author_name());
        if let Some(channel) = &info.channel {
            author = author.with_url(format!("https://www.youtube.com/@{}", channel));
        }

        let mut result = ParseResult::new(Platform::YouTube)
            .with_title(&info.title)
            .with_author(author)
            .with_url(video_url);

        if let Some(desc) = &info.description
            && !desc.is_empty()
            && desc.len() < 500
        {
            result = result.with_text(desc);
        }

        if let Some(ts) = info.timestamp {
            result = result.with_timestamp(ts);
        }

        // Add duration info
        if let Some(duration) = info.duration_secs() {
            let minutes = duration / 60;
            let seconds = duration % 60;
            result.add_extra("时长", format!("{}:{:02}", minutes, seconds));
        }

        // Download video only if allowed and within duration limit.
        if self.download_media {
            if let Some(duration) = info.duration_secs()
                && self.duration_maximum > 0
                && duration > self.duration_maximum
            {
                if let Some(thumb) = &info.thumbnail {
                    result.add_content(Content::Image(ImageContent::new().with_url(thumb)));
                }
                return Ok(result);
            }

            match ytdlp.download_video(video_url).await {
                Ok(path) => {
                    let mut video = VideoContent::new().with_path(path);
                    if let Some(thumb) = &info.thumbnail {
                        video = video.with_cover(thumb);
                    }
                    result.add_content(Content::Video(video));
                }
                Err(e) => {
                    tracing::warn!("Failed to download YouTube video: {}", e);
                    if let Some(thumb) = &info.thumbnail {
                        result.add_content(Content::Image(ImageContent::new().with_url(thumb)));
                    }
                }
            }
        } else if let Some(thumb) = &info.thumbnail {
            result.add_content(Content::Image(ImageContent::new().with_url(thumb)));
        }

        Ok(result)
    }

    /// Parse using oEmbed API (basic info only)
    async fn parse_with_oembed(&self, video_id: &str) -> Result<ParseResult> {
        let client = self.downloader.client();

        // Use oEmbed API to get basic info
        let oembed_url = format!(
            "https://www.youtube.com/oembed?url=https://www.youtube.com/watch?v={}&format=json",
            video_id
        );

        let response = client
            .get(&oembed_url)
            .headers(self.get_headers())
            .send()
            .await?;

        http::check_status(&response)?;

        let oembed: OEmbedResponse = response.json().await?;

        let title = oembed.title.unwrap_or_default();
        let author_name = oembed.author_name.unwrap_or_else(|| "YouTube".to_string());
        let thumbnail = oembed.thumbnail_url;

        let mut author = Author::new(&author_name);
        if let Some(author_url) = oembed.author_url {
            author = author.with_url(author_url);
        }

        let video_url = format!("https://www.youtube.com/watch?v={}", video_id);

        // For YouTube without yt-dlp, we just provide the thumbnail and URL
        let mut contents = Vec::new();

        if let Some(thumb) = thumbnail {
            // Use high-quality thumbnail
            let hq_thumb = thumb.replace("hqdefault", "maxresdefault");
            contents.push(Content::Image(ImageContent::new().with_url(hq_thumb)));
        }

        Ok(ParseResult::new(Platform::YouTube)
            .with_title(title)
            .with_author(author)
            .with_url(video_url)
            .with_contents(contents))
    }
}

#[async_trait]
impl Parser for YouTubeParser {
    fn platform(&self) -> Platform {
        Platform::YouTube
    }

    fn patterns(&self) -> &[RegisteredPattern] {
        &self.patterns
    }

    async fn parse(&self, match_result: MatchResult<'_>) -> Result<ParseResult> {
        tracing::debug!(
            pattern = match_result.pattern,
            matched = match_result.full_match,
            "YouTube: parsing"
        );
        let video_id = match_result.group(1).unwrap_or("");

        match match_result.pattern {
            // "ym <YouTube url>" extracts audio instead of video.
            "youtube_audio" => self.parse_audio(video_id).await,
            _ => self.parse_video(video_id).await,
        }
    }
}
