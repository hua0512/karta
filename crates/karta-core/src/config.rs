use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::constants::{
    DEFAULT_CACHE_SIZE, DEFAULT_MAX_DURATION, DEFAULT_MAX_SIZE_MB, DEFAULT_PARSE_TIMEOUT, Platform,
};

mod access;
mod bilibili;
mod douyin;
mod emoji;
mod hongguo;
mod render;

pub use access::{AccessControlConfig, AccessMode};
pub use bilibili::{BiliCdnMode, BiliVideoCodec, BiliVideoQuality};
pub use douyin::DouyinMobileConfig;
pub use emoji::EmojiStyleConfig;
pub use hongguo::HongguoApiAuth;
pub use render::{ImageHeaderStyle, ImageThemeMode, RenderType};

/// Plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Bilibili cookie for authenticated requests
    #[serde(default)]
    pub bili_cookie: Option<String>,

    /// Refresh the Bilibili cookie when Bilibili asks for it: checked at
    /// startup and every 12 hours. A refresh logs out the session the old
    /// cookie belonged to; it needs the cookie's refresh token
    /// (`ac_time_value`), which QR login stores.
    #[serde(default = "default_bili_cookie_auto_refresh")]
    pub bili_cookie_auto_refresh: bool,

    /// YouTube cookie for authenticated requests
    #[serde(default)]
    pub ytb_cookie: Option<String>,

    /// Xiaohongshu cookie for authenticated requests
    #[serde(default)]
    pub xhs_cookie: Option<String>,

    /// Twitter/X cookie for authenticated requests
    #[serde(default)]
    pub twitter_cookie: Option<String>,

    /// Weibo cookie for authenticated requests
    #[serde(default)]
    pub weibo_cookie: Option<String>,

    /// Xiaoheihe cookie for authenticated requests
    #[serde(default)]
    pub xhh_cookie: Option<String>,

    /// Douyin cookie for authenticated requests (enables official API with a-bogus signing)
    #[serde(default)]
    pub douyin_cookie: Option<String>,

    /// Optional registered Android identity for the signed mobile detail fallback.
    #[serde(default)]
    pub douyin_mobile: Option<DouyinMobileConfig>,

    /// Instagram cookie for authenticated requests (enables GraphQL API and stories)
    #[serde(default)]
    pub instagram_cookie: Option<String>,

    /// Authorized ByteDance app session for Hongguo motion-comic video resolution
    #[serde(default)]
    pub hongguo_api_auth: Option<HongguoApiAuth>,

    /// Tencent Yuanbao web cookie used for WeChat video-channel (视频号) parsing.
    ///
    /// Obtain by logging into <https://yuanbao.tencent.com> in a browser and
    /// copying the request Cookie header. No WeChat login is required.
    #[serde(default)]
    pub wechat_channel_cookie: Option<String>,

    /// HTTP proxy URL
    #[serde(default)]
    pub proxy: Option<String>,

    /// Maximum file size in MB
    #[serde(default = "default_max_size")]
    pub max_size: u64,

    /// Maximum video duration in seconds
    #[serde(default = "default_duration_maximum")]
    pub duration_maximum: u64,

    /// Render type (text or image)
    #[serde(default)]
    pub render_type: RenderType,

    /// Native card treatment (blur_background, auto, mesh_glow, blur or soft_prism).
    #[serde(default)]
    pub image_header_style: ImageHeaderStyle,

    /// Native card color polarity (auto, light or dark).
    #[serde(default)]
    pub image_theme_mode: ImageThemeMode,

    /// List of disabled platforms
    #[serde(default)]
    pub disabled_platforms: Vec<Platform>,

    /// Cache size (number of entries)
    #[serde(default = "default_cache_size")]
    pub cache_size: usize,

    /// Whether to download media files
    #[serde(default = "default_download_media")]
    pub download_media: bool,

    /// Maximum number of concurrent media downloads during rendering.
    ///
    /// This limits parallel HTTP requests (and ffmpeg conversions) to reduce the
    /// chance of triggering upstream rate limits while still improving latency
    /// for multi-image posts.
    #[serde(default = "default_download_concurrency")]
    pub download_concurrency: usize,

    /// FFmpeg path (if not in PATH)
    #[serde(default)]
    pub ffmpeg_path: Option<String>,

    /// Bilibili video quality preference
    #[serde(default)]
    pub bili_video_quality: BiliVideoQuality,

    /// Bilibili video codec preferences (in order of preference)
    #[serde(default = "default_bili_video_codecs")]
    pub bili_video_codecs: Vec<BiliVideoCodec>,

    /// How to choose among the CDN hosts Bilibili offers for a stream
    #[serde(default)]
    pub bili_cdn_mode: BiliCdnMode,

    /// Step `bili_video_quality` down when the estimated merged size would
    /// exceed `max_size`, instead of failing the download outright.
    #[serde(default)]
    pub bili_smart_resolution: bool,

    /// Seconds of a Bilibili live stream to record and send (0 disables it and
    /// sends room metadata only).
    #[serde(default)]
    pub bili_live_record_seconds: u64,

    /// Path to yt-dlp executable (if not in PATH)
    #[serde(default)]
    pub ytdlp_path: Option<String>,

    /// Path to cookies file for yt-dlp (Netscape format)
    #[serde(default)]
    pub ytdlp_cookies_file: Option<String>,

    /// Custom font path for image rendering
    #[serde(default)]
    pub custom_font: Option<String>,

    /// Emoji style for image rendering (twemoji, apple, google, facebook)
    #[serde(default)]
    pub emoji_style: EmojiStyleConfig,

    /// Custom emoji CDN URL (overrides emoji_style)
    #[serde(default)]
    pub emoji_cdn_url: Option<String>,

    /// Whether to enable emoji rendering
    #[serde(default = "default_emoji_enabled")]
    pub emoji_enabled: bool,

    /// Whether to append original URL to parse results
    #[serde(default)]
    pub append_url: bool,

    /// Whether to use base64 encoding for images/audio/video
    #[serde(default)]
    pub use_base64: bool,

    /// Upload standalone audio/video as chat files instead of inline media.
    #[serde(default)]
    pub need_upload: bool,

    /// Access control configuration (blacklist/whitelist)
    #[serde(default)]
    pub access_control: AccessControlConfig,

    /// Whether the runtime `/karta on|off` group toggle acts as a blacklist.
    ///
    /// `true` (default): every group parses unless disabled via `/karta off`.
    /// `false` (whitelist): no group parses until enabled via `/karta on`.
    /// Both modes share the same persisted group set in `state.json`.
    #[serde(default = "default_group_blacklist_enabled")]
    pub group_blacklist_enabled: bool,

    /// Whether to forward media contents in messages
    #[serde(default = "default_forward_contents")]
    pub forward_contents: bool,

    #[serde(default = "default_reaction_enabled")]
    pub reaction_enabled: bool,

    /// Per-parse timeout in seconds (0 = no timeout).
    ///
    /// Adapters apply the same value as a separate, equal budget for the
    /// render stage (downloads + ffmpeg merges).
    #[serde(default = "default_parse_timeout")]
    pub parse_timeout: u64,

    /// Maximum number of messages parsed and rendered concurrently (0 = unlimited).
    ///
    /// Bounds bot-wide parse+render work so a burst of link messages cannot
    /// spawn unbounded parallel downloads.
    #[serde(default = "default_max_concurrent_parses")]
    pub max_concurrent_parses: usize,

    /// Auto-forward when combined text length exceeds this threshold (0 disables).
    #[serde(default = "default_forward_text_threshold")]
    pub forward_text_threshold: usize,

    /// Auto-forward when the number of media segments exceeds this threshold (0 disables).
    #[serde(default = "default_forward_media_threshold")]
    pub forward_media_threshold: usize,
}

