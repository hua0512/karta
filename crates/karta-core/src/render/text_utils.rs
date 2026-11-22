/// Split a long text into chunks of at most `max_chars` chars while preserving
/// line breaks. Used to keep individual text messages / forward nodes within
/// the QQ payload limit.
pub(crate) fn split_text_segments(text: &str, max_chars: usize) -> Vec<String> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }

    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cur_len: usize = 0;

    for line in text.lines() {
        let line = line.trim_end();
        let line_len = line.chars().count();
        let extra = if cur.is_empty() { 0 } else { 1 };
        if cur_len + extra + line_len > max_chars && !cur.is_empty() {
            out.push(cur.trim_end().to_string());
            cur.clear();
            cur_len = 0;
        }

        // A paragraph can exceed the limit even without a newline. Split it
        // on Unicode character boundaries, retaining every character.
        if max_chars > 0 && line_len > max_chars {
            for ch in line.chars() {
                cur.push(ch);
                cur_len += 1;
                if cur_len == max_chars {
                    out.push(std::mem::take(&mut cur));
                    cur_len = 0;
                }
            }
            continue;
        }

        if !cur.is_empty() {
            cur.push('\n');
            cur_len += 1;
        }
        cur.push_str(line);
        cur_len += line_len;
    }

    if !cur.is_empty() {
        out.push(cur.trim_end().to_string());
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_text_segments_keeps_lines_within_the_limit() {
        // (case, text, max chars, chunks)
        let cases: [(&str, &str, usize, &[&str]); 3] = [
            ("breaks between lines", "ab\ncd\nef", 5, &["ab\ncd", "ef"]),
            (
                "splits an over-long line by chars",
                "你好世界呀",
                2,
                &["你好", "世界", "呀"],
            ),
            (
                "an over-long line after a short one",
                "a\nbcdef",
                3,
                &["a", "bcd", "ef"],
            ),
        ];
        for (case, text, max_chars, expected) in cases {
            assert_eq!(split_text_segments(text, max_chars), expected, "{case}");
        }
    }
}
