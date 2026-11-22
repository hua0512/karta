use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use wreq::Client;
use wreq::header::HeaderMap;
use wreq::redirect::Policy;
use wreq_util::Emulation;

use crate::cache::FileCache;
use crate::config::Config;
use crate::constants::{DEFAULT_TIMEOUT, DEFAULT_USER_AGENT, DOWNLOAD_TIMEOUT};
use crate::error::DownloadError;
use crate::http;

use super::ffmpeg::FFmpeg;

mod extensions;
mod single_flight;
mod transfer;

use extensions::{audio_extension_from_url, image_extension_from_url, video_extension_from_url};
use single_flight::SingleFlight;
use transfer::{DownloadTransfer, finalize_part, part_path};

/// Progress callback type for download progress reporting
pub type ProgressCallback = Box<dyn Fn(u64, Option<u64>) + Send + Sync>;

/// Default GIF conversion frame rate.
const GIF_DEFAULT_FPS: u32 = 10;
/// Default GIF width in pixels; height is auto-calculated to keep the aspect.
const GIF_DEFAULT_WIDTH: u32 = 320;

/// Stream downloader with progress tracking and caching
pub struct StreamDownloader {
    client: Client,
    file_cache: FileCache,
    ffmpeg: FFmpeg,
    max_size: u64,
    max_duration: u64,
    /// Per-cache-entry locks so concurrent requests for the same media
    /// download it exactly once.
    flights: SingleFlight,
}

impl StreamDownloader {
    /// Create a new StreamDownloader
    pub fn new(config: &Config, data_path: &Path) -> Result<Self, DownloadError> {
        let client = Self::production_client_builder(config)?
            .build()
            .map_err(DownloadError::Http)?;

        let cache_dir = Config::cache_dir(data_path);
        let file_cache = FileCache::new(cache_dir)?;

        let ffmpeg = FFmpeg::new(config.ffmpeg_path.clone());

        Ok(Self {
            client,
            file_cache,
            ffmpeg,
            max_size: config.max_size_bytes(),
            max_duration: config.duration_maximum,
            flights: SingleFlight::default(),
        })
    }

    /// The HTTP client builder `new` uses: browser emulation, redirect and
    /// timeout policy, and the configured proxy (an invalid proxy is an error).
    ///
    /// Exposed so tests can layer interception (DNS, TLS) on top of the
    /// production policy and pass the result to [`Self::with_client`].
    #[doc(hidden)]
    pub fn production_client_builder(
        config: &Config,
    ) -> Result<wreq::ClientBuilder, DownloadError> {
        let mut client_builder = Client::builder()
            .emulation(Emulation::Chrome131)
            .redirect(Policy::limited(20))
            .timeout(Duration::from_secs(DOWNLOAD_TIMEOUT))
            .connect_timeout(Duration::from_secs(DEFAULT_TIMEOUT))
            .gzip(true)
            .brotli(true);

        if let Some(proxy_url) = config.proxy_url() {
            // Refuse to start rather than silently download without the
            // configured proxy.
            let proxy = wreq::Proxy::all(&proxy_url).map_err(|e| invalid_proxy_error(&e))?;
            client_builder = client_builder.proxy(proxy);
        }

        Ok(client_builder)
    }

    /// Create a new StreamDownloader with a custom HTTP client (for testing)
    pub fn with_client(
        config: &Config,
        data_path: &Path,
        client: Client,
    ) -> Result<Self, DownloadError> {
        let cache_dir = Config::cache_dir(data_path);
        let file_cache = FileCache::new(cache_dir)?;
        let ffmpeg = FFmpeg::new(config.ffmpeg_path.clone());

        Ok(Self {
            client,
            file_cache,
            ffmpeg,
            max_size: config.max_size_bytes(),
            max_duration: config.duration_maximum,
            flights: SingleFlight::default(),
        })
    }

    /// Get the HTTP client
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Get the FFmpeg instance
    pub fn ffmpeg(&self) -> &FFmpeg {
        &self.ffmpeg
    }

