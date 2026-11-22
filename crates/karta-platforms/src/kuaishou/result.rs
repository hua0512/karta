use karta_core::Platform;
use karta_core::data::{Author, Content, ImageContent, ParseResult, VideoContent};
use karta_core::error::{ParserError, Result};
use karta_core::extract;

use super::api::PhotoInfo;

/// Marker preceding the embedded JSON state on Kuaishou share pages.
const INIT_STATE_MARKER: &str = "window.INIT_STATE";

/// Extract the `window.INIT_STATE` JSON object from a share page.
pub(super) fn extract_init_state(html: &str) -> Result<serde_json::Value> {
    let raw = extract::extract_json_after(html, INIT_STATE_MARKER)
        .ok_or_else(|| ParserError::Parse("Cannot find INIT_STATE".to_string()))?;
    serde_json::from_str(raw)
        .map_err(|e| ParserError::Parse(format!("Failed to parse Kuaishou data: {}", e)))
}

/// Build a [`ParseResult`] from the `INIT_STATE` blob.
///
/// The photo payload lives under the first state entry with a non-null
/// `photo` value; the surrounding entries are heterogeneous and skipped.
pub(super) fn build_state_result(state: &serde_json::Value) -> Result<ParseResult> {
    let photo = state
        .as_object()
        .and_then(|obj| {
            obj.values()
                .find_map(|v| v.get("photo").filter(|p| !p.is_null()))
        })
        .ok_or_else(|| ParserError::ContentNotFound("Photo data not found".to_string()))?;

    let photo: PhotoInfo = serde_json::from_value(photo.clone())
        .map_err(|e| ParserError::Parse(format!("Failed to parse Kuaishou data: {}", e)))?;

    Ok(build_photo_result(&photo))
}

fn build_photo_result(photo: &PhotoInfo) -> ParseResult {
    let mut author = Author::new(photo.user_name.as_deref().unwrap_or("快手用户"));
    if let Some(avatar) = &photo.head_url {
        author = author.with_avatar(avatar);
    }

    let mut contents = Vec::new();

    // Check for video
    if let Some(video_url) = photo
        .main_mv_urls
        .as_ref()
        .and_then(|urls| urls.first())
        .and_then(|info| info.url.as_deref())
    {
        let mut video = VideoContent::new().with_url(video_url);
        if let Some(cover) = &photo.cover_url {
            video = video.with_cover(cover);
        }
        if let Some(duration) = photo.duration.map(|d| d as u64) {
            video = video.with_duration(duration / 1000);
        }
        contents.push(Content::Video(video));
    }

    // Check for images (slides)
    if contents.is_empty()
        && let Some(images) = photo.ext_params.as_ref().and_then(|ext| ext.photo.as_ref())
    {
        for image in images {
            if let Some(url) = &image.url {
                contents.push(Content::Image(ImageContent::new().with_url(url)));
            }
        }
    }

    let mut result = ParseResult::new(Platform::Kuaishou)
        .with_title(photo.caption.as_deref().unwrap_or(""))
        .with_author(author)
        .with_contents(contents);
    if let Some(timestamp) = photo.timestamp {
        result = result.with_timestamp(timestamp / 1000);
    }
    result
}
