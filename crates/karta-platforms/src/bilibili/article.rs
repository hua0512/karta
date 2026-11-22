//! Bilibili article request flow and result construction.

use std::sync::LazyLock;

use karta_core::Platform;
use karta_core::data::{Author, Content, ImageContent, ParseResult};
use karta_core::error::{ParserError, Result};
use karta_core::{extract, http};
use serde_json::Value;

use super::BilibiliParser;
use super::api::{ApiResponse, ArticleStats};
use super::format::format_article_stats;

static OPUS_URL_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"bilibili\.com/opus/(\d+)").expect("valid opus url regex"));

impl BilibiliParser {
    /// Resolve the opus id a 专栏 URL redirects to, if any.
    ///
    /// A failure here is not fatal: the caller still has the article API and
    /// HTML scraping to fall back on.
    async fn resolve_article_opus_id(&self, article_url: &str) -> Option<String> {
        let resolved = http::resolve_redirect(
            self.downloader.client(),
            article_url,
            self.get_headers().await,
        )
        .await
        .ok()?;

        opus_id_from_url(&resolved.final_url).map(ToOwned::to_owned)
    }

    /// Parse article (CV)
    ///
    /// Tries the structured API first (more reliable), falls back to HTML
    /// scraping via `__INITIAL_STATE__` if the API fails.
    pub(super) async fn fetch_article(&self, cv_id: &str) -> Result<ParseResult> {
        let client = self.downloader.client();
        let article_url = format!("https://www.bilibili.com/read/cv{}", cv_id);

        // Preferred: the opus body, which keeps the article's text and images
        // interleaved as written. The article API returns them as a summary
        // plus a flat image list, losing the layout.
        if let Some(opus_id) = self.resolve_article_opus_id(&article_url).await {
            match self.fetch_opus(&opus_id).await {
                // Show the link the user actually shared, not the redirect
                // target.
                Ok(result) => return Ok(result.with_url(&article_url)),
                Err(e) => tracing::warn!(
                    cv_id,
                    opus_id,
                    error = %e,
                    "Opus detail failed for an article; falling back to the article API"
                ),
            }
        }

        // Primary: article API (structured JSON, no fragile HTML parsing)
        let api_url = format!("https://api.bilibili.com/x/article/view?id={}", cv_id);

        if let Ok(response) = client
            .get(&api_url)
            .headers(self.get_headers().await)
            .send()
            .await
            && let Ok(resp) = response.json::<ApiResponse<Value>>().await
            && resp.code == 0
            && let Some(data) = resp.data
        {
            return Ok(build_article_result(&data, cv_id));
        }

        // Fallback: scrape __INITIAL_STATE__ from HTML
        tracing::debug!("Article API failed for cv{}, trying HTML scraping", cv_id);

        let response = client
            .get(&article_url)
            .headers(self.get_headers().await)
            .send()
            .await?;
        let html = response.text().await?;

        if let Some(read_info) = extract_initial_state_read_info(&html) {
            return Ok(build_article_result(&read_info, cv_id));
        }

        Err(ParserError::ContentNotFound(format!(
            "Could not parse article cv{}",
            cv_id
        )))
    }
}

/// The opus id a 专栏 URL resolves to, when the article is opus-backed.
///
/// Bilibili folded 专栏 into 图文, so `/read/cv<id>` now redirects to
/// `/opus/<id>`; only that endpoint still returns the body in its original
/// text/image order.
fn opus_id_from_url(url: &str) -> Option<&str> {
    OPUS_URL_RE
        .captures(url)
        .and_then(|captures| captures.get(1))
        .map(|id| id.as_str())
}

/// Extract article `readInfo` JSON from an HTML page.
fn extract_initial_state_read_info(html: &str) -> Option<Value> {
    let mut state = extract::extract_json_value(html, "window.__INITIAL_STATE__")?;
    Some(state.get_mut("readInfo")?.take())
}

/// Build article result from JSON data.
fn build_article_result(data: &Value, cv_id: &str) -> ParseResult {
    let title = data
        .get("title")
        .and_then(|value| value.as_str())
        .unwrap_or("无标题");
    let summary = data
        .get("summary")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let author_name = data
        .pointer("/author/name")
        .and_then(|value| value.as_str())
        .unwrap_or("未知作者");
    let author_face = data
        .pointer("/author/face")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let author_mid = data
        .pointer("/author/mid")
        .and_then(|value| value.as_i64())
        .unwrap_or(0);
    let timestamp = data
        .get("publish_time")
        .and_then(|value| value.as_i64())
        .filter(|&ts| ts > 0);

    let mut result = ParseResult::new(Platform::Bilibili)
        .with_title(title)
        .with_text(summary)
        .with_url(format!("https://www.bilibili.com/read/cv{}", cv_id))
        .with_author(
            Author::new(author_name)
                .with_uid(author_mid.to_string())
                .with_avatar(author_face)
                .with_url(format!("https://space.bilibili.com/{}", author_mid)),
        );
    // A missing or zero publish_time stays unknown rather than becoming 1970.
    if let Some(timestamp) = timestamp {
        result = result.with_timestamp(timestamp);
    }

    if let Some(stats) = data.get("stats") {
        let get = |key: &str| stats.get(key).and_then(|value| value.as_i64()).unwrap_or(0);
        let stat = ArticleStats {
            view: get("view"),
            favorite: get("favorite"),
            like: get("like"),
            reply: get("reply"),
            share: get("share"),
            coin: get("coin"),
        };
        result.add_extra("stats", format_article_stats(&stat));
    }

    if let Some(image_urls) = data.get("image_urls").and_then(|value| value.as_array()) {
        for image_url in image_urls.iter().take(9) {
            if let Some(url) = image_url.as_str() {
                result.add_content(Content::Image(ImageContent::new().with_url(url)));
            }
        }
    } else if let Some(banner_url) = data.get("banner_url").and_then(|value| value.as_str())
        && !banner_url.is_empty()
    {
        result.add_content(Content::Image(ImageContent::new().with_url(banner_url)));
    }

    result
}
