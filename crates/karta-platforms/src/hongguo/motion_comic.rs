//! Authorized resolution of Hongguo motion-comic (漫剧) video streams.
//!
//! The ByteDance reading app exposes a `multi_video_model` endpoint that returns
//! the real playable stream for a motion-comic episode. Anonymous callers are
//! risk-control gated (`code 101000 "服务异常"`), so this module is only ever
//! reached when the operator has configured [`HongguoApiAuth`] with a captured
//! authorized session (device id, cookies, ...).
//!
//! Streams are usually CENC-AES-CTR encrypted; the content key is unwrapped
//! from `encrypt_info.spade_a` (see [`super::spade`]) and attached to the
//! [`VideoContent`] so the render layer can decrypt after downloading.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use wreq::Client;
use wreq::header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, USER_AGENT};

use karta_core::config::HongguoApiAuth;
use karta_core::data::VideoContent;
use karta_core::error::{ParserError, Result};
use karta_core::http;

use super::spade::unwrap_cenc_key;

/// Default `multi_video_model` endpoint (overridable via `HongguoApiAuth::url`).
const MOTION_COMIC_API_URL: &str =
    "https://reading.snssdk.com/novel/player/multi_video_model/default/v1/";

/// App build identifiers captured from the Android client.
const APP_VERSION_CODE: &str = "72732";
const APP_UPDATE_VERSION_CODE: &str = "7273299";

/// User-Agent of the Android reading app; the API rejects browser agents.
const APP_USER_AGENT: &str = "com.dragon.read/72732 (Linux; U; Android 14; zh_CN; Pixel 7; \
     Build/UP1A.231105.003; Cronet/TTNetVersion:5f3d03c5)";

/// Only CENC-AES-CTR is understood by the render layer's decryptor.
const SUPPORTED_ENCRYPTION_METHOD: &str = "cenc-aes-ctr";

/// Resolve one motion-comic episode into a playable (possibly encrypted) video.
pub(super) async fn resolve(
    client: &Client,
    auth: &HongguoApiAuth,
    series_id: &str,
    vid: &str,
) -> Result<VideoContent> {
    if vid.is_empty() {
        return Err(ParserError::Parse(
            "Hongguo motion comic has no playable vid".to_string(),
        ));
    }

    // A stable device id keeps repeated requests for the same series coherent.
    let device_id = auth
        .params
        .get("device_id")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| fallback_device_id(series_id));

    let endpoint = auth
        .url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .unwrap_or(MOTION_COMIC_API_URL);

    let params = merge_params(default_params(&device_id), &auth.params);
    let body = merge_json(default_body(vid), &auth.body);

    let response = client
        .post(endpoint)
        .headers(build_headers(&auth.headers))
        .query(&params)
        .json(&body)
        .send()
        .await?;
    http::check_status(&response)?;

    let payload: Value = response.json().await?;
    build_video(&payload, vid)
}

// ---------------------------------------------------------------------------
// Request construction
// ---------------------------------------------------------------------------

/// Query string the Android client sends with every reading-app API call.
fn default_params(device_id: &str) -> Vec<(String, String)> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    [
        ("aid", "8662".to_string()),
        ("app_name", "novelapp".to_string()),
        ("version_code", APP_VERSION_CODE.to_string()),
        ("device_platform", "android".to_string()),
        ("device_id", device_id.to_string()),
        ("iid", device_id.to_string()),
        ("os_version", "14".to_string()),
        ("device_type", "Pixel 7".to_string()),
        ("update_version_code", APP_UPDATE_VERSION_CODE.to_string()),
        ("_rticket", now.as_millis().to_string()),
        ("ts", now.as_secs().to_string()),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_string(), value))
    .collect()
}