    /// Get maximum allowed media duration in seconds (0 = unlimited)
    pub fn max_duration(&self) -> u64 {
        self.max_duration
    }

    /// Get the maximum allowed media size in bytes.
    pub fn max_size(&self) -> u64 {
        self.max_size
    }

    /// Get the file cache
    pub fn file_cache(&self) -> &FileCache {
        &self.file_cache
    }

    /// Download a file from URL
    pub async fn download(
        &self,
        url: &str,
        extension: &str,
        headers: Option<HeaderMap>,
    ) -> Result<PathBuf, DownloadError> {
        self.transfer()
            .download_to_cache(url, extension, headers, None)
            .await
    }

    /// Download a video file
    pub async fn download_video(
        &self,
        url: &str,
        headers: Option<HeaderMap>,
    ) -> Result<PathBuf, DownloadError> {
        let extension = video_extension_from_url(url);
        self.download(url, extension, headers).await
    }

    /// Download the first URL in `urls` that succeeds.
    ///
    /// `urls` is a ranked candidate list for the *same* media (a primary CDN
    /// followed by its backups). Only [`DownloadError::is_retriable`] failures
    /// advance to the next candidate; a size or duration rejection aborts
    /// immediately because every mirror serves identical bytes.
    async fn download_first_available(
        &self,
        urls: &[String],
        extension: &str,
        headers: Option<HeaderMap>,
    ) -> Result<PathBuf, DownloadError> {
        let mut last_error: Option<DownloadError> = None;

        for (index, url) in urls.iter().enumerate() {
            match self.download(url, extension, headers.clone()).await {
                Ok(path) => {
                    if index > 0 {
                        tracing::debug!(index, url, "Fell back to a backup download URL");
                    }
                    return Ok(path);
                }
                Err(e) if e.is_retriable() && index + 1 < urls.len() => {
                    tracing::warn!(error = %e, url, "Download URL failed, trying the next candidate");
                    last_error = Some(e);
                }
                Err(e) => return Err(e),
            }
        }

        Err(last_error
            .unwrap_or_else(|| DownloadError::InvalidUrl("no download URL provided".to_string())))
    }

    /// Download a video, falling back through backup URLs.
    pub async fn download_video_candidates(
        &self,
        urls: &[String],
        headers: Option<HeaderMap>,
    ) -> Result<PathBuf, DownloadError> {
        let extension = urls
            .first()
            .map(|url| video_extension_from_url(url))
            .unwrap_or("mp4");
        self.download_first_available(urls, extension, headers)
            .await
    }

    /// Download an encrypted video file and decrypt it with FFmpeg.
    ///
    /// `decryption_key` is the 32 lowercase hex characters of the CENC-AES-CTR key.
    pub async fn download_video_decrypt(
        &self,
        url: &str,
        decryption_key: &str,
        headers: Option<HeaderMap>,
    ) -> Result<PathBuf, DownloadError> {
        let cache_key = FileCache::key_from_url(&format!("decrypt:{}", url));
        let output_path = self.file_cache.get_path(&cache_key, "mp4");

        // Hold the per-entry flight across check + download + decrypt so a
        // concurrent request waits and then re-checks the cache.
        let _flight = self.flights.acquire(&output_path.to_string_lossy()).await;

        // Check cache first
        if let Some(cached_path) = self.file_cache.get(&cache_key, "mp4") {
            tracing::debug!("Cache hit for decrypted video");
            return Ok(cached_path);
        }

        // Download the encrypted file first
        let encrypted_path = self.download(url, "enc.mp4", headers).await?;

        // Ensure parent directory exists
        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // ffmpeg writes to a `.part` file that is only renamed into the cache
        // on success, so an interrupted run is never served as a cache hit.
        let part = part_path(&output_path);
        let result = match self
            .ffmpeg
            .decrypt(&encrypted_path, &part, decryption_key)
            .await
        {
            Ok(()) => self.check_output_size(&part).await,
            Err(e) => Err(e),
        };

        // Only this method downloads the `enc.mp4` entry, and it does so
        // under the flight held above, so no other request can be reading it.
        let _ = tokio::fs::remove_file(&encrypted_path).await;

        finalize_part(result, &part, &output_path).await
    }

