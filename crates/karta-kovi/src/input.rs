use kovi::{Message, Segment};

/// Known URL field names inside QQ JSON card `meta.*` objects, in priority
/// order: `qqdocurl` (the shared document/mini-program deep link) is the most
/// specific, then `jumpUrl` (generic card target), then a bare `url`.
const CARD_URL_FIELDS: &[&str] = &["qqdocurl", "jumpUrl", "url"];

/// Only http(s) URLs are parseable; cards also carry app-scheme deep links
/// (`mqqapi://`, `tianxuan://`, ...) that must never reach the parsers.
fn is_http_url(url: &str) -> bool {
    let scheme_end = url.find("://").unwrap_or(0);
    url[..scheme_end].eq_ignore_ascii_case("http")
        || url[..scheme_end].eq_ignore_ascii_case("https")
}

/// Extract a URL from a QQ JSON card / mini-program message segment.
///
/// QQ apps (Bilibili, Douyin, Xiaoheihe, etc.) share links as JSON "card"
/// segments rather than plain text. The card payload lives in
/// `segment.data["data"]` as a JSON string whose structure is:
///
/// ```json
/// { "meta": { "<app_key>": { "jumpUrl": "...", "qqdocurl": "...", ... } } }
/// ```
///
/// Different apps use different `meta` keys (`detail_1`, `news`, `music`,
/// `appbar`, etc.) and different URL fields. Instead of hardcoding each
/// combination, we iterate over all `meta` entries and probe for known URL
/// field names.
pub(crate) fn extract_url_from_json_card(segment: &Segment) -> Option<String> {
    if segment.kind != "json" {
        return None;
    }

    let raw_str = segment.data.get("data").and_then(|v| v.as_str())?;
    let raw: serde_json::Value = serde_json::from_str(raw_str).ok()?;
    let meta = raw.get("meta")?.as_object()?;

    // Field priority outranks meta-entry position: every entry is probed for
    // `qqdocurl` before any `jumpUrl` is considered, and so on. Within one
    // field, entries scan in serde_json's sorted key order, so the pick stays
    // deterministic when several apps populate the same field. Non-http(s)
    // candidates are skipped so scanning falls through to the next one.
    for field in CARD_URL_FIELDS {
        for obj in meta.values() {
            let Some(obj) = obj.as_object() else {
                continue;
            };
            if let Some(url) = obj
                .get(*field)
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|u| !u.is_empty() && is_http_url(u))
            {
                return Some(url.to_string());
            }
        }
    }

    tracing::debug!(
        "JSON card had no extractable URL, meta keys: {:?}",
        meta.keys().collect::<Vec<_>>()
    );
    None
}

/// Extract parseable text from a Kovi message event.
///
/// Checks JSON card segments first (mini-programs / app shares), then falls
/// back to plain text.
pub(crate) fn extract_text_from_message(
    message: &Message,
    plain_text: Option<&str>,
) -> Option<String> {
    for segment in message.iter() {
        if let Some(url) = extract_url_from_json_card(segment) {
            tracing::debug!("Extracted URL from JSON card: {}", url);
            return Some(url);
        }
    }

    plain_text
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn card(payload: &str) -> Segment {
        Segment::new("json", json!({ "data": payload }))
    }

    #[test]
    fn json_card_url_cases() {
        // (case, segment, extracted URL)
        let cases = [
            (
                "app-scheme link falls through to the next field",
                card(
                    r#"{"meta":{"detail_1":{"qqdocurl":"tianxuan://open?id=1","jumpUrl":"https://example.com/ok"}}}"#,
                ),
                Some("https://example.com/ok"),
            ),
            (
                // "aaa" sorts before "zzz", but qqdocurl outranks jumpUrl.
                "field priority beats meta key order",
                card(
                    r#"{"meta":{"aaa":{"jumpUrl":"https://example.com/jump"},"zzz":{"qqdocurl":"https://example.com/doc"}}}"#,
                ),
                Some("https://example.com/doc"),
            ),
        ];

        for (case, segment, expected) in cases {
            assert_eq!(
                extract_url_from_json_card(&segment).as_deref(),
                expected,
                "{case}"
            );
        }
    }
}
