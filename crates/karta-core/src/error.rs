use thiserror::Error;

use crate::constants::Platform;

#[derive(Error, Debug)]
pub enum ParserError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] wreq::Error),

    #[error("JSON parsing failed: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Pattern not matched: {0}")]
    PatternNotMatched(String),

    #[error("Content not found: {0}")]
    ContentNotFound(String),

    #[error("API error: {code} - {message}")]
    Api { code: i64, message: String },

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Parse timed out after {seconds}s")]
    Timeout { seconds: u64 },

    #[error("Download error: {0}")]
    Download(#[from] DownloadError),

    #[error("Render error: {0}")]
    Render(#[from] RenderError),

    #[error("Platform disabled: {0}")]
    PlatformDisabled(String),

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("[{platform}] {source} (url: {url})")]
    Context {
        platform: Platform,
        url: String,
        #[source]
        source: Box<ParserError>,
    },
}

impl ParserError {
    /// A user-facing explanation when a configured media limit rejects a request.
    pub fn limit_notice(&self) -> Option<String> {
        match self {
            Self::Download(error) => error.limit_notice(),
            Self::Context { source, .. } => source.limit_notice(),
            _ => None,
        }
    }

    /// Wrap this error with platform and URL context.
    pub fn with_context(self, platform: Platform, url: impl Into<String>) -> Self {
        ParserError::Context {
            platform,
            url: url.into(),
            source: Box::new(self),
        }
    }
}

/// Extension trait for adding platform/URL context to `Result<T, ParserError>`.
pub trait ResultExt<T> {
    /// Wrap the error with platform and URL context.
    fn with_context(self, platform: Platform, url: impl Into<String>) -> Result<T>;
}

impl<T> ResultExt<T> for Result<T> {
    fn with_context(self, platform: Platform, url: impl Into<String>) -> Result<T> {
        self.map_err(|e| e.with_context(platform, url))
    }
}

#[derive(Error, Debug)]
pub enum DownloadError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] wreq::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("File too large: {size} bytes exceeds limit of {limit} bytes")]
    FileTooLarge { size: u64, limit: u64 },

    #[error("Duration too long: {duration}s exceeds limit of {limit}s")]
    DurationTooLong { duration: u64, limit: u64 },

    #[error("FFmpeg error: {0}")]
    FFmpeg(String),

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("Download timeout")]
    Timeout,

    #[error("File size is 0")]
    ZeroSize,

    #[error("Incomplete download: {0}")]
    Incomplete(String),

    #[error("yt-dlp error: {0}")]
    Ytdlp(String),

    #[error("Parse error: {0}")]
    Parse(String),
}

impl DownloadError {
    /// A user-facing explanation that does not expose upstream URLs or errors.
    pub fn limit_notice(&self) -> Option<String> {
        match self {
            Self::FileTooLarge { limit, .. } => Some(format!(
                "媒体文件超过大小限制（上限 {:.0} MiB），已跳过发送。",
                *limit as f64 / (1024.0 * 1024.0)
            )),
            Self::DurationTooLong { limit, .. } => {
                Some(format!("视频超过时长限制（上限 {limit} 秒），已跳过发送。"))
            }
            _ => None,
        }
    }

    /// Whether retrying against a different URL for the same media could help.
    ///
    /// Transport failures are worth retrying on a backup CDN. Content-level
    /// rejections (size/duration caps, ffmpeg failures) describe the media
    /// itself and would recur identically on every mirror, so retrying them
    /// only multiplies the wasted transfer.
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            Self::Http(_) | Self::Io(_) | Self::Timeout | Self::ZeroSize | Self::Incomplete(_)
        )
    }
}

#[derive(Error, Debug)]
pub enum RenderError {
    #[error("Image processing error: {0}")]
    Image(#[from] image::ImageError),

    #[error("Font loading error: {0}")]
    Font(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Layout error: {0}")]
    Layout(String),

    #[error("Other error: {0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, ParserError>;
pub type DownloadResult<T> = std::result::Result<T, DownloadError>;
pub type RenderResult<T> = std::result::Result<T, RenderError>;