fn default_bili_cookie_auto_refresh() -> bool {
    true
}

fn default_max_size() -> u64 {
    DEFAULT_MAX_SIZE_MB
}

fn default_duration_maximum() -> u64 {
    DEFAULT_MAX_DURATION
}

fn default_cache_size() -> usize {
    DEFAULT_CACHE_SIZE
}

fn default_download_media() -> bool {
    true
}

fn default_download_concurrency() -> usize {
    4
}

fn default_bili_video_codecs() -> Vec<BiliVideoCodec> {
    vec![
        BiliVideoCodec::AVC,
        BiliVideoCodec::AV1,
        BiliVideoCodec::HEVC,
    ]
}

fn default_emoji_enabled() -> bool {
    true
}

fn default_forward_contents() -> bool {
    true
}

fn default_group_blacklist_enabled() -> bool {
    true
}

fn default_parse_timeout() -> u64 {
    DEFAULT_PARSE_TIMEOUT
}

fn default_max_concurrent_parses() -> usize {
    8
}

fn default_forward_text_threshold() -> usize {
    300
}

fn default_forward_media_threshold() -> usize {
    4
}

fn default_reaction_enabled() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        // Single source of truth: every field carries a serde default, so an
        // empty JSON object deserializes into the canonical default config.
        serde_json::from_str("{}").expect("empty object must deserialize into default Config")
    }
}

