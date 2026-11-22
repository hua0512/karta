//! Marker-anchored JSON extraction from HTML documents.
//!
//! Promoted from the Hongguo parser so every platform that embeds SSR state
//! (`window._ROUTER_DATA`, `window.__INITIAL_STATE__`, ...) can share the
//! same string/escape-aware brace scanner instead of fragile regexes.

/// Slice the first balanced JSON object that follows `marker` in `html`.
///
/// Locates the marker, finds the first `{` after it, then scans char-by-char
/// tracking string state and brace depth until the matching `}` (depth back
/// to 0). The structural characters (`{ } " \`) are all ASCII, so scanning
/// bytes is safe even though the embedded content is UTF-8.
///
/// Returns `None` if the marker is absent or no balanced object follows it.
pub fn extract_json_after<'a>(html: &'a str, marker: &str) -> Option<&'a str> {
    let idx = html.find(marker)? + marker.len();

    let bytes = html.as_bytes();
    let start = bytes[idx..].iter().position(|&b| b == b'{')? + idx;

    let mut depth: i64 = 0;
    let mut in_string = false;
    let mut escape = false;

    for (offset, &b) in bytes[start..].iter().enumerate() {
        if in_string {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&html[start..=start + offset]);
                }
            }
            _ => {}
        }
    }

    None
}

/// Parse the object located by [`extract_json_after`] into a
/// [`serde_json::Value`]. Returns `None` when no object is found or the
/// slice is not valid JSON.
pub fn extract_json_value(html: &str, marker: &str) -> Option<serde_json::Value> {
    serde_json::from_str(extract_json_after(html, marker)?).ok()
}
