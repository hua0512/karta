use std::path::PathBuf;
use std::time::Duration;

use image::{RgbaImage, imageops};
use tokio::fs;
use tracing::{debug, warn};

use crate::cache::Cache;

use super::config::EmojiConfig;
use super::detection::emoji_to_cdn_codepoints;

/// Upper bound on in-memory emoji images (LRU-evicted beyond this; evicted
/// entries reload from the disk cache).
const MEMORY_CACHE_CAPACITY: usize = 128;

/// Connect timeout for emoji CDN fetches. The default CDNs may be blackholed
/// (hang, not refuse) in some regions, so an unbounded client would stall the
/// whole card render.
const EMOJI_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Total per-request timeout for a single emoji CDN fetch.
const EMOJI_FETCH_TIMEOUT: Duration = Duration::from_secs(10);

/// Upper bound on remembered failed codepoint fetches.
const NEGATIVE_CACHE_CAPACITY: usize = 256;

/// How long a failed codepoint fetch is remembered before the CDN is retried.
const NEGATIVE_CACHE_TTL: Duration = Duration::from_secs(10 * 60);

/// Emoji source for fetching and caching emoji images
pub struct EmojiSource {
    config: EmojiConfig,
    client: wreq::Client,
    /// Bounded in-memory LRU cache of loaded emoji images
    cache: Cache<String, RgbaImage>,
    /// Negative cache of codepoints whose CDN fetch recently failed, so a
    /// dead CDN is not re-hit on every message.
    failed: Cache<String, ()>,
}

impl EmojiSource {
    /// Create a new emoji source with the given configuration
    pub fn new(config: EmojiConfig) -> Self {
        let client = wreq::Client::builder()
            .connect_timeout(EMOJI_CONNECT_TIMEOUT)
            .timeout(EMOJI_FETCH_TIMEOUT)
            .build()
            .unwrap_or_else(|e| {
                warn!("Failed to build emoji HTTP client with timeouts: {e}");
                wreq::Client::new()
            });
        Self::with_client(config, client)
    }

    /// Create a new emoji source with a custom HTTP client
    pub fn with_client(config: EmojiConfig, client: wreq::Client) -> Self {
        Self {
            config,
            client,
            cache: Cache::new(MEMORY_CACHE_CAPACITY),
            failed: Cache::with_ttl(NEGATIVE_CACHE_CAPACITY, NEGATIVE_CACHE_TTL),
        }
    }

    /// Check if emoji rendering is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// The emoji configuration in effect for this source
    pub fn config(&self) -> &EmojiConfig {
        &self.config
    }

    /// Get the emoji image for a given emoji string
    ///
    /// Returns None if the emoji cannot be fetched or loaded
    pub async fn get_emoji_image(&self, emoji: &str) -> Option<RgbaImage> {
        if !self.config.enabled {
            return None;
        }

        let codepoints = emoji_to_cdn_codepoints(emoji, self.config.style);

        if let Some(img) = self.cache.get_cloned(&codepoints) {
            return Some(img);
        }

        // A recently failed fetch means the CDN is unreachable or lacks this
        // codepoint; skip further lookups until the entry expires. (Checked
        // before the disk cache: a failed fetch never wrote a disk entry, so
        // in normal operation the two cannot both hold the same codepoint.)
        if self.failed.contains_key(&codepoints) {
            return None;
        }

        let cache_path = self.get_cache_path(&codepoints);
        if cache_path.exists()
            && let Ok(img) = image::open(&cache_path)
        {
            let rgba = img.to_rgba8();
            self.cache.insert(codepoints.clone(), rgba.clone());
            return Some(rgba);
        }

        match self.fetch_emoji(&codepoints).await {
            Ok(img) => {
                self.cache.insert(codepoints, img.clone());
                Some(img)
            }
            Err(e) => {
                debug!("Failed to fetch emoji {}: {}", emoji, e);
                self.failed.insert(codepoints, ());
                None
            }
        }
    }

    /// Get emoji image resized to the specified size
    pub async fn get_emoji_image_sized(&self, emoji: &str, size: u32) -> Option<RgbaImage> {
        let img = self.get_emoji_image(emoji).await?;

        if img.width() == size && img.height() == size {
            Some(img)
        } else {
            Some(imageops::resize(
                &img,
                size,
                size,
                imageops::FilterType::Lanczos3,
            ))
        }
    }

    /// Clear the in-memory cache
    pub fn clear_memory_cache(&self) {
        self.cache.clear();
    }

    /// Clear both memory and disk cache
    pub async fn clear_all_cache(&self) -> Result<(), String> {
        self.clear_memory_cache();

        if self.config.cache_dir.exists() {
            fs::remove_dir_all(&self.config.cache_dir)
                .await
                .map_err(|e| format!("Failed to remove cache dir: {}", e))?;
        }

        Ok(())
    }

    fn get_cache_path(&self, codepoints: &str) -> PathBuf {
        self.config.source_cache_dir().join(format!(
            "{}{}",
            codepoints,
            self.config.style.extension()
        ))
    }

    async fn fetch_emoji(&self, codepoints: &str) -> Result<RgbaImage, String> {
        if let Some(custom_url) = &self.config.custom_cdn_url {
            let url = format!(
                "{}/{}{}",
                custom_url,
                codepoints,
                self.config.style.extension()
            );
            return self.fetch_from_url(&url, codepoints).await;
        }

        let fallback_urls = self.config.style.fallback_cdn_urls();
        let mut last_error = String::new();

        for base_url in fallback_urls {
            let url = format!(
                "{}/{}{}",
                base_url,
                codepoints,
                self.config.style.extension()
            );

            match self.fetch_from_url(&url, codepoints).await {
                Ok(img) => return Ok(img),
                Err(e) => {
                    debug!("CDN {} failed for {}: {}", base_url, codepoints, e);
                    last_error = e;
                }
            }
        }

        Err(format!("All CDNs failed: {}", last_error))
    }

    async fn fetch_from_url(&self, url: &str, codepoints: &str) -> Result<RgbaImage, String> {
        debug!("Fetching emoji from: {}", url);

        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("HTTP error: {}", response.status()));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        // Decode before caching so a non-image 200 (e.g. a captive portal
        // page) is never persisted.
        let img = image::load_from_memory(&bytes)
            .map_err(|e| format!("Failed to decode image: {}", e))?;

        if let Err(e) = self.save_to_cache(codepoints, &bytes).await {
            warn!("Failed to cache emoji: {}", e);
        }

        Ok(img.to_rgba8())
    }

    async fn save_to_cache(&self, codepoints: &str, data: &[u8]) -> Result<(), String> {
        fs::create_dir_all(self.config.source_cache_dir())
            .await
            .map_err(|e| format!("Failed to create cache dir: {}", e))?;

        let cache_path = self.get_cache_path(codepoints);
        fs::write(&cache_path, data)
            .await
            .map_err(|e| format!("Failed to write cache file: {}", e))?;

        Ok(())
    }
}
