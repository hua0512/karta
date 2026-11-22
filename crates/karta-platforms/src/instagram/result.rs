use karta_core::Platform;
use karta_core::data::{Author, Content, ImageContent, ParseResult, VideoContent};
use karta_core::error::{ParserError, Result};

use super::api::{ApiMediaItem, EmbedContext, MediaNode};

/// Builds a [`ParseResult`] from Instagram embed context data.
pub(super) fn build_result_from_context(
    ctx: EmbedContext,
    canonical_url: &str,
) -> Result<ParseResult> {
    let media = ctx
        .gql_data
        .and_then(|g| g.shortcode_media)
        .or(ctx.media)
        .ok_or_else(|| {
            ParserError::ContentNotFound("Instagram embed contains no media data".to_string())
        })?;

    Ok(build_result_from_media(&media, canonical_url))
}

/// Builds a [`ParseResult`] from an embed or GraphQL media node.
pub(super) fn build_result_from_media(media: &MediaNode, canonical_url: &str) -> ParseResult {
    let author = media.owner.as_ref().map(|owner| {
        let name = owner
            .full_name
            .as_deref()
            .filter(|name| !name.is_empty())
            .or(owner.username.as_deref())
            .unwrap_or("Instagram User");

        let mut author = Author::new(name);
        if let Some(avatar) = &owner.profile_pic_url {
            author = author.with_avatar(avatar);
        }
        if let Some(username) = &owner.username {
            author = author.with_url(format!("https://www.instagram.com/{username}/"));
        }
        author
    });

    let caption = media
        .edge_media_to_caption
        .as_ref()
        .and_then(|container| container.edges.first())
        .and_then(|edge| edge.node.text.as_deref());

    let mut result = ParseResult::new(Platform::Instagram)
        .with_url(canonical_url)
        .with_contents(collect_media_contents(media));

    if let Some(author) = author {
        result = result.with_author(author);
    }
    if let Some(text) = caption {
        result = result.with_text(text);
    }
    if let Some(timestamp) = media.taken_at_timestamp {
        result = result.with_timestamp(timestamp);
    }
    if let Some(likes) = media
        .edge_media_preview_like
        .as_ref()
        .and_then(|like| like.count)
    {
        result.add_extra("likes", likes.to_string());
    }
    if let Some(comments) = media.commenter_count {
        result.add_extra("comments", comments.to_string());
    }

    result
}

/// Builds a [`ParseResult`] from Instagram's REST media item shape.
pub(super) fn build_result_from_api_item(item: &ApiMediaItem, canonical_url: &str) -> ParseResult {
    let author = item.user.as_ref().map(|user| {
        let name = user
            .full_name
            .as_deref()
            .filter(|name| !name.is_empty())
            .or(user.username.as_deref())
            .unwrap_or("Instagram User");

        let mut author = Author::new(name);
        if let Some(avatar) = &user.profile_pic_url {
            author = author.with_avatar(avatar);
        }
        if let Some(username) = &user.username {
            author = author.with_url(format!("https://www.instagram.com/{username}/"));
        }
        author
    });

    let caption = item
        .caption
        .as_ref()
        .and_then(|caption| caption.text.as_deref());

    let mut result = ParseResult::new(Platform::Instagram)
        .with_url(canonical_url)
        .with_contents(collect_api_contents(item));

    if let Some(author) = author {
        result = result.with_author(author);
    }
    if let Some(text) = caption {
        result = result.with_text(text);
    }
    if let Some(timestamp) = item.taken_at {
        result = result.with_timestamp(timestamp);
    }
    if let Some(likes) = item.like_count {
        result.add_extra("likes", likes.to_string());
    }
    if let Some(comments) = item.comment_count {
        result.add_extra("comments", comments.to_string());
    }

    result
}

fn collect_media_contents(media: &MediaNode) -> Vec<Content> {
    // Carousels carry their slides in `edge_sidecar_to_children`. The
    // `__typename` differs between GraphQL shapes (`GraphSidecar`,
    // `XDTGraphSidecar`), so the children decide.
    if let Some(sidecar) = &media.edge_sidecar_to_children
        && !sidecar.edges.is_empty()
    {
        return sidecar
            .edges
            .iter()
            .filter_map(|edge| media_node_to_content(&edge.node))
            .collect();
    }

    media_node_to_content(media).into_iter().collect()
}

fn media_node_to_content(node: &MediaNode) -> Option<Content> {
    if node.is_video {
        let mut video = VideoContent::new().with_url(node.video_url.as_deref()?);
        if let Some(cover_url) = best_display_url(node) {
            video = video.with_cover(cover_url);
        }
        if let Some(duration) = node.video_duration {
            video = video.with_duration(duration as u64);
        }
        Some(Content::Video(video))
    } else {
        best_display_url(node).map(|url| Content::Image(ImageContent::new().with_url(url)))
    }
}

fn collect_api_contents(item: &ApiMediaItem) -> Vec<Content> {
    if item.media_type == Some(8)
        && let Some(children) = &item.carousel_media
    {
        return children.iter().filter_map(api_item_to_content).collect();
    }

    api_item_to_content(item).into_iter().collect()
}

fn api_item_to_content(item: &ApiMediaItem) -> Option<Content> {
    if item.media_type == Some(2) {
        let best = item
            .video_versions
            .as_ref()?
            .iter()
            .max_by_key(|version| version.width)?;
        let mut video = VideoContent::new().with_url(&best.url);
        if let Some(cover) = best_image_candidate_url(item) {
            video = video.with_cover(cover);
        }
        if let Some(duration) = item.video_duration {
            video = video.with_duration(duration as u64);
        }
        Some(Content::Video(video))
    } else {
        best_image_candidate_url(item).map(|url| Content::Image(ImageContent::new().with_url(url)))
    }
}

fn best_image_candidate_url(item: &ApiMediaItem) -> Option<&str> {
    item.image_versions2
        .as_ref()
        .and_then(|versions| {
            versions
                .candidates
                .iter()
                .max_by_key(|candidate| candidate.width)
        })
        .map(|candidate| candidate.url.as_str())
}

fn best_display_url(node: &MediaNode) -> Option<&str> {
    node.display_resources
        .iter()
        .max_by_key(|resource| resource.config_width)
        .map(|resource| resource.src.as_str())
        .or(node.display_url.as_deref())
}