    /// Download an image file
    pub async fn download_image(
        &self,
        url: &str,
        headers: Option<HeaderMap>,
    ) -> Result<PathBuf, DownloadError> {
        let extension = image_extension_from_url(url);
        self.download(url, extension, headers).await
    }

    /// Download an audio file
    pub async fn download_audio(
        &self,
        url: &str,
        headers: Option<HeaderMap>,
    ) -> Result<PathBuf, DownloadError> {
        let extension = audio_extension_from_url(url);
        self.download(url, extension, headers).await
    }

    /// Download an audio file, falling back through backup URLs.
    pub async fn download_audio_candidates(
        &self,
        urls: &[String],
        headers: Option<HeaderMap>,
    ) -> Result<PathBuf, DownloadError> {
        let extension = urls
            .first()
            .map(|url| audio_extension_from_url(url))
            .unwrap_or("mp3");
        self.download_first_available(urls, extension, headers)
            .await
    }

    /// Download and merge video + audio streams
    pub async fn download_and_merge(
        &self,
        video_url: &str,
        audio_url: &str,
        headers: Option<HeaderMap>,
    ) -> Result<PathBuf, DownloadError> {
        let video_urls = [video_url.to_string()];
        let audio_urls = [audio_url.to_string()];
        self.download_and_merge_candidates(&video_urls, &audio_urls, headers)
            .await
    }

    /// Download and merge video + audio streams, falling back through backup
    /// URLs on either side.
    ///
    /// The cache key is derived from the primary URLs only, so a run that had
    /// to fall back to a backup CDN still populates (and later hits) the same
    /// merged entry.
    pub async fn download_and_merge_candidates(
        &self,
        video_urls: &[String],
        audio_urls: &[String],
        headers: Option<HeaderMap>,
    ) -> Result<PathBuf, DownloadError> {
        let (Some(primary_video), Some(primary_audio)) = (video_urls.first(), audio_urls.first())
        else {
            return Err(DownloadError::InvalidUrl(
                "missing video or audio stream URL".to_string(),
            ));
        };

        // Generate cache key from both primary URLs
        let cache_key = FileCache::key_from_url(&format!("{}|{}", primary_video, primary_audio));
        let output_path = self.file_cache.get_path(&cache_key, "mp4");

        let _flight = self.flights.acquire(&output_path.to_string_lossy()).await;

        // Check cache first
        if let Some(cached_path) = self.file_cache.get(&cache_key, "mp4") {
            tracing::debug!("Cache hit for merged video");
            return Ok(cached_path);
        }

        // Ensure parent directory exists
        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Download video and audio in parallel
        let video_headers = headers.clone();
        let audio_headers = headers;

        let (video_result, audio_result) = tokio::join!(
            self.download_first_available(video_urls, "m4v", video_headers),
            self.download_first_available(audio_urls, "m4a", audio_headers)
        );

        let video_path = video_result?;
        let audio_path = audio_result?;

        // Merge with FFmpeg into a `.part` file, promoted only on success
        let part = part_path(&output_path);
        let result = match self.ffmpeg.merge_av(&video_path, &audio_path, &part).await {
            // The per-stream cap bounds each input separately; the merged file
            // is what actually gets sent, so it needs its own check.
            Ok(()) => self.check_output_size(&part).await,
            Err(e) => Err(e),
        };

        // The inputs are URL-keyed cache entries that other requests may be
        // reading right now, so they stay for the periodic cache cleanup.
        finalize_part(result, &part, &output_path).await
    }

