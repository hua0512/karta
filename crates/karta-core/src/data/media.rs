use std::path::PathBuf;

use serde::Serialize;

fn serialize_path_opt<S: serde::Serializer>(
    path: &Option<PathBuf>,
    s: S,
) -> Result<S::Ok, S::Error> {
    match path {
        Some(p) => s.serialize_some(&p.to_string_lossy().into_owned()),
        None => s.serialize_none(),
    }
}

/// Media content types
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Content {
    Text(String),
    Video(VideoContent),
    Image(ImageContent),
    Audio(AudioContent),
    Dynamic(DynamicContent),
    Graphics(GraphicsContent),
}

/// Audio content
#[derive(Debug, Clone, Serialize)]
pub struct AudioContent {
    /// Local file path (after download)
    #[serde(serialize_with = "serialize_path_opt")]
    pub path: Option<PathBuf>,
    /// Remote URL
    pub url: Option<String>,
    /// Duration in seconds
    pub duration: Option<u64>,
    /// File size in bytes
    pub size: Option<u64>,
    /// Title
    pub title: Option<String>,
    /// Artist
    pub artist: Option<String>,
}

impl AudioContent {
    pub fn new() -> Self {
        Self {
            path: None,
            url: None,
            duration: None,
            size: None,
            title: None,
            artist: None,
        }
    }

    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    pub fn with_path(mut self, path: PathBuf) -> Self {
        self.path = Some(path);
        self
    }

    pub fn with_duration(mut self, duration: u64) -> Self {
        self.duration = Some(duration);
        self
    }

    pub fn with_size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_artist(mut self, artist: impl Into<String>) -> Self {
        self.artist = Some(artist.into());
        self
    }
}

impl Default for AudioContent {
    fn default() -> Self {
        Self::new()
    }
}

/// Image content
#[derive(Debug, Clone, Serialize)]
pub struct ImageContent {
    /// Local file path (after download)
    #[serde(serialize_with = "serialize_path_opt")]
    pub path: Option<PathBuf>,
    /// Remote URL
    pub url: Option<String>,
    /// Width in pixels
    pub width: Option<u32>,
    /// Height in pixels
    pub height: Option<u32>,
    /// File size in bytes
    pub size: Option<u64>,
    /// Caption describing the image, when the source distinguishes one from
    /// the surrounding body text (e.g. a Bilibili opus image caption).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
}

impl ImageContent {
    pub fn new() -> Self {
        Self {
            path: None,
            url: None,
            width: None,
            height: None,
            size: None,
            alt: None,
        }
    }

    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    pub fn with_path(mut self, path: PathBuf) -> Self {
        self.path = Some(path);
        self
    }

    pub fn with_dimensions(mut self, width: u32, height: u32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }

    pub fn with_size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }

    pub fn with_alt(mut self, alt: impl Into<String>) -> Self {
        self.alt = Some(alt.into());
        self
    }
}

impl Default for ImageContent {
    fn default() -> Self {
        Self::new()
    }
}

/// Video content
#[derive(Debug, Clone, Serialize)]
pub struct VideoContent {
    /// Local file path (after download)
    #[serde(serialize_with = "serialize_path_opt")]
    pub path: Option<PathBuf>,
    /// Remote URL
    pub url: Option<String>,
    /// Alternate URLs for the same combined stream, in retry order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub backup_urls: Vec<String>,
    /// Video stream URL (for separate video/audio streams)
    pub video_url: Option<String>,
    /// Alternate video-only stream URLs, in retry order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub video_backup_urls: Vec<String>,
    /// Audio stream URL (for separate video/audio streams)
    pub audio_url: Option<String>,
    /// Alternate audio-only stream URLs, in retry order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub audio_backup_urls: Vec<String>,
    /// Cover image URL
    pub cover: Option<String>,
    /// Duration in seconds
    pub duration: Option<u64>,
    /// Width in pixels
    pub width: Option<u32>,
    /// Height in pixels
    pub height: Option<u32>,
    /// File size in bytes
    pub size: Option<u64>,
    /// Quality label (e.g., "1080p", "720p")
    pub quality: Option<String>,
    /// CENC-AES-CTR decryption key (32 lowercase hex chars), if the media is encrypted
    pub decryption_key: Option<String>,
    /// Custom HTTP headers for downloading this media, overriding the platform
    /// default. Set when the video's CDN needs headers the platform default
    /// doesn't provide — e.g. an app `User-Agent` for a host that rejects
    /// browser agents (Hongguo motion-comic streams).
    pub download_headers: Option<Vec<(String, String)>>,
}

impl VideoContent {
    pub fn new() -> Self {
        Self {
            path: None,
            url: None,
            backup_urls: Vec::new(),
            video_url: None,
            video_backup_urls: Vec::new(),
            audio_url: None,
            audio_backup_urls: Vec::new(),
            cover: None,
            duration: None,
            width: None,
            height: None,
            size: None,
            quality: None,
            decryption_key: None,
            download_headers: None,
        }
    }

    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    pub fn with_url_candidates(mut self, mut urls: Vec<String>) -> Self {
        if !urls.is_empty() {
            self.url = Some(urls.remove(0));
            self.backup_urls = urls;
        }
        self
    }

    pub fn with_streams(
        mut self,
        video_url: impl Into<String>,
        audio_url: impl Into<String>,
    ) -> Self {
        self.video_url = Some(video_url.into());
        self.audio_url = Some(audio_url.into());
        self
    }

