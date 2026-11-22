//! Advanced image card renderer
//!
//! This module provides a full-featured image renderer that creates
//! visually appealing cards for parsed social media content.

mod backdrop;
mod constants;
mod controls;
mod corners;
mod draw;
pub mod emoji;
mod font;
mod metadata;
mod palette;
mod processing;
mod sections;
mod shadow;
mod text;
mod theme;
mod types;
mod utils;

use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures::{StreamExt, stream};
use image::{ImageFormat, RgbaImage};
use parking_lot::Mutex;

use crate::constants::Platform;
use crate::data::{Content, ParseResult};
use crate::download::SharedDownloader;
use crate::error::RenderResult;
use crate::message::{MessageOutput, MessageSegment, RenderOutput};

use self::constants::*;
use self::draw::draw_sections;
use self::emoji::{EmojiConfig, EmojiSource};
use self::sections::{calculate_sections, calculate_total_height};
use self::text::EmojiImageMap;
use self::types::*;
use self::utils::*;

use super::{RenderOptions, Renderer, hydrate_media, save_png_to_cache};

/// Global resources (loaded once)
static RESOURCES: OnceLock<RenderResources> = OnceLock::new();

/// Concurrent emoji CDN fetches per card render.
const EMOJI_FETCH_CONCURRENCY: usize = 4;

/// Cap on the total emoji prefetch time per card. A blackholed CDN (hangs
/// instead of refusing) must not gate the render: on expiry the card is drawn
/// with whatever fetched in time, text glyphs covering the rest.
const EMOJI_PREFETCH_TIMEOUT: Duration = Duration::from_secs(10);

/// Advanced image card renderer
pub struct ImageRenderer {
    data_path: PathBuf,
    options: RenderOptions,
    emoji_source: EmojiSource,
}

impl ImageRenderer {
    /// Create a new image renderer.
    ///
    /// Emoji settings (enabled flag, style, and custom CDN URL) come from
    /// `options`; use [`Self::with_emoji_config`] for full control over the
    /// emoji configuration (e.g. a custom cache directory).
    pub fn new(data_path: PathBuf, options: RenderOptions) -> Self {
        let emoji_config = EmojiConfig {
            style: options.emoji_style.into(),
            custom_cdn_url: options.emoji_cdn_url.clone(),
            cache_dir: EmojiConfig::default_cache_dir(&data_path),
            enabled: options.emoji_enabled,
        };
        Self::with_emoji_config(data_path, options, emoji_config)
    }

    /// Create a new image renderer with a full emoji configuration.
    pub fn with_emoji_config(
        data_path: PathBuf,
        options: RenderOptions,
        emoji_config: EmojiConfig,
    ) -> Self {
        // Initialize resources on first use
        let _ = RESOURCES.get_or_init(|| load_resources(&data_path));
        Self {
            data_path,
            options,
            emoji_source: EmojiSource::new(emoji_config),
        }
    }

    /// Render a parse result to a PNG image
    async fn render_card(
        &self,
        result: &ParseResult,
        _downloader: &SharedDownloader,
    ) -> RenderResult<Vec<u8>> {
        self.render_card_with(result, |image| {
            let mut buffer = Cursor::new(Vec::new());
            // Cards are fully opaque: encode as RGB8 to skip compressing a constant alpha channel.
            image::DynamicImage::ImageRgba8(image)
                .into_rgb8()
                .write_to(&mut buffer, ImageFormat::Png)?;
            Ok(buffer.into_inner())
        })
        .await
    }

    /// Draw an already hydrated result into a pixel buffer, without PNG encoding.
    ///
    /// Media must have local paths. Emoji fetching follows the renderer options.
    pub async fn render_card_image(&self, result: &ParseResult) -> RenderResult<RgbaImage> {
        self.render_card_with(result, Ok).await
    }

    /// Resolve the media needed by the card using the normal downloader and cache.
    pub async fn prepare_card(
        &self,
        result: &ParseResult,
        downloader: &SharedDownloader,
    ) -> RenderResult<ParseResult> {
        hydrate_media(result.clone(), downloader, &self.options).await
    }