/// Overlay the configured params on the defaults, replacing in place so the
/// captured request keeps its ordering. Unknown keys are appended in sorted
/// order to keep the produced query deterministic.
fn merge_params(
    mut params: Vec<(String, String)>,
    overrides: &HashMap<String, String>,
) -> Vec<(String, String)> {
    let mut extra: Vec<(&String, &String)> = overrides.iter().collect();
    extra.sort_by(|left, right| left.0.cmp(right.0));

    for (key, value) in extra {
        match params.iter_mut().find(|(name, _)| name == key) {
            Some(slot) => slot.1 = value.clone(),
            None => params.push((key.clone(), value.clone())),
        }
    }
    params
}

/// Request body for a single motion-comic episode. `1004` is the motion-comic
/// content type in `mixed_video_id_map`.
fn default_body(vid: &str) -> Value {
    json!({
        "mixed_video_id_map": { "1004": [vid] },
        "biz_param": {
            "video_platform": 1024,
            "need_all_video_definition": true,
            "need_mp4_align": true,
            "use_os_player": false,
            "caller_scene": "download",
        },
        "dr_scene": "default",
    })
}

/// Recursively overlay `overlay` on `base`; nested objects are merged, every
/// other value is replaced outright.
fn merge_json(base: Value, overlay: &Value) -> Value {
    let Some(overlay) = overlay.as_object() else {
        return base;
    };
    let Value::Object(mut target) = base else {
        return Value::Object(overlay.clone());
    };

    for (key, value) in overlay {
        match target.get_mut(key) {
            Some(existing) if existing.is_object() && value.is_object() => {
                let merged = merge_json(existing.take(), value);
                *existing = merged;
            }
            _ => {
                target.insert(key.clone(), value.clone());
            }
        }
    }
    Value::Object(target)
}

/// App headers plus any configured overrides (typically `Cookie`).
fn build_headers(overrides: &HashMap<String, String>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(APP_USER_AGENT));
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );

    for (name, value) in overrides {
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(value),
        ) {
            headers.insert(name, value);
        }
    }
    headers
}

// ---------------------------------------------------------------------------
// Response parsing
// ---------------------------------------------------------------------------

/// Turn a `multi_video_model` response into a [`VideoContent`].
///
/// Every missing or unexpected field is an error so the caller can fall back to
/// the anonymous image gallery.
fn build_video(payload: &Value, vid: &str) -> Result<VideoContent> {
    if let Some(code) = get_case(payload, &["code", "status_code"]).and_then(to_i64)
        && code != 0
    {
        let message = get_case(payload, &["message", "status_message"])
            .and_then(|value| value.as_str())
            .unwrap_or("unknown error");
        return Err(ParserError::Api {
            code,
            message: format!("Hongguo motion-comic API rejected vid {}: {}", vid, message),
        });
    }

    let entry = get_case(payload, &["data"])
        .and_then(|data| data.get(vid))
        .filter(|entry| entry.is_object())
        .ok_or_else(|| {
            ParserError::Parse(format!(
                "Hongguo motion-comic API returned no model for vid {}",
                vid
            ))
        })?;

    // `video_model` arrives either inline or as a nested JSON *string*.
    let model = match get_case(entry, &["video_model"]) {
        Some(Value::String(raw)) => serde_json::from_str::<Value>(raw).map_err(|e| {
            ParserError::Parse(format!(
                "Hongguo video_model for vid {} is invalid JSON: {}",
                vid, e
            ))
        })?,
        Some(value) if value.is_object() => value.clone(),
        _ => {
            return Err(ParserError::Parse(format!(
                "Hongguo motion-comic API returned no video_model for vid {}",
                vid
            )));
        }
    };

    let selected = get_case(&model, &["video_list"])
        .and_then(|value| value.as_array())
        .and_then(|items| {
            items
                .iter()
                .filter(|item| item.is_object())
                // `rev()` breaks ties in favor of the first listed entry.
                .rev()
                .max_by_key(|item| quality_rank(item))
        })
        .ok_or_else(|| {
            ParserError::Parse(format!(
                "Hongguo video_model for vid {} contains no quality list",
                vid
            ))
        })?;

    let media_url = ["main_url", "backup_url"]
        .into_iter()
        .filter_map(|key| get_case(selected, &[key]).and_then(|value| value.as_str()))
        .find(|url| url.starts_with("https://") || url.starts_with("http://"))
        .ok_or_else(|| {
            ParserError::Parse(format!(
                "Hongguo highest quality for vid {} has no media URL",
                vid
            ))
        })?;

    let decryption_key = extract_decryption_key(selected)?;

    let meta = get_case(selected, &["video_meta"]);
    let width = nested_int(meta, &["vwidth", "width"]).and_then(|n| u32::try_from(n).ok());
    let height = nested_int(meta, &["vheight", "height"]).and_then(|n| u32::try_from(n).ok());
    let size = nested_int(meta, &["size"]).and_then(|n| u64::try_from(n).ok());
    let duration = get_case(&model, &["video_duration"])
        .and_then(to_i64)
        .and_then(|n| u64::try_from(n).ok());
    let quality = nested_str(meta, &["definition"])
        .map(str::to_string)
        .or_else(|| super::definition_label(width, height));

    // The motion-comic CDN is served to the app, not the browser: download with
    // the app User-Agent (and no `hongguoduanju.com` referer, which is wrong for
    // the ByteDance CDN) so the encrypted byte-range fetch is not rejected.
    let mut video = VideoContent::new()
        .with_url(media_url)
        .with_download_headers(vec![("User-Agent".to_string(), APP_USER_AGENT.to_string())]);
    if let Some(key) = decryption_key {
        video = video.with_decryption_key(key);
    }
    if let Some(duration) = duration {
        video = video.with_duration(duration);
    }
    if let (Some(width), Some(height)) = (width, height) {
        video = video.with_dimensions(width, height);
    }
    if let Some(size) = size {
        video = video.with_size(size);
    }
    if let Some(quality) = quality {
        video = video.with_quality(quality);
    }
    Ok(video)
}

