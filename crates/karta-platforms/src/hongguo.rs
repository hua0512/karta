//! Parser for `hongguoduanju.com` (红果短剧 / Hongguo short dramas).
//!
//! The public web frontend renders SSR HTML that embeds a
//! `window._ROUTER_DATA = {...};` blob. That blob holds the full series
//! metadata and, on the `/player/{series_id}/{vid}` route, the actual media
//! URL plus its duration and dimensions. This parser scans out that JSON and
//! builds a [`ParseResult`] — a full video for player URLs, or an info card
//! (图文) for detail URLs.
//!
//! The `/comic/{book_id}` route hosts motion comics (漫剧). Its SSR payload
//! carries the readable chapter's page images, which render as a 图文 gallery
//! and always work anonymously. The real motion-comic *video* lives behind the
//! ByteDance reading app's `multi_video_model` API, which risk-control gates
//! anonymous callers — so it is only attempted when `hongguo_api_auth` supplies
//! an authorized session, and any failure falls back to the gallery.

use async_trait::async_trait;

use wreq::header::HeaderMap;

use karta_core::Platform;
use karta_core::config::HongguoApiAuth;
use karta_core::constants::DEFAULT_USER_AGENT;
use karta_core::data::{Content, GraphicsContent, ImageContent, ParseResult, VideoContent};
use karta_core::download::SharedDownloader;
use karta_core::error::{ParserError, Result};
use karta_core::http;
use karta_core::parser::{MatchResult, Parser, RegisteredPattern};
use karta_core::patterns;

mod motion_comic;
mod spade;

/// Upper bound on the page images sent for one motion-comic chapter. Chapters
/// can run to hundreds of pages, which would swamp a chat message.
const MAX_COMIC_IMAGES: usize = 50;

/// Hongguo short-drama parser.
pub struct HongguoParser {
    downloader: SharedDownloader,
    /// Optional authorized app session for motion-comic video resolution.
    api_auth: Option<HongguoApiAuth>,
    patterns: Vec<RegisteredPattern>,
}

impl HongguoParser {
    pub fn new(downloader: SharedDownloader, api_auth: Option<HongguoApiAuth>) -> Self {
        // Comic first, then player, then detail: the routes are disjoint, but
        // registration order decides RegexSet precedence so keep the most
        // specific route in front.
        let patterns = patterns![
            "hongguo_comic" => r"hongguoduanju\.com/comic/(\d+)",
            "hongguo_player" => r"hongguoduanju\.com/player/(\d+)/(\d+)",
            "hongguo_detail" => r"hongguoduanju\.com/detail(?:\?series_id=|/)(\d+)",
        ];

        Self {
            downloader,
            api_auth,
            patterns,
        }
    }

    fn get_headers(&self) -> HeaderMap {
        http::headers_with_referer(DEFAULT_USER_AGENT, "https://hongguoduanju.com/")
    }

