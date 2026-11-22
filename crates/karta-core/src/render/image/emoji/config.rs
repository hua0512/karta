use std::path::PathBuf;

use crate::config::EmojiStyleConfig;

/// Emoji style variants available from CDN
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EmojiStyle {
    /// Twitter's Twemoji style (default)
    #[default]
    Twemoji,
    /// Apple-style emojis
    Apple,
    /// Google Noto emojis
    Google,
    /// Facebook emojis
    Facebook,
}

impl From<EmojiStyleConfig> for EmojiStyle {
    fn from(config: EmojiStyleConfig) -> Self {
        match config {
            EmojiStyleConfig::Twemoji => EmojiStyle::Twemoji,
            EmojiStyleConfig::Apple => EmojiStyle::Apple,
            EmojiStyleConfig::Google => EmojiStyle::Google,
            EmojiStyleConfig::Facebook => EmojiStyle::Facebook,
        }
    }
}

impl EmojiStyle {
    /// Get the CDN base URL for this style
    pub fn cdn_base_url(&self) -> &'static str {
        match self {
            EmojiStyle::Twemoji => {
                "https://cdn.jsdelivr.net/gh/twitter/twemoji@latest/assets/72x72"
            }
            EmojiStyle::Apple => {
                "https://cdn.jsdelivr.net/npm/emoji-datasource-apple@15.1.2/img/apple/64"
            }
            EmojiStyle::Google => {
                "https://cdn.jsdelivr.net/npm/emoji-datasource-google@15.1.2/img/google/64"
            }
            EmojiStyle::Facebook => {
                "https://cdn.jsdelivr.net/npm/emoji-datasource-facebook@15.1.2/img/facebook/64"
            }
        }
    }

    /// Get fallback CDN URLs for this style (in order of preference)
    pub fn fallback_cdn_urls(&self) -> Vec<&'static str> {
        match self {
            EmojiStyle::Twemoji => vec![
                "https://cdn.jsdelivr.net/gh/twitter/twemoji@latest/assets/72x72",
                "https://raw.githubusercontent.com/twitter/twemoji/master/assets/72x72",
            ],
            EmojiStyle::Apple => vec![
                "https://cdn.jsdelivr.net/npm/emoji-datasource-apple@15.1.2/img/apple/64",
                "https://unpkg.com/emoji-datasource-apple@15.1.2/img/apple/64",
            ],
            EmojiStyle::Google => vec![
                "https://cdn.jsdelivr.net/npm/emoji-datasource-google@15.1.2/img/google/64",
                "https://unpkg.com/emoji-datasource-google@15.1.2/img/google/64",
            ],
            EmojiStyle::Facebook => vec![
                "https://cdn.jsdelivr.net/npm/emoji-datasource-facebook@15.1.2/img/facebook/64",
                "https://unpkg.com/emoji-datasource-facebook@15.1.2/img/facebook/64",
            ],
        }
    }

    /// Stable identifier used to namespace this style's disk cache.
    pub fn slug(&self) -> &'static str {
        match self {
            EmojiStyle::Twemoji => "twemoji",
            EmojiStyle::Apple => "apple",
            EmojiStyle::Google => "google",
            EmojiStyle::Facebook => "facebook",
        }
    }

    /// Get the file extension for emoji images
    pub fn extension(&self) -> &'static str {
        ".png"
    }
}

/// Configuration for emoji rendering
#[derive(Debug, Clone)]
pub struct EmojiConfig {
    /// Emoji style to use
    pub style: EmojiStyle,
    /// Custom CDN base URL (overrides style's default)
    pub custom_cdn_url: Option<String>,
    /// Cache directory for downloaded emoji images
    pub cache_dir: PathBuf,
    /// Whether to enable emoji rendering
    pub enabled: bool,
}

impl Default for EmojiConfig {
    fn default() -> Self {
        Self {
            style: EmojiStyle::default(),
            custom_cdn_url: None,
            cache_dir: std::env::temp_dir().join("kovi_parser_emoji_cache"),
            enabled: true,
        }
    }
}

impl EmojiConfig {
    /// Create a new emoji config with the specified cache directory
    pub fn with_cache_dir(cache_dir: PathBuf) -> Self {
        Self {
            cache_dir,
            ..Default::default()
        }
    }

    /// Default emoji disk-cache directory under the bot's data path.
    ///
    /// Deliberately OUTSIDE the media cache root (`data_path/cache`): the
    /// media [`crate::cache::FileCache`] sweeps its root daily and expects a
    /// flat directory of plain files, so nesting the emoji cache there would
    /// make the sweep trip over a stale subdirectory.
    pub fn default_cache_dir(data_path: &std::path::Path) -> PathBuf {
        data_path.join("emoji_cache")
    }

    /// Create emoji config from the main application config
    pub fn from_config(config: &crate::config::Config, data_path: &std::path::Path) -> Self {
        Self {
            style: config.emoji_style.into(),
            custom_cdn_url: config.emoji_cdn_url.clone(),
            cache_dir: Self::default_cache_dir(data_path),
            enabled: config.emoji_enabled,
        }
    }

    /// Disk-cache directory for the image source in effect.
    ///
    /// Styles and CDNs serve different artwork under the same codepoint
    /// filename, so each source caches into its own subdirectory of
    /// `cache_dir`: a custom CDN by a hash of its URL, otherwise by style.
    pub fn source_cache_dir(&self) -> PathBuf {
        let namespace = match &self.custom_cdn_url {
            Some(url) => format!("custom-{}", crate::cache::cache_key(url)),
            None => self.style.slug().to_string(),
        };
        self.cache_dir.join(namespace)
    }

    /// Get the CDN base URL
    pub fn cdn_base_url(&self) -> &str {
        self.custom_cdn_url
            .as_deref()
            .unwrap_or_else(|| self.style.cdn_base_url())
    }
}
