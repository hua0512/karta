//! HTML extraction for WeChat official-account article pages.
//!
//! Article pages embed metadata twice: as `og:*` meta tags and as inline JS
//! vars (`msg_title`, `msg_cdn_url`, `ct`, ...). Both are consulted so the
//! parser survives either being absent. The body lives in `#js_content`,
//! where images are lazy-loaded via `data-src` (the `src` attribute is
//! usually a placeholder).

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use scraper::{Html, Selector};

use karta_core::Platform;
use karta_core::data::{Author, Content, ImageContent, ParseResult};
use karta_core::error::{ParserError, Result};
use karta_core::html;

/// Upper bound on images attached to one article card (cover included).
/// WeChat posts can embed hundreds of images, which would swamp a chat
/// message; mirrors the Hongguo comic parser's cap.
const MAX_IMAGES: usize = 50;

/// Upper bound (in characters) on the extracted body text, in line with the
/// NGA (500) / V2EX (700) text caps for article-like cards.
const MAX_TEXT_CHARS: usize = 1000;

static OG_TITLE_SELECTOR: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse(r#"meta[property="og:title"]"#).expect("valid og:title selector")
});
static OG_DESC_SELECTOR: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse(r#"meta[property="og:description"]"#).expect("valid og:description selector")
});
static OG_IMAGE_SELECTOR: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse(r#"meta[property="og:image"]"#).expect("valid og:image selector")
});
static META_AUTHOR_SELECTOR: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse(r#"meta[name="author"]"#).expect("valid meta author selector")
});
static ACTIVITY_NAME_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("#activity-name").expect("valid activity-name selector"));
static JS_NAME_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("#js_name").expect("valid js_name selector"));
static JS_AUTHOR_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("#js_author_name").expect("valid js_author_name selector"));
static JS_CONTENT_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("#js_content").expect("valid js_content selector"));
static WEUI_MSG_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(".weui-msg").expect("valid weui-msg selector"));
static WEUI_TITLE_SELECTOR: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse(".weui-msg .weui-msg__title").expect("valid weui-msg title selector")
});
static WEUI_DESC_SELECTOR: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse(".weui-msg .weui-msg__desc").expect("valid weui-msg desc selector")
});

static MSG_TITLE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"var\s+msg_title\s*=\s*(?:'([^']*)'|"([^"]*)")"#).expect("valid msg_title regex")
});
static MSG_DESC_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"var\s+msg_desc\s*=\s*(?:htmlDecode\(\s*)?(?:'([^']*)'|"([^"]*)")"#)
        .expect("valid msg_desc regex")
});
static MSG_CDN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"var\s+msg_cdn_url\s*=\s*(?:'([^']*)'|"([^"]*)")"#)
        .expect("valid msg_cdn_url regex")
});
static NICKNAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\bnickname\s*=\s*(?:'([^']*)'|"([^"]*)")"#).expect("valid nickname regex")
});
/// Publish time appears as `var ct = "1712345678"`, `var create_time = ...`
/// or `oriCreateTime = ...` depending on page generation; quoted or bare.
static CREATE_TIME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\b(?:var\s+ct|create_time|oriCreateTime)\s*=\s*['"]?(\d{10})['"]?"#)
        .expect("valid create_time regex")
});

static IMG_TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<img\s[^>]*>").expect("valid img tag regex"));
static IMG_DATA_SRC_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)data-src\s*=\s*(?:"([^"]+)"|'([^']+)')"#).expect("valid data-src regex")
});
/// The `[^-\w]` guard keeps this from matching inside `data-src` or
/// `data-croporisrc`.
static IMG_SRC_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)[^-\w]src\s*=\s*(?:"([^"]+)"|'([^']+)')"#).expect("valid src regex")
});

