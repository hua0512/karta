pub(super) fn mid_to_id(mid: &str) -> String {
    let mid: String = mid.chars().rev().collect();
    let size = mid.len().div_ceil(7);
    let mut result = Vec::new();

    for index in 0..size {
        let start = index * 7;
        let end = std::cmp::min(start + 7, mid.len());
        let chunk: String = mid[start..end].chars().rev().collect();

        if let Ok(num) = chunk.parse::<u64>() {
            let encoded = base62_encode(num);
            if index < size - 1 && encoded.len() < 4 {
                result.push(format!("{encoded:0>4}"));
            } else {
                result.push(encoded);
            }
        }
    }

    result.reverse();
    result.join("")
}

fn base62_encode(mut num: u64) -> String {
    const ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

    if num == 0 {
        return "0".to_string();
    }

    let mut result = Vec::new();
    while num > 0 {
        result.push(ALPHABET[(num % 62) as usize] as char);
        num /= 62;
    }

    result.reverse();
    result.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mid_to_id_encodes_chunks() {
        // Each 7-digit chunk is encoded in base62, with every chunk except the
        // leading one left-padded to 4 chars.
        let cases = [
            ("3501756485200075", "z0JH2lOMb"),
            ("5000000000000001", "O00000001"),
        ];

        for (mid, id) in cases {
            assert_eq!(mid_to_id(mid), id, "mid {mid}");
        }
    }
}
