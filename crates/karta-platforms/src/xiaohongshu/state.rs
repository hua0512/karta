//! Xiaohongshu SSR state contains a small subset of JavaScript literals.

use karta_core::error::{ParserError, Result};
use karta_core::extract;

pub(super) fn extract_initial_state(html: &str) -> Result<serde_json::Value> {
    let raw = extract::extract_json_after(html, "window.__INITIAL_STATE__").ok_or_else(|| {
        ParserError::Parse("小红书页面缺少笔记数据，可能需要登录或验证".to_string())
    })?;

    serde_json::from_str(raw)
        .or_else(|_| serde_json::from_str(&normalize_js_state(raw)))
        .map_err(|e| ParserError::Parse(format!("Failed to parse Xiaohongshu data: {e}")))
}

/// Normalize value tokens, preserving quoted text byte for byte. Collections
/// belong to unused SSR stores and can be discarded without evaluating JS.
fn normalize_js_state(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut normalized = String::with_capacity(raw.len());
    let mut copied = 0;
    let mut cursor = 0;
    let mut previous = 0;

    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if matches!(byte, b'"' | b'\'') {
            previous = byte;
            cursor = quoted_end(bytes, cursor);
            continue;
        }

        if matches!(previous, b':' | b',' | b'[') {
            let tail = &bytes[cursor..];
            let replacement = if tail.starts_with(b"undefined")
                && !tail.get(9).is_some_and(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$') || !byte.is_ascii()
                }) {
                Some((cursor + 9, "null"))
            } else {
                collection_end(tail).map(|end| (cursor + end, "{}"))
            };
            if let Some((end, value)) = replacement {
                normalized.push_str(&raw[copied..cursor]);
                normalized.push_str(value);
                copied = end;
                cursor = end;
                previous = b'}';
                continue;
            }
        }

        if !byte.is_ascii_whitespace() {
            previous = byte;
        }
        cursor += 1;
    }

    normalized.push_str(&raw[copied..]);
    normalized
}

fn quoted_end(bytes: &[u8], start: usize) -> usize {
    let quote = bytes[start];
    let mut cursor = start + 1;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' => cursor = (cursor + 2).min(bytes.len()),
            byte if byte == quote => return cursor + 1,
            _ => cursor += 1,
        }
    }
    bytes.len()
}

/// Find the matching closing parenthesis, including nested constructors and
/// parentheses inside escaped strings. Incomplete expressions stay invalid.
fn collection_end(bytes: &[u8]) -> Option<usize> {
    let rest = bytes.strip_prefix(b"new")?;
    if !rest.first()?.is_ascii_whitespace() {
        return None;
    }
    let mut cursor = 3;
    while bytes.get(cursor)?.is_ascii_whitespace() {
        cursor += 1;
    }
    if !bytes[cursor..].starts_with(b"Map") && !bytes[cursor..].starts_with(b"Set") {
        return None;
    }
    cursor += 3;
    while bytes.get(cursor)?.is_ascii_whitespace() {
        cursor += 1;
    }
    if bytes[cursor] != b'(' {
        return None;
    }

    let mut depth = 0;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'"' | b'\'' => {
                cursor = quoted_end(bytes, cursor);
                continue;
            }
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(cursor + 1);
                }
            }
            _ => {}
        }
        cursor += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_initial_state_normalizes_js_values_without_changing_strings() {
        let desc = r#"保留 [undefined] and :undefined; new Map([]); "quoted" \"#;
        let json = serde_json::json!({"desc": desc});
        let js = format!(
            r#"{{"desc":{},"a":undefined,"list":[undefined,1, undefined],"b": undefined,"map":new Map([["key)",new Set(["value)"])]]),"set":new Set(["(quoted)",new Map([])])}}"#,
            serde_json::to_string(desc).unwrap()
        );
        for (raw, expected) in [
            (json.to_string(), json),
            (
                js,
                serde_json::json!({"desc": desc, "a": null, "list": [null, 1, null], "b": null, "map": {}, "set": {}}),
            ),
        ] {
            let html = format!("<script>window.__INITIAL_STATE__={raw};</script>");
            assert_eq!(extract_initial_state(&html).unwrap(), expected);
        }

        assert!(
            extract_initial_state(
                r#"<script>window.__INITIAL_STATE__={"map":new Map([]};</script>"#
            )
            .is_err(),
            "an unclosed constructor must remain a parse error"
        );
    }
}
