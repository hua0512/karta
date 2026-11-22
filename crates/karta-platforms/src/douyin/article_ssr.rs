use std::collections::HashMap;

use karta_core::error::{ParserError, Result};
use serde_json::{Map, Value, json};

use super::api::AwemeDetail;

const MAX_PAGE_BYTES: usize = 8 * 1024 * 1024;
const MAX_RECORDS: usize = 4096;
const MAX_NODES: usize = 32_768;
const MAX_DEPTH: usize = 64;

/// Read the requested article from data-only React Flight records.
pub(super) fn extract_article(html: &str, item_id: &str) -> Result<AwemeDetail> {
    if html.len() > MAX_PAGE_BYTES {
        return Err(invalid("Douyin article page is too large"));
    }
    let stream = flight_stream(html)?;
    let (records, texts) = flight_records(&stream)?;
    let mut pending: Vec<&Value> = records.iter().collect();
    let mut visited = 0;
    while let Some(value) = pending.pop() {
        visited += 1;
        if visited > MAX_NODES {
            return Err(invalid("Douyin article state has too many values"));
        }
        match value {
            Value::Object(object) => {
                if matches_article(object, item_id)
                    && let Ok(detail) = normalize_article(value, &texts)
                {
                    return Ok(detail);
                }
                pending.extend(object.values());
            }
            Value::Array(array) => pending.extend(array),
            _ => {}
        }
    }
    Err(invalid(
        "Requested Douyin article body was not found in PC SSR",
    ))
}

fn invalid(message: &str) -> ParserError {
    ParserError::Parse(message.into())
}

fn flight_stream(html: &str) -> Result<String> {
    const MARKER: &str = "__pace_f.push(";
    let mut stream = String::new();
    let mut rest = html;
    let mut chunks = 0;
    while let Some(start) = rest.find(MARKER) {
        rest = &rest[start + MARKER.len()..];
        let Some(array) = rest.trim_start().strip_prefix('[') else {
            continue;
        };
        let array = array.trim_start();
        let tag_end = array
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(array.len());
        if tag_end == 0 {
            continue;
        }
        let Some(payload) = array[tag_end..].trim_start().strip_prefix(',') else {
            continue;
        };
        let payload = payload.trim_start();
        let Some(end) = string_end(payload) else {
            continue;
        };
        let Some(chunk) = decode_string(&payload[..end]) else {
            rest = &payload[end..];
            continue;
        };
        chunks += 1;
        if chunks > MAX_RECORDS || stream.len().saturating_add(chunk.len()) > MAX_PAGE_BYTES {
            return Err(invalid("Douyin article Flight data exceeds limits"));
        }
        stream.push_str(&chunk);
        rest = &payload[end..];
    }
    if stream.is_empty() {
        return Err(invalid("Cannot find Douyin article Flight data"));
    }
    Ok(stream)
}

fn string_end(source: &str) -> Option<usize> {
    let bytes = source.as_bytes();
    if bytes.first() != Some(&b'"') {
        return None;
    }
    let mut index = 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'"' => return Some(index + 1),
            _ => index += 1,
        }
    }
    None
}

fn decode_string(literal: &str) -> Option<String> {
    if let Ok(value) = serde_json::from_str(literal) {
        return Some(value);
    }
    // JavaScript permits an escaped apostrophe inside a double-quoted string.
    let mut json = Vec::with_capacity(literal.len());
    let bytes = literal.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\\' && index + 1 < bytes.len() {
            if bytes[index + 1] != b'\'' {
                json.push(bytes[index]);
            }
            json.push(bytes[index + 1]);
            index += 2;
        } else {
            json.push(bytes[index]);
            index += 1;
        }
    }
    serde_json::from_slice(&json).ok()
}

