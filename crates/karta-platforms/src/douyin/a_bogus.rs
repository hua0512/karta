//! Douyin a-bogus signature generation.
//!
//! Generates the `a_bogus` query parameter required by Douyin's web API
//! to pass anti-bot verification.

#![allow(dead_code)]

mod cipher;
mod encoding;
mod payload;
pub(super) mod sm3;

use encoding::{ALPHABET_S4, custom_b64_encode, random_bytes, random_values};
use payload::{generate_rc4_bb_payload, timestamp_now_millis};

/// Generate the `a_bogus` parameter for a Douyin API request.
///
/// # Arguments
///
/// * `url_search_params` - The full query string without leading `?`.
/// * `user_agent` - The User-Agent header value.
///
/// # Returns
///
/// The a_bogus signature string, ready to append as `&a_bogus={value}`.
pub(super) fn generate_a_bogus(url_search_params: &str, user_agent: &str) -> String {
    sign(
        url_search_params,
        user_agent,
        random_values(),
        timestamp_now_millis,
    )
}

/// [`generate_a_bogus`] with its random values and clock supplied.
fn sign(
    url_search_params: &str,
    user_agent: &str,
    random: [u16; 3],
    now_millis: impl FnMut() -> u64,
) -> String {
    let random_bytes = random_bytes(random);
    let rc4_payload = generate_rc4_bb_payload(url_search_params, user_agent, now_millis);

    let mut combined = Vec::with_capacity(random_bytes.len() + rc4_payload.len());
    combined.extend_from_slice(&random_bytes);
    combined.extend_from_slice(&rc4_payload);

    let mut result = custom_b64_encode(&combined, ALPHABET_S4);
    result.push('=');
    result
}

#[cfg(test)]
mod tests {
    use super::sm3::sm3_hash;
    use super::*;

    const DOUYIN_UA: &str = "Mozilla/5.0 (Linux; Android 5.0; SM-G900P Build/LRX21T) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/70.0.3538.25 Mobile Safari/537.36";

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    /// SM3 test vectors including multi-block inputs, since signed query
    /// strings are hundreds of bytes long.
    #[test]
    fn sm3_matches_reference_vectors() {
        let one_block = "abcd".repeat(16);
        let long_input =
            "device_platform=webapp&aid=6383&channel=channel_pc_web&aweme_id=7521023890996514083"
                .repeat(3);
        let single: [(&[u8], &str); 3] = [
            (
                b"abc",
                "66c7f0f462eeedd9d1f2d46bdc10e4e24167c4875cf2f7a2297da02b8f4ba8e0",
            ),
            // GB/T 32905 example 2: exactly one block before padding.
            (
                one_block.as_bytes(),
                "debe9ff92275b8a138604889c18e5a4d6fdb70e5387e5765293dcba39c0c5732",
            ),
            (
                long_input.as_bytes(),
                "12da3cb1164a9ed2a48bbd909cbb66292b0ef3d1a929daa5317999716e48a21c",
            ),
        ];
        for (input, expected) in single {
            assert_eq!(
                hex(&sm3_hash(input)),
                expected,
                "sm3({} bytes)",
                input.len()
            );
        }
    }

    /// With the random values and the clock fixed the signature is
    /// deterministic.
    #[test]
    fn signature_is_fixed_for_given_random_values_and_clock() {
        let params = "device_platform=webapp&aid=6383&aweme_id=7521023890996514083";
        let mut clock = [1_700_000_000_000u64, 1_700_000_000_007].into_iter();

        let signature = sign(params, DOUYIN_UA, [5000, 3000, 7000], || {
            clock.next().expect("two clock reads")
        });

        assert_eq!(
            signature,
            "xfmZ/5LfdEgs6DSv56KLfY3q6VZ3Y0OI0trEMD2funf-qL39HMYD9exoIBGvXYfjwG/-IeYjy4hbT3ohrQ2y8qwf9W0L/25gsDSkKl12so0j53inCLf/E0iE5hsAtFH8svr4iKi8owICSYyhldAJ5kIlO62-zo0/93S="
        );
        assert_eq!(signature.len(), 164);
    }
}
