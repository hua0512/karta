use std::sync::LazyLock;

use karta_core::extract;
use regex::Regex;

use super::api::{EmbedContextWrapper, GraphqlData};

/// Matches the `contextJSON` value inside the embed page's JavaScript.
/// A regex fits here (the value is an *escaped JSON string*, not a bare
/// object); `(?s)` keeps `\\.` newline-safe.
static CONTEXT_JSON_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?s)"contextJSON"\s*:\s*"((?:[^"\\]|\\.)*)""#).expect("valid contextJSON regex")
});

pub(super) fn extract_context_json(html: &str) -> Option<EmbedContextWrapper> {
    let escaped = CONTEXT_JSON_RE
        .captures(html)
        .and_then(|captures| captures.get(1))
        .map(|matched| matched.as_str())?;
    let unescaped = unescape_json_string(escaped);
    serde_json::from_str(&unescaped).ok()
}

/// Fallback: extracts `gql_data` as a raw JSON object when contextJSON is
/// absent, via the shared balanced-brace scanner.
pub(super) fn extract_gql_data(html: &str) -> Option<GraphqlData> {
    serde_json::from_str(extract::extract_json_after(html, r#""gql_data""#)?).ok()
}

fn unescape_json_string(s: &str) -> String {
    let quoted = format!("\"{}\"", s);
    serde_json::from_str::<String>(&quoted)
        .unwrap_or_else(|_| s.replace("\\\"", "\"").replace("\\\\", "\\"))
}
