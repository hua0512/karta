//! Reusable HTML and BBCode text-cleaning utilities.
//!
//! These are extracted from the Weibo and NGA parsers so other modules can
//! share the same logic without duplicating regex compilations.

use std::borrow::Cow;
use std::sync::LazyLock;

/// Apply a regex replacement, committing a new allocation only when the
/// pattern actually matched.
fn regex_replace<'a, R: regex::Replacer>(
    text: Cow<'a, str>,
    re: &regex::Regex,
    replacement: R,
) -> Cow<'a, str> {
    match re.replace_all(&text, replacement) {
        Cow::Owned(replaced) => Cow::Owned(replaced),
        // No match: the borrowed result is `text` itself.
        Cow::Borrowed(_) => text,
    }
}

/// Substring replacement, committing a new allocation only on a match.
fn replace_in_place(text: &mut Cow<'_, str>, from: &str, to: &str) {
    if text.contains(from) {
        *text = Cow::Owned(text.replace(from, to));
    }
}

// ---------------------------------------------------------------------------
// HTML → plain-text
// ---------------------------------------------------------------------------

static HTML_TAG_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"<[^>]+>").expect("valid regex"));
static HTML_IMG_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#"<img[^>]+src="([^"]+)"[^>]*>"#).expect("valid regex"));
static SCRIPT_TAG_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?is)<script[^>]*>.*?</script>").expect("valid regex"));
static STYLE_TAG_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?is)<style[^>]*>.*?</style>").expect("valid regex"));
static HTML_BR_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?i)<br\s*/?>").expect("valid regex"));
static HTML_P_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?i)</p>").expect("valid regex"));
static HTML_NUMERIC_ENTITY_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"&#(\d+);").expect("valid regex"));
static HTML_HEX_ENTITY_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"&#x([0-9a-fA-F]+);").expect("valid regex"));
static HTML_WHITESPACE_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"[ \t]+").expect("valid regex"));
static HTML_NEWLINES_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"\n{3,}").expect("valid regex"));

