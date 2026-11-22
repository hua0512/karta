//! yt-dlp integration for video/audio downloads
//!
//! This module provides a wrapper around the yt-dlp CLI tool for downloading
//! videos and extracting audio from various platforms.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use tokio::process::Command;

use crate::cache::Cache;
use crate::constants::{DEFAULT_CACHE_SIZE, DEFAULT_MAX_SIZE_MB};
use crate::error::{DownloadError, DownloadResult};

mod command;
use command::DownloadKind;

/// Video information extracted by yt-dlp
#[derive(Debug, Clone, Deserialize)]
pub struct VideoInfo {
    /// Video title
    pub title: String,
    /// Channel/creator name
    #[serde(default)]
    pub channel: Option<String>,
    /// Uploader name/ID
    #[serde(default)]
    pub uploader: Option<String>,
    /// Duration in seconds
    #[serde(default)]
    pub duration: Option<f64>,
    /// Publication timestamp
    #[serde(default)]
    pub timestamp: Option<i64>,
    /// Thumbnail URL
    #[serde(default)]
    pub thumbnail: Option<String>,
    /// Video description
    #[serde(default)]
    pub description: Option<String>,
    /// Video ID
    #[serde(default)]
    pub id: Option<String>,
    /// Extractor name (youtube, tiktok, etc.)
    #[serde(default)]
    pub extractor: Option<String>,
}

impl VideoInfo {
    /// Get the author name (channel or uploader)
    pub fn author_name(&self) -> String {
        match (&self.channel, &self.uploader) {
            (Some(channel), Some(uploader)) if channel != uploader => {
                format!("{}@{}", channel, uploader)
            }
            (Some(channel), _) => channel.clone(),
            (_, Some(uploader)) => uploader.clone(),
            _ => "Unknown".to_string(),
        }
    }

    /// Get duration as u64 seconds
    pub fn duration_secs(&self) -> Option<u64> {
        self.duration
            .filter(|duration| duration.is_finite() && *duration >= 0.0)
            .map(|duration| duration.ceil() as u64)
    }
}

/// yt-dlp downloader configuration
#[derive(Debug, Clone)]
pub struct YtdlpConfig {
    /// Path to yt-dlp executable (default: "yt-dlp")
    pub ytdlp_path: String,
    /// Path to cookies file (Netscape format)
    pub cookies_file: Option<PathBuf>,
    /// Proxy URL
    pub proxy: Option<String>,
    /// Maximum video duration in seconds (0 = unlimited)
    pub max_duration: u64,
    /// Maximum file size in bytes, passed to yt-dlp as `--max-filesize`
    /// (0 = unlimited)
    pub max_size: u64,
    /// Output directory for downloads
    pub output_dir: PathBuf,
}

impl Default for YtdlpConfig {
    fn default() -> Self {
        Self {
            ytdlp_path: "yt-dlp".to_string(),
            cookies_file: None,
            proxy: None,
            max_duration: 0,
            max_size: DEFAULT_MAX_SIZE_MB * 1024 * 1024,
            output_dir: std::env::temp_dir().join("ytdlp_cache"),
        }
    }
}

/// yt-dlp downloader
#[derive(Clone)]
pub struct YtdlpDownloader {
    config: YtdlpConfig,
    /// LRU cache for video info, bounded so long-running processes cannot
    /// accumulate one entry per URL ever seen
    info_cache: Arc<Cache<String, VideoInfo>>,
}

impl YtdlpDownloader {
    /// Create a new yt-dlp downloader with default config
    pub fn new() -> Self {
        Self::with_config(YtdlpConfig::default())
    }

    /// Create a new yt-dlp downloader with custom config
    pub fn with_config(config: YtdlpConfig) -> Self {
        // Ensure output directory exists
        if let Err(e) = std::fs::create_dir_all(&config.output_dir) {
            tracing::warn!("Failed to create yt-dlp output directory: {}", e);
        }

        Self {
            config,
            info_cache: Arc::new(Cache::new(DEFAULT_CACHE_SIZE)),
        }
    }

    /// Set the cookies file path
    pub fn with_cookies(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.cookies_file = Some(path.into());
        self
    }

    /// Set the proxy URL
    pub fn with_proxy(mut self, proxy: impl Into<String>) -> Self {
        self.config.proxy = Some(proxy.into());
        self
    }

    /// Set the output directory
    pub fn with_output_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.config.output_dir = dir.into();
        if let Err(e) = std::fs::create_dir_all(&self.config.output_dir) {
            tracing::warn!("Failed to create yt-dlp output directory: {}", e);
        }
        self
    }

    /// Check if yt-dlp is available
    pub async fn is_available(&self) -> bool {
        // `--version` should return instantly; a hung binary reads as
        // unavailable and is reaped via kill_on_drop.
        let status = tokio::time::timeout(
            Duration::from_secs(10),
            Command::new(&self.config.ytdlp_path)
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .kill_on_drop(true)
                .status(),
        )
        .await;
        matches!(status, Ok(Ok(s)) if s.success())
    }

    /// Directory where yt-dlp writes downloaded media (for external cleanup)
    pub fn output_dir(&self) -> &Path {
        &self.config.output_dir
    }

    /// Extract video information without downloading
    pub async fn extract_info(&self, url: &str) -> DownloadResult<VideoInfo> {
        // Check cache first
        if let Some(info) = self.info_cache.get_cloned(&url.to_string()) {
            return Ok(info);
        }

        let cmd = self.build_info_command(url);
        let output = Self::run_command(cmd, "yt-dlp failed").await?;

        let info: VideoInfo = serde_json::from_slice(&output.stdout)
            .map_err(|e| DownloadError::Parse(format!("Failed to parse yt-dlp output: {}", e)))?;

        // Cache the result
        self.info_cache.insert(url.to_string(), info.clone());

        Ok(info)
    }

    /// Download video to a file
    ///
    /// Returns the path to the downloaded file
    pub async fn download_video(&self, url: &str) -> DownloadResult<PathBuf> {
        self.download_media(url, DownloadKind::Video).await
    }

    /// Download audio only (extracted to FLAC format)
    ///
    /// Returns the path to the downloaded audio file
    pub async fn download_audio(&self, url: &str) -> DownloadResult<PathBuf> {
        self.download_media(url, DownloadKind::Audio).await
    }

    /// Clear the info cache
    pub fn clear_cache(&self) {
        self.info_cache.clear();
    }
}

impl Default for YtdlpDownloader {
    fn default() -> Self {
        Self::new()
    }
}