    /// Resolve one episode's playable video from its public SSR payload.
    async fn parse_episode(&self, series_id: &str, vid: &str) -> Result<ParseResult> {
        let url = format!("https://hongguoduanju.com/player/{}/{}", series_id, vid);
        let client = self.downloader.client();

        let response = client.get(&url).headers(self.get_headers()).send().await?;
        let html = response.text().await?;

        let data = extract_router_data(&html)?;
        let loader = data
            .get("loaderData")
            .ok_or_else(|| ParserError::Parse("Hongguo loaderData missing".to_string()))?;

        // The player page lives under a literal route key (parens and slashes
        // are not interpolated).
        let page = match loader.get("player_(series_id)/(vid)/page") {
            Some(p) if p.get("isSuccess").and_then(|v| v.as_bool()) == Some(true) => p,
            _ => {
                return Err(ParserError::Parse(
                    "Hongguo episode is not publicly accessible".to_string(),
                ));
            }
        };

        let vpi = page.get("video_player_info").ok_or_else(|| {
            ParserError::Parse("Hongguo episode is not publicly accessible".to_string())
        })?;

        // Prefer main_url; fall back to backup_url when main is missing/empty.
        let chosen_url = vpi
            .get("main_url")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                vpi.get("backup_url")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
            })
            .ok_or_else(|| {
                ParserError::Parse("Hongguo episode is not publicly accessible".to_string())
            })?;

        let duration = vpi.get("duration").and_then(json_to_u64);
        let width = vpi.get("width").and_then(json_to_u64).map(|n| n as u32);
        let height = vpi.get("height").and_then(json_to_u64).map(|n| n as u32);
        let quality = definition_label(width, height);

        // seriesDetail lives under a different loaderData page; find it.
        let series_detail = find_series_detail(loader);
        let series_name = series_detail
            .and_then(|sd| sd.get("series_name"))
            .and_then(|v| v.as_str());
        let intro = series_detail
            .and_then(|sd| sd.get("series_intro"))
            .and_then(|v| v.as_str());
        let cover = pick_cover(vpi, series_detail);

        let mut video = VideoContent::new().with_url(chosen_url);
        if let Some(cover) = cover {
            video = video.with_cover(cover);
        }
        if let Some(dur) = duration {
            video = video.with_duration(dur);
        }
        if let (Some(w), Some(h)) = (width, height) {
            video = video.with_dimensions(w, h);
        }
        if let Some(q) = quality {
            video = video.with_quality(q);
        }

        let mut result = ParseResult::new(Platform::Hongguo).with_url(url);
        if let Some(name) = series_name {
            result = result.with_title(name);
        }
        if let Some(intro) = intro {
            result = result.with_text(intro);
        }
        result = result
            .with_content(Content::Video(video))
            .with_extra("series_id", series_id)
            .with_extra("vid", vid);

        Ok(result)
    }

    /// Resolve a series' info card (title, intro, cover) — no playable video.
    async fn parse_series_card(&self, series_id: &str) -> Result<ParseResult> {
        let url = format!("https://hongguoduanju.com/detail?series_id={}", series_id);
        let client = self.downloader.client();

        let response = client.get(&url).headers(self.get_headers()).send().await?;
        let html = response.text().await?;

        let data = extract_router_data(&html)?;
        let series_detail = data
            .get("loaderData")
            .and_then(find_series_detail)
            .ok_or_else(|| ParserError::Parse("Hongguo seriesDetail missing".to_string()))?;

        let name = series_detail.get("series_name").and_then(|v| v.as_str());
        let intro = series_detail.get("series_intro").and_then(|v| v.as_str());
        let cover = series_detail.get("series_cover").and_then(|v| v.as_str());
        let tags: Vec<&str> = series_detail
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|t| t.as_str()).collect())
            .unwrap_or_default();
        let episode_count = series_detail.get("episode_cnt").and_then(json_to_u64);
        let accessible_count = series_detail
            .get("accessible_episode_cnt")
            .and_then(json_to_u64);

        let mut result = ParseResult::new(Platform::Hongguo).with_url(url);
        if let Some(name) = name {
            result = result.with_title(name);
        }

        // Compose the body text from intro plus (optionally) the tag list.
        let mut text_parts: Vec<String> = Vec::new();
        if let Some(intro) = intro
            && !intro.is_empty()
        {
            text_parts.push(intro.to_string());
        }
        if !tags.is_empty() {
            text_parts.push(format!("标签: {}", tags.join(" / ")));
        }
        if !text_parts.is_empty() {
            result = result.with_text(text_parts.join("\n\n"));
        }

        // A cover image makes this render as a 图文 card.
        if let Some(cover) = cover {
            result = result.with_content(Content::Image(ImageContent::new().with_url(cover)));
        }

        result = result.with_extra("series_id", series_id);
        if let Some(count) = episode_count {
            result = result.with_extra("episode_count", count.to_string());
        }
        if let Some(count) = accessible_count {
            result = result.with_extra("accessible_episode_count", count.to_string());
        }

        Ok(result)
    }

    /// Resolve a motion comic (漫剧) from its public SSR payload.
    ///
    /// The readable chapter's page images always render, and are used as the
    /// fallback whenever the authorized video path is unavailable or fails.
    async fn parse_comic(&self, book_id: &str) -> Result<ParseResult> {
        let url = format!("https://hongguoduanju.com/comic/{}", book_id);
        let client = self.downloader.client();

        let response = client.get(&url).headers(self.get_headers()).send().await?;
        let html = response.text().await?;

        let data = extract_router_data(&html)?;
        let loader = data
            .get("loaderData")
            .ok_or_else(|| ParserError::Parse("Hongguo loaderData missing".to_string()))?;

        // Literal route key (same convention as `player_(series_id)/(vid)/page`).
        let page = match loader.get("comic_(id)/page") {
            Some(p) if p.get("isSuccess").and_then(|v| v.as_bool()) == Some(true) => p,
            _ => {
                return Err(ParserError::Parse(
                    "Hongguo comic is not publicly accessible".to_string(),
                ));
            }
        };

        let title = page
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let series_id = page
            .get("book_id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(book_id);
        let next_vid = page
            .get("next_vid")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let readable_count = page.get("can_read_episode_cnt").and_then(json_to_u64);

        // Optional upgrade: an authorized app session can resolve the real
        // (usually CENC-encrypted) motion-comic stream. The anonymous endpoint
        // is risk-control gated, so never touch it without credentials.
        if self.api_auth.is_some() && !next_vid.is_empty() {
            match self.resolve_motion_comic_video(series_id, next_vid).await {
                Ok(video) => {
                    let mut result = ParseResult::new(Platform::Hongguo)
                        .with_url(url.clone())
                        .with_content(Content::Video(video))
                        .with_extra("book_id", series_id)
                        .with_extra("vid", next_vid);
                    if !title.is_empty() {
                        result = result.with_title(title);
                    }
                    if let Some(count) = readable_count {
                        result = result.with_extra("readable_episode_count", count.to_string());
                    }
                    return Ok(result);
                }
                Err(e) => {
                    tracing::warn!(
                        book_id = series_id,
                        vid = next_vid,
                        error = %e,
                        "Hongguo motion-comic video unavailable; falling back to the image gallery"
                    );
                }
            }
        }

        let images = comic_gallery_images(&comic_content(page));
        if images.is_empty() {
            return Err(ParserError::ContentNotFound(
                "Hongguo comic has no readable pages".to_string(),
            ));
        }

        let total = images.len();
        let truncated = total > MAX_COMIC_IMAGES;
        let images: Vec<ImageContent> = images.into_iter().take(MAX_COMIC_IMAGES).collect();

        let mut graphics = GraphicsContent::new().with_images(images);
        if !title.is_empty() {
            graphics = graphics.with_title(title);
        }

        let mut text = format!("漫剧 · 共 {} 页", total);
        if truncated {
            text.push_str(&format!("，显示前 {} 页", MAX_COMIC_IMAGES));
        }

        let mut result = ParseResult::new(Platform::Hongguo).with_url(url);
        if !title.is_empty() {
            result = result.with_title(title);
        }
        result = result
            .with_text(text)
            .with_content(Content::Graphics(graphics))
            .with_extra("book_id", series_id);
        if !next_vid.is_empty() {
            result = result.with_extra("vid", next_vid);
        }
        if let Some(count) = readable_count {
            result = result.with_extra("readable_episode_count", count.to_string());
        }

        Ok(result)
    }

    /// Resolve one motion-comic episode's real stream through the authorized
    /// `multi_video_model` API.
    ///
    /// Only meaningful when `hongguo_api_auth` is configured; every failure
    /// mode is an `Err` so [`Self::parse_comic`] can fall back to the gallery.
    async fn resolve_motion_comic_video(&self, series_id: &str, vid: &str) -> Result<VideoContent> {
        let auth = self.api_auth.as_ref().ok_or_else(|| {
            ParserError::Parse(
                "Hongguo motion-comic video resolution requires hongguo_api_auth".to_string(),
            )
        })?;

        motion_comic::resolve(self.downloader.client(), auth, series_id, vid).await
    }
}

