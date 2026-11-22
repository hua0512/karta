use karta_core::Platform;
use karta_core::data::{Author, Content, ImageContent, ParseResult, VideoContent};
use karta_core::error::{ParserError, Result};

pub(super) fn build_from_explore_data(
    data: &serde_json::Value,
    note_id: &str,
) -> Result<ParseResult> {
    let note = data
        .pointer("/note/noteDetailMap")
        .and_then(|map| map.as_object())
        .and_then(|map| map.get(note_id))
        .and_then(|note| note.get("note"))
        .ok_or_else(|| ParserError::ContentNotFound("Note data not found".to_string()))?;

    build_result_from_note(note)
}

pub(super) fn build_from_discovery_data(data: &serde_json::Value) -> Result<ParseResult> {
    let note_data = data
        .pointer("/noteData/data/noteData")
        .ok_or_else(|| ParserError::ContentNotFound("Discovery note data not found".to_string()))?;
    let preload_data = data.pointer("/noteData/normalNotePreloadData");

    build_result_from_discovery(note_data, preload_data)
}

fn build_result_from_note(note: &serde_json::Value) -> Result<ParseResult> {
    let title = string_field(note, "title").unwrap_or_default().to_owned();
    let desc = string_field(note, "desc").unwrap_or_default().to_owned();

    let mut result = ParseResult::new(Platform::Xiaohongshu)
        .with_title(title)
        .with_author(author_from_user(note.get("user"), "nickname"))
        .with_contents(collect_explore_contents(note));
    if let Some(timestamp) = timestamp_seconds(note) {
        result = result.with_timestamp(timestamp);
    }

    add_description_text(&mut result, &desc);
    add_common_metadata(&mut result, note);

    Ok(result)
}

fn build_result_from_discovery(
    note_data: &serde_json::Value,
    preload_data: Option<&serde_json::Value>,
) -> Result<ParseResult> {
    let title = string_field(note_data, "title")
        .unwrap_or_default()
        .to_owned();
    let desc = string_field(note_data, "desc")
        .unwrap_or_default()
        .to_owned();

    let mut result = ParseResult::new(Platform::Xiaohongshu)
        .with_title(title)
        .with_author(author_from_user(note_data.get("user"), "nickName"))
        .with_contents(collect_discovery_contents(note_data, preload_data));
    if let Some(timestamp) = timestamp_seconds(note_data) {
        result = result.with_timestamp(timestamp);
    }

    add_description_text(&mut result, &desc);
    add_common_metadata(&mut result, note_data);

    Ok(result)
}

fn author_from_user(user: Option<&serde_json::Value>, name_key: &str) -> Author {
    let nickname = user
        .and_then(|user| user.get(name_key))
        .and_then(|value| value.as_str())
        .unwrap_or("小红书用户");
    let mut author = Author::new(nickname);
    if let Some(avatar_url) = user
        .and_then(|user| user.get("avatar"))
        .and_then(|value| value.as_str())
    {
        author = author.with_avatar(avatar_url);
    }
    author
}

fn collect_explore_contents(note: &serde_json::Value) -> Vec<Content> {
    let mut contents = Vec::new();
    let note_type = string_field(note, "type").unwrap_or_default();

    if note_type == "video"
        && let Some(stream) = note.pointer("/video/media/stream")
        && let Some(video_url) = best_stream_url(stream)
    {
        let mut video = VideoContent::new().with_url(video_url);
        if let Some(cover) = note
            .get("imageList")
            .and_then(|images| images.as_array())
            .and_then(|images| images.first())
            .and_then(|image| string_field(image, "urlDefault"))
        {
            video = video.with_cover(cover);
        }
        contents.push(Content::Video(video));
    }

    if contents.is_empty()
        && let Some(images) = note.get("imageList").and_then(|value| value.as_array())
    {
        contents.extend(images.iter().filter_map(|image| {
            string_field(image, "urlSizeLarge")
                .or_else(|| string_field(image, "urlDefault"))
                .map(|url| Content::Image(ImageContent::new().with_url(url)))
        }));
    }

    contents
}

fn collect_discovery_contents(
    note_data: &serde_json::Value,
    preload_data: Option<&serde_json::Value>,
) -> Vec<Content> {
    let mut contents = Vec::new();
    let note_type = string_field(note_data, "type").unwrap_or_default();

    if note_type == "video"
        && let Some(video) = note_data.get("video")
        && let Some(video_url) = string_field(video, "url")
            .or_else(|| video.pointer("/media/stream").and_then(best_stream_url))
    {
        let mut video_content = VideoContent::new().with_url(video_url);
        if let Some(cover) = discovery_cover_url(note_data, preload_data) {
            video_content = video_content.with_cover(cover);
        }
        contents.push(Content::Video(video_content));
    }

    if contents.is_empty() {
        contents.extend(
            discovery_image_urls(note_data, preload_data)
                .into_iter()
                .map(|url| Content::Image(ImageContent::new().with_url(url))),
        );
    }

    contents
}

