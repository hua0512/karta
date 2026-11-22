use std::sync::LazyLock;

use scraper::{Html, Selector};

use karta_core::Platform;
use karta_core::data::{Author, Content, ImageContent, ParseResult};
use karta_core::extract;
use karta_core::html;

/// Marker preceding the embedded user-info JSON: `commonui.userInfo.setAll( {...} )`.
const USER_INFO_MARKER: &str = "commonui.userInfo.setAll";

static UID_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"[?&]uid=(\d+)").expect("valid nga uid regex"));
static IMG_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"\[img\](.*?)\[/img\]").expect("valid nga img tag regex"));

static TITLE_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("#postsubject0").expect("valid NGA title selector"));
static AUTHOR_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("#postauthor0").expect("valid NGA author selector"));
static TIME_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("#postdate0").expect("valid NGA time selector"));
static CONTENT_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("#postcontent0").expect("valid NGA content selector"));

/// Build a [`ParseResult`] from a fetched thread page.
pub(super) fn build_thread_result(page: &str, url: String) -> ParseResult {
    let document = Html::parse_document(page);

    let mut result = ParseResult::new(Platform::Nga).with_url(url);

    if let Some(title) = extract_title(&document) {
        result = result.with_title(title);
    }

    if let Some(author) = extract_author(&document, page) {
        result = result.with_author(author);
    }

    if let Some(timestamp) = extract_timestamp(&document) {
        result = result.with_timestamp(timestamp);
    }

    let (text, contents) = extract_contents(&document);
    if let Some(text) = text {
        result = result.with_text(text);
    }
    result.with_contents(contents)
}

fn extract_title(document: &Html) -> Option<String> {
    document
        .select(&TITLE_SELECTOR)
        .next()
        .map(|el| el.text().collect::<String>().trim().to_string())
}

/// Resolve the floor-0 author: uid from the `#postauthor0` link, username from
/// the `commonui.userInfo.setAll({...})` JSON embedded in a script tag.
fn extract_author(document: &Html, page: &str) -> Option<Author> {
    let author_el = document.select(&AUTHOR_SELECTOR).next()?;
    let href = author_el.value().attr("href")?;
    let uid = UID_RE.captures(href)?.get(1)?.as_str();

    let user_info = extract::extract_json_value(page, USER_INFO_MARKER)?;
    let username = user_info.get(uid)?.get("username")?.as_str()?;
    Some(Author::new(username))
}

fn extract_timestamp(document: &Html) -> Option<i64> {
    let el = document.select(&TIME_SELECTOR).next()?;
    let time_str = el.text().collect::<String>();
    let time_str = time_str.trim();
    // "YYYY-MM-DD HH:MM" in Beijing time (UTC+8), which NGA always displays.
    let beijing = chrono::FixedOffset::east_opt(8 * 3600)?;
    chrono::NaiveDateTime::parse_from_str(time_str, "%Y-%m-%d %H:%M")
        .ok()?
        .and_local_timezone(beijing)
        .single()
        .map(|dt| dt.timestamp())
}

fn extract_contents(document: &Html) -> (Option<String>, Vec<Content>) {
    let Some(content_el) = document.select(&CONTENT_SELECTOR).next() else {
        return (None, Vec::new());
    };

    let raw_text = content_el.text().collect::<String>();

    // Extract images from [img] tags
    let contents = IMG_RE
        .captures_iter(&raw_text)
        .filter_map(|caps| caps.get(1))
        .map(|img_url| {
            let url = img_url.as_str();
            // Reconstruct full URL
            let full_url = if url.starts_with('/') {
                format!("https://img.nga.178.com/attachments{}", url)
            } else {
                url.to_string()
            };
            Content::Image(ImageContent::new().with_url(full_url))
        })
        .collect();

    // Clean BBCode from text
    (Some(html::clean_bbcode(&raw_text, 500)), contents)
}
