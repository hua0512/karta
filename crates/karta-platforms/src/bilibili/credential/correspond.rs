//! `correspondPath` for the web cookie refresh: the lowercase hex of
//! RSA-OAEP (SHA-256, MGF1-SHA-256, empty label) over `refresh_{timestamp}`
//! under Bilibili's published public key.
//!
//! BoringSSL's safe bindings only offer OAEP with SHA-1, so the EME-OAEP
//! encoding (RFC 8017 §7.1.1) is done here and BoringSSL performs the raw
//! RSA operation on the encoded block.

use btls::rsa::{Padding, Rsa};
use sha2::{Digest, Sha256};

use karta_core::error::{ParserError, Result};

/// Key from bilibili-API-collect `docs/login/cookie_refresh.md`.
const PUBLIC_KEY_PEM: &[u8] = b"-----BEGIN PUBLIC KEY-----
MIGfMA0GCSqGSIb3DQEBAQUAA4GNADCBiQKBgQDLgd2OAkcGVtoE3ThUREbio0Eg
Uc/prcajMKXvkCKFCWhJYJcLkcM2DKKcSeFpD/j6Boy538YXnR6VhcuUJOhH2x71
nzPjfdTcqMz7djHum0qSZA0AyCBDABUqCrfNgCiJ00Ra7GmRj+YCK1NJEuewlb40
JNrRuoEUXpabUzGB8QIDAQAB
-----END PUBLIC KEY-----
";

/// SHA-256 output length.
const H_LEN: usize = 32;

/// The `correspondPath` for `timestamp_ms` (the `data.timestamp` of
/// `cookie/info`). OAEP is randomised, so every call gives a different path.
pub(super) fn correspond_path(timestamp_ms: i64) -> Result<String> {
    correspond_path_with_seed(timestamp_ms, &rand::random())
}

fn correspond_path_with_seed(timestamp_ms: i64, seed: &[u8; H_LEN]) -> Result<String> {
    let message = format!("refresh_{timestamp_ms}");
    let ciphertext = encrypt(PUBLIC_KEY_PEM, message.as_bytes(), seed)?;
    Ok(ciphertext.iter().map(|b| format!("{b:02x}")).collect())
}

/// RSA-OAEP-SHA256 encryption of `msg` under the PEM public key `pem`, with
/// the OAEP `seed` supplied by the caller.
fn encrypt(pem: &[u8], msg: &[u8], seed: &[u8; H_LEN]) -> Result<Vec<u8>> {
    let rsa_error = |e: btls::error::ErrorStack| ParserError::Parse(format!("RSA: {e}"));
    let key = Rsa::public_key_from_pem(pem).map_err(rsa_error)?;
    let k = key.size() as usize;
    if msg.len() + 2 * H_LEN + 2 > k {
        return Err(ParserError::Parse("RSA: message too long for OAEP".into()));
    }
    let encoded = oaep_sha256_encode(msg, k, seed);
    // The encoded block starts with 0x00, so it is below the modulus and
    // raw RSA accepts it; the output is always exactly `k` bytes.
    let mut ciphertext = vec![0; k];
    let len = key
        .public_encrypt(&encoded, &mut ciphertext, Padding::NONE)
        .map_err(rsa_error)?;
    ciphertext.truncate(len);
    Ok(ciphertext)
}

/// EME-OAEP encoding (RFC 8017 §7.1.1 step 2) with SHA-256 as both the label
/// hash and the MGF1 hash, and an empty label:
///
/// `EM = 0x00 || maskedSeed || maskedDB`, where
/// `DB = SHA256("") || 0x00.. || 0x01 || msg`,
/// `maskedDB = DB ^ MGF1(seed)` and `maskedSeed = seed ^ MGF1(maskedDB)`.
///
/// `k` is the modulus length in bytes; the caller ensures
/// `msg.len() <= k - 2 * 32 - 2`.
fn oaep_sha256_encode(msg: &[u8], k: usize, seed: &[u8; H_LEN]) -> Vec<u8> {
    let mut db = Vec::with_capacity(k - H_LEN - 1);
    db.extend_from_slice(&Sha256::digest(b""));
    // Zero padding up to the 0x01 separator.
    db.resize(k - msg.len() - H_LEN - 2, 0);
    db.push(0x01);
    db.extend_from_slice(msg);

    xor_mgf1(&mut db, seed);
    let mut masked_seed = *seed;
    xor_mgf1(&mut masked_seed, &db);

    let mut em = Vec::with_capacity(k);
    em.push(0x00);
    em.extend_from_slice(&masked_seed);
    em.extend_from_slice(&db);
    em
}

/// XOR `MGF1-SHA256(seed, out.len())` into `out`: the mask is
/// `SHA256(seed || counter)` for a big-endian `u32` counter from 0,
/// concatenated and cut to length.
fn xor_mgf1(out: &mut [u8], seed: &[u8]) {
    for (counter, chunk) in (0u32..).zip(out.chunks_mut(H_LEN)) {
        let mask = Sha256::new()
            .chain_update(seed)
            .chain_update(counter.to_be_bytes())
            .finalize();
        for (byte, mask) in chunk.iter_mut().zip(mask) {
            *byte ^= mask;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The encoded block for this seed decrypts to `refresh_1684466082562`
    /// with OpenSSL's OAEP-SHA256 under a throwaway key, and OpenSSL's raw
    /// (`rsa_padding_mode:none`) encryption of that block under the Bilibili
    /// key is this ciphertext.
    #[test]
    fn known_answer_under_the_bilibili_key() {
        const SEED: [u8; H_LEN] = *b"0123456789abcdef0123456789abcdef";
        assert_eq!(
            correspond_path_with_seed(1684466082562, &SEED).unwrap(),
            "808e67d681ae498d93edc766a1fa48b15abf31e34b4780d83698e4ea13362856aa238cbb5a3321916c23813f16294a3a7e65981134a5dd64174b3e1bb3220ed9f3735f3b5433beba0cfe567bca85a0f0fdc450ca3ad8df0fec3e8065c7d6b91fb26bfcea41f104ea2d72b50d3513a2c7c099b818f401a5e43052ea93cf37253c"
        );
    }
}