fn discovery_image_urls(
    note_data: &serde_json::Value,
    preload_data: Option<&serde_json::Value>,
) -> Vec<String> {
    let preload_images = preload_data
        .and_then(|preload| preload.get("imagesList"))
        .and_then(|value| value.as_array())
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let note_images = note_data
        .get("imageList")
        .and_then(|value| value.as_array())
        .map(Vec::as_slice)
        .unwrap_or(&[]);

    let count = preload_images.len().max(note_images.len());
    (0..count)
        .filter_map(|index| {
            preload_images
                .get(index)
                .and_then(discovery_image_url)
                .or_else(|| note_images.get(index).and_then(discovery_image_url))
                .map(str::to_owned)
        })
        .collect()
}

fn discovery_image_url(image: &serde_json::Value) -> Option<&str> {
    string_field(image, "urlSizeLarge")
        .or_else(|| string_field(image, "url"))
        .or_else(|| string_field(image, "urlDefault"))
        .or_else(|| {
            image
                .get("infoList")
                .and_then(|value| value.as_array())
                .and_then(|infos| {
                    infos
                        .iter()
                        .find(|info| string_field(info, "imageScene") == Some("H5_DTL"))
                        .or_else(|| infos.first())
                })
                .and_then(|info| string_field(info, "url"))
        })
}

fn best_stream_url(stream: &serde_json::Value) -> Option<&str> {
    const PREFERRED_GROUPS: [&str; 6] = ["h265", "h264", "av1", "h266", "EF5", "EF4"];
    let groups = stream.as_object()?;

    PREFERRED_GROUPS
        .into_iter()
        .filter_map(|name| groups.get(name))
        .chain(
            groups
                .iter()
                .filter(|(name, _)| !PREFERRED_GROUPS.contains(&name.as_str()))
                .map(|(_, group)| group),
        )
        .filter_map(|group| group.as_array())
        .flatten()
        .find_map(|entry| string_field(entry, "masterUrl").filter(|url| !url.is_empty()))
}

fn discovery_cover_url<'a>(
    note_data: &'a serde_json::Value,
    preload_data: Option<&'a serde_json::Value>,
) -> Option<&'a str> {
    preload_data
        .and_then(|preload| preload.get("imagesList"))
        .and_then(|images| images.as_array())
        .and_then(|images| images.first())
        .and_then(discovery_image_url)
        .or_else(|| {
            note_data
                .get("imageList")
                .and_then(|images| images.as_array())
                .and_then(|images| images.first())
                .and_then(discovery_image_url)
        })
}

fn add_description_text(result: &mut ParseResult, desc: &str) {
    if !desc.is_empty() {
        result.contents.push(Content::Text(desc.to_owned()));
        result.text = Some(desc.to_owned());
    }
}

fn add_common_metadata(result: &mut ParseResult, note: &serde_json::Value) {
    if let Some(interact_info) = note.get("interactInfo") {
        for (key, label) in [
            ("likedCount", "点赞"),
            ("collectedCount", "收藏"),
            ("commentCount", "评论"),
            ("shareCount", "分享"),
        ] {
            if let Some(value) = string_field(interact_info, key) {
                result.add_extra(label, value);
            }
        }
    }

    if let Some(tags) = note.get("tagList").and_then(|value| value.as_array()) {
        let tags = tags
            .iter()
            .filter_map(|tag| string_field(tag, "name"))
            .map(|name| format!("#{name}"))
            .collect::<Vec<_>>();
        if !tags.is_empty() {
            result.add_extra("标签", tags.join(" "));
        }
    }

    if let Some(ip_location) = string_field(note, "ipLocation").filter(|value| !value.is_empty()) {
        result.add_extra("IP属地", ip_location);
    }
}

fn timestamp_seconds(note: &serde_json::Value) -> Option<i64> {
    note.get("time")
        .and_then(|value| value.as_i64())
        .map(|time| time / 1000)
}

fn string_field<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(|value| value.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_result_keeps_note_images_when_preload_has_only_cover() {
        let data = serde_json::json!({
            "noteData": {
                "data": {
                    "noteData": {
                        "title": "Discovery title",
                        "type": "normal",
                        "user": {"nickName": "Mobile Author"},
                        "imageList": [
                            {"url": "https://example.com/watermarked-cover.jpg"},
                            {"url": "https://example.com/slide2.jpg"},
                            {"infoList": [
                                {"imageScene": "H5_PRV", "url": "https://example.com/slide3-preview.jpg"},
                                {"imageScene": "H5_DTL", "url": "https://example.com/slide3-detail.jpg"}
                            ]}
                        ]
                    }
                },
                "normalNotePreloadData": {
                    "imagesList": [{"urlSizeLarge": "https://example.com/preload-cover.jpg"}]
                }
            }
        });

        let result = build_from_discovery_data(&data).unwrap();
        let urls = result
            .images()
            .into_iter()
            .filter_map(|image| image.url.as_deref())
            .collect::<Vec<_>>();

        assert_eq!(
            urls,
            vec![
                "https://example.com/preload-cover.jpg",
                "https://example.com/slide2.jpg",
                "https://example.com/slide3-detail.jpg"
            ]
        );
    }
}
