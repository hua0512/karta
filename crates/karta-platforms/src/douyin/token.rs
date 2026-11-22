/// Generate a `verifyFp` token for live stream type 2 requests.
///
/// Produces `verify_{base36_timestamp}_{uuid-like}`.
pub(super) fn gen_verify_fp() -> String {
    use rand::RngExt;

    const BASE_STR: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    let t = BASE_STR.len();

    let mut millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let mut base36 = String::new();
    if millis == 0 {
        base36.push('0');
    } else {
        while millis > 0 {
            let remainder = (millis % 36) as u8;
            let ch = if remainder < 10 {
                b'0' + remainder
            } else {
                b'a' + remainder - 10
            };
            base36.insert(0, ch as char);
            millis /= 36;
        }
    }

    let mut rng = rand::rng();
    let mut uuid = [0u8; 36];
    uuid[8] = b'_';
    uuid[13] = b'_';
    uuid[18] = b'_';
    uuid[23] = b'_';
    uuid[14] = b'4';
    for (i, byte) in uuid.iter_mut().enumerate() {
        if *byte != 0 {
            continue;
        }
        let mut n = rng.random_range(0..t);
        if i == 19 {
            n = (3 & n) | 8;
        }
        *byte = BASE_STR[n];
    }

    let uuid_part: String = uuid.iter().map(|&b| b as char).collect();
    format!("verify_{}_{}", base36, uuid_part)
}
