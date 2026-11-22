/// Instagram's custom base-64 alphabet for shortcode to numeric pk conversion.
const IG_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Generate a random 32-char hex CSRF token.
///
/// Instagram accepts any token that matches the cookie value, so this only needs
/// to be unique per parser session.
pub(super) fn generate_csrf_token() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        ^ (std::process::id() as u128);
    format!("{seed:032x}")
}

/// Decodes an Instagram shortcode to a numeric media pk.
///
/// Private posts may append a 28-character suffix. When the shortcode is longer
/// than that suffix length, strip it before decoding, matching yt-dlp behavior.
pub(super) fn shortcode_to_pk(shortcode: &str) -> Option<u64> {
    let code = if shortcode.len() > 28 {
        &shortcode[..shortcode.len() - 28]
    } else {
        shortcode
    };

    let mut pk: u64 = 0;
    for &byte in code.as_bytes() {
        let pos = IG_ALPHABET
            .iter()
            .position(|&candidate| candidate == byte)?;
        pk = pk.checked_mul(64)?.checked_add(pos as u64)?;
    }
    Some(pk)
}
