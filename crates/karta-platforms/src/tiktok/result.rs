use karta_core::Platform;
use karta_core::data::{Author, Content, ImageContent, ParseResult, VideoContent};
use karta_core::error::{ParserError, Result};

const DEFAULT_AUTHOR: &str = "TikTok User";

pub(super) fn build_from_sigi_state(
    data: &serde_json::Value,
    video_id: &str,
    url: &str,
) -> Result<ParseResult> {
    let item = data
        .pointer(&format!("/ItemModule/{video_id}"))
        .ok_or_else(|| ParserError::ContentNotFound("Video data not found".to_string()))?;

    Ok(build_item_result(
        item,
        url,
        &[
            &["nickname"],
            &["author", "nickname"],
            &["authorMeta", "nickName"],
        ],
        &[&["authorMeta", "avatar"], &["author", "avatarThumb"]],
    ))
}

pub(super) fn build_from_universal_data(
    data: &serde_json::Value,
    url: &str,
) -> Result<ParseResult> {
    let item = data
        .pointer("/__DEFAULT_SCOPE__/webapp.video-detail/itemInfo/itemStruct")
        .ok_or_else(|| ParserError::ContentNotFound("Video data not found".to_string()))?;

    Ok(build_item_result(
        item,
        url,
        &[&["author", "nickname"], &["nickname"]],
        &[&["author", "avatarThumb"], &["authorMeta", "avatar"]],
    ))
}

fn build_item_result(
    item: &serde_json::Value,
    url: &str,
    author_name_paths: &[&[&str]],
    author_avatar_paths: &[&[&str]],
) -> ParseResult {
    let mut result = ParseResult::new(Platform::TikTok)
        .with_author(author_from_item(
            item,
            author_name_paths,
            author_avatar_paths,
        ))
        .with_url(url)
        .with_contents(collect_contents(item));

    if let Some(desc) = string_field(item, "desc") {
        result = result.with_title(desc);
    }

    if let Some(timestamp) = integer_field(item, "createTime") {
        result = result.with_timestamp(timestamp);
    }

    result
}

fn author_from_item(
    item: &serde_json::Value,
    name_paths: &[&[&str]],
    avatar_paths: &[&[&str]],
) -> Author {
    let name = first_string_path(item, name_paths).unwrap_or(DEFAULT_AUTHOR);
    let mut author = Author::new(name);

    if let Some(avatar) = first_string_path(item, avatar_paths) {
        author = author.with_avatar(avatar);
    }

    author
}

fn collect_contents(item: &serde_json::Value) -> Vec<Content> {
    let mut contents = Vec::new();

    if let Some(video) = item.get("video")
        && let Some(content) = content_from_video(video)
    {
        contents.push(content);
    }

    contents
}

fn content_from_video(video: &serde_json::Value) -> Option<Content> {
    let video_url = string_field(video, "downloadAddr").or_else(|| string_field(video, "playAddr"));
    let cover = string_field(video, "cover").or_else(|| string_field(video, "originCover"));
    let duration = integer_field(video, "duration").map(|duration| duration as u64);

    if let Some(video_url) = video_url {
        let mut video = VideoContent::new().with_url(video_url);
        if let Some(cover) = cover {
            video = video.with_cover(cover);
        }
        if let Some(duration) = duration {
            video = video.with_duration(duration);
        }
        Some(Content::Video(video))
    } else {
        cover.map(|cover| Content::Image(ImageContent::new().with_url(cover)))
    }
}

fn string_field<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
}

fn integer_field(value: &serde_json::Value, key: &str) -> Option<i64> {
    value
        .get(key)
        .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
}

fn first_string_path<'a>(value: &'a serde_json::Value, paths: &[&[&str]]) -> Option<&'a str> {
    paths.iter().find_map(|path| {
        path.iter()
            .try_fold(value, |current, key| current.get(*key))
            .and_then(|value| value.as_str())
            .filter(|value| !value.is_empty())
    })
}
