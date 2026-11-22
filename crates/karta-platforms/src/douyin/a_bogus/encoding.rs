pub(super) const ALPHABET_S3: &[u8] =
    b"ckdp1h4ZKsUB80/Mfvw36XIgR25+WQAlEi7NLboqYTOPuzmFjJnryx9HVGDaStCe";
pub(super) const ALPHABET_S4: &[u8] =
    b"Dkdpgh2ZmsQB80/MfvV36XI1R45-WUAlEixNLwoqYTOPuzKFjJnry79HbGcaStCe";

/// Encode raw bytes using a shuffled base64-like alphabet.
///
/// Emits `ceil(len * 4 / 3)` characters without `=` padding; missing bytes in
/// a trailing partial group are treated as 0.
pub(super) fn custom_b64_encode(data: &[u8], alphabet: &[u8]) -> String {
    let limit = (data.len() as f64 / 3.0) * 4.0;
    let char_count = limit.ceil() as usize;
    let mut result = String::with_capacity(char_count);

    let mut round = 0usize;
    let mut triplet = get_triplet(0, data);

    for i in 0..char_count {
        let new_round = i / 4;
        if new_round != round {
            round = new_round;
            triplet = get_triplet(round, data);
        }

        let idx = match i % 4 {
            0 => (triplet >> 18) & 0x3f,
            1 => (triplet >> 12) & 0x3f,
            2 => (triplet >> 6) & 0x3f,
            _ => triplet & 0x3f,
        };
        result.push(alphabet[idx as usize] as char);
    }

    result
}

#[inline]
fn get_triplet(round: usize, data: &[u8]) -> u32 {
    let base = round * 3;
    let b0 = data.get(base).copied().unwrap_or(0) as u32;
    let b1 = data.get(base + 1).copied().unwrap_or(0) as u32;
    let b2 = data.get(base + 2).copied().unwrap_or(0) as u32;
    (b0 << 16) | (b1 << 8) | b2
}

/// Mix a random u16 with a 2-byte option using alternating-bit masking.
pub(super) fn gener_random(random: u16, option: [u8; 2]) -> [u8; 4] {
    let lo = (random & 0xff) as u8;
    let hi = ((random >> 8) & 0xff) as u8;

    [
        (lo & 0xaa) | (option[0] & 0x55),
        (lo & 0x55) | (option[0] & 0xaa),
        (hi & 0xaa) | (option[1] & 0x55),
        (hi & 0x55) | (option[1] & 0xaa),
    ]
}

/// Three random values in `0..10000`.
pub(super) fn random_values() -> [u16; 3] {
    [(); 3].map(|_| (rand::random::<f64>() * 10000.0) as u16)
}

/// Mix three random values with the fingerprint bits into 12 bytes.
pub(super) fn random_bytes([r1, r2, r3]: [u16; 3]) -> [u8; 12] {
    let mut out = [0u8; 12];
    out[0..4].copy_from_slice(&gener_random(r1, [3, 45]));
    out[4..8].copy_from_slice(&gener_random(r2, [1, 0]));
    out[8..12].copy_from_slice(&gener_random(r3, [1, 5]));
    out
}
