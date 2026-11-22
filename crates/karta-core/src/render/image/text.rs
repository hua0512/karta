//! Text rendering utilities with CJK and emoji support

use std::collections::HashMap;

use image::{RgbaImage, imageops};

use super::emoji::{TextSegment, segment_text};
use super::types::FontInfo;

/// Emoji string -> base emoji image, prefetched asynchronously before the
/// blocking draw path runs. An empty map means "draw text glyphs only".
pub type EmojiImageMap = HashMap<String, RgbaImage>;

/// Characters that should not appear at the start of a line
const NO_LINE_START_PUNCTUATION: &str = "，。！？；：、）】》〉」』〕〗〙〛…—·,.;:!?)]}";

/// Wrap text to fit within a maximum width, with CJK and emoji support
///
/// # Arguments
/// * `text` - The text to wrap
/// * `max_width` - Maximum width in pixels
/// * `font` - Font information for width calculation
///
/// # Returns
/// A vector of wrapped lines
pub fn wrap_text(text: &str, max_width: u32, font: &FontInfo) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }

    let mut lines = Vec::new();

    // Split by newlines first (preserve paragraphs)
    for paragraph in text.lines() {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }

        // Segment text into emoji and non-emoji parts
        let segments = segment_text(paragraph);

        let mut current_line = String::new();
        let mut current_width: u32 = 0;

        for segment in segments {
            match segment {
                TextSegment::Text(text_part) => {
                    // Process each character in the text part
                    for c in text_part.chars() {
                        let char_width = font.get_char_width(c);

                        // If current line is empty, always add the character
                        if current_line.is_empty() {
                            current_line.push(c);
                            current_width = char_width;
                            continue;
                        }

                        // If this is punctuation that shouldn't start a line, add it anyway
                        if NO_LINE_START_PUNCTUATION.contains(c) {
                            current_line.push(c);
                            current_width += char_width;
                            continue;
                        }

                        // Check if adding this character would exceed max width
                        if current_width + char_width <= max_width {
                            current_line.push(c);
                            current_width += char_width;
                        } else {
                            // Start a new line
                            lines.push(current_line);
                            current_line = c.to_string();
                            current_width = char_width;
                        }
                    }
                }
                TextSegment::Emoji { emoji, .. } => {
                    // Emoji width is typically the font size
                    let emoji_width = font.size as u32;

                    // If current line is empty, always add the emoji
                    if current_line.is_empty() {
                        current_line.push_str(&emoji);
                        current_width = emoji_width;
                        continue;
                    }

                    // Check if adding this emoji would exceed max width
                    if current_width + emoji_width <= max_width {
                        current_line.push_str(&emoji);
                        current_width += emoji_width;
                    } else {
                        // Start a new line
                        lines.push(current_line);
                        current_line = emoji;
                        current_width = emoji_width;
                    }
                }
            }
        }

        // Don't forget the last line
        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }

    lines
}

/// Draw multiple lines of text, compositing prefetched emoji images.
///
/// # Returns
/// The total height of drawn text
pub fn draw_text_lines(
    img: &mut RgbaImage,
    x: u32,
    y: u32,
    lines: &[String],
    font: &FontInfo,
    emoji_images: &EmojiImageMap,
) -> u32 {
    let mut current_y = y as i32;

    for line in lines {
        draw_line(img, x as i32, current_y, line, font, emoji_images);
        current_y += font.line_height as i32;
    }

    (lines.len() as u32) * font.line_height
}

/// Draw a single line of text, compositing prefetched emoji images.
///
/// # Returns
/// The height of the drawn text (line height)
pub fn draw_single_line(
    img: &mut RgbaImage,
    x: u32,
    y: u32,
    text: &str,
    font: &FontInfo,
    emoji_images: &EmojiImageMap,
) -> u32 {
    draw_line(img, x as i32, y as i32, text, font, emoji_images);
    font.line_height
}

/// Draw one line: plain glyphs when no emoji images are available, otherwise
/// text runs interleaved with composited emoji bitmaps. An emoji missing from
/// the map degrades to a font glyph (the pre-emoji behavior).
fn draw_line(
    img: &mut RgbaImage,
    x: i32,
    y: i32,
    line: &str,
    font: &FontInfo,
    emoji_images: &EmojiImageMap,
) {
    if emoji_images.is_empty() {
        font.font
            .draw_weighted(img, font.fill, x, y, font.size, font.embolden, line);
        return;
    }

    let emoji_size = font.size as u32;
    let mut current_x = x;

    for segment in segment_text(line) {
        match segment {
            TextSegment::Text(text) => {
                let advance = font.font.draw_weighted(
                    img,
                    font.fill,
                    current_x,
                    y,
                    font.size,
                    font.embolden,
                    &text,
                );
                current_x += advance;
            }
            TextSegment::Emoji { emoji, .. } => {
                if let Some(base) = emoji_images.get(&emoji) {
                    let sized;
                    let emoji_img = if base.width() == emoji_size && base.height() == emoji_size {
                        base
                    } else {
                        sized = imageops::resize(
                            base,
                            emoji_size,
                            emoji_size,
                            imageops::FilterType::Lanczos3,
                        );
                        &sized
                    };
                    // `Font::draw_weighted` top-aligns the `ascent + descent == font.size` glyph
                    // box at `y` (extra leading stays below), so align emoji to `font.size` rather
                    // than centering inside `font.line_height`.
                    let emoji_y = y + (font.size.round() as i32 - emoji_size as i32) / 2;
                    imageops::overlay(img, emoji_img, current_x as i64, emoji_y.max(0) as i64);
                    current_x += emoji_size as i32;
                } else {
                    let advance = font.font.draw_weighted(
                        img,
                        font.fill,
                        current_x,
                        y,
                        font.size,
                        font.embolden,
                        &emoji,
                    );
                    current_x += advance.max(emoji_size as i32);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::image::font::Font;
    use image::Rgba;

    fn test_font() -> FontInfo {
        let font = Font::new(
            include_bytes!("../../../../../resources/fonts/HYSongYunLangHeiW-1.ttf").as_slice(),
        )
        .expect("Failed to load embedded font");
        // CJK characters and emoji are 24 px wide at this size.
        FontInfo::new(font, Rgba([0, 0, 0, 255]), 24.0)
    }

    #[test]
    fn test_wrap_text_newlines() {
        // (case, text, max width, lines)
        let mut cases: Vec<(&str, &str, u32, &[&str])> = vec![
            (
                "paragraphs",
                "Line 1\nLine 2\n\nLine 3",
                1000,
                &["Line 1", "Line 2", "", "Line 3"],
            ),
            ("wraps at the width", "你好世界", 48, &["你好", "世界"]),
            (
                "punctuation never starts a line",
                "你好。世界",
                48,
                &["你好。", "世界"],
            ),
        ];
        if cfg!(feature = "emoji") {
            cases.push(("emoji fits", "Hello 😀 World", 1000, &["Hello 😀 World"]));
            cases.push(("emoji wraps", "😀😀", 24, &["😀", "😀"]));
        }
        let font = test_font();
        for (case, text, width, expected) in cases {
            assert_eq!(wrap_text(text, width, &font), expected, "{case}");
        }
    }
}