fn flight_records(stream: &str) -> Result<(Vec<Value>, HashMap<&str, &str>)> {
    let mut records = Vec::new();
    let mut texts = HashMap::new();
    let mut cursor = 0;
    let mut count = 0;
    while cursor < stream.len() {
        let rest = &stream[cursor..];
        let line_end = rest.find('\n').unwrap_or(rest.len());
        let line = rest[..line_end].trim_end_matches('\r');
        cursor += line_end + usize::from(line_end < rest.len());
        if line.is_empty() {
            continue;
        }
        count += 1;
        if count > MAX_RECORDS {
            return Err(invalid("Douyin article has too many Flight records"));
        }
        let Some((id, payload)) = line.split_once(':') else {
            continue;
        };
        if id.is_empty() || id.len() > 32 || !id.bytes().all(|b| b.is_ascii_alphanumeric()) {
            continue;
        }
        if let Some(header) = payload.strip_prefix('T') {
            let Some((length, _)) = header.split_once(',') else {
                return Err(invalid("Incomplete Douyin article Flight text header"));
            };
            let byte_length = usize::from_str_radix(length, 16)
                .ok()
                .filter(|&length| length <= MAX_PAGE_BYTES)
                .ok_or_else(|| invalid("Invalid Douyin article Flight text length"))?;
            let payload_start = id.len() + length.len() + 3;
            let payload_end = payload_start
                .checked_add(byte_length)
                .ok_or_else(|| invalid("Invalid Douyin article Flight text length"))?;
            let text = rest
                .get(payload_start..payload_end)
                .ok_or_else(|| invalid("Incomplete Douyin article Flight text"))?;
            if texts.insert(id, text).is_some() {
                return Err(invalid("Duplicate Douyin article Flight text reference"));
            }
            cursor = stream.len() - rest.len() + payload_end;
        } else if (payload.starts_with('{') || payload.starts_with('['))
            && let Ok(value) = serde_json::from_str(payload)
        {
            records.push(value);
        }
    }
    Ok((records, texts))
}

fn field<'a>(object: &'a Map<String, Value>, snake: &str, camel: &str) -> Option<&'a Value> {
    object.get(snake).or_else(|| object.get(camel))
}

fn id_matches(value: &Value, item_id: &str) -> bool {
    value.as_str() == Some(item_id) || value.as_u64().is_some_and(|id| id.to_string() == item_id)
}

fn matches_article(object: &Map<String, Value>, item_id: &str) -> bool {
    let Some(info) = field(object, "article_info", "articleInfo").and_then(Value::as_object) else {
        return false;
    };
    if let Some(id) = field(object, "aweme_id", "awemeId") {
        return id_matches(id, item_id);
    }
    field(info, "article_id", "articleId").is_some_and(|id| id_matches(id, item_id))
}

fn normalize_article(value: &Value, texts: &HashMap<&str, &str>) -> Result<AwemeDetail> {
    let mut value = normalize(value, 0)?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| invalid("Invalid Douyin article detail"))?;
    let info = object
        .get_mut("article_info")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| invalid("Missing Douyin article metadata"))?;
    let raw_body = info
        .get("article_content")
        .ok_or_else(|| invalid("Missing Douyin article body"))?;
    let body = article_body(raw_body, texts, 0)
        .ok_or_else(|| invalid("Empty or unresolved Douyin article body"))?;
    info.insert("article_content".into(), Value::String(body.to_string()));
    if let Some(fe_data) = info.get_mut("fe_data") {
        if let Some(raw) = fe_data.as_str()
            && let Ok(decoded) = serde_json::from_str::<Value>(raw)
        {
            *fe_data = normalize(&decoded, 0)?;
        }
        if !fe_data.is_string() {
            *fe_data = Value::String(fe_data.to_string());
        }
    }
    normalize_covers(object);
    // Article media comes from the body and fe_data; feed-only media schemas
    // must not invalidate an otherwise usable article.
    object.remove("images");
    object.remove("music");
    serde_json::from_value(value)
        .map_err(|error| ParserError::Parse(format!("Invalid Douyin article detail: {error}")))
}

fn article_body(value: &Value, texts: &HashMap<&str, &str>, depth: usize) -> Option<Value> {
    if depth > 8 {
        return None;
    }
    if let Some(object) = value.as_object() {
        let markdown = ["markdown", "content", "text", "long_article_abstract"]
            .iter()
            .filter_map(|key| object.get(*key))
            .find_map(|value| article_body(value, texts, depth + 1))?;
        let mut body = markdown;
        if let Some(abstract_text) = object
            .get("long_article_abstract")
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
        {
            body["long_article_abstract"] = Value::String(abstract_text.to_owned());
        }
        return Some(body);
    }
    let text = value.as_str()?.trim();
    if text.is_empty() || matches!(text, "$undefined" | "$null") {
        return None;
    }
    if let Some(literal) = text.strip_prefix("$$") {
        return Some(json!({ "markdown": format!("${literal}") }));
    }
    if let Some(id) = text.strip_prefix('$')
        && !id.is_empty()
        && id.len() <= 32
        && id.bytes().all(|b| b.is_ascii_alphanumeric())
    {
        return article_body(&Value::String(texts.get(id)?.to_string()), texts, depth + 1);
    }
    if text.starts_with('{')
        && let Ok(decoded) = serde_json::from_str::<Value>(text)
    {
        return article_body(&decoded, texts, depth + 1);
    }
    Some(json!({ "markdown": text }))
}

