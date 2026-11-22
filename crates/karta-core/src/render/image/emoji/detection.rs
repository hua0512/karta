use super::config::EmojiStyle;

/// Represents an emoji found in text
#[derive(Debug, Clone)]
pub struct EmojiMatch {
    /// The emoji string
    pub emoji: String,
    /// Start byte position in the original text
    pub start: usize,
    /// End byte position in the original text
    pub end: usize,
    /// Unicode codepoints for CDN URL construction
    pub codepoints: String,
}

/// Convert an emoji string to its codepoint representation for CDN URLs
///
/// Example: "😀" -> "1f600", "👨‍👩‍👧" -> "1f468-200d-1f469-200d-1f467"
pub fn emoji_to_codepoints(emoji: &str) -> String {
    emoji
        .chars()
        .filter(|&c| c != '\u{FE0F}')
        .map(|c| format!("{:x}", c as u32))
        .collect::<Vec<_>>()
        .join("-")
}

/// Convert an emoji to the codepoint sequence used in CDN file names for a
/// given [`EmojiStyle`].
///
/// The U+FE0F variation selector rules differ per asset set:
/// - Twemoji omits FE0F for standalone emojis (`2764.png`) but keeps it
///   inside ZWJ sequences (`2764-fe0f-200d-1f525.png`).
/// - The emoji-datasource sets (Apple / Google / Facebook) keep FE0F even
///   for standalone emojis (`2764-fe0f.png`).
///
/// Stripping FE0F unconditionally (see [`emoji_to_codepoints`]) 404s those
/// files and degrades the emoji to a text glyph.
pub fn emoji_to_cdn_codepoints(emoji: &str, style: EmojiStyle) -> String {
    let keep_fe0f = match style {
        EmojiStyle::Twemoji => emoji.chars().any(|c| c == '\u{200D}'),
        EmojiStyle::Apple | EmojiStyle::Google | EmojiStyle::Facebook => true,
    };
    emoji
        .chars()
        .filter(|&c| keep_fe0f || c != '\u{FE0F}')
        .map(|c| format!("{:x}", c as u32))
        .collect::<Vec<_>>()
        .join("-")
}

/// Maximum chars an emoji cluster can span (flags, ZWJ family sequences).
#[cfg(feature = "emoji")]
const MAX_EMOJI_CHAR_LOOKAHEAD: usize = 10;

/// Find all emojis in a text string
///
/// Returns a list of emoji matches with their positions. This is the single
/// scan loop; [`segment_text`] is layered on top of it.
#[cfg(feature = "emoji")]
pub fn find_emojis(text: &str) -> Vec<EmojiMatch> {
    let mut matches = Vec::new();
    let char_indices = text.char_indices().collect::<Vec<_>>();
    let mut i = 0;

    while i < char_indices.len() {
        let (byte_pos, _) = char_indices[i];
        let mut best_match: Option<(String, usize)> = None;

        // Greedy longest match within the lookahead window, so multi-char
        // clusters (flags, ZWJ sequences) win over their prefixes.
        for end in (i + 1)..=char_indices.len().min(i + MAX_EMOJI_CHAR_LOOKAHEAD) {
            let end_byte = if end < char_indices.len() {
                char_indices[end].0
            } else {
                text.len()
            };

            let substr = &text[byte_pos..end_byte];
            if let Some(emoji) = emojis::get(substr) {
                best_match = Some((emoji.as_str().to_string(), end_byte));
            }
        }

        if let Some((emoji, end_byte)) = best_match {
            matches.push(EmojiMatch {
                codepoints: emoji_to_codepoints(&emoji),
                emoji,
                start: byte_pos,
                end: end_byte,
            });
            while i < char_indices.len() && char_indices[i].0 < end_byte {
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    matches
}

/// Find all emojis in a text string (fallback when emoji feature is disabled)
#[cfg(not(feature = "emoji"))]
pub fn find_emojis(_text: &str) -> Vec<EmojiMatch> {
    Vec::new()
}

/// Check if a character is an emoji
#[cfg(feature = "emoji")]
pub fn is_emoji(c: char) -> bool {
    let mut buf = [0u8; 4];
    emojis::get(c.encode_utf8(&mut buf)).is_some()
}

/// Check if a character is an emoji (fallback)
#[cfg(not(feature = "emoji"))]
pub fn is_emoji(_c: char) -> bool {
    false
}

/// Segment text into emoji and non-emoji parts
#[derive(Debug, Clone)]
pub enum TextSegment {
    /// Regular text
    Text(String),
    /// Emoji with its codepoints
    Emoji { emoji: String, codepoints: String },
}

/// Segment text into emoji and non-emoji parts
#[cfg(feature = "emoji")]
pub fn segment_text(text: &str) -> Vec<TextSegment> {
    let mut segments = Vec::new();
    let mut last_text_start = 0;

    for m in find_emojis(text) {
        if m.start > last_text_start {
            segments.push(TextSegment::Text(
                text[last_text_start..m.start].to_string(),
            ));
        }
        segments.push(TextSegment::Emoji {
            emoji: m.emoji,
            codepoints: m.codepoints,
        });
        last_text_start = m.end;
    }

    if last_text_start < text.len() {
        segments.push(TextSegment::Text(text[last_text_start..].to_string()));
    }

    segments
}

/// Segment text (fallback when emoji feature is disabled)
#[cfg(not(feature = "emoji"))]
pub fn segment_text(text: &str) -> Vec<TextSegment> {
    vec![TextSegment::Text(text.to_string())]
}

#[cfg(all(test, feature = "emoji"))]
mod tests {
    use super::*;

    #[test]
    fn find_emojis_prefers_longest_cluster() {
        // Flag = two regional indicators; must match as one emoji.
        let matches = find_emojis("🇺🇸");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].codepoints, "1f1fa-1f1f8");
    }

    #[test]
    fn cdn_codepoints_follow_per_style_fe0f_rules() {
        use EmojiStyle::{Apple, Facebook, Google, Twemoji};

        let cases = [
            // Lone red heart (U+2764 U+FE0F): Twemoji strips FE0F, the
            // emoji-datasource styles keep it.
            ("❤️", Twemoji, "2764"),
            ("❤️", Apple, "2764-fe0f"),
            ("❤️", Facebook, "2764-fe0f"),
            // Heart on fire (ZWJ sequence): every style keeps FE0F.
            ("❤️‍🔥", Twemoji, "2764-fe0f-200d-1f525"),
            ("❤️‍🔥", Google, "2764-fe0f-200d-1f525"),
        ];
        for (emoji, style, expected) in cases {
            assert_eq!(
                emoji_to_cdn_codepoints(emoji, style),
                expected,
                "{emoji} {style:?}"
            );
        }
    }
}
