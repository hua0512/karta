use karta_core::Platform;
use karta_core::data::{Author, Content, ParseResult, VideoContent};

use super::api::{KsPlayInfo, VideoInfo};

pub(super) fn build_video_result(acid: &str, info: &VideoInfo) -> ParseResult {
    let mut author = Author::new(
        info.user
            .as_ref()
            .and_then(|user| user.name.as_deref())
            .unwrap_or("AcFun用户"),
    );
    if let Some(avatar) = info.user.as_ref().and_then(|user| user.head_url.as_deref()) {
        author = author.with_avatar(avatar);
    }

    let mut result = ParseResult::new(Platform::AcFun)
        .with_title(info.title.as_deref().unwrap_or(""))
        .with_author(author)
        .with_url(format!("https://www.acfun.cn/v/ac{}", acid));

    if let Some(millis) = info.create_time_millis {
        result = result.with_timestamp(millis / 1000);
    }

    if let Some(video) = build_video_content(info) {
        result.add_content(Content::Video(video));
    }

    if let Some(desc) = &info.description {
        result = result.with_text(desc);
    }

    result
}

fn build_video_content(info: &VideoInfo) -> Option<VideoContent> {
    let current = info.current_video_info.as_ref()?;
    let play: KsPlayInfo = serde_json::from_str(current.ks_play_json.as_deref()?).ok()?;
    let url = first_stream_url(&play)?;

    let mut video = VideoContent::new().with_url(url);
    if let Some(cover) = &info.cover_url {
        video = video.with_cover(cover);
    }
    if let Some(duration) = current.duration_millis {
        video = video.with_duration((duration / 1000) as u64);
    }
    Some(video)
}

/// URL of the first adaptation set whose *first* representation exposes a
/// direct URL.
fn first_stream_url(play: &KsPlayInfo) -> Option<String> {
    play.adaptation_set
        .as_ref()?
        .iter()
        .find_map(|set| set.representation.as_ref()?.first()?.url.clone())
}
