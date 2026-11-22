const SALT: &[u8] = b"AB45STUVWZEFGJ6CH01D237IXYPQRKLMN89";

/// AES-like GF(2^8) doubling: x*2 in GF(2^8) with irreducible polynomial
/// x^8+x^4+x^3+x+1.
fn da(e: u8) -> u8 {
    let shifted = e.wrapping_shl(1);
    if e & 128 != 0 { shifted ^ 27 } else { shifted }
}

fn ba(e: u8) -> u8 {
    da(e) ^ e
}

fn na(e: u8) -> u8 {
    ba(da(e))
}

fn fa(e: u8) -> u8 {
    na(ba(da(e)))
}

fn ua(e: u8) -> u8 {
    fa(e) ^ na(e) ^ ba(e)
}

fn za(input: &str, table: &[u8], trim_from_end: usize) -> String {
    let sub = &table[..table.len() - trim_from_end];
    input
        .bytes()
        .map(|byte| sub[byte as usize % sub.len()] as char)
        .collect()
}

fn wa(input: &str, table: &[u8]) -> String {
    input
        .bytes()
        .map(|byte| table[byte as usize % table.len()] as char)
        .collect()
}

fn interleave(strings: &[String]) -> String {
    let max_len = strings.iter().map(|s| s.len()).max().unwrap_or(0);
    let mut result = String::with_capacity(max_len * strings.len());
    for i in 0..max_len {
        for s in strings {
            if let Some(ch) = s.as_bytes().get(i) {
                result.push(*ch as char);
            }
        }
    }
    result
}

fn generate_hkey(path: &str, timestamp: u64, nonce: &str) -> String {
    let normalized = format!(
        "/{}/",
        path.split('/')
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>()
            .join("/")
    );

    let s1 = za(&timestamp.to_string(), SALT, 2);
    let s2 = wa(&normalized, SALT);
    let s3 = wa(nonce, SALT);
    let interleaved = interleave(&[s1, s2, s3]);
    let truncated: String = interleaved.chars().take(20).collect();

    let digest = md5::compute(truncated.as_bytes());
    let hex = format!("{:x}", digest);

    let tail = &hex[hex.len() - 6..];
    let mut codes: Vec<u8> = tail.bytes().collect();

    let t0 = ua(codes[0]) ^ fa(codes[1]) ^ na(codes[2]) ^ ba(codes[3]);
    let t1 = ba(codes[0]) ^ ua(codes[1]) ^ fa(codes[2]) ^ na(codes[3]);
    let t2 = na(codes[0]) ^ ba(codes[1]) ^ ua(codes[2]) ^ fa(codes[3]);
    let t3 = fa(codes[0]) ^ na(codes[1]) ^ ba(codes[2]) ^ ua(codes[3]);
    codes[0] = t0;
    codes[1] = t1;
    codes[2] = t2;
    codes[3] = t3;

    let sum: u32 = codes.iter().map(|&byte| byte as u32).sum();
    let suffix = format!("{:02}", sum % 100);
    let prefix = za(&hex[..5], SALT, 4);
    format!("{prefix}{suffix}")
}

pub(super) fn build_auth_params(api_path: &str) -> Vec<(String, String)> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let nonce_input = format!("{}{}", timestamp, rand::random::<f64>());
    let nonce_digest = md5::compute(nonce_input.as_bytes());
    let nonce = format!("{:X}", nonce_digest);
    let hkey = generate_hkey(api_path, timestamp + 1, &nonce);

    vec![
        ("os_type".into(), "web".into()),
        ("version".into(), "999.0.4".into()),
        ("hkey".into(), hkey),
        ("_time".into(), timestamp.to_string()),
        ("nonce".into(), nonce),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hkey_matches_the_web_client() {
        // Expected values from the web client's `getHkey(path, time, nonce)`.
        let cases = [
            (
                "bbs/app/link/tree",
                1700000000,
                "ABCDEF1234567890ABCDEF1234567890",
                "V2V1Z67",
            ),
            ("game/get_game_detail", 1700000000, "AAAA", "ST2Y763"),
            (
                "/bbs/app/api/link/post",
                1736000000,
                "0123456789ABCDEF0123456789ABCDEF",
                "PP2TY82",
            ),
        ];

        for (path, timestamp, nonce, hkey) in cases {
            assert_eq!(generate_hkey(path, timestamp, nonce), hkey, "{path}");
        }
    }
}
