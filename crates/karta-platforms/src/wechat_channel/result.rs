//! Maps the two-step API payloads onto a [`ParseResult`].

use karta_core::Platform;
use karta_core::data::{Author, Content, ParseResult, VideoContent};
use karta_core::error::{ParserError, Result};

use super::api::{AuthorInfo, FeedInfo, FeedInfoResponse, YuanbaoParseData};

/// Validate the step-2 response and build the final result. Step-1 values
/// backfill cover / description / author when step 2 omits them.
pub(super) fn build_result(
    parse_data: &YuanbaoParseData,
    feed: &FeedInfoResponse,
    share_url: &str,
) -> Result<ParseResult> {
    // Only a *present*, non-zero errCode is an error — some success responses
    // omit the field entirely.
    if let Some(code) = feed.err_code
        && code != 0
    {
        return Err(ParserError::Api {
            code,
            message: format!(
                "视频号接口返回错误: {}",
                feed.err_msg.as_deref().unwrap_or("未知错误")
            ),
        });
    }

    let data = feed
        .data
        .as_ref()
        .filter(|data| data.feed_info.is_some() || data.author_info.is_some())
        .ok_or_else(|| {
            ParserError::Parse(
                "视频号接口返回数据为空，可能是分享链接已失效或 token 过期".to_string(),
            )
        })?;

    // Either half may be missing on its own (e.g. only `authorInfo` present);
    // an empty stand-in keeps the field access uniform.
    let default_feed = FeedInfo::default();
    let default_author = AuthorInfo::default();
    let feed_info = data.feed_info.as_ref().unwrap_or(&default_feed);
    let author_info = data.author_info.as_ref().unwrap_or(&default_author);

    // Original CDN URL first (keeps all params), H.264 variant as fallback.
    let video_url = feed_info
        .video_url
        .as_deref()
        .filter(|url| !url.is_empty())
        .or_else(|| {
            feed_info
                .h264_video_info
                .as_ref()
                .and_then(|info| info.video_url.as_deref())
                .filter(|url| !url.is_empty())
        })
        .ok_or_else(|| {
            ParserError::ContentNotFound(format!(
                "未获取到视频地址，可能是图文动态或不支持的内容类型 (mediaType: {})",
                feed_info.media_type.unwrap_or(0)
            ))
        })?;

    let mut author = Author::new(
        non_empty(author_info.nickname.as_deref())
            .or_else(|| non_empty(parse_data.author.as_deref()))
            .unwrap_or("视频号用户"),
    );
    if let Some(avatar) = non_empty(author_info.head_img_url.as_deref())
        .or_else(|| non_empty(parse_data.author_icon.as_deref()))
    {
        author = author.with_avatar(avatar);
    }

    let mut video = VideoContent::new().with_url(video_url);
    if let Some(cover) = non_empty(feed_info.cover_url.as_deref())
        .or_else(|| non_empty(parse_data.cover_url.as_deref()))
    {
        video = video.with_cover(cover);
    }

    let mut result = ParseResult::new(Platform::WechatChannel)
        .with_url(share_url)
        .with_author(author)
        .with_content(Content::Video(video));

    if let Some(description) = non_empty(feed_info.description.as_deref())
        .or_else(|| non_empty(parse_data.desc.as_deref()))
    {
        result = result.with_text(description);
    }
    if let Some(timestamp) = feed_info.createtime.filter(|&ts| ts > 0) {
        result = result.with_timestamp(timestamp);
    }
    if let Some(stats) = format_stats(feed_info) {
        result.add_extra("stats", stats);
    }

    Ok(result)
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.filter(|s| !s.is_empty())
}

/// Format the pre-formatted interaction counts, skipping empty parts.
///
/// ❤️ maps to `likeCountFmt` (红心) and 👍 maps to `favCountFmt` (喜欢),
/// matching the 视频号 App UI despite the inverted field names.
fn format_stats(feed_info: &FeedInfo) -> Option<String> {
    let parts: Vec<String> = [
        ("❤️", feed_info.like_count_fmt.as_deref()),
        ("👍", feed_info.fav_count_fmt.as_deref()),
        ("🔄", feed_info.forward_count_fmt.as_deref()),
        ("💬", feed_info.comment_count_fmt.as_deref()),
    ]
    .into_iter()
    .filter_map(|(emoji, value)| non_empty(value).map(|value| format!("{emoji}{value}")))
    .collect();

    (!parts.is_empty()).then(|| parts.join(" "))
}
