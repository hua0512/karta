use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::cache::Cache;
use crate::config::Config;
use crate::constants::Platform;
use crate::data::ParseResult;
use crate::download::StreamDownloader;
use crate::error::{ParserError, Result};
use crate::message::RenderOutput;
use crate::parser::{Parser, ParserRegistry};
use crate::render::{Renderer, get_renderer};

/// How long a cached [`ParseResult`] stays valid.
///
/// Results embed signed CDN stream URLs that expire within minutes, so the
/// TTL must stay short enough that cache hits never serve dead links.
const PARSE_CACHE_TTL: Duration = Duration::from_secs(10 * 60);

/// Main parser engine that coordinates parsing, downloading, and rendering
pub struct ParserEngine {
    config: Config,
    downloader: Arc<StreamDownloader>,
    registry: ParserRegistry,
    renderer: Box<dyn Renderer>,
    cache: Cache<String, Arc<ParseResult>>,
}

impl ParserEngine {
    /// Create a new parser engine with externally-provided parsers and downloader.
    pub fn with_parsers(
        config: Config,
        data_path: PathBuf,
        downloader: Arc<StreamDownloader>,
        parsers: Vec<Arc<dyn Parser>>,
    ) -> Self {
        let registry = ParserRegistry::new(parsers);
        let renderer = get_renderer(&config, data_path);
        let cache = Cache::with_ttl(config.cache_size, PARSE_CACHE_TTL);

        Self {
            config,
            downloader,
            registry,
            renderer,
            cache,
        }
    }

    /// Create a new parser engine with no parsers registered.
    ///
    /// Useful for testing or when parsers will be provided via `with_parsers`.
    pub fn new(config: Config, data_path: PathBuf) -> Result<Self> {
        let downloader = Arc::new(StreamDownloader::new(&config, &data_path)?);
        Ok(Self::with_parsers(
            config,
            data_path,
            downloader,
            Vec::new(),
        ))
    }

    /// Replace the parse cache with an empty one whose entries expire after
    /// `ttl` (the default is ten minutes).
    #[doc(hidden)]
    pub fn with_parse_cache_ttl(mut self, ttl: Duration) -> Self {
        self.cache = Cache::with_ttl(self.config.cache_size, ttl);
        self
    }

    /// Try to parse a URL from the given text
    ///
    /// Returns `None` if no matching pattern is found. The result is shared
    /// with the internal cache; deref coercion keeps `&ParseResult` callers
    /// working.
    pub async fn parse(&self, text: &str) -> Result<Option<Arc<ParseResult>>> {
        // Try to match any parser pattern
        let Some((parser, match_result)) = self.registry.search(text) else {
            return Ok(None);
        };

        let cache_key = match_result.full_match.to_string();
        tracing::debug!(
            platform = %parser.platform(),
            pattern = match_result.pattern,
            matched = %cache_key,
            "Pattern matched"
        );

        // Check cache first
        if let Some(cached) = self.cache.get_cloned(&cache_key) {
            tracing::debug!(key = %cache_key, "Cache hit");
            return Ok(Some(cached));
        }

        // Parse the content
        tracing::debug!(key = %cache_key, "Cache miss, parsing");
        let parse_future = parser.parse(match_result);
        let result = if self.config.parse_timeout > 0 {
            tokio::time::timeout(Duration::from_secs(self.config.parse_timeout), parse_future)
                .await
                .map_err(|_| {
                    tracing::warn!(
                        platform = %parser.platform(),
                        key = %cache_key,
                        timeout = self.config.parse_timeout,
                        "Parse timed out"
                    );
                    ParserError::Timeout {
                        seconds: self.config.parse_timeout,
                    }
                })??
        } else {
            parse_future.await?
        };
        tracing::debug!(
            platform = %result.platform,
            title = result.title.as_deref().unwrap_or(""),
            contents = result.contents.len(),
            "Parse complete"
        );

        // Store in cache
        let result = Arc::new(result);
        self.cache.insert(cache_key, Arc::clone(&result));

        Ok(Some(result))
    }

    /// Render a parse result into deliverable outputs
    pub async fn render(&self, result: &ParseResult) -> Result<Vec<RenderOutput>> {
        tracing::debug!(
            platform = %result.platform,
            contents = result.contents.len(),
            "Rendering"
        );
        let messages = self.renderer.render(result, &self.downloader).await?;
        Ok(messages)
    }

    /// Parse and render in one call
    pub async fn parse_and_render(&self, text: &str) -> Result<Option<Vec<RenderOutput>>> {
        match self.parse(text).await? {
            Some(result) => Ok(Some(self.render(&result).await?)),
            None => Ok(None),
        }
    }

    /// Check whether any parser pattern matches, without extracting captures.
    pub fn would_match(&self, text: &str) -> bool {
        self.registry.is_match(text)
    }

    /// Identify which platform would handle `text`, without extracting captures.
    pub fn match_platform(&self, text: &str) -> Option<Platform> {
        self.registry.matched_platform(text)
    }

    /// Check if FFmpeg is available
    pub async fn is_ffmpeg_available(&self) -> bool {
        self.downloader.ffmpeg().is_available().await
    }

    /// Get the configuration
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Get the downloader
    pub fn downloader(&self) -> &Arc<StreamDownloader> {
        &self.downloader
    }

    /// Clear the parse result cache
    pub fn clear_cache(&self) {
        self.cache.clear();
    }
}