#[async_trait]
impl Parser for HongguoParser {
    fn platform(&self) -> Platform {
        Platform::Hongguo
    }

    fn patterns(&self) -> &[RegisteredPattern] {
        &self.patterns
    }

    async fn parse(&self, match_result: MatchResult<'_>) -> Result<ParseResult> {
        tracing::debug!(
            pattern = match_result.pattern,
            matched = match_result.full_match,
            "Hongguo: parsing"
        );

        match match_result.pattern {
            "hongguo_comic" => {
                let book_id = match_result.group(1).unwrap_or("");
                self.parse_comic(book_id).await
            }
            "hongguo_player" => {
                let series_id = match_result.group(1).unwrap_or("");
                let vid = match_result.group(2).unwrap_or("");
                self.parse_episode(series_id, vid).await
            }
            "hongguo_detail" => {
                let series_id = match_result.group(1).unwrap_or("");
                self.parse_series_card(series_id).await
            }
            other => Err(ParserError::PatternNotMatched(format!(
                "Unknown pattern: {}",
                other
            ))),
        }
    }
}

/// Extract the `window._ROUTER_DATA` JSON object from an SSR page.
///
/// Delegates to the shared string/escape-aware brace scanner in
/// [`karta_core::extract`] (which was promoted from this parser).
fn extract_router_data(html: &str) -> Result<serde_json::Value> {
    let raw =
        karta_core::extract::extract_json_after(html, "window._ROUTER_DATA").ok_or_else(|| {
            ParserError::Parse("page did not embed a window._ROUTER_DATA object".to_string())
        })?;

    serde_json::from_str(raw)
        .map_err(|e| ParserError::Parse(format!("invalid _ROUTER_DATA JSON: {}", e)))
}

