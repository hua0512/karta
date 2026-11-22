//! Emoji rendering support with CDN-based image fetching
//!
//! This module provides emoji detection and rendering by fetching emoji images
//! from a CDN (default: jsDelivr Twemoji) and compositing them into rendered text.

mod config;
mod detection;
mod source;

pub use config::{EmojiConfig, EmojiStyle};
pub use detection::{
    EmojiMatch, TextSegment, emoji_to_cdn_codepoints, emoji_to_codepoints, find_emojis, is_emoji,
    segment_text,
};
pub use source::EmojiSource;
