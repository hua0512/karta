use std::collections::HashSet;
use std::sync::LazyLock;

use karta_core::data::{Content, ImageContent};
use karta_core::html;

/// Append backslash to image URLs with query params to get original quality
/// from Xiaoheihe's CDN.
pub(super) fn optimize_image_url(url: &str) -> String {
    if url.contains('?') {
        format!("{url}\\")
    } else {
        url.to_string()
    }
}

fn image_dedupe_key(url: &str) -> String {
    let trimmed = url.trim_end_matches('\\');
    let base = trimmed.split('?').next().unwrap_or(trimmed);

    let no_scheme = base.split_once("://").map(|x| x.1).unwrap_or(base);
    match no_scheme.find('/') {
        Some(index) => no_scheme[index..].to_string(),
        None => base.to_string(),
    }
}

pub(super) fn dedupe_images(contents: &mut Vec<Content>) {
    let mut seen = HashSet::new();
    contents.retain(|content| {
        let Content::Image(image) = content else {
            return true;
        };
        let Some(url) = &image.url else {
            return true;
        };
        seen.insert(image_dedupe_key(url))
    });
}

/// Lazy-loaded images carry a placeholder in `src` and the real URL in
/// `data-original` or `data-src`, so those win regardless of attribute order.
fn img_url<'a>(attr_re: &regex::Regex, tag: &'a str) -> Option<&'a str> {
    const PRIORITY: [&str; 3] = ["data-original", "data-src", "src"];

    let mut best: Option<(usize, &str)> = None;
    for capture in attr_re.captures_iter(tag) {
        let (Some(name), Some(value)) = (capture.get(1), capture.get(2)) else {
            continue;
        };
        if value.as_str().is_empty() {
            continue;
        }
        let Some(rank) = PRIORITY
            .iter()
            .position(|attr| name.as_str().eq_ignore_ascii_case(attr))
        else {
            continue;
        };
        if best.is_none_or(|(best_rank, _)| rank < best_rank) {
            best = Some((rank, value.as_str()));
        }
    }
    best.map(|(_, url)| url)
}

fn extract_ordered_from_html(html_val: &str, contents: &mut Vec<Content>, parts: &mut Vec<String>) {
    static TAG_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"(?is)(<img\s[^>]*?>|<iframe[^>]*>.*?</iframe>)").unwrap()
    });
    static ATTR_RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r#"([\w:-]+)\s*=\s*\"([^\"]*)\""#).unwrap());
    static IFRAME_SRC_RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r#"src\s*=\s*\"([^\"]+)\""#).unwrap());

    let mut last = 0;
    for matched in TAG_RE.find_iter(html_val) {
        let before = html::html_to_text(&html_val[last..matched.start()]);
        if !before.trim().is_empty() {
            let before = before.trim().to_string();
            contents.push(Content::Text(before.clone()));
            parts.push(before);
        }

        let tag = &html_val[matched.start()..matched.end()];
        if tag.trim_start().to_ascii_lowercase().starts_with("<img") {
            if let Some(url) = img_url(&ATTR_RE, tag) {
                contents.push(Content::Image(
                    ImageContent::new().with_url(optimize_image_url(url)),
                ));
            }
        } else if tag.trim_start().to_ascii_lowercase().starts_with("<iframe")
            && let Some(capture) = IFRAME_SRC_RE.captures(tag).and_then(|c| c.get(1))
        {
            let mut src = capture.as_str().replace('\\', "");
            if src.starts_with("//") {
                src = format!("https:{src}");
            }
            let text = format!("({src})");
            contents.push(Content::Text(text.clone()));
            parts.push(text);
        }

        last = matched.end();
    }

    let after = html::html_to_text(&html_val[last..]);
    if !after.trim().is_empty() {
        let after = after.trim().to_string();
        contents.push(Content::Text(after.clone()));
        parts.push(after);
    }
}

pub(super) fn extract_text_and_media_from_entities(
    text_val: &str,
    contents: &mut Vec<Content>,
) -> Option<String> {
    let items: Vec<serde_json::Value> = serde_json::from_str(text_val).ok()?;
    let mut parts = Vec::new();

    for item in &items {
        match item.get("type").and_then(|v| v.as_str()) {
            Some("img") => {
                let url = item
                    .get("url")
                    .or_else(|| item.get("src"))
                    .and_then(|v| v.as_str());
                if let Some(url) = url {
                    contents.push(Content::Image(
                        ImageContent::new().with_url(optimize_image_url(url)),
                    ));
                }
            }
            Some("html") => {
                if let Some(raw_html) = item.get("text").and_then(|v| v.as_str()) {
                    extract_ordered_from_html(raw_html, contents, &mut parts);
                }
            }
            Some("text") => {
                let text = item
                    .get("content")
                    .or_else(|| item.get("text"))
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty());
                if let Some(text) = text {
                    contents.push(Content::Text(text.to_string()));
                    parts.push(text.to_string());
                }
            }
            _ => {}
        }
    }

    let joined = parts.join("\n").trim().to_string();
    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn html_image_urls(html: &str) -> Vec<String> {
        let text_json = serde_json::json!([{ "type": "html", "text": html }]).to_string();

        let mut contents = Vec::new();
        extract_text_and_media_from_entities(&text_json, &mut contents);

        contents
            .into_iter()
            .filter_map(|content| match content {
                Content::Image(image) => image.url,
                _ => None,
            })
            .collect()
    }

    #[test]
    fn html_image_uses_the_real_url_not_the_placeholder() {
        let cases = [
            (
                "data-original over placeholder src",
                r#"<img src="data:image/gif;base64,R0lGOD" data-original="https://cdn.max-c.com/real.jpg">"#,
            ),
            (
                "data-src over placeholder src",
                r#"<img src="data:image/gif;base64,R0lGOD" data-src="https://cdn.max-c.com/real.jpg">"#,
            ),
            (
                "attributes ending in src are not src",
                r#"<img data-lazy-src="https://cdn.max-c.com/other.jpg" src="https://cdn.max-c.com/real.jpg">"#,
            ),
        ];
        for (case, html) in cases {
            assert_eq!(
                html_image_urls(html),
                ["https://cdn.max-c.com/real.jpg"],
                "{case}"
            );
        }
    }
}
