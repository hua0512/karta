//! Bilibili URL and command pattern registration.

use karta_core::parser::RegisteredPattern;

/// Build Bilibili parser patterns in precedence order.
pub(super) fn registered_patterns() -> Vec<RegisteredPattern> {
    vec![
        // Audio-only trigger (`bm` command).
        // This must stay before BV/AV patterns so "bm BV..." is audio.
        RegisteredPattern::new_static("bm", r"\bbm\s+(BV[0-9a-zA-Z]{10})(?:\s+(\d+))?"),
        RegisteredPattern::new_static("b23", r"b23\.tv/([A-Za-z\d\._?%&+\-=/#]+)"),
        RegisteredPattern::new_static("bili2233", r"bili2233\.cn/([A-Za-z\d\._?%&+\-=/#]+)"),
        RegisteredPattern::new_static("bv_page", r"(BV[0-9a-zA-Z]{10})/?\?(?:[^#\s&]+&)*p=(\d+)"),
        RegisteredPattern::new_static("bv_space", r"(BV[0-9a-zA-Z]{10})\s+(\d+)"),
        RegisteredPattern::new_static("bv", r"(BV[0-9a-zA-Z]{10})"),
        // ASCII word boundary `(?-u:\b)` instead of `\b`: it rejects embedded
        // matches like "nav123" while still matching after CJK text
        // ("看av170001"), which Unicode `\b` would treat as a word character.
        RegisteredPattern::new_static("av_page", r"(?-u:\b)av(\d+)/?\?(?:[^#\s&]+&)*p=(\d+)"),
        RegisteredPattern::new_static("av_space", r"(?-u:\b)av(\d+)\s+(\d+)"),
        RegisteredPattern::new_static("av", r"(?-u:\b)av(\d+)"),
        RegisteredPattern::new_static("dynamic", r"t\.bilibili\.com/(\d+)"),
        // `space.bilibili.com/<uid>/dynamic/<id>` has the uid in between, so
        // it cannot collide with this one and keeps its own pattern below.
        RegisteredPattern::new_static("dynamic_page", r"bilibili\.com/dynamic/(\d+)"),
        RegisteredPattern::new_static("opus", r"bilibili\.com/opus/(\d+)"),
        RegisteredPattern::new_static("live", r"live\.bilibili\.com/(\d+)"),
        RegisteredPattern::new_static("space_dynamic", r"space\.bilibili\.com/\d+/dynamic/(\d+)"),
        RegisteredPattern::new_static("bangumi", r"bilibili\.com/bangumi/play/ep(\d+)"),
        RegisteredPattern::new_static("article", r"bilibili\.com/read/cv(\d+)"),
        RegisteredPattern::new_static("favorites", r"favlist\?fid=(\d+)"),
    ]
}
