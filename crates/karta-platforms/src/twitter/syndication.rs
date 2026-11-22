pub(super) fn parse_twitter_timestamp(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_str(s, "%a %b %d %H:%M:%S %z %Y")
        .ok()
        .map(|dt| dt.timestamp())
        .or_else(|| {
            let parts: Vec<&str> = s.splitn(6, ' ').collect();
            if parts.len() == 6 {
                let naive_str = format!("{} {} {} {}", parts[1], parts[2], parts[3], parts[5]);
                chrono::NaiveDateTime::parse_from_str(&naive_str, "%b %d %H:%M:%S %Y")
                    .ok()
                    .map(|dt| dt.and_utc().timestamp())
            } else {
                None
            }
        })
}

pub(super) fn twitter_original_image_url(url: &str) -> String {
    let separator = if url.contains('?') { '&' } else { '?' };
    format!("{url}{separator}format=jpg&name=orig")
}

/// Generate an authentication token for Twitter's syndication API.
///
/// Computes `((id / 1e15) * π)` formatted in radix 36 with `0` and `.` stripped.
pub(super) fn syndication_token(id: u64) -> String {
    let val = (id as f64 / 1e15) * std::f64::consts::PI;
    f64_to_radix_string(val, 36).replace(['.', '0'], "")
}

/// Format a finite `f64` in `radix` using IEEE-754 shortest-representation
/// rounding rules: fraction digits stop once they fall below the input's
/// precision, with round-half-to-even on the last digit.
fn f64_to_radix_string(value: f64, radix: u32) -> String {
    const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";

    if value == 0.0 {
        return "0".to_string();
    }
    let radix_f = f64::from(radix);
    let negative = value < 0.0;
    let value = value.abs();

    let mut integer = value.floor();
    let mut fraction = value - integer;
    // Half the gap to the next representable double, at least the smallest one.
    let mut delta = (0.5 * (next_double(value) - value)).max(f64::from_bits(1));

    let mut fraction_digits = Vec::new();
    if fraction >= delta {
        loop {
            fraction *= radix_f;
            delta *= radix_f;
            let digit = fraction as u32;
            fraction_digits.push(digit);
            fraction -= f64::from(digit);
            if (fraction > 0.5 || (fraction == 0.5 && digit & 1 == 1)) && fraction + delta > 1.0 {
                // Round up, carrying through trailing radix-1 digits.
                loop {
                    match fraction_digits.pop() {
                        None => {
                            integer += 1.0;
                            break;
                        }
                        Some(last) if last + 1 < radix => {
                            fraction_digits.push(last + 1);
                            break;
                        }
                        Some(_) => {}
                    }
                }
                break;
            }
            if fraction < delta {
                break;
            }
        }
    }

    // Digits below the double's precision are written as zeros.
    let mut integer_digits = Vec::new();
    while double_exponent(integer / radix_f) > 0 {
        integer /= radix_f;
        integer_digits.push(0);
    }
    loop {
        let remainder = integer % radix_f;
        integer_digits.push(remainder as u32);
        integer = (integer - remainder) / radix_f;
        if integer <= 0.0 {
            break;
        }
    }

    let mut out = String::new();
    if negative {
        out.push('-');
    }
    out.extend(
        integer_digits
            .iter()
            .rev()
            .map(|&digit| DIGITS[digit as usize] as char),
    );
    if !fraction_digits.is_empty() {
        out.push('.');
        out.extend(
            fraction_digits
                .iter()
                .map(|&digit| DIGITS[digit as usize] as char),
        );
    }
    out
}

pub(super) fn best_video_variant(media: &serde_json::Value) -> Option<String> {
    media
        .pointer("/video_info/variants")
        .and_then(|value| value.as_array())
        .and_then(|variants| {
            variants
                .iter()
                .filter(|variant| {
                    variant.get("content_type").and_then(|value| value.as_str())
                        == Some("video/mp4")
                })
                .max_by_key(|variant| {
                    variant
                        .get("bitrate")
                        .and_then(|value| value.as_i64())
                        .unwrap_or(0)
                })
                .and_then(|variant| variant.get("url").and_then(|value| value.as_str()))
                .map(ToOwned::to_owned)
        })
}

/// The next representable double above a finite, non-negative `value`.
fn next_double(value: f64) -> f64 {
    f64::from_bits(value.to_bits() + 1)
}

/// Unbiased exponent of the 53-bit significand, so values of 2^53 and above
/// are positive.
fn double_exponent(value: f64) -> i32 {
    const EXPONENT_BIAS: i32 = 0x3FF + 52;
    let biased = ((value.to_bits() >> 52) & 0x7FF) as i32;
    if biased == 0 {
        1 - EXPONENT_BIAS
    } else {
        biased - EXPONENT_BIAS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Known tweet ID to syndication token test vectors.
    const WIDGET_TOKENS: [(u64, &str); 16] = [
        (0, ""),
        (1, "bhi2ay3f28n"),
        (20, "6dq1a2xwd93"),
        (34567, "8icy3ydbsbv"),
        (1000000000, "59yka5rnu5w"),
        (20000000000, "2xj79n7d8rc"),
        (999999999999999, "353i5ab8p54"),
        (1000000000000000, "353i5ab8p5f"),
        (9007199254740993, "saoujnx4as"),
        (463440424141459456, "14fxvks611f"),
        (1234567890123456789, "2zqic77uqyk"),
        (1585341984679469056, "3uchycv2wqc"),
        (1790000000000000000, "4c7g8auqyik"),
        (1850000000000000001, "4hfy2jnxph"),
        (1980000000000000123, "4sscq2r6afb"),
        (u64::MAX, "18ps5lqos4lc"),
    ];

    #[test]
    fn syndication_token_equals_the_embed_widget_token() {
        for (id, token) in WIDGET_TOKENS {
            assert_eq!(syndication_token(id), token, "tweet {id}");
        }
    }
}