fn normalize(value: &Value, depth: usize) -> Result<Value> {
    if depth > MAX_DEPTH {
        return Err(invalid("Douyin article state is too deeply nested"));
    }
    match value {
        Value::Array(values) => values
            .iter()
            .map(|value| normalize(value, depth + 1))
            .collect::<Result<Vec<_>>>()
            .map(Value::Array),
        Value::Object(object) => {
            let mut normalized = Map::new();
            for (key, value) in object {
                let canonical = match key.as_str() {
                    "awemeId" => "aweme_id",
                    "awemeType" => "aweme_type",
                    "itemTitle" => "item_title",
                    "createTime" => "create_time",
                    "shareUrl" => "share_url",
                    "authorInfo" => "author",
                    "avatarThumb" => "avatar_thumb",
                    "urlList" => "url_list",
                    "coverOriginalScale" | "originCover" | "origin_cover" => "cover_original_scale",
                    "articleInfo" => "article_info",
                    "articleId" => "article_id",
                    "articleTitle" => "article_title",
                    "articleContent" => "article_content",
                    "longArticleAbstract" => "long_article_abstract",
                    "feData" => "fe_data",
                    "readTime" => "read_time",
                    "hasMore" => "has_more",
                    "headPosterList" => "head_poster_list",
                    "imageList" => "image_list",
                    "playAddr" => "play_addr",
                    "playAddrH264" => "play_addr_h264",
                    "clipType" => "clip_type",
                    "stats" => "statistics",
                    "diggCount" => "digg_count",
                    "commentCount" => "comment_count",
                    "collectCount" => "collect_count",
                    "shareCount" => "share_count",
                    _ => key,
                };
                if canonical != key && object.contains_key(canonical) {
                    continue;
                }
                let mut value = normalize(value, depth + 1)?;
                if matches!(canonical, "aweme_id" | "article_id" | "uid") && value.is_number() {
                    value = Value::String(value.to_string());
                }
                normalized.insert(canonical.to_owned(), value);
            }
            Ok(Value::Object(normalized))
        }
        _ => Ok(value.clone()),
    }
}

fn normalize_covers(object: &mut Map<String, Value>) {
    if let Some(author) = object.get_mut("author").and_then(Value::as_object_mut)
        && let Some(avatar) = author.get_mut("avatar_thumb")
    {
        *avatar = url_list(avatar).unwrap_or(Value::Null);
    }
    let fallback = object
        .get("article_info")
        .and_then(|info| info.get("cover"))
        .and_then(url_list)
        .or_else(|| object.get("images")?.as_array()?.first().and_then(url_list));
    let source = object.remove("video");
    let original = source
        .as_ref()
        .and_then(|video| video.get("cover_original_scale"))
        .and_then(url_list);
    let cover = source
        .as_ref()
        .and_then(|video| video.get("cover"))
        .and_then(url_list)
        .or_else(|| original.clone())
        .or(fallback);
    let mut video = Map::new();
    if let Some(cover) = cover {
        video.insert("cover".into(), cover);
    }
    if let Some(original) = original {
        video.insert("cover_original_scale".into(), original);
    }
    if !video.is_empty() {
        object.insert("video".into(), Value::Object(video));
    }
}

fn url_list(value: &Value) -> Option<Value> {
    match value {
        Value::String(url) if !url.trim().is_empty() => Some(json!({ "url_list": [url] })),
        Value::Array(urls) => {
            let urls: Vec<_> = urls
                .iter()
                .filter_map(Value::as_str)
                .filter(|url| !url.trim().is_empty())
                .collect();
            (!urls.is_empty()).then(|| json!({ "url_list": urls }))
        }
        Value::Object(object) => object.get("url_list").and_then(url_list),
        _ => None,
    }
}
