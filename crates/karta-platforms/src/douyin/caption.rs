/// Recover the complete caption when a legacy endpoint substitutes an upgrade notice.
/// Ordinary descriptions retain their existing formatting and title handling.
pub(super) fn complete_desc<'a>(
    desc: Option<&'a str>,
    title: Option<&str>,
    caption: Option<&str>,
) -> std::borrow::Cow<'a, str> {
    let desc = desc.unwrap_or_default();
    if desc.trim_end().ends_with("版本过低，升级后可展示全部信息")
        && let Some(caption) = caption.filter(|value| !value.trim().is_empty())
    {
        let title = title.unwrap_or_default().trim();
        return if title.is_empty() {
            caption.to_owned().into()
        } else {
            format!("{title}\n{caption}").into()
        };
    }
    desc.into()
}

/// Split a Douyin `desc` caption into an optional headline and body text.
///
/// When the caption contains a newline, creators commonly use the first
/// line as a headline. Without a newline, the whole caption is body text.
pub(super) fn split_desc(desc: &str) -> (Option<String>, Option<String>) {
    let trimmed = desc.trim();
    if trimmed.is_empty() {
        return (None, None);
    }
    if let Some((head, rest)) = trimmed.split_once('\n') {
        let head = head.trim();
        let rest = rest.trim();
        if !head.is_empty() && !rest.is_empty() {
            return (Some(head.to_owned()), Some(rest.to_owned()));
        }
        if !head.is_empty() {
            return (None, Some(head.to_owned()));
        }
    }
    (None, Some(trimmed.to_owned()))
}
