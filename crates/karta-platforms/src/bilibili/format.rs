//! Bilibili stat formatting helpers.

use super::api::{ArticleStats, VideoStat};

/// Format large numbers with Chinese units.
#[inline]
fn format_number(n: i64) -> String {
    if n >= 100_000_000 {
        format!("{:.1}亿", n as f64 / 100_000_000.0)
    } else if n >= 10_000 {
        format!("{:.1}万", n as f64 / 10_000.0)
    } else {
        n.to_string()
    }
}

/// Format a slice of emoji/value pairs into an emoji stats string.
///
/// Builds the string directly to avoid an intermediate allocation.
fn format_emoji_stats(mapping: &[(&str, i64)]) -> String {
    use std::fmt::Write;

    let mut out = String::with_capacity(mapping.len() * 12);
    for (i, (emoji, val)) in mapping.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let _ = write!(out, "{} {}", emoji, format_number(*val));
    }
    out
}

/// Format video stats with emoji prefixes.
pub(super) fn format_stats(stat: &VideoStat) -> String {
    format_emoji_stats(&[
        ("👍", stat.like),
        ("🪙", stat.coin),
        ("⭐", stat.favorite),
        ("↩️", stat.share),
        ("💬", stat.reply),
        ("👀", stat.view),
        ("💭", stat.danmaku),
    ])
}

/// Format article stats with emoji prefixes.
pub(super) fn format_article_stats(stat: &ArticleStats) -> String {
    format_emoji_stats(&[
        ("👍", stat.like),
        ("🪙", stat.coin),
        ("⭐", stat.favorite),
        ("↩️", stat.share),
        ("💬", stat.reply),
        ("👀", stat.view),
    ])
}