/// Un-nest a comic page's `content` field.
///
/// Hongguo serializes the chapter list as a JSON string inside the router
/// payload, so it needs a second `from_str`. A malformed or absent value
/// yields `Null`, which the caller treats as "no pages".
fn comic_content(page: &serde_json::Value) -> serde_json::Value {
    let raw = page.get("content").and_then(|v| v.as_str()).unwrap_or("{}");
    serde_json::from_str(raw).unwrap_or(serde_json::Value::Null)
}

/// Collect the page images of the first chapter that actually has any.
///
/// Locked chapters ship with an empty `pic_list`, so the first non-empty one is
/// the readable chapter.
fn comic_gallery_images(content: &serde_json::Value) -> Vec<ImageContent> {
    let Some(chapters) = content.get("chapter_info_list").and_then(|v| v.as_array()) else {
        return Vec::new();
    };

    for chapter in chapters {
        let images: Vec<ImageContent> = chapter
            .get("pic_list")
            .and_then(|v| v.as_array())
            .map(|pics| pics.iter().filter_map(comic_page_image).collect())
            .unwrap_or_default();
        if !images.is_empty() {
            return images;
        }
    }
    Vec::new()
}

/// Build one gallery image, preferring the full-resolution `pic_url`.
fn comic_page_image(pic: &serde_json::Value) -> Option<ImageContent> {
    let url = ["pic_url", "low_pic_url"]
        .into_iter()
        .filter_map(|key| pic.get(key).and_then(|v| v.as_str()))
        .find(|value| !value.is_empty())?;

    let mut image = ImageContent::new().with_url(url);
    let width = pic.get("width").and_then(json_to_u64).map(|n| n as u32);
    let height = pic.get("height").and_then(json_to_u64).map(|n| n as u32);
    if let (Some(width), Some(height)) = (width, height)
        && width > 0
        && height > 0
    {
        image = image.with_dimensions(width, height);
    }
    Some(image)
}

/// Find the first `loaderData` page value that carries a `seriesDetail`,
/// returning that nested `seriesDetail` object.
fn find_series_detail(loader: &serde_json::Value) -> Option<&serde_json::Value> {
    for value in loader.as_object()?.values() {
        if value.is_object()
            && let Some(detail) = value.get("seriesDetail")
        {
            return Some(detail);
        }
    }
    None
}

/// Pick the best cover for an episode: the per-episode `poster_url` from the
/// player payload (observed on live pages), falling back to the series cover.
fn pick_cover<'a>(
    vpi: &'a serde_json::Value,
    series_detail: Option<&'a serde_json::Value>,
) -> Option<&'a str> {
    vpi.get("poster_url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            series_detail
                .and_then(|sd| sd.get("series_cover"))
                .and_then(|v| v.as_str())
        })
}

/// Best-effort conversion of a JSON number (integer or float) to `u64`.
fn json_to_u64(value: &serde_json::Value) -> Option<u64> {
    value.as_u64().or_else(|| value.as_f64().map(|f| f as u64))
}

/// Label a stream by its pixel size.
///
/// Uses the short side, so portrait video gets the same label as its landscape
/// equivalent (720x1280 is 720p), matching the `definition` Hongguo reports.
/// Only produces a label when both width and height are known.
fn definition_label(width: Option<u32>, height: Option<u32>) -> Option<String> {
    match (width, height) {
        (Some(w), Some(h)) => {
            let short = w.min(h);
            Some(if short >= 1080 {
                "1080p".to_string()
            } else if short >= 720 {
                "720p".to_string()
            } else {
                format!("{}p", short)
            })
        }
        _ => None,
    }
}