    pub fn with_stream_candidates(
        mut self,
        mut video_urls: Vec<String>,
        mut audio_urls: Vec<String>,
    ) -> Self {
        if !video_urls.is_empty() {
            self.video_url = Some(video_urls.remove(0));
            self.video_backup_urls = video_urls;
        }
        if !audio_urls.is_empty() {
            self.audio_url = Some(audio_urls.remove(0));
            self.audio_backup_urls = audio_urls;
        }
        self
    }

    pub fn video_url_candidates(&self) -> Vec<String> {
        self.video_url
            .iter()
            .cloned()
            .chain(self.video_backup_urls.iter().cloned())
            .collect()
    }

    pub fn audio_url_candidates(&self) -> Vec<String> {
        self.audio_url
            .iter()
            .cloned()
            .chain(self.audio_backup_urls.iter().cloned())
            .collect()
    }

    pub fn url_candidates(&self) -> Vec<String> {
        self.url
            .iter()
            .cloned()
            .chain(self.backup_urls.iter().cloned())
            .collect()
    }

    pub fn with_cover(mut self, cover: impl Into<String>) -> Self {
        self.cover = Some(cover.into());
        self
    }

    pub fn with_duration(mut self, duration: u64) -> Self {
        self.duration = Some(duration);
        self
    }

    pub fn with_dimensions(mut self, width: u32, height: u32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }

    pub fn with_size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }

    pub fn with_quality(mut self, quality: impl Into<String>) -> Self {
        self.quality = Some(quality.into());
        self
    }

    pub fn with_decryption_key(mut self, key: impl Into<String>) -> Self {
        self.decryption_key = Some(key.into());
        self
    }

    pub fn with_download_headers(mut self, headers: Vec<(String, String)>) -> Self {
        self.download_headers = Some(headers);
        self
    }

    pub fn with_path(mut self, path: PathBuf) -> Self {
        self.path = Some(path);
        self
    }

    /// Format duration as "MM:SS" or "HH:MM:SS" for display
    pub fn display_duration(&self) -> Option<String> {
        self.duration.map(|d| {
            let hours = d / 3600;
            let minutes = (d % 3600) / 60;
            let seconds = d % 60;
            if hours > 0 {
                format!("{hours}:{minutes:02}:{seconds:02}")
            } else {
                format!("{minutes}:{seconds:02}")
            }
        })
    }

    /// Check if this video has separate streams that need merging
    pub fn needs_merge(&self) -> bool {
        self.video_url.is_some() && self.audio_url.is_some()
    }

    /// Get the download URL (prefers combined URL over separate streams)
    pub fn download_url(&self) -> Option<&str> {
        self.url.as_deref()
    }
}

impl Default for VideoContent {
    fn default() -> Self {
        Self::new()
    }
}

/// Dynamic content (e.g., Bilibili dynamics, Weibo posts)
/// Supports video-to-GIF conversion for short clips.
#[derive(Debug, Clone, Serialize)]
pub struct DynamicContent {
    /// Dynamic ID
    pub id: String,
    /// Dynamic type (e.g., "DYNAMIC_TYPE_DRAW", "DYNAMIC_TYPE_FORWARD")
    pub dynamic_type: String,
    /// Rich text content
    pub rich_text: Option<String>,
    /// Embedded media
    pub media: Vec<Content>,
    /// Whether this dynamic should be converted to GIF (for short videos)
    pub convert_to_gif: bool,
    /// Maximum duration for GIF conversion (in seconds)
    pub gif_max_duration: Option<u64>,
    /// Original video URL (for GIF conversion)
    pub video_url: Option<String>,
    /// Cover image URL
    pub cover_url: Option<String>,
}

impl DynamicContent {
    pub fn new(id: impl Into<String>, dynamic_type: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            dynamic_type: dynamic_type.into(),
            rich_text: None,
            media: Vec::new(),
            convert_to_gif: false,
            gif_max_duration: None,
            video_url: None,
            cover_url: None,
        }
    }

    pub fn with_rich_text(mut self, text: impl Into<String>) -> Self {
        self.rich_text = Some(text.into());
        self
    }

    pub fn with_media(mut self, media: Content) -> Self {
        self.media.push(media);
        self
    }

    pub fn with_gif_conversion(mut self, video_url: impl Into<String>, max_duration: u64) -> Self {
        self.convert_to_gif = true;
        self.video_url = Some(video_url.into());
        self.gif_max_duration = Some(max_duration);
        self
    }

    pub fn with_cover(mut self, cover_url: impl Into<String>) -> Self {
        self.cover_url = Some(cover_url.into());
        self
    }

    /// Check if this dynamic content should be converted to GIF
    pub fn should_convert_to_gif(&self) -> bool {
        self.convert_to_gif && self.video_url.is_some()
    }
}

/// Graphics content (text + image combination with alt text support)
#[derive(Debug, Clone, Serialize)]
pub struct GraphicsContent {
    /// Images in the gallery
    pub images: Vec<ImageContent>,
    /// Title
    pub title: Option<String>,
    /// Alt text for accessibility
    pub alt_text: Option<String>,
    /// Description text to display alongside images
    pub description: Option<String>,
}

impl GraphicsContent {
    pub fn new() -> Self {
        Self {
            images: Vec::new(),
            title: None,
            alt_text: None,
            description: None,
        }
    }

    pub fn with_images(mut self, images: Vec<ImageContent>) -> Self {
        self.images = images;
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_alt_text(mut self, alt_text: impl Into<String>) -> Self {
        self.alt_text = Some(alt_text.into());
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn add_image(&mut self, image: ImageContent) {
        self.images.push(image);
    }
}

impl Default for GraphicsContent {
    fn default() -> Self {
        Self::new()
    }
}
