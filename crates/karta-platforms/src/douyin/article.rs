//! Douyin long-article (长文章) result construction.
//!
//! Articles ride on the regular aweme detail payload: the item carries an
//! `article_info` block whose `article_content` is a JSON string holding the
//! body as Markdown (plus a server-written abstract), and whose `fe_data` is
//! a JSON string with the header poster and layout hints. The aweme's own
//! `video` is a zero-duration placeholder, so none of the video machinery
//! applies.
//!
//! The body is emitted like the WeChat / Bilibili-opus article cards: ordered
//! `Content::Text` and `Content::Image` items (cover first), a short summary
//! in `result.text` for the card renderers, and engagement stats in `extra`.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use serde::Deserialize;

use karta_core::Platform;
use karta_core::data::{Content, ImageContent, ParseResult};
use karta_core::html;

use super::api::{ApiArticleInfo, ApiStatistics, AwemeDetail};
use super::result::{aweme_author, aweme_share_url};

/// Upper bound on images attached to one article (cover included); mirrors
/// the WeChat / Hongguo caps.
const MAX_IMAGES: usize = 50;

/// Upper bound on the card summary placed in `result.text`.
const MAX_SUMMARY_CHARS: usize = 500;

/// Decoded `article_info.article_content`.
#[derive(Debug, Default, Deserialize)]
struct ArticleContent {
    #[serde(default)]
    markdown: Option<String>,
    #[serde(default)]
    long_article_abstract: Option<String>,
}

/// Decoded `article_info.fe_data` (only the fields used here).
#[derive(Debug, Default, Deserialize)]
struct FeData {
    #[serde(default)]
    head_poster_list: Option<FeImage>,
    /// Inline images of rich-media articles. Shape is not pinned down by a
    /// captured sample, so entries are decoded leniently (see [`fe_image_url`]).
    #[serde(default)]
    image_list: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Default, Deserialize)]
struct FeImage {
    #[serde(default)]
    url_list: Option<Vec<String>>,
}

// Markdown image `![alt](url "title")` or an inline HTML `<img …>` tag.
static MEDIA_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?s)!\[(?P<alt>[^\]]*)\]\(\s*<?(?P<url>[^)\s>]+)>?(?:\s+["'][^"']*["'])?\s*\)|<img\s[^>]*>"#,
    )
    .expect("valid markdown media regex")
});
/// The `(?:^|[^-\w])` guard keeps this off `data-src`.
static IMG_SRC_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(?:^|[^-\w])src\s*=\s*(?:"([^"]+)"|'([^']+)')"#).expect("valid img src regex")
});
static IMG_ALT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(?:^|[^-\w])alt\s*=\s*(?:"([^"]*)"|'([^']*)')"#).expect("valid img alt regex")
});