    async fn render_card_with<T, F>(&self, result: &ParseResult, finish: F) -> RenderResult<T>
    where
        T: Send + 'static,
        F: FnOnce(RgbaImage) -> RenderResult<T> + Send + 'static,
    {
        let resources = RESOURCES.get().expect("Resources not initialized");

        // Fetch emoji images up front (network I/O) so the blocking draw
        // path below stays purely synchronous.
        let emoji_images = self.collect_emoji_images(result).await;

        // CPU-bound image creation + PNG encoding: keep it off the async executor.
        let result = result.clone();
        let header_style = self.options.image_header_style;
        let theme_mode = self.options.image_theme_mode;
        tokio::task::spawn_blocking(move || {
            let img = create_card_image_sync(
                &result,
                resources,
                true,
                &emoji_images,
                header_style,
                theme_mode,
            )?;

            finish(img)
        })
        .await
        .map_err(|e| crate::error::RenderError::Other(e.to_string()))?
    }

    /// Prefetch emoji images for every emoji appearing in card text.
    ///
    /// Any failed fetch simply leaves the emoji out of the map, degrading to
    /// the old text-glyph rendering for that emoji. The whole prefetch is
    /// bounded by [`EMOJI_PREFETCH_TIMEOUT`]; on expiry the partial map is
    /// returned so the card still renders.
    async fn collect_emoji_images(&self, result: &ParseResult) -> EmojiImageMap {
        if !self.emoji_source.is_enabled() {
            return EmojiImageMap::new();
        }

        // Without the `emoji` cargo feature `collect_unique_emojis` finds
        // nothing, so the pipeline short-circuits here.
        let emojis = collect_unique_emojis(result);
        if emojis.is_empty() {
            return EmojiImageMap::new();
        }

        // Completed fetches land in the map incrementally so a timeout still
        // keeps everything that resolved in time.
        let map = Mutex::new(EmojiImageMap::new());
        let source = &self.emoji_source;
        let prefetch = stream::iter(emojis.into_iter().map(|emoji| async move {
            let image = source.get_emoji_image(&emoji).await;
            (emoji, image)
        }))
        .buffer_unordered(EMOJI_FETCH_CONCURRENCY)
        .for_each(|(emoji, image)| {
            if let Some(image) = image {
                map.lock().insert(emoji, image);
            }
            std::future::ready(())
        });

        if tokio::time::timeout(EMOJI_PREFETCH_TIMEOUT, prefetch)
            .await
            .is_err()
        {
            tracing::warn!(
                "Emoji prefetch timed out after {EMOJI_PREFETCH_TIMEOUT:?}; \
                 rendering card with partial emoji set"
            );
        }
        map.into_inner()
    }
}

/// Collect every text the card will draw (see `sections.rs`): author name,
/// title, body text, extra info, graphics captions, and the repost's texts.
fn collect_card_texts(result: &ParseResult, texts: &mut Vec<String>) {
    if let Some(author) = &result.author {
        texts.push(author.name.clone());
    }
    if let Some(title) = &result.title {
        texts.push(title.clone());
    }
    if let Some(text) = &result.text {
        texts.push(text.clone());
    }
    for (key, value) in &result.extra {
        if super::extra::is_hidden_key(key)
            || (key == "stats" && metadata::parses_as_metrics(value))
        {
            continue;
        }
        texts.push(format!("{}: {}", key, value));
    }
    for content in &result.contents {
        if let Content::Graphics(graphics) = content {
            if let Some(description) = &graphics.description {
                texts.push(description.clone());
            }
            if let Some(alt_text) = &graphics.alt_text {
                texts.push(alt_text.clone());
            }
        }
    }
    if let Some(repost) = &result.repost {
        collect_card_texts(repost, texts);
    }
}

/// Deduplicated emojis across all card texts, in first-seen order.
fn collect_unique_emojis(result: &ParseResult) -> Vec<String> {
    let mut texts = Vec::new();
    collect_card_texts(result, &mut texts);

    let mut emojis: Vec<String> = Vec::new();
    for text in &texts {
        for found in emoji::find_emojis(text) {
            if !emojis.contains(&found.emoji) {
                emojis.push(found.emoji);
            }
        }
    }
    emojis
}