/// Build a [`ParseResult`] from a fetched article page, or a clear error for
/// deleted/violation pages and the anti-crawler verification page.
pub(super) fn build_article_result(page: &str, url: String) -> Result<ParseResult> {
    let document = Html::parse_document(page);

    let content_el = document.select(&JS_CONTENT_SELECTOR).next();
    if content_el.is_none() {
        check_failure_page(page, &document)?;
    }

    let mut result = ParseResult::new(Platform::Wechat).with_url(url);

    let title = meta_content(&document, &OG_TITLE_SELECTOR)
        .or_else(|| js_var(page, &MSG_TITLE_RE))
        .or_else(|| element_text(&document, &ACTIVITY_NAME_SELECTOR));
    if let Some(title) = title {
        result = result.with_title(title);
    }

    let account = element_text(&document, &JS_NAME_SELECTOR).or_else(|| js_var(page, &NICKNAME_RE));
    let byline = meta_content(&document, &META_AUTHOR_SELECTOR)
        .or_else(|| element_text(&document, &JS_AUTHOR_SELECTOR));
    match (account, byline) {
        (Some(account), Some(byline)) => {
            if byline != account {
                result.add_extra("作者", byline);
            }
            result = result.with_author(Author::new(account));
        }
        (Some(account), None) => result = result.with_author(Author::new(account)),
        (None, Some(byline)) => result = result.with_author(Author::new(byline)),
        (None, None) => {}
    }

    if let Some(timestamp) = extract_timestamp(page) {
        result = result.with_timestamp(timestamp);
    }

    let mut body = ArticleBody::new();

    let cover = js_var(page, &MSG_CDN_RE).or_else(|| meta_content(&document, &OG_IMAGE_SELECTOR));
    if let Some(cover) = cover.as_deref().and_then(normalize_img_url) {
        body.push_image(cover);
    }

    if let Some(content_el) = content_el {
        body.walk_content_html(&content_el.inner_html());
    }

    let text = body
        .joined_text()
        .or_else(|| meta_content(&document, &OG_DESC_SELECTOR))
        .or_else(|| js_var(page, &MSG_DESC_RE));
    if let Some(text) = text {
        result = result.with_text(text);
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
    result = result.with_contents(body.contents);

    if result.title.is_none() && result.text.is_none() && result.contents.is_empty() {
        return Err(ParserError::Parse(
            "未能从微信文章页面提取内容，请在浏览器中打开原文链接".into(),
        ));
    }

    Ok(result)
}

/// Reject failure pages with a targeted error. Checked only when
/// `#js_content` is absent, so article bodies mentioning these phrases are
/// unaffected.
fn check_failure_page(page: &str, document: &Html) -> Result<()> {
    // Anti-crawler interstitial: "环境异常，完成验证后即可继续访问".
    if page.contains("环境异常") || page.contains("完成验证后即可继续访问") {
        return Err(ParserError::Parse(
            "微信文章触发访问验证（环境异常），请在浏览器中打开原文链接完成验证后重试".into(),
        ));
    }

    // Deleted / violation / blocked-account pages render a `.weui-msg` box;
    // surface its exact text (e.g. "该内容已被发布者删除").
    if let Some(title) = element_text(document, &WEUI_TITLE_SELECTOR) {
        let message = match element_text(document, &WEUI_DESC_SELECTOR) {
            Some(desc) => format!("{title} {desc}"),
            None => title,
        };
        return Err(ParserError::ContentNotFound(message));
    }

    // Expired/invalid links (e.g. a stale `sn` parameter) return a shell page
    // with an empty `.weui-msg` container whose message is filled in by JS.
    if document.select(&WEUI_MSG_SELECTOR).next().is_some() {
        return Err(ParserError::ContentNotFound(
            "文章不存在或链接已失效".into(),
        ));
    }

    Ok(())
}

/// Ordered body contents (text segments interleaved with images) plus the
/// bookkeeping for the text/image caps.
struct ArticleBody {
    contents: Vec<Content>,
    text_parts: Vec<String>,
    seen_urls: HashSet<String>,
    text_budget: usize,
    images_attached: usize,
    images_skipped: usize,
}

impl ArticleBody {
    fn new() -> Self {
        Self {
            contents: Vec::new(),
            text_parts: Vec::new(),
            seen_urls: HashSet::new(),
            text_budget: MAX_TEXT_CHARS,
            images_attached: 0,
            images_skipped: 0,
        }
    }

    /// Split `#js_content` inner HTML on `<img>` tags, preserving document
    /// order: text between images becomes `Content::Text`, each image a
    /// `Content::Image`.
    fn walk_content_html(&mut self, content_html: &str) {
        let mut last = 0;
        for tag in IMG_TAG_RE.find_iter(content_html) {
            self.push_text_html(&content_html[last..tag.start()]);
            if let Some(url) = img_url(tag.as_str()) {
                self.push_image(url);
            }
            last = tag.end();
        }
        self.push_text_html(&content_html[last..]);
    }

    fn push_text_html(&mut self, chunk: &str) {
        if self.text_budget == 0 || chunk.trim().is_empty() {
            return;
        }
        // WeChat uses `<section>` as its paragraph container; map it to the
        // `</p>` → newline rule already implemented in `html_to_text`.
        let chunk = chunk.replace("</section>", "</p>");
        let text = html::html_to_text(&chunk);
        if text.is_empty() {
            return;
        }

        let text = if text.chars().count() > self.text_budget {
            let truncated: String = text.chars().take(self.text_budget).collect();
            self.text_budget = 0;
            format!("{}…", truncated.trim_end())
        } else {
            self.text_budget -= text.chars().count();
            text
        };
        self.text_parts.push(text.clone());
        self.contents.push(Content::Text(text));
    }

    fn push_image(&mut self, url: String) {
        if !self.seen_urls.insert(url.clone()) {
            return;
        }
        if self.images_attached >= MAX_IMAGES {
            self.images_skipped += 1;
            return;
        }
        self.images_attached += 1;
        self.contents
            .push(Content::Image(ImageContent::new().with_url(url)));
    }

    fn joined_text(&self) -> Option<String> {
        let joined = self.text_parts.join("\n");
        let joined = joined.trim();
        (!joined.is_empty()).then(|| joined.to_string())
    }
}

/// Pull the image URL out of an `<img>` tag, preferring the lazy-load
/// `data-src` over the (usually placeholder) `src`.
fn img_url(tag: &str) -> Option<String> {
    let raw = IMG_DATA_SRC_RE
        .captures(tag)
        .or_else(|| IMG_SRC_RE.captures(tag))
        .and_then(|caps| caps.get(1).or_else(|| caps.get(2)))?;
    normalize_img_url(raw.as_str())
}

/// Validate and normalize an image URL: decode `&amp;` (values regex-lifted
/// from raw HTML keep their entities), fix scheme-less `//` URLs, and drop
/// `data:` placeholders and anything that is not http(s).
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

fn meta_content(document: &Html, selector: &Selector) -> Option<String> {
    let content = document
        .select(selector)
        .next()?
        .value()
        .attr("content")?
        .trim();
    (!content.is_empty()).then(|| content.to_string())
}

fn element_text(document: &Html, selector: &Selector) -> Option<String> {
    let el = document.select(selector).next()?;
    let text = el.text().collect::<String>();
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (!text.is_empty()).then_some(text)
}

/// Extract a quoted (or bare) inline JS string var; group 1 is the
/// single-quoted form, group 2 the double-quoted one. Values are
/// HTML-escaped in the page source, so decode entities on the way out.
fn js_var(page: &str, re: &Regex) -> Option<String> {
    let caps = re.captures(page)?;
    let raw = caps.get(1).or_else(|| caps.get(2))?.as_str();
    let text = html::html_to_text(raw);
    (!text.is_empty()).then_some(text)
}

fn extract_timestamp(page: &str) -> Option<i64> {
    CREATE_TIME_RE.captures(page)?.get(1)?.as_str().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARTICLE_URL: &str = "https://mp.weixin.qq.com/s/dQnHCXfeqJ0WHkeriL4kOw";

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

    #[test]
    fn falls_back_to_inline_js_vars_when_meta_missing() {
        let page = r#"<html><head><title>t</title></head><body>
            <script nonce="n">
            var msg_title = '内联标题'.html(false);
            var msg_desc = htmlDecode("内联摘要");
            var msg_cdn_url = "https://mmbiz.qpic.cn/mmbiz_jpg/INLINE/640?wx_fmt=jpeg";
            var create_time = "1700000000" * 1;
            var nickname = "内联公众号" || "";
            </script>
            </body></html>"#;

        let result = build_article_result(page, ARTICLE_URL.to_string()).unwrap();

        assert_eq!(result.title.as_deref(), Some("内联标题"));
        assert_eq!(
            result.author.as_ref().map(|author| author.name.as_str()),
            Some("内联公众号")
        );
        assert_eq!(result.timestamp, Some(1_700_000_000));
        // No body → description fallback
        assert_eq!(result.text.as_deref(), Some("内联摘要"));
        assert_eq!(
            image_urls(&result),
            ["https://mmbiz.qpic.cn/mmbiz_jpg/INLINE/640?wx_fmt=jpeg"]
        );
    }
}
