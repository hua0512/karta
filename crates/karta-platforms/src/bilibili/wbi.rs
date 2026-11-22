//! WBI signature for Bilibili Web API risk control.
//!
//! Implements the signing algorithm described at:
//! <https://github.com/SocialSisterYi/bilibili-API-collect/blob/master/docs/misc/sign/wbi.md>
//!
//! The algorithm:
//! 1. Fetch `img_key` and `sub_key` from the nav API response.
//! 2. Generate `mixin_key` by reordering `img_key + sub_key` via a fixed lookup table.
//! 3. Sign request params: add `wts` (timestamp), sort by key, filter forbidden chars,
//!    URL-encode, append mixin_key, MD5 hash → `w_rid`.

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use parking_lot::RwLock;

/// Fixed reorder table for generating mixin_key from img_key + sub_key.
const MIXIN_KEY_ENC_TAB: [usize; 64] = [
    46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42, 19, 29,
    28, 14, 39, 12, 38, 41, 13, 37, 48, 7, 16, 24, 55, 40, 61, 26, 17, 0, 1, 60, 51, 30, 4, 22, 25,
    54, 21, 56, 59, 6, 63, 57, 62, 11, 36, 20, 34, 44, 52,
];

/// Characters that must be removed from parameter values before signing.
const FORBIDDEN_CHARS: &[char] = &['!', '\'', '(', ')', '*'];

/// How long fetched keys are trusted before they are fetched again.
///
/// Bilibili rotates `img_key`/`sub_key` about once a day; refreshing well
/// inside that window keeps signatures valid without a nav call per request.
pub const KEY_TTL: Duration = Duration::from_secs(4 * 60 * 60);

/// API code Bilibili answers with when its risk control rejects a request,
/// which includes a missing or wrong WBI signature.
const RISK_CONTROL_CODE: i64 = -352;

struct MixinKey {
    key: String,
    fetched_at: Instant,
}

/// Cached WBI signing state.
#[derive(Clone)]
pub struct WbiSigner {
    inner: Arc<RwLock<Option<MixinKey>>>,
}

impl WbiSigner {
    /// Create a new (empty) WBI signer. Must call `update_keys` before signing.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(None)),
        }
    }

    /// Update the mixin_key from fresh img_url and sub_url obtained from the nav API.
    ///
    /// The URLs look like: `https://i0.hdslb.com/bfs/wbi/{hash}.png`
    /// We extract the hash (filename without extension) as the key.
    pub fn update_keys(&self, img_url: &str, sub_url: &str) {
        self.update_keys_at(img_url, sub_url, Instant::now());
    }

    fn update_keys_at(&self, img_url: &str, sub_url: &str, fetched_at: Instant) {
        let img_key = extract_key_from_url(img_url);
        let sub_key = extract_key_from_url(sub_url);
        let key = generate_mixin_key(&img_key, &sub_key);
        *self.inner.write() = Some(MixinKey { key, fetched_at });
    }

    /// Whether the keys are missing or older than [`KEY_TTL`].
    pub fn needs_refresh(&self) -> bool {
        self.needs_refresh_at(Instant::now())
    }

    fn needs_refresh_at(&self, now: Instant) -> bool {
        match self.inner.read().as_ref() {
            Some(mixin) => now.saturating_duration_since(mixin.fetched_at) >= KEY_TTL,
            None => true,
        }
    }

    /// Sign the given query parameters and return a new list with `wts` and `w_rid` appended.
    ///
    /// Returns `None` if the mixin_key has not been set yet.
    pub fn sign_params(&self, params: &[(&str, &str)]) -> Option<Vec<(String, String)>> {
        let mixin_key = self.inner.read().as_ref()?.key.clone();
        let wts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Some(sign_params_with_key(params, &mixin_key, wts))
    }
}

/// Whether an API response is Bilibili rejecting the request's WBI signature.
///
/// A missing or stale signature is answered with code -352 or, on some
/// endpoints, code 0 whose `data` holds only a `v_voucher` captcha ticket.
pub(super) fn is_signature_rejection(response: &serde_json::Value) -> bool {
    response.get("code").and_then(|v| v.as_i64()) == Some(RISK_CONTROL_CODE)
        || response.pointer("/data/v_voucher").is_some()
}

