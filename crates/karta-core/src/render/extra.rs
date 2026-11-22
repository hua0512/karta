//! Shared formatting rules for user-facing `ParseResult::extra` metadata across
//! both the image card renderer and the plain-text renderer.

/// Whether an `extra` key carries an internal identifier that should not be
/// displayed to end users.
pub(crate) fn is_hidden_key(key: &str) -> bool {
    matches!(key, "series_id" | "book_id" | "vid")
}

/// Format a single `(key, value)` metadata entry for user-facing display.
///
/// Returns `None` for hidden internal keys or blank values. Keys whose values
/// are already self-describing (`stats`, `ai_summary`) omit the key prefix;
/// known English keys are mapped to concise Chinese labels.
pub(crate) fn format_extra_entry(key: &str, value: &str, separator: &str) -> Option<String> {
    if is_hidden_key(key) {
        return None;
    }
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if matches!(key, "stats" | "ai_summary") {
        return Some(trimmed.to_string());
    }
    let label = match key {
        "views" => "浏览",
        "likes" => "点赞",
        "comments" => "评论",
        "favorites" | "bookmarks" => "收藏",
        "reposts" | "retweets" => "转发",
        "shares" => "分享",
        "coins" => "投币",
        "danmaku" => "弹幕",
        "tags" => "标签",
        "content_type" => "类型",
        "duration" => "时长",
        "status" => "状态",
        "area" => "分区",
        "media_count" => "内容数",
        "episode_count" => "总集数",
        "accessible_episode_count" => "可看集数",
        "readable_episode_count" => "可读话数",
        _ => key,
    };
    Some(format!("{label}{separator}{trimmed}"))
}
