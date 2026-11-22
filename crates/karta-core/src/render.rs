pub mod image;
pub mod text;

mod cache;
pub(crate) mod extra;
mod media;
mod text_utils;
mod video;

use std::path::PathBuf;

use async_trait::async_trait;

use crate::config::Config;
use crate::config::EmojiStyleConfig;
use crate::config::ImageHeaderStyle;
use crate::config::ImageThemeMode;
use crate::config::RenderType;
use crate::data::ParseResult;
use crate::download::SharedDownloader;
use crate::error::RenderResult;
use crate::message::RenderOutput;

pub(crate) use cache::save_png_to_cache;
pub(crate) use media::{append_media_messages, hydrate_media};
pub(crate) use text_utils::split_text_segments;

/// Maximum characters per text message / bundled message (QQ payload limit).
pub(crate) const MAX_TEXT_SEGMENT_CHARS: usize = 800;

/// Runtime rendering options derived from [`Config`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderOptions {
    pub image_header_style: ImageHeaderStyle,
    pub image_theme_mode: ImageThemeMode,
    pub append_url: bool,
    pub download_media: bool,
    pub use_base64: bool,
    pub need_upload: bool,
    pub forward_contents: bool,
    pub forward_text_threshold: usize,
    pub forward_media_threshold: usize,
    pub download_concurrency: usize,
    pub emoji_enabled: bool,
    pub emoji_style: EmojiStyleConfig,
    /// Custom emoji CDN URL (overrides the style's default CDNs). Carried in
    /// the options so every renderer construction path honors
    /// `Config::emoji_cdn_url`.
    pub emoji_cdn_url: Option<String>,
}

impl From<&Config> for RenderOptions {
    fn from(config: &Config) -> Self {
        Self {
            image_header_style: config.image_header_style,
            image_theme_mode: config.image_theme_mode,
            append_url: config.append_url,
            download_media: config.download_media,
            use_base64: config.use_base64,
            need_upload: config.need_upload,
            forward_contents: config.forward_contents,
            forward_text_threshold: config.forward_text_threshold,
            forward_media_threshold: config.forward_media_threshold,
            download_concurrency: config.download_concurrency,
            emoji_enabled: config.emoji_enabled,
            emoji_style: config.emoji_style,
            emoji_cdn_url: config.emoji_cdn_url.clone(),
        }
    }
}

impl Default for RenderOptions {
    /// Derived from [`Config::default`] so renderer defaults can never drift
    /// from the config serde defaults.
    fn default() -> Self {
        Self::from(&Config::default())
    }
}

/// Renderer trait for output generation
#[async_trait]
pub trait Renderer: Send + Sync {
    /// Render a parse result into deliverable outputs (single messages and
    /// grouped bundles).
    async fn render(
        &self,
        result: &ParseResult,
        downloader: &SharedDownloader,
    ) -> RenderResult<Vec<RenderOutput>>;
}

/// Get the appropriate renderer based on config.
pub fn get_renderer(config: &Config, data_path: PathBuf) -> Box<dyn Renderer> {
    let options = RenderOptions::from(config);
    match config.render_type {
        RenderType::Text => Box::new(text::TextRenderer::new(options)),
        RenderType::Image => {
            let emoji_config = image::emoji::EmojiConfig::from_config(config, &data_path);
            Box::new(image::ImageRenderer::with_emoji_config(
                data_path,
                options,
                emoji_config,
            ))
        }
    }
}
