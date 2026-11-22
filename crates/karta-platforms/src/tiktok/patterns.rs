use std::sync::LazyLock;

use karta_core::parser::RegisteredPattern;
use karta_core::patterns;

pub(super) static VIDEO_ID_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"/video/(\d+)").expect("valid tiktok video id regex"));

pub(super) fn registered_patterns() -> Vec<RegisteredPattern> {
    patterns![
        "tiktok_short" => r"(?:vm|vt)\.tiktok\.com/[A-Za-z0-9\._\?%&+\-=/#@]+",
        "tiktok"       => r"(?:www\.)?tiktok\.com/@[^/]+/video/(\d+)",
    ]
}