/// Unwrap the CENC content key when the selected stream is encrypted.
fn extract_decryption_key(selected: &Value) -> Result<Option<String>> {
    let Some(info) = get_case(selected, &["encrypt_info"]) else {
        return Ok(None);
    };
    if !get_case(info, &["encrypt"]).is_some_and(is_truthy) {
        return Ok(None);
    }

    let method = get_case(info, &["encryption_method"])
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    if !method.eq_ignore_ascii_case(SUPPORTED_ENCRYPTION_METHOD) {
        return Err(ParserError::Parse(format!(
            "unsupported Hongguo media encryption method {:?}",
            method
        )));
    }

    let spade_a = get_case(info, &["spade_a"])
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ParserError::Parse("encrypted Hongguo video_model has no spade_a".to_string())
        })?;
    Ok(Some(unwrap_cenc_key(spade_a)?))
}

/// Sortable quality key: pixel count, then height, width, definition label,
/// bitrate and finally byte size.
fn quality_rank(item: &Value) -> (u64, u64, u64, u64, u64, u64) {
    let meta = get_case(item, &["video_meta"]);
    let width = nested_u64(meta, &["vwidth", "width"]);
    let height = nested_u64(meta, &["vheight", "height"]);
    let definition = nested_str(meta, &["definition"])
        .and_then(leading_number)
        .unwrap_or(0);

    (
        width.saturating_mul(height),
        height,
        width,
        definition,
        nested_u64(meta, &["real_bitrate", "bitrate"]),
        nested_u64(meta, &["size"]),
    )
}

/// Integer field of an optional nested object.
fn nested_int(parent: Option<&Value>, names: &[&str]) -> Option<i64> {
    parent
        .and_then(|value| get_case(value, names))
        .and_then(to_i64)
}

/// Like [`nested_int`], defaulting to `0` and clamped to `u64` — for sort keys.
fn nested_u64(parent: Option<&Value>, names: &[&str]) -> u64 {
    nested_int(parent, names)
        .and_then(|n| u64::try_from(n).ok())
        .unwrap_or(0)
}

/// Non-empty string field of an optional nested object.
fn nested_str<'a>(parent: Option<&'a Value>, names: &[&str]) -> Option<&'a str> {
    parent
        .and_then(|value| get_case(value, names))
        .and_then(|value| value.as_str())
        .filter(|text| !text.is_empty())
}