fn create_card_image_sync(
    result: &ParseResult,
    resources: &RenderResources,
    is_main: bool,
    emoji_images: &EmojiImageMap,
    header_style: crate::config::ImageHeaderStyle,
    theme_mode: crate::config::ImageThemeMode,
) -> RenderResult<RgbaImage> {
    let card_width = CARD_WIDTH;
    let content_width = card_width - 2 * PADDING;

    // Calculate sections
    let stage_start = Instant::now();
    let sections = calculate_sections(result, content_width, resources, !is_main);
    let layout_time = stage_start.elapsed();

    // Calculate total height
    let brand_height = if is_main && result.author.is_none() {
        BRAND_HEADER_HEIGHT
    } else {
        0
    };
    let card_height = calculate_total_height(&sections)
        + brand_height
        + if is_main { SHADOW_BOTTOM_PADDING } else { 0 };

    // Create canvas
    let stage_start = Instant::now();
    let theme = palette::CardTheme::from_sections(result.platform, &sections, theme_mode);
    let palette_time = stage_start.elapsed();
    let bg_color = if is_main { theme.page } else { REPOST_BG_COLOR };
    let mut img = RgbaImage::from_pixel(card_width, card_height, bg_color);
    let stage_start = Instant::now();
    if is_main {
        theme::frame(
            &mut img,
            result.platform,
            &sections,
            resources,
            emoji_images,
            &theme,
            header_style.resolve(theme.seed),
        );
    }
    let frame_time = stage_start.elapsed();

    // Create render context
    let mut ctx = RenderContext::new(card_width, is_main);
    ctx.y_pos += brand_height;
    ctx.accent = theme.accent;
    ctx.is_dark = theme.is_dark;
    if theme.is_dark {
        ctx.extra_fill = backdrop::DARK_MUTED_TEXT;
    } else if header_style == crate::config::ImageHeaderStyle::BlurBackground {
        ctx.extra_fill = backdrop::MUTED_TEXT;
    }

    // Draw all sections
    let stage_start = Instant::now();
    draw_sections(&mut img, &sections, &mut ctx, resources, emoji_images);
    let draw_time = stage_start.elapsed();

    // The paper canvas is opaque. Floating-point compositing can round a few
    // fully composited alpha values down to 254; keep the finished card opaque.
    for pixel in img.pixels_mut() {
        pixel[3] = 255;
    }

    tracing::debug!(
        layout_us = layout_time.as_micros() as u64,
        palette_us = palette_time.as_micros() as u64,
        frame_us = frame_time.as_micros() as u64,
        draw_us = draw_time.as_micros() as u64,
        width = card_width,
        height = card_height,
        "Native card stages"
    );

    Ok(img)
}