    /// Download video + BGM and merge with audio mixing (amix).
    ///
    /// Unlike [`Self::download_and_merge`] which simply muxes separate streams, this
    /// overlays the BGM onto the video's existing audio track using ffmpeg's
    /// `amix` filter, falling back to a simple merge if the video has no audio.
    pub async fn download_and_merge_bgm(
        &self,
        video_url: &str,
        bgm_url: &str,
        headers: Option<HeaderMap>,
    ) -> Result<PathBuf, DownloadError> {
        let cache_key = FileCache::key_from_url(&format!("{}|bgm|{}", video_url, bgm_url));
        let output_path = self.file_cache.get_path(&cache_key, "mp4");

        let _flight = self.flights.acquire(&output_path.to_string_lossy()).await;

        if let Some(cached_path) = self.file_cache.get(&cache_key, "mp4") {
            tracing::debug!("Cache hit for BGM-merged video");
            return Ok(cached_path);
        }

        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let (video_result, audio_result) = tokio::join!(
            self.download(video_url, "mp4", headers.clone()),
            self.download(bgm_url, "mp3", headers)
        );

        let video_path = video_result?;
        let audio_path = audio_result?;

        let part = part_path(&output_path);
        let result = match self
            .ffmpeg
            .merge_with_bgm(&video_path, &audio_path, &part)
            .await
        {
            Ok(()) => self.check_output_size(&part).await,
            Err(e) => Err(e),
        };

        // Inputs are shared cache entries; see `download_and_merge_candidates`.
        finalize_part(result, &part, &output_path).await
    }

    /// Record a bounded clip from a live stream URL.
    ///
    /// Live streams have no end and no `Content-Length`, so the normal
    /// download path cannot bound them; `seconds` is the only limit, and the
    /// resulting clip is still checked against the size cap.
    pub async fn download_stream_segment(
        &self,
        stream_url: &str,
        seconds: u64,
        headers: Option<&[(&str, &str)]>,
    ) -> Result<PathBuf, DownloadError> {
        let cache_key = FileCache::key_from_url(&format!("live:{seconds}:{stream_url}"));
        let output_path = self.file_cache.get_path(&cache_key, "mp4");

        let _flight = self.flights.acquire(&output_path.to_string_lossy()).await;

        if let Some(cached_path) = self.file_cache.get(&cache_key, "mp4") {
            tracing::debug!("Cache hit for live stream clip");
            return Ok(cached_path);
        }

        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let part = part_path(&output_path);
        let result = match self
            .ffmpeg
            .record_stream(stream_url, &part, seconds, headers)
            .await
        {
            Ok(()) => self.check_output_size(&part).await,
            Err(e) => Err(e),
        };

        finalize_part(result, &part, &output_path).await
    }

    /// Reject an ffmpeg output file that exceeds the configured size cap.
    ///
    /// Streaming downloads are capped as they arrive, but a merged, decrypted
    /// or transcoded file is produced locally and can land above the limit even
    /// when every input stayed under it.
    async fn check_output_size(&self, path: &Path) -> Result<(), DownloadError> {
        let size = tokio::fs::metadata(path).await?.len();
        if size > self.max_size {
            tracing::warn!(
                size,
                limit = self.max_size,
                "FFmpeg output exceeds the size limit"
            );
            return Err(DownloadError::FileTooLarge {
                size,
                limit: self.max_size,
            });
        }
        Ok(())
    }

    /// Check if duration is within limits
    pub fn check_duration(&self, duration: u64) -> Result<(), DownloadError> {
        if self.max_duration > 0 && duration > self.max_duration {
            return Err(DownloadError::DurationTooLong {
                duration,
                limit: self.max_duration,
            });
        }
        Ok(())
    }

    /// Create headers with referer
    pub fn headers_with_referer(referer: &str) -> HeaderMap {
        http::headers_with_dynamic_referer(DEFAULT_USER_AGENT, referer)
    }

    /// Download a file with progress callback
    pub async fn download_with_progress(
        &self,
        url: &str,
        extension: &str,
        headers: Option<HeaderMap>,
        progress: Option<ProgressCallback>,
    ) -> Result<PathBuf, DownloadError> {
        self.transfer()
            .download_to_cache(url, extension, headers, progress)
            .await
    }

