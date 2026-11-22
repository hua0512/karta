//! Instagram story result construction.

use karta_core::Platform;
use karta_core::data::{Author, Content, ImageContent, ParseResult, VideoContent};

use super::api::{ReelMedia, StoryItem};

/// Builds a [`ParseResult`] from a selected story item.
pub(super) fn build_story_result(
    reel: &ReelMedia,
    item: &StoryItem,
    username: &str,
    story_url: &str,
) -> ParseResult {
    let author = reel.user.as_ref().map(|user| {
        let name = user
            .full_name
            .as_deref()
            .filter(|name| !name.is_empty())
            .or(user.username.as_deref())
            .unwrap_or(username);

        let mut author = Author::new(name);
        if let Some(avatar) = &user.profile_pic_url {
            author = author.with_avatar(avatar);
        }
        author.with_url(format!("https://www.instagram.com/{username}/"))
    });

    let mut result = ParseResult::new(Platform::Instagram).with_url(story_url);

    if let Some(author) = author {
        result = result.with_author(author);
    }
    if let Some(timestamp) = item.taken_at {
        result = result.with_timestamp(timestamp);
    }
    if let Some(content) = story_item_to_content(item) {
        result = result.with_content(content);
    }

    result
}

fn story_item_to_content(item: &StoryItem) -> Option<Content> {
    if item.media_type == Some(2) {
        let best = item
            .video_versions
            .as_ref()?
            .iter()
            .max_by_key(|version| version.width)?;
        let mut video = VideoContent::new().with_url(&best.url);
        if let Some(cover) = item.image_versions2.as_ref().and_then(|versions| {
            versions
                .candidates
                .iter()
                .max_by_key(|candidate| candidate.width)
        }) {
            video = video.with_cover(&cover.url);
        }
        Some(Content::Video(video))
    } else {
        item.image_versions2
            .as_ref()
            .and_then(|versions| {
                versions
                    .candidates
                    .iter()
                    .max_by_key(|candidate| candidate.width)
            })
            .map(|candidate| Content::Image(ImageContent::new().with_url(&candidate.url)))
    }
}