#[async_trait]
impl Renderer for ImageRenderer {
    async fn render(
        &self,
        result: &ParseResult,
        downloader: &SharedDownloader,
    ) -> RenderResult<Vec<RenderOutput>> {
        let mut messages = Vec::new();

        let hydrated = self.prepare_card(result, downloader).await?;

        // Render card image
        match self.render_card(&hydrated, downloader).await {
            Ok(png_data) => {
                // Save to cache and send as file
                match save_png_to_cache(&png_data, &hydrated, &self.data_path).await {
                    Ok(path) => {
                        messages.push(RenderOutput::Message(MessageOutput::from_segment(
                            MessageSegment::image_file(path),
                        )));
                    }
                    Err(e) => {
                        tracing::warn!("Failed to save card to cache: {}", e);
                        // Fallback to text renderer
                        let text_renderer = super::text::TextRenderer::new(self.options.clone());
                        return text_renderer.render(&hydrated, downloader).await;
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to render card: {}", e);
                // Fallback to text renderer
                let text_renderer = super::text::TextRenderer::new(self.options.clone());
                return text_renderer.render(&hydrated, downloader).await;
            }
        }

        if self.options.append_url
            && let Some(url) = &hydrated.url
        {
            messages.push(RenderOutput::Message(MessageOutput::from_segment(
                MessageSegment::text(url),
            )));
        }

        // Also send media files
        if self.options.download_media {
            super::append_media_messages(&mut messages, &hydrated, downloader, &self.options).await;
        }

        if self.options.use_base64 {
            for output in &mut messages {
                for msg in output.messages_mut() {
                    for seg in &mut msg.segments {
                        let _ = seg.to_base64().await;
                    }
                }
            }
        }

        Ok(messages)
    }
}

/// Load all resources (fonts and logos)
fn load_resources(data_path: &Path) -> RenderResources {
    // Load font
    let font = load_font(data_path);
    let fonts = FontSet::new(font);
    let repost_fonts = fonts.repost();

    // Load platform logos (pre-scaled to header bounds)
    let logos = load_logos(data_path);

    RenderResources {
        fonts,
        repost_fonts,
        logos,
    }
}

/// Load the font: an override under `data_path` or the working directory,
/// falling back directly to the embedded copy.
fn load_font(data_path: &Path) -> font::Font {
    let candidates = [
        data_path
            .join("resources/fonts")
            .join(DEFAULT_FONT_FILENAME),
        PathBuf::from("resources/fonts").join(DEFAULT_FONT_FILENAME),
    ];
    load_first_font(&candidates)
}

/// The first candidate that reads and parses as a font, else the embedded
/// font.
fn load_first_font(candidates: &[PathBuf]) -> font::Font {
    for path in candidates {
        if let Ok(font_data) = std::fs::read(path)
            && let Some(font) = font::Font::new(font_data)
        {
            tracing::info!("Loaded font from {:?}", path);
            return font;
        }
    }

    tracing::info!("Using embedded font");
    font::Font::new(
        include_bytes!("../../../../resources/fonts/HYSongYunLangHeiW-1.ttf").as_slice(),
    )
    .expect("Failed to load embedded font")
}

/// Load platform logos from disk, falling back to embedded copies for bundled platforms.
fn load_logos(data_path: &Path) -> HashMap<Platform, RgbaImage> {
    let mut logos = HashMap::new();

    let logo_dirs = [
        data_path.join("resources/logos"),
        PathBuf::from("resources/logos"),
    ];

    for logo_dir in &logo_dirs {
        for &platform in Platform::ALL {
            if logos.contains_key(&platform) {
                continue;
            }
            let logo_path = logo_dir.join(format!("{}.png", platform.slug()));
            if let Some(img) = load_image(&logo_path) {
                logos.insert(platform, img);
            }
        }
    }

    let embedded: [(Platform, &[u8]); 10] = [
        (
            Platform::Bilibili,
            include_bytes!("../../../../resources/logos/bilibili.png"),
        ),
        (
            Platform::Douyin,
            include_bytes!("../../../../resources/logos/douyin.png"),
        ),
        (
            Platform::Instagram,
            include_bytes!("../../../../resources/logos/instagram.png"),
        ),
        (
            Platform::Kuaishou,
            include_bytes!("../../../../resources/logos/kuaishou.png"),
        ),
        (
            Platform::TikTok,
            include_bytes!("../../../../resources/logos/tiktok.png"),
        ),
        (
            Platform::Twitter,
            include_bytes!("../../../../resources/logos/twitter.png"),
        ),
        (
            Platform::Weibo,
            include_bytes!("../../../../resources/logos/weibo.png"),
        ),
        (
            Platform::Xiaoheihe,
            include_bytes!("../../../../resources/logos/xiaoheihe.png"),
        ),
        (
            Platform::Xiaohongshu,
            include_bytes!("../../../../resources/logos/xiaohongshu.png"),
        ),
        (
            Platform::YouTube,
            include_bytes!("../../../../resources/logos/youtube.png"),
        ),
    ];

    for (platform, bytes) in embedded {
        logos.entry(platform).or_insert_with(|| {
            image::load_from_memory(bytes)
                .expect("bundled platform logo must be a valid PNG")
                .into_rgba8()
        });
    }

    // Pre-scale oversized logos (e.g. high-res icons or disk overrides) once at startup so
    // per-card rendering avoids repeated large downscales and extra memory retention.
    for logo in logos.values_mut() {
        let scale =
            (116.0 / f64::from(logo.width().max(1))).min(30.0 / f64::from(logo.height().max(1)));
        if scale < 1.0 {
            let w = (f64::from(logo.width()) * scale).round().max(1.0) as u32;
            let h = (f64::from(logo.height()) * scale).round().max(1.0) as u32;
            *logo = processing::resize(logo, w, h, fast_image_resize::FilterType::Lanczos3);
        }
    }

    tracing::info!("Loaded {} platform logos", logos.len());
    logos
}