    fn transfer(&self) -> DownloadTransfer<'_> {
        DownloadTransfer::new(&self.client, &self.file_cache, self.max_size, &self.flights)
    }

    /// Convert a video to GIF (for short clips)
    ///
    /// # Arguments
    /// * `video_url` - URL of the video to convert
    /// * `headers` - Optional headers for the request
    /// * `max_duration` - Maximum duration in seconds (videos longer than this will fail)
    /// * `fps` - Frames per second for the GIF (default: 10)
    /// * `width` - Width of the GIF (default: 320, height is auto-calculated)
    pub async fn download_as_gif(
        &self,
        video_url: &str,
        headers: Option<HeaderMap>,
        max_duration: Option<u64>,
        fps: Option<u32>,
        width: Option<u32>,
    ) -> Result<PathBuf, DownloadError> {
        let cache_key = FileCache::key_from_url(&format!("gif:{}", video_url));
        let output_path = self.file_cache.get_path(&cache_key, "gif");

        let _flight = self.flights.acquire(&output_path.to_string_lossy()).await;

        // Check cache first
        if let Some(cached_path) = self.file_cache.get(&cache_key, "gif") {
            tracing::debug!("Cache hit for GIF");
            return Ok(cached_path);
        }

        // Download the video first
        let video_path = self.download(video_url, "mp4", headers).await?;

        // Check duration if specified
        if let Some(max_dur) = max_duration {
            match self.ffmpeg.get_duration(video_path.as_path()).await {
                Ok(duration) if duration > max_dur => {
                    return Err(DownloadError::DurationTooLong {
                        duration,
                        limit: max_dur,
                    });
                }
                Ok(_) => {}
                // The size cap in download() still bounds the GIF source.
                Err(e) => {
                    tracing::warn!(error = %e, "ffprobe failed, skipping GIF duration check");
                }
            }
        }

        // Ensure parent directory exists
        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Convert to GIF via a `.part` file, promoted only on success
        let part = part_path(&output_path);
        let result = match self
            .ffmpeg
            .video_to_gif(
                &video_path,
                &part,
                fps.unwrap_or(GIF_DEFAULT_FPS),
                width.unwrap_or(GIF_DEFAULT_WIDTH),
            )
            .await
        {
            Ok(()) => self.check_output_size(&part).await,
            Err(e) => Err(e),
        };

        // The source video is a shared cache entry; see
        // `download_and_merge_candidates`.
        finalize_part(result, &part, &output_path).await
    }

    /// Download an avatar image and process it for rendering
    pub async fn download_avatar(
        &self,
        url: &str,
        headers: Option<HeaderMap>,
    ) -> Result<PathBuf, DownloadError> {
        self.download_image(url, headers).await
    }

    /// Download a cover image for rendering
    pub async fn download_cover(
        &self,
        url: &str,
        headers: Option<HeaderMap>,
    ) -> Result<PathBuf, DownloadError> {
        self.download_image(url, headers).await
    }
}

/// Shared downloader type
pub type SharedDownloader = Arc<StreamDownloader>;

/// Describe an unusable `proxy` setting without echoing the URL itself,
/// which may carry credentials.
fn invalid_proxy_error(error: &wreq::Error) -> DownloadError {
    let reason = std::error::Error::source(error)
        .map(ToString::to_string)
        .unwrap_or_else(|| "unparsable URL".to_string());
    DownloadError::InvalidUrl(format!(
        "config `proxy` is not a valid proxy URL ({reason}); expected e.g. \
         http://127.0.0.1:7890 or socks5://127.0.0.1:1080"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_proxy_is_rejected_instead_of_ignored() {
        let temp_dir = tempfile::tempdir().unwrap();
        for proxy in ["not a proxy", "http://user:secret@[::1", "http://[::1"] {
            let config = Config {
                proxy: Some(proxy.to_string()),
                ..Config::default()
            };
            let err = match StreamDownloader::new(&config, temp_dir.path()) {
                Ok(_) => panic!("invalid proxy {proxy:?} must not fall back to direct downloads"),
                Err(e) => e.to_string(),
            };
            assert!(err.contains("proxy"), "{err}");
            assert!(!err.contains("secret"), "proxy credentials leaked: {err}");
        }
    }
}
