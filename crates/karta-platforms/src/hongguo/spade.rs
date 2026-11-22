//! Decoding of the `spade_a` native wrapper used by Hongguo motion comics.
//!
//! Encrypted motion-comic streams (CENC-AES-CTR) ship their content key inside
//! an obfuscated, Base64-wrapped blob called `spade_a`. The wrapper is a rolling
//! XOR with a per-index delta, plus a short trailing "suffix" whose decoded form
//! selects between the legacy and the `*_v2` variants of the transform.
//!
//! Arithmetic is performed in `i32` and masked with `& 0xFF` to wrap negative
//! intermediates modulo 256.

use base64::{Engine, engine::general_purpose::STANDARD as BASE64_STANDARD};

use karta_core::error::{ParserError, Result};

/// Rolling seeds for `previous_two` / `previous_one`.
const SEED_TWO: u8 = 0xFA;
const SEED_ONE: u8 = 0x55;

/// Base magnitude of the per-index delta; the popcount of the index is added.
const DELTA_BASE: i32 = 21;

/// Version tags whose prefix, when it matches the decoded suffix, selects the
/// "v2" transform (adjacent-byte swapping plus a `previous_one` subtraction).
const VERSION_TAGS: [&[u8]; 2] = [b"app_v2", b"web_v2"];

/// Swap adjacent byte pairs in place: `(0,1)`, `(2,3)`, ... A trailing odd byte
/// is left where it is.
fn swap_adjacent(data: &mut [u8]) {
    let mut index = 0;
    while index + 1 < data.len() {
        data.swap(index, index + 1);
        index += 2;
    }
}

/// Parse a single ASCII hex digit byte into its numeric value.
fn hex_digit(byte: u8) -> Option<usize> {
    (byte as char).to_digit(16).map(|digit| digit as usize)
}

/// Decode a byte slice as ASCII, rejecting anything with the high bit set.
fn ascii_string(bytes: &[u8]) -> Option<String> {
    if bytes.is_ascii() {
        std::str::from_utf8(bytes).ok().map(str::to_string)
    } else {
        None
    }
}

/// Unwrap a `spade_a` blob into its `(payload, payload_type)` pair.
///
/// The wrapper does not record the sign of its per-index delta, so both signs
/// are attempted (negative first) and the first one that yields a well-formed
/// payload wins.
pub(super) fn decode_spade_payload(value: &str) -> Result<(String, String)> {
    let raw = BASE64_STANDARD
        .decode(value.as_bytes())
        .map_err(|_| ParserError::Parse("spade_a is not valid Base64".to_string()))?;
    if raw.len() < 8 {
        return Err(ParserError::Parse(
            "spade_a payload is too short".to_string(),
        ));
    }

    // The first three bytes XOR to an ASCII digit giving the suffix length.
    let header = raw[0] ^ raw[1] ^ raw[2];
    let suffix_length = i64::from(header) - i64::from(b'0');
    let body_length = raw.len() as i64 - suffix_length - 1;
    if !(1..=15).contains(&suffix_length) || body_length < 3 {
        return Err(ParserError::Parse(
            "spade_a has an invalid native wrapper header".to_string(),
        ));
    }
    let body_length = body_length as usize;

    // raw[-suffix_length - 2] ^ raw[-suffix_length - 1], i.e. the last two body
    // bytes, key the suffix. The suffix itself follows the body.
    let suffix_key = raw[body_length - 1] ^ raw[body_length];
    let suffix: Vec<u8> = raw[body_length + 1..]
        .iter()
        .map(|byte| byte ^ suffix_key)
        .collect();
    let versioned = VERSION_TAGS.iter().any(|tag| tag.starts_with(&suffix));

    for positive_delta in [false, true] {
        // `source` holds the unmutated input for the rolling window while
        // `body` receives the transformed output.
        let mut source = raw[1..=body_length].to_vec();
        if versioned {
            swap_adjacent(&mut source);
        }
        let mut body = source.clone();

        let (mut previous_two, mut previous_one) = (SEED_TWO, SEED_ONE);
        for (index, &current) in source.iter().enumerate() {
            let magnitude = DELTA_BASE + (index as u32).count_ones() as i32;
            let delta = if positive_delta {
                magnitude
            } else {
                -magnitude
            };
            let mut transformed = i32::from(current ^ previous_two);
            if versioned {
                transformed -= i32::from(previous_one);
            }
            body[index] = ((transformed + delta) & 0xFF) as u8;
            previous_two = previous_one;
            previous_one = current;
        }
        if versioned {
            swap_adjacent(&mut body);
        }

        // body = [type_length nibble][payload][payload_type]
        let Some(type_length) = hex_digit(body[0]) else {
            continue;
        };
        if type_length == 0 || type_length + 1 >= body.len() {
            continue;
        }
        let split = body.len() - type_length;
        let (Some(payload), Some(payload_type)) =
            (ascii_string(&body[1..split]), ascii_string(&body[split..]))
        else {
            continue;
        };
        if !payload.is_empty()
            && !payload_type.is_empty()
            && payload_type.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Ok((payload, payload_type));
        }
    }

    Err(ParserError::Parse(
        "spade_a native wrapper could not be decoded".to_string(),
    ))
}

/// Extract the 128-bit CENC content key (32 lowercase hex chars) from `spade_a`.
pub(super) fn unwrap_cenc_key(spade_a: &str) -> Result<String> {
    let (payload, payload_type) = decode_spade_payload(spade_a)?;
    if payload_type != "1" {
        return Err(ParserError::Parse(format!(
            "unsupported spade_a key type {:?}",
            payload_type
        )));
    }
    if payload.len() != 32 || !payload.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ParserError::Parse(
            "spade_a did not contain a 128-bit CENC key".to_string(),
        ));
    }
    Ok(payload.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic `spade_a` test vectors covering both wrapper variants.
    const LEGACY: &str = "nrwT+1qyEf5fsxDjaZoS5myaK9JhmC3VYpsyykiwNM1LsQPK";
    /// `web_v2` transform selected by a suffix that is only a prefix of the
    /// tag, with the positive delta (tried second).
    const WEB_V2_POSITIVE_DELTA: &str = "TSJcXi2RgTlcHNkVv8SISGxuBcmIRG9vD8+ESG1tDPKIbIWejIs=";

    #[test]
    fn test_spade_round_trip() {
        let cases = [
            (LEGACY, "0123456789abcdef0123456789abcdef"),
            (WEB_V2_POSITIVE_DELTA, "ffeeddccbbaa99887766554433221100"),
        ];
        for (spade_a, key) in cases {
            assert_eq!(unwrap_cenc_key(spade_a).unwrap(), key, "{spade_a}");
        }
    }
}