static HEADING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^[ \t]{0,3}#{1,6}[ \t]+").expect("valid heading regex"));
static BLOCKQUOTE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^[ \t]{0,3}>[ \t]?").expect("valid blockquote regex"));
static HR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^[ \t]{0,3}(?:-{3,}|\*{3,}|_{3,})[ \t]*$").expect("valid hr regex")
});
static STRONG_STAR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)\*\*(.+?)\*\*").expect("valid strong regex"));
static STRONG_UNDERSCORE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)__(.+?)__").expect("valid strong regex"));
static STRIKE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)~~(.+?)~~").expect("valid strike regex"));
static CODE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"`([^`\n]+)`").expect("valid code regex"));
static LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[([^\]]*)\]\([^)]*\)").expect("valid link regex"));
static ESCAPE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\\([\\`*_{}\[\]()#+\-.!>~|])").expect("valid escape regex"));

/// Build a [`ParseResult`] for an aweme that carries a long article.
pub(super) fn build_article_result(item: &AwemeDetail, info: &ApiArticleInfo) -> ParseResult {
    let content: ArticleContent = info
        .article_content
        .as_deref()
        .map(decode_article_content)
        .unwrap_or_default();
    let fe_data: FeData = info
        .fe_data
        .as_deref()
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or_default();

    let mut result = ParseResult::new(Platform::Douyin).with_author(aweme_author(item));

    let title = info
        .article_title
        .as_deref()
        .or(item.desc.as_deref())
        .map(str::trim)
        .filter(|title| !title.is_empty());
    if let Some(title) = title {
        result = result.with_title(title);
    }
    if let Some(create_time) = item.create_time.filter(|&ts| ts > 0) {
        result = result.with_timestamp(create_time);
    }

    let article_id = info
        .article_id
        .as_deref()
        .or(item.aweme_id.as_deref())
        .filter(|id| !id.is_empty());
    let url = aweme_share_url(item)
        .or_else(|| article_id.map(|id| format!("https://www.douyin.com/note/{id}")));
    if let Some(url) = url {
        result = result.with_url(url);
    }

    let mut body = ArticleBody::new();

    // Header poster first; the placeholder video's cover is the feed card
    // image and only stands in when there is no poster.
    let cover = fe_data
        .head_poster_list
        .as_ref()
        .and_then(|poster| poster.url_list.as_ref())
        .and_then(|urls| urls.iter().find(|url| !url.trim().is_empty()))
        .cloned()
        .or_else(|| {
            item.video
                .as_ref()
                .and_then(|video| video.cover.as_ref())
                .and_then(|cover| cover.url_list.as_ref())
                .and_then(|urls| urls.first())
                .cloned()
        });
    if let Some(cover) = cover.as_deref().and_then(normalize_img_url) {
        body.push_image(cover, None);
    }

    if let Some(markdown) = content.markdown.as_deref() {
        body.walk_markdown(markdown);
    }

    // Rich-media articles list their pictures in `fe_data.image_list`; when
    // the Markdown did not reference them inline, append them as a gallery
    // so they are not lost.
    if body.body_images == 0
        && let Some(images) = &fe_data.image_list
    {
        for url in images.iter().filter_map(fe_image_url) {
            body.push_image(url, None);
        }
    }

    let summary = content
        .long_article_abstract
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
        .or_else(|| body.joined_text())
        .map(|text| truncate_chars(&text, MAX_SUMMARY_CHARS));
    if let Some(summary) = summary {
        result = result.with_text(summary);
    }

    result.add_extra("类型", "文章");
    if let Some(minutes) = info.read_time.filter(|&minutes| minutes > 0) {
        result.add_extra("阅读时长", format!("{minutes} 分钟"));
    }
    if let Some(stats) = item.statistics.as_ref().and_then(format_stats) {
        result.add_extra("stats", stats);
    }
    if body.images_skipped > 0 {
        result.add_extra(
            "图片",
            format!(
                "图片过多，仅附前 {MAX_IMAGES} 张（共 {} 张）",
                body.images_attached + body.images_skipped
            ),
        );
    }

    result.with_contents(body.contents)
}

pub(super) fn has_body(info: &ApiArticleInfo) -> bool {
    if info
        .article_content
        .as_deref()
        .map(decode_article_content)
        .and_then(|content| content.markdown)
        .is_some_and(|markdown| !markdown.trim().is_empty())
    {
        return true;
    }
    info.fe_data
        .as_deref()
        .and_then(|raw| serde_json::from_str::<FeData>(raw).ok())
        .and_then(|data| data.image_list)
        .is_some_and(|images| images.iter().any(|image| fe_image_url(image).is_some()))
}

/// `article_content` is normally a JSON object; older payloads may carry the
/// body directly as a string, which is treated as Markdown.
fn decode_article_content(raw: &str) -> ArticleContent {
    let trimmed = raw.trim();
    if trimmed.starts_with('{')
        && let Ok(content) = serde_json::from_str::<ArticleContent>(trimmed)
    {
        return content;
    }
    ArticleContent {
        markdown: (!trimmed.is_empty()).then(|| trimmed.to_owned()),
        long_article_abstract: None,
    }
}

/// Ordered body contents (text interleaved with images) plus the bookkeeping
/// for the image cap.
struct ArticleBody {
    contents: Vec<Content>,
    text_parts: Vec<String>,
    seen_urls: HashSet<String>,
    images_attached: usize,
    images_skipped: usize,
    /// Images that came from the body itself (not the cover).
    body_images: usize,
}

impl ArticleBody {
    fn new() -> Self {
        Self {
            contents: Vec::new(),
            text_parts: Vec::new(),
            seen_urls: HashSet::new(),
            images_attached: 0,
            images_skipped: 0,
            body_images: 0,
        }
    }

    /// Split the Markdown on image constructs, preserving document order:
    /// text between images becomes `Content::Text`, each image a
    /// `Content::Image`.
    fn walk_markdown(&mut self, markdown: &str) {
        let mut last = 0;
        for found in MEDIA_RE.captures_iter(markdown) {
            let whole = found.get(0).expect("group 0");
            self.push_text_markdown(&markdown[last..whole.start()]);
            if let Some((url, alt)) = media_url_and_alt(&found) {
                self.body_images += 1;
                self.push_image(url, alt);
            }
            last = whole.end();
        }
        self.push_text_markdown(&markdown[last..]);
    }

    fn push_text_markdown(&mut self, chunk: &str) {
        if chunk.trim().is_empty() {
            return;
        }
        let text = markdown_to_text(chunk);
        if text.is_empty() {
            return;
        }

        self.text_parts.push(text.clone());

        // Keep adjacent paragraphs in one segment so skipped/deduped images
        // do not fragment the body into many tiny messages.
        if let Some(Content::Text(previous)) = self.contents.last_mut() {
            previous.push_str("\n\n");
            previous.push_str(&text);
        } else {
            self.contents.push(Content::Text(text));
        }
    }

    fn push_image(&mut self, url: String, alt: Option<String>) {
        if !self.seen_urls.insert(url.clone()) {
            return;
        }
        if self.images_attached >= MAX_IMAGES {
            self.images_skipped += 1;
            return;
        }
        self.images_attached += 1;
        let mut image = ImageContent::new().with_url(url);
        if let Some(alt) = alt {
            image = image.with_alt(alt);
        }
        self.contents.push(Content::Image(image));
    }

    fn joined_text(&self) -> Option<String> {
        let joined = self.text_parts.join("\n\n");
        let joined = joined.trim();
        (!joined.is_empty()).then(|| joined.to_owned())
    }
}

/// Resolve an image match to `(url, alt)`, dropping placeholders and
/// non-http(s) sources.
fn media_url_and_alt(found: &regex::Captures<'_>) -> Option<(String, Option<String>)> {
    if let Some(url) = found.name("url") {
        let alt = found
            .name("alt")
            .map(|alt| alt.as_str().trim())
            .filter(|alt| !alt.is_empty())
            .map(str::to_owned);
        return normalize_img_url(url.as_str()).map(|url| (url, alt));
    }

    let tag = found.get(0)?.as_str();
    let src = IMG_SRC_RE
        .captures(tag)
        .and_then(|caps| caps.get(1).or_else(|| caps.get(2)))?;
    let alt = IMG_ALT_RE
        .captures(tag)
        .and_then(|caps| caps.get(1).or_else(|| caps.get(2)))
        .map(|alt| html::html_to_text(alt.as_str()))
        .filter(|alt| !alt.is_empty());
    normalize_img_url(src.as_str()).map(|url| (url, alt))
}

/// Best-effort URL of an `fe_data.image_list` entry: an object with a
/// `url_list`, an object with `url`/`uri`, or a bare string.
fn fe_image_url(value: &serde_json::Value) -> Option<String> {
    let raw = match value {
        serde_json::Value::String(url) => url.as_str(),
        serde_json::Value::Object(object) => object
            .get("url_list")
            .and_then(|urls| urls.as_array())
            .and_then(|urls| urls.iter().find_map(|url| url.as_str()))
            .or_else(|| object.get("url").and_then(|url| url.as_str()))
            .or_else(|| object.get("uri").and_then(|url| url.as_str()))?,
        _ => return None,
    };
    normalize_img_url(raw)
}

/// Validate and normalize an image URL: decode `&amp;`, fix scheme-less `//`
/// URLs, and drop `data:` placeholders and anything that is not http(s).
fn normalize_img_url(raw: &str) -> Option<String> {
    let url = raw.trim().replace("&amp;", "&");
    if url.is_empty() || url.starts_with("data:") {
        return None;
    }
    if let Some(rest) = url.strip_prefix("//") {
        return Some(format!("https://{rest}"));
    }
    (url.starts_with("http://") || url.starts_with("https://")).then_some(url)
}

/// Reduce a Markdown fragment (no images) to readable plain text: drop block
/// markers and inline emphasis, keep link text, then let the shared HTML
/// cleaner handle embedded tags (`<br/>` runs are common) and entities.
fn markdown_to_text(chunk: &str) -> String {
    let text = HEADING_RE.replace_all(chunk, "");
    let text = BLOCKQUOTE_RE.replace_all(&text, "");
    let text = HR_RE.replace_all(&text, "");
    let text = STRONG_STAR_RE.replace_all(&text, "$1");
    let text = STRONG_UNDERSCORE_RE.replace_all(&text, "$1");
    let text = STRIKE_RE.replace_all(&text, "$1");
    let text = CODE_RE.replace_all(&text, "$1");
    let text = LINK_RE.replace_all(&text, "$1");
    let text = ESCAPE_RE.replace_all(&text, "$1");
    html::html_to_text(&text)
}

/// Truncate to `max_chars` characters, appending an ellipsis when cut.
fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    let truncated: String = text.chars().take(max_chars).collect();
    format!("{}…", truncated.trim_end())
}

fn format_stats(stats: &ApiStatistics) -> Option<String> {
    let entries = [
        ("👍", stats.digg_count),
        ("💬", stats.comment_count),
        ("⭐", stats.collect_count),
        ("↩️", stats.share_count),
    ];
    let parts: Vec<String> = entries
        .iter()
        .filter_map(|(emoji, count)| count.map(|count| format!("{emoji} {}", format_count(count))))
        .collect();
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// Format large numbers with Chinese units (matches the Bilibili cards).
fn format_count(n: i64) -> String {
    if n >= 100_000_000 {
        format!("{:.1}亿", n as f64 / 100_000_000.0)
    } else if n >= 10_000 {
        format!("{:.1}万", n as f64 / 10_000.0)
    } else {
        n.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::super::result::build_aweme_result;
    use super::*;

    fn image_urls(result: &ParseResult) -> Vec<&str> {
        result
            .contents
            .iter()
            .filter_map(|content| match content {
                Content::Image(image) => image.url.as_deref(),
                _ => None,
            })
            .collect()
    }

    fn text_parts(result: &ParseResult) -> Vec<&str> {
        result
            .contents
            .iter()
            .filter_map(|content| match content {
                Content::Text(text) => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    fn article_detail(markdown: &str, fe_data: &str) -> AwemeDetail {
        let content = serde_json::json!({ "markdown": markdown }).to_string();
        let json = serde_json::json!({
            "aweme_id": "1",
            "aweme_type": 0,
            "desc": "desc",
            "create_time": 5,
            "author": { "nickname": "作者" },
            "video": { "duration": 0, "cover": { "url_list": ["https://img.example/feed-cover.jpg"] } },
            "article_info": {
                "article_id": "1",
                "article_title": "标题",
                "article_content": content,
                "fe_data": fe_data,
                "read_time": 0
            }
        });
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn markdown_images_interleave_with_text_in_order() {
        let markdown = "## 第一节\n\n开头 **重点** 段落，见[链接](https://example.com)。\n\n![配图一](https://img.example/1.jpg \"title\")\n\n> 引用一句\n\n<img src=\"https://img.example/2.jpg?a=1&amp;b=2\" alt=\"第二张\"/>\n\n结尾 ~~删除~~ `code` 文字\\*。\n\n---\n";
        let detail = article_detail(markdown, "{}");
        let result = build_aweme_result(&detail);

        let kinds: Vec<&str> = result
            .contents
            .iter()
            .map(|content| match content {
                Content::Text(_) => "text",
                Content::Image(_) => "image",
                _ => "other",
            })
            .collect();
        // Feed cover (no poster in fe_data), then body in document order.
        assert_eq!(kinds, ["image", "text", "image", "text", "image", "text"]);

        assert_eq!(
            image_urls(&result),
            [
                "https://img.example/feed-cover.jpg",
                "https://img.example/1.jpg",
                "https://img.example/2.jpg?a=1&b=2",
            ]
        );
        let alts: Vec<Option<&str>> = result
            .contents
            .iter()
            .filter_map(|content| match content {
                Content::Image(image) => Some(image.alt.as_deref()),
                _ => None,
            })
            .collect();
        assert_eq!(alts, [None, Some("配图一"), Some("第二张")]);

        let texts = text_parts(&result);
        assert_eq!(texts[0], "第一节\n\n开头 重点 段落，见链接。");
        assert_eq!(texts[1], "引用一句");
        assert_eq!(texts[2], "结尾 删除 code 文字*。");

        // No abstract: summary derives from the body text.
        assert_eq!(
            result.text.as_deref(),
            Some("第一节\n\n开头 重点 段落，见链接。\n\n引用一句\n\n结尾 删除 code 文字*。")
        );
    }
}
