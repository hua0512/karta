use super::cipher::rc4_encrypt;
use super::encoding::{ALPHABET_S3, custom_b64_encode};
use super::sm3::{sm3_double, sm3_hash};

/// Hardcoded browser environment fingerprint string.
pub(super) const WINDOW_ENV_STR: &str =
    "1536|747|1536|834|0|30|0|0|1536|834|1536|864|1525|747|24|24|Win32";

const ARGUMENTS: [u32; 3] = [0, 1, 14];
const AID: u32 = 6383;
const PAGE_ID: u32 = 6241;

/// Decompose a Unix millisecond timestamp into 6 bytes.
pub(super) fn timestamp_to_6_bytes(ms: u64) -> [u8; 6] {
    [
        ((ms >> 24) & 0xff) as u8,
        ((ms >> 16) & 0xff) as u8,
        ((ms >> 8) & 0xff) as u8,
        (ms & 0xff) as u8,
        ((ms >> 32) & 0xff) as u8,
        ((ms >> 40) & 0xff) as u8,
    ]
}

/// Build the encrypted payload. `now_millis` is read once before hashing and
/// once after to record start and end timestamps.
pub(super) fn generate_rc4_bb_payload(
    url_search_params: &str,
    user_agent: &str,
    mut now_millis: impl FnMut() -> u64,
) -> Vec<u8> {
    let suffix = "cus";
    let start_time = now_millis();

    let mut combined = Vec::with_capacity(url_search_params.len() + suffix.len());
    combined.extend_from_slice(url_search_params.as_bytes());
    combined.extend_from_slice(suffix.as_bytes());
    let params_hash = sm3_double(&combined);
    let suffix_hash = sm3_double(suffix.as_bytes());

    let ua_rc4 = rc4_encrypt(user_agent.as_bytes(), &[0, 1, 14]);
    let ua_b64 = custom_b64_encode(&ua_rc4, ALPHABET_S3);
    let ua_hash = sm3_hash(ua_b64.as_bytes());

    let end_time = now_millis();
    let start_bytes = timestamp_to_6_bytes(start_time);
    let end_bytes = timestamp_to_6_bytes(end_time);

    let arg0_bytes = ARGUMENTS[0].to_be_bytes();
    let arg1_lo = (ARGUMENTS[1] / 256) as u8;
    let arg1_mod = (ARGUMENTS[1] % 256) as u8;
    let arg1_hi = ARGUMENTS[1].to_be_bytes();
    let arg2_bytes = ARGUMENTS[2].to_be_bytes();

    let page_id_bytes = PAGE_ID.to_be_bytes();
    let aid_b57 = (AID & 0xff) as u8;
    let aid_b58 = ((AID >> 8) & 0xff) as u8;
    let aid_b59 = ((AID >> 16) & 0xff) as u8;
    let aid_b60 = ((AID >> 24) & 0xff) as u8;

    let window_env = WINDOW_ENV_STR.as_bytes();
    let env_len = window_env.len() as u16;
    let env_len_lo = (env_len & 0xff) as u8;
    let env_len_hi = ((env_len >> 8) & 0xff) as u8;

    let b18 = 44u8;
    let b20 = start_bytes[0];
    let b21 = start_bytes[1];
    let b22 = start_bytes[2];
    let b23 = start_bytes[3];
    let b24 = start_bytes[4];
    let b25 = start_bytes[5];
    let b26 = arg0_bytes[0];
    let b27 = arg0_bytes[1];
    let b28 = arg0_bytes[2];
    let b29 = arg0_bytes[3];
    let b30 = arg1_lo;
    let b31 = arg1_mod;
    let b32 = arg1_hi[0];
    let b33 = arg1_hi[1];
    let b34 = arg2_bytes[0];
    let b35 = arg2_bytes[1];
    let b36 = arg2_bytes[2];
    let b37 = arg2_bytes[3];
    let b38 = params_hash[21];
    let b39 = params_hash[22];
    let b40 = suffix_hash[21];
    let b41 = suffix_hash[22];
    let b42 = ua_hash[23];
    let b43 = ua_hash[24];
    let b44 = end_bytes[0];
    let b45 = end_bytes[1];
    let b46 = end_bytes[2];
    let b47 = end_bytes[3];
    let b48 = 3u8;
    let b49 = end_bytes[4];
    let b50 = end_bytes[5];
    let b52 = page_id_bytes[0];
    let b53 = page_id_bytes[1];
    let b54 = page_id_bytes[2];
    let b55 = page_id_bytes[3];
    let b57 = aid_b57;
    let b58 = aid_b58;
    let b59 = aid_b59;
    let b60 = aid_b60;
    let b65 = env_len_lo;
    let b66 = env_len_hi;
    let b70 = 0u8;
    let b71 = 0u8;

    let checksum = b18
        ^ b20
        ^ b26
        ^ b30
        ^ b38
        ^ b40
        ^ b42
        ^ b21
        ^ b27
        ^ b31
        ^ b35
        ^ b39
        ^ b41
        ^ b43
        ^ b22
        ^ b28
        ^ b32
        ^ b36
        ^ b23
        ^ b29
        ^ b33
        ^ b37
        ^ b44
        ^ b45
        ^ b46
        ^ b47
        ^ b48
        ^ b49
        ^ b50
        ^ b24
        ^ b25
        ^ b52
        ^ b53
        ^ b54
        ^ b55
        ^ b57
        ^ b58
        ^ b59
        ^ b60
        ^ b65
        ^ b66
        ^ b70
        ^ b71;

    let mut payload = vec![
        b18, b20, b52, b26, b30, b34, b58, b38, b40, b53, b42, b21, b27, b54, b55, b31, b35, b57,
        b39, b41, b43, b22, b28, b32, b60, b36, b23, b29, b33, b37, b44, b45, b59, b46, b47, b48,
        b49, b50, b24, b25, b65, b66, b70, b71,
    ];
    payload.extend_from_slice(window_env);
    payload.push(checksum);

    rc4_encrypt(&payload, &[0x79])
}

pub(super) fn timestamp_now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