/// First run of digits in `text` (e.g. `"1080p"` -> `1080`).
fn leading_number(text: &str) -> Option<u64> {
    let start = text.find(|c: char| c.is_ascii_digit())?;
    text[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .ok()
}

/// Case- and separator-insensitive key lookup (`video-id` matches `video_id`).
fn get_case<'a>(value: &'a Value, names: &[&str]) -> Option<&'a Value> {
    let object = value.as_object()?;

    for name in names {
        for (key, found) in object {
            if key.eq_ignore_ascii_case(name) {
                return Some(found);
            }
        }
    }
    for name in names {
        let wanted = canonical_key(name);
        for (key, found) in object {
            if canonical_key(key) == wanted {
                return Some(found);
            }
        }
    }
    None
}

/// Lowercase a key and drop every non-alphanumeric character.
fn canonical_key(key: &str) -> String {
    key.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Best-effort integer coercion from bool, number, or comma-formatted string.
fn to_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Bool(flag) => Some(i64::from(*flag)),
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_f64().map(|float| float as i64)),
        Value::String(text) => {
            let text = text.trim().replace(',', "");
            if text.is_empty() {
                return None;
            }
            text.parse::<i64>()
                .ok()
                .or_else(|| text.parse::<f64>().ok().map(|float| float as i64))
        }
        _ => None,
    }
}

/// Evaluate JSON truthiness (non-zero number, non-empty string/collection, or `true`).
fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64().is_some_and(|float| float != 0.0),
        Value::String(text) => !text.is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(fields) => !fields.is_empty(),
        Value::Null => false,
    }
}

// ---------------------------------------------------------------------------
// Device id
// ---------------------------------------------------------------------------

/// Derive a stable pseudo device id from a series id:
/// `7e18 + be_u64(sha256(series_id)[..8]) % 2e18`.
fn fallback_device_id(series_id: &str) -> String {
    let digest = sha256(series_id.as_bytes());
    let mut head = [0u8; 8];
    head.copy_from_slice(&digest[..8]);
    let value =
        7_000_000_000_000_000_000u64 + u64::from_be_bytes(head) % 2_000_000_000_000_000_000u64;
    value.to_string()
}

/// SHA-256 round constants.
#[rustfmt::skip]
const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// Minimal SHA-256, used only for [`fallback_device_id`]. Kept local so the
/// crate does not need a hashing dependency for one 8-byte digest prefix.
fn sha256(data: &[u8]) -> [u8; 32] {
    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    let bit_length = (data.len() as u64).wrapping_mul(8);
    let mut message = data.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_length.to_be_bytes());

    for chunk in message.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for (index, word) in chunk.as_chunks::<4>().0.iter().enumerate() {
            w[index] = u32::from_be_bytes(*word);
        }
        for index in 16..64 {
            let previous = w[index - 15];
            let s0 = previous.rotate_right(7) ^ previous.rotate_right(18) ^ (previous >> 3);
            let recent = w[index - 2];
            let s1 = recent.rotate_right(17) ^ recent.rotate_right(19) ^ (recent >> 10);
            w[index] = w[index - 16]
                .wrapping_add(s0)
                .wrapping_add(w[index - 7])
                .wrapping_add(s1);
        }

        let mut a = state[0];
        let mut b = state[1];
        let mut c = state[2];
        let mut d = state[3];
        let mut e = state[4];
        let mut f = state[5];
        let mut g = state[6];
        let mut h = state[7];

        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choose = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(choose)
                .wrapping_add(SHA256_K[index])
                .wrapping_add(w[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(majority);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        for (slot, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(value);
        }
    }

    let mut digest = [0u8; 32];
    for (index, word) in state.iter().enumerate() {
        digest[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    digest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fallback_device_id_is_stable_and_in_range() {
        let first = fallback_device_id("7111943588446997534");
        // 7e18 + be_u64(sha256(id)[..8]) % 2e18
        assert_eq!(first, "7503431643688584130");
    }
}