impl Default for WbiSigner {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract the key (filename without extension) from a WBI URL.
///
/// Input:  `https://i0.hdslb.com/bfs/wbi/7cd084941338484aae1ad9425b84077c.png`
/// Output: `7cd084941338484aae1ad9425b84077c`
#[inline]
fn extract_key_from_url(url: &str) -> String {
    url.rsplit('/')
        .next()
        .unwrap_or("")
        .split('.')
        .next()
        .unwrap_or("")
        .to_string()
}

/// Generate the mixin_key from img_key and sub_key using the reorder table.
fn generate_mixin_key(img_key: &str, sub_key: &str) -> String {
    let raw = format!("{}{}", img_key, sub_key);
    let raw_bytes: Vec<u8> = raw.bytes().collect();
    MIXIN_KEY_ENC_TAB
        .iter()
        .filter_map(|&idx| raw_bytes.get(idx).copied())
        .take(32)
        .map(|b| b as char)
        .collect()
}

/// Sign parameters with the given mixin_key.
///
/// Returns a new parameter list containing all original params plus `wts`
/// (the Unix timestamp in seconds) and `w_rid`.
fn sign_params_with_key(
    params: &[(&str, &str)],
    mixin_key: &str,
    wts: u64,
) -> Vec<(String, String)> {
    let wts = wts.to_string();

    // Collect all params including wts
    let mut all_params: Vec<(String, String)> = params
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    all_params.push(("wts".to_string(), wts.clone()));

    // Sort by key
    all_params.sort_by(|a, b| a.0.cmp(&b.0));

    // Build query string with filtered values (remove forbidden chars)
    let query_string: String = all_params
        .iter()
        .map(|(k, v)| {
            let filtered_v: String = v.chars().filter(|c| !FORBIDDEN_CHARS.contains(c)).collect();
            format!("{}={}", percent_encode(k), percent_encode(&filtered_v))
        })
        .collect::<Vec<_>>()
        .join("&");

    // Compute w_rid = MD5(query_string + mixin_key)
    let to_hash = format!("{}{}", query_string, mixin_key);
    let w_rid = format!("{:x}", md5::compute(to_hash.as_bytes()));

    // Return original params (unsorted) + wts + w_rid
    let mut result: Vec<(String, String)> = params
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    result.push(("wts".to_string(), wts));
    result.push(("w_rid".to_string(), w_rid));
    result
}

/// Minimal percent-encoding for WBI signing.
///
/// Bilibili requires uppercase hex and `%20` for spaces (not `+`).
fn percent_encode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(b as char);
            }
            _ => {
                result.push_str(&format!("%{:02X}", b));
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Known WBI signing test vectors, including a value that requires
    /// forbidden-character filtering and percent-encoding.
    #[test]
    fn signer_matches_documented_vectors() {
        let signer = WbiSigner::new();
        signer.update_keys(
            "https://i0.hdslb.com/bfs/wbi/7cd084941338484aae1ad9425b84077c.png",
            "https://i0.hdslb.com/bfs/wbi/4932caff0ff746eab6f01bf08b70ac45.png",
        );
        let mixin_key = signer.inner.read().as_ref().unwrap().key.clone();
        assert_eq!(mixin_key, "ea1db124af3c7062474693fa704f4ff8");

        let cases: [(&[(&str, &str)], &str); 2] = [
            (
                &[("foo", "114"), ("bar", "514"), ("zab", "1919810")],
                "8f6f2b5b3d485fe1886cec6a0be8c5d4",
            ),
            (
                &[("keyword", "中文 a!'()*b"), ("foo", "114")],
                "8d4c4be4227044b70500ba56374e3aff",
            ),
        ];
        for (params, w_rid) in cases {
            let signed = sign_params_with_key(params, &mixin_key, 1702204169);

            let mut expected: Vec<(String, String)> = params
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            expected.push(("wts".to_string(), "1702204169".to_string()));
            expected.push(("w_rid".to_string(), w_rid.to_string()));
            assert_eq!(signed, expected, "{params:?}");
        }
    }
}