/// Atomically write `contents` to `path` via a `<path>.tmp` sibling and rename,
/// so a crash mid-write never leaves a truncated file behind.
pub fn atomic_write(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let mut tmp_name = path.as_os_str().to_owned();
    tmp_name.push(".tmp");
    let tmp_path = PathBuf::from(tmp_name);

    {
        let mut file = std::fs::File::create(&tmp_path)?;
        file.write_all(contents)?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp_path, path)
}

impl Config {
    /// The configured proxy as a URL, or `None` when unset or blank.
    ///
    /// A bare `host:port` (as proxy clients usually display it) means an HTTP
    /// proxy, so it gets an `http://` scheme.
    pub fn proxy_url(&self) -> Option<String> {
        let proxy = self.proxy.as_deref()?.trim();
        if proxy.is_empty() {
            None
        } else if proxy.contains("://") {
            Some(proxy.to_string())
        } else {
            Some(format!("http://{proxy}"))
        }
    }

    /// Load configuration from the plugin data directory.
    ///
    /// A default config file is only written when none exists yet. A file that
    /// fails to read or parse is preserved (copied to `config.json.bad`) and
    /// in-memory defaults are used without saving, so a typo can never destroy
    /// user-supplied secrets like cookies.
    pub fn load(data_path: &Path) -> Self {
        let config_path = data_path.join("config.json");

        if !config_path.exists() {
            let config = Config::default();
            let _ = config.save(data_path);
            return config;
        }

        let parsed = std::fs::read_to_string(&config_path)
            .map_err(|e| e.to_string())
            .and_then(|content| serde_json::from_str(&content).map_err(|e| e.to_string()));

        match parsed {
            Ok(config) => config,
            Err(e) => {
                let bad_path = data_path.join("config.json.bad");
                let _ = std::fs::copy(&config_path, &bad_path);
                tracing::error!(
                    "Failed to load {}: {}. A copy was saved to {} for recovery; \
                     running with in-memory defaults (config file left untouched). \
                     Fix config.json and restart.",
                    config_path.display(),
                    e,
                    bad_path.display()
                );
                Config::default()
            }
        }
    }

    /// Save configuration to the plugin data directory
    pub fn save(&self, data_path: &Path) -> std::io::Result<()> {
        let config_path = data_path.join("config.json");

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        atomic_write(&config_path, content.as_bytes())
    }

    /// Get the cache directory path
    pub fn cache_dir(data_path: &Path) -> PathBuf {
        data_path.join("cache")
    }

    /// Get the download directory path
    pub fn download_dir(data_path: &Path) -> PathBuf {
        data_path.join("downloads")
    }

    /// Get the yt-dlp output directory path.
    ///
    /// Single source of truth for where `YtdlpDownloader` writes media
    /// (used both when constructing `YtdlpConfig` and by the adapter's
    /// stale-file cleanup cron).
    pub fn ytdlp_dir(data_path: &Path) -> PathBuf {
        Self::download_dir(data_path).join("ytdlp")
    }

    /// Check if a platform is disabled
    pub fn is_platform_disabled(&self, platform: Platform) -> bool {
        self.disabled_platforms.contains(&platform)
    }

    /// Get max size in bytes
    pub fn max_size_bytes(&self) -> u64 {
        self.max_size * 1024 * 1024
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_preserves_broken_config_and_falls_back_to_defaults() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config_path = dir.path().join("config.json");
        let broken = r#"{"bili_cookie": "secret", oops}"#;
        std::fs::write(&config_path, broken).expect("write broken config");

        let config = Config::load(dir.path());

        // The broken file must not be overwritten, and a recovery copy exists.
        assert_eq!(
            std::fs::read_to_string(&config_path).expect("read config"),
            broken
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("config.json.bad")).expect("read bad copy"),
            broken
        );
        assert_eq!(config.bili_cookie, None);
        assert_eq!(config.cache_size, DEFAULT_CACHE_SIZE);
    }
}