/// Extract image URLs from `<img>` tags.
pub fn extract_img_urls(html: &str) -> Vec<String> {
    HTML_IMG_RE
        .captures_iter(html)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

/// Named HTML entities decoded before the numeric/hex passes.
///
/// `&amp;` is deliberately absent: it must decode last (see `html_to_text`).
const NAMED_ENTITIES: &[(&str, &str)] = &[
    ("&nbsp;", " "),
    ("&lt;", "<"),
    ("&gt;", ">"),
    ("&quot;", "\""),
    ("&apos;", "'"),
    ("&#39;", "'"),
    ("&#x27;", "'"),
    ("&ldquo;", "\u{201C}"),
    ("&rdquo;", "\u{201D}"),
    ("&lsquo;", "\u{2018}"),
    ("&rsquo;", "\u{2019}"),
    ("&mdash;", "\u{2014}"),
    ("&ndash;", "\u{2013}"),
    ("&hellip;", "\u{2026}"),
    ("&copy;", "\u{00A9}"),
    ("&reg;", "\u{00AE}"),
    ("&trade;", "\u{2122}"),
    ("\u{200b}", ""),
];

/// Convert HTML to plain text with entity decoding.
pub fn html_to_text(html: &str) -> String {
    // Remove script and style tags with their content
    let mut text = regex_replace(Cow::Borrowed(html), &SCRIPT_TAG_RE, "");
    text = regex_replace(text, &STYLE_TAG_RE, "");

    // Replace <br> and <p> with newlines
    text = regex_replace(text, &HTML_BR_RE, "\n");
    text = regex_replace(text, &HTML_P_RE, "\n");

    // Remove all remaining HTML tags
    text = regex_replace(text, &HTML_TAG_RE, "");

    // Decode HTML entities
    for (from, to) in NAMED_ENTITIES {
        replace_in_place(&mut text, from, to);
    }

    // Decode numeric HTML entities (&#123; or &#x7B;)
    text = regex_replace(text, &HTML_NUMERIC_ENTITY_RE, |caps: &regex::Captures| {
        caps.get(1)
            .and_then(|m| m.as_str().parse::<u32>().ok())
            .and_then(char::from_u32)
            .map(|c| c.to_string())
            .unwrap_or_default()
    });

    text = regex_replace(text, &HTML_HEX_ENTITY_RE, |caps: &regex::Captures| {
        caps.get(1)
            .and_then(|m| u32::from_str_radix(m.as_str(), 16).ok())
            .and_then(char::from_u32)
            .map(|c| c.to_string())
            .unwrap_or_default()
    });

    // `&amp;` decodes last so double-encoded input like `&amp;#39;` yields
    // the literal `&#39;` instead of double-decoding into `'`.
    replace_in_place(&mut text, "&amp;", "&");

    // Normalize whitespace
    text = regex_replace(text, &HTML_WHITESPACE_RE, " ");

    // Normalize multiple newlines
    text = regex_replace(text, &HTML_NEWLINES_RE, "\n\n");

    text.trim().to_string()
}

// ---------------------------------------------------------------------------
// BBCode → plain-text
// ---------------------------------------------------------------------------

static BBCODE_RULES: LazyLock<Vec<(regex::Regex, &'static str)>> = LazyLock::new(|| {
    vec![
        // Remove image tags (complete and incomplete)
        (
            regex::Regex::new(r"\[img\][^\[\]]*\[/img\]").expect("valid regex"),
            "",
        ),
        (
            regex::Regex::new(r"\[img\][^\[\]]*").expect("valid regex"),
            "",
        ),
        // Process URL tags, keep link text
        (
            regex::Regex::new(r"\[url=[^\]]*\]([^\[]*?)\[/url\]").expect("valid regex"),
            "$1",
        ),
        (
            regex::Regex::new(r"\[url\]([^\[]*?)\[/url\]").expect("valid regex"),
            "$1",
        ),
        // Remove quote tags
        (
            regex::Regex::new(r"(?s)\[quote\].*?\[/quote\]").expect("valid regex"),
            "",
        ),
        // Process format tags (b, i, u), keep text
        (
            regex::Regex::new(r"(?s)\[b\](.*?)\[/b\]").expect("valid regex"),
            "$1",
        ),
        (
            regex::Regex::new(r"(?s)\[i\](.*?)\[/i\]").expect("valid regex"),
            "$1",
        ),
        (
            regex::Regex::new(r"(?s)\[u\](.*?)\[/u\]").expect("valid regex"),
            "$1",
        ),
        // Process attribute tags (color, size), keep text
        (
            regex::Regex::new(r"(?s)\[color=[^\]]*\](.*?)\[/color\]").expect("valid regex"),
            "$1",
        ),
        (
            regex::Regex::new(r"(?s)\[size=[^\]]*\](.*?)\[/size\]").expect("valid regex"),
            "$1",
        ),
        // Remove other unmatched tags
        (regex::Regex::new(r"\[[^]]+\]").expect("valid regex"), ""),
        // Clean whitespace
        (regex::Regex::new(r"\n{3,}").expect("valid regex"), "\n\n"),
        (regex::Regex::new(r"[ \t]+").expect("valid regex"), " "),
        (regex::Regex::new(r"\n\s+\n").expect("valid regex"), "\n\n"),
    ]
});

/// Clean BBCode tags from text, returning at most `max_length` characters.
pub fn clean_bbcode(text: &str, max_length: usize) -> String {
    let mut result = Cow::Borrowed(text);
    for (re, replacement) in BBCODE_RULES.iter() {
        result = regex_replace(result, re, *replacement);
    }

    // Remove zero-width spaces
    replace_in_place(&mut result, "\u{200b}", "");

    let result = result.trim();

    // Limit length (character-based for proper CJK support)
    if result.chars().count() > max_length {
        let truncated: String = result.chars().take(max_length).collect();
        format!("{}...", truncated)
    } else {
        result.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_to_text_entities() {
        for (input, expected) in [
            ("&lt;tag&gt; &amp; &quot;text&quot;", "<tag> & \"text\""),
            ("&#65;&#x42;", "AB"),
            ("before<script>alert('xss')</script>after", "beforeafter"),
            ("a<style>p { color: red }</style>b", "ab"),
        ] {
            assert_eq!(html_to_text(input), expected, "{input}");
        }
    }

    #[test]
    fn test_html_to_text_double_encoded_entities_decode_once() {
        // `&amp;` decodes last: double-encoded entities must only lose one
        // level of encoding.
        assert_eq!(html_to_text("&amp;#39;"), "&#39;");
        assert_eq!(html_to_text("&amp;#x27;"), "&#x27;");
        assert_eq!(html_to_text("&amp;lt;"), "&lt;");
        assert_eq!(html_to_text("&amp;amp;"), "&amp;");
    }
}
