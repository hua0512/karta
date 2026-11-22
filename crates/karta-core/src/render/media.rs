use futures::{StreamExt, stream};

use crate::data::{Content, ImageContent, ParseResult};
use crate::download::SharedDownloader;
use crate::error::RenderResult;
use crate::http;
use crate::message::{MessageOutput, MessageSegment, RenderOutput, SegmentType};

use super::video::{CoverFallback, resolve_video_segment};
use super::{MAX_TEXT_SEGMENT_CHARS, RenderOptions, split_text_segments};

enum Patch {
    AuthorAvatar(String),
    VideoCover {
        idx: usize,
        path: String,
    },
    ImagePath {
        idx: usize,
        path: std::path::PathBuf,
    },
    GraphicsImagePath {
        idx: usize,
        img_idx: usize,
        path: std::path::PathBuf,
    },
    DynamicGifCover {
        idx: usize,
        path: String,
    },
}

/// Apply option-driven mutations that hold for every render, whether or not
/// media downloads run. Currently: hide the URL section when `append_url` is
/// off (card renderers show the URL only when `result.url` is set).
fn apply_url_visibility(result: &mut ParseResult, options: &RenderOptions) {
    if !options.append_url {
        result.url = None;
    }
}

/// Download avatars, covers, and images into local paths.
///
/// Also applies [`apply_url_visibility`] so the produced result is ready for
/// card rendering. Takes `ParseResult` **by value** so callers that already
/// own it avoid an extra clone. Returns the mutated result with local paths
/// filled in.
pub(crate) async fn hydrate_media(
    mut result: ParseResult,
    downloader: &SharedDownloader,
    options: &RenderOptions,
) -> RenderResult<ParseResult> {
    apply_url_visibility(&mut result, options);

    if !options.download_media {
        return Ok(result);
    }

    let concurrency = options.download_concurrency.max(1);
    let downloader = downloader.clone();
    let platform = result.platform;

    let mut futures: Vec<
        std::pin::Pin<Box<dyn std::future::Future<Output = Option<Patch>> + Send>>,
    > = Vec::new();

    if let Some(author) = &result.author
        && let Some(avatar_url) = author.avatar.clone()
    {
        let downloader = downloader.clone();
        futures.push(Box::pin(async move {
            let headers = http::download_headers_for_platform(platform);
            let path = match downloader.download_avatar(&avatar_url, Some(headers)).await {
                Ok(path) => path,
                Err(e) => {
                    tracing::warn!(url = %avatar_url, "Failed to download avatar: {e}");
                    return None;
                }
            };
            Some(Patch::AuthorAvatar(path.to_string_lossy().to_string()))
        }));
    }

    for (idx, content) in result.contents.iter().enumerate() {
        match content {
            Content::Video(video) => {
                if let Some(cover_url) = video.cover.clone() {
                    let downloader = downloader.clone();
                    futures.push(Box::pin(async move {
                        let headers = http::download_headers_for_platform(platform);
                        let path = match downloader.download_cover(&cover_url, Some(headers)).await
                        {
                            Ok(path) => path,
                            Err(e) => {
                                tracing::warn!(
                                    url = %cover_url,
                                    "Failed to download video cover: {e}"
                                );
                                return None;
                            }
                        };
                        Some(Patch::VideoCover {
                            idx,
                            path: path.to_string_lossy().to_string(),
                        })
                    }));
                }
            }
            Content::Image(image) => {
                if image.path.is_none()
                    && let Some(url) = image.url.clone()
                {
                    let downloader = downloader.clone();
                    futures.push(Box::pin(async move {
                        let headers = http::download_headers_for_platform(platform);
                        let path = match downloader.download_image(&url, Some(headers)).await {
                            Ok(path) => path,
                            Err(e) => {
                                tracing::warn!(url = %url, "Failed to download image: {e}");
                                return None;
                            }
                        };
                        Some(Patch::ImagePath { idx, path })
                    }));
                }
            }
            Content::Graphics(graphics) => {
                for (img_idx, ImageContent { path, url, .. }) in graphics.images.iter().enumerate()
                {
                    if path.is_none()
                        && let Some(url) = url.clone()
                    {
                        let downloader = downloader.clone();
                        futures.push(Box::pin(async move {
                            let headers = http::download_headers_for_platform(platform);
                            let path = match downloader.download_image(&url, Some(headers)).await {
                                Ok(path) => path,
                                Err(e) => {
                                    tracing::warn!(
                                        url = %url,
                                        "Failed to download graphics image: {e}"
                                    );
                                    return None;
                                }
                            };
                            Some(Patch::GraphicsImagePath { idx, img_idx, path })
                        }));
                    }
                }
            }
            Content::Dynamic(dynamic) => {
                if dynamic.should_convert_to_gif()
                    && let Some(video_url) = dynamic.video_url.clone()
                {
                    let max_duration = dynamic.gif_max_duration;
                    let downloader = downloader.clone();
                    futures.push(Box::pin(async move {
                        let headers = http::download_headers_for_platform(platform);
                        let path = match downloader
                            .download_as_gif(&video_url, Some(headers), max_duration, None, None)
                            .await
                        {
                            Ok(path) => path,
                            Err(e) => {
                                tracing::warn!(
                                    url = %video_url,
                                    "Failed to convert dynamic cover to GIF: {e}"
                                );
                                return None;
                            }
                        };
                        Some(Patch::DynamicGifCover {
                            idx,
                            path: path.to_string_lossy().to_string(),
                        })
                    }));
                }
            }
            _ => {}
        }
    }

    let patches: Vec<Patch> = stream::iter(futures)
        .buffer_unordered(concurrency)
        .filter_map(|patch| async move { patch })
        .collect()
        .await;

    for patch in patches {
        match patch {
            Patch::AuthorAvatar(path) => {
                if let Some(author) = &mut result.author {
                    author.avatar = Some(path);
                }
            }
            Patch::VideoCover { idx, path } => {
                if let Some(Content::Video(video)) = result.contents.get_mut(idx) {
                    video.cover = Some(path);
                }
            }
            Patch::ImagePath { idx, path } => {
                if let Some(Content::Image(image)) = result.contents.get_mut(idx) {
                    image.path = Some(path);
                    image.url = None;
                }
            }
            Patch::GraphicsImagePath { idx, img_idx, path } => {
                if let Some(Content::Graphics(graphics)) = result.contents.get_mut(idx)
                    && let Some(image) = graphics.images.get_mut(img_idx)
                {
                    image.path = Some(path);
                    image.url = None;
                }
            }
            Patch::DynamicGifCover { idx, path } => {
                if let Some(Content::Dynamic(dynamic)) = result.contents.get_mut(idx) {
                    dynamic.cover_url = Some(path);
                }
            }
        }
    }

    if let Some(repost) = result.repost.take() {
        let hydrated_repost = Box::pin(hydrate_media(*repost, &downloader, options)).await?;
        result.repost = Some(Box::new(hydrated_repost));
    }

    Ok(result)
}

/// Append downloaded media (video, image, audio) as separate outputs.
///
/// Used by [`super::image::ImageRenderer`]. Downloads run with bounded concurrency
/// (`download_concurrency`) while output order follows content order.
/// Mergeable segments (images) become one [`RenderOutput::Bundle`] when the
/// total media count exceeds `forward_media_threshold`. Text content joins
/// the bundle so captions stay with their photos; how a bundle is presented
/// (QQ merged-forward, Telegram media group, ...) is the adapter's call.
pub(crate) async fn append_media_messages(
    messages: &mut Vec<RenderOutput>,
    result: &ParseResult,
    downloader: &SharedDownloader,
    options: &RenderOptions,
) {
    let platform = result.platform;

    let mut content_texts: Vec<String> = Vec::new();
    let mut immediate: Vec<(usize, MessageSegment)> = Vec::new();
    let mut downloads: Vec<futures::future::BoxFuture<'static, (usize, Option<MessageSegment>)>> =
        Vec::new();

    for (order, content) in result.contents.iter().enumerate() {
        match content {
            Content::Video(video) => {
                let video = video.clone();
                let downloader = downloader.clone();
                downloads.push(Box::pin(async move {
                    (
                        order,
                        // Skip the cover fallback: this runs after a card that
                        // already displays the cover, so emitting it again
                        // would duplicate the image.
                        resolve_video_segment(video, downloader, platform, CoverFallback::Skip)
                            .await,
                    )
                }));
            }
            Content::Image(image) => {
                if let Some(path) = &image.path {
                    immediate.push((order, MessageSegment::image_file(path.clone())));
                } else if let Some(url) = &image.url {
                    immediate.push((order, MessageSegment::image_url(url)));
                }
            }
            Content::Text(text) => {
                content_texts.push(text.clone());
            }
            Content::Audio(audio) => {
                if let Some(path) = &audio.path {
                    immediate.push((order, MessageSegment::audio_file(path.clone())));
                } else if let Some(url) = audio.url.as_ref().filter(|url| !url.trim().is_empty()) {
                    let url = url.clone();
                    let downloader = downloader.clone();
                    downloads.push(Box::pin(async move {
                        let headers = http::download_headers_for_platform(platform);
                        let segment = match downloader.download_audio(&url, Some(headers)).await {
                            Ok(path) => MessageSegment::audio_file(path),
                            Err(e) => {
                                tracing::warn!("Failed to download audio: {e}");
                                MessageSegment::audio_url(&url)
                            }
                        };
                        (order, Some(segment))
                    }));
                }
            }
            _ => {}
        }
    }

    let concurrency = options.download_concurrency.max(1);
    let downloaded: Vec<(usize, Option<MessageSegment>)> = stream::iter(downloads)
        .buffer_unordered(concurrency)
        .collect()
        .await;

    let mut ordered = immediate;
    ordered.extend(
        downloaded
            .into_iter()
            .filter_map(|(order, segment)| segment.map(|segment| (order, segment))),
    );
    ordered.sort_by_key(|(order, _)| *order);

    // Images can be grouped into a bundle; audio/video must stay as
    // standalone messages.
    let mut mergeable: Vec<MessageSegment> = Vec::new();
    let mut deferred: Vec<MessageSegment> = Vec::new();
    for (_, segment) in ordered {
        match segment.segment_type {
            SegmentType::Audio | SegmentType::Video => deferred.push(segment),
            _ if segment.is_failure_notice => deferred.push(segment),
            _ => mergeable.push(segment),
        }
    }

    let total = mergeable.len() + deferred.len();
    let should_bundle = options.forward_contents
        || (options.forward_media_threshold > 0 && total > options.forward_media_threshold);

    let mut text_chunks: Vec<String> = Vec::new();
    if !content_texts.is_empty() {
        for text in &content_texts {
            text_chunks.extend(split_text_segments(text, MAX_TEXT_SEGMENT_CHARS));
        }
    } else if let Some(text) = result
        .text
        .as_ref()
        .map(|text| text.trim())
        .filter(|text| !text.is_empty())
    {
        text_chunks.extend(split_text_segments(text, MAX_TEXT_SEGMENT_CHARS));
    }

    if should_bundle && !mergeable.is_empty() {
        let mut bundle: Vec<MessageOutput> = text_chunks
            .into_iter()
            .map(MessageOutput::from_text)
            .collect();
        bundle.extend(mergeable.into_iter().map(MessageOutput::from_segment));
        messages.push(RenderOutput::Bundle(bundle));
    } else {
        for segment in mergeable {
            messages.push(RenderOutput::Message(MessageOutput::from_segment(segment)));
        }
    }

    for segment in deferred {
        messages.push(RenderOutput::Message(MessageOutput::from_segment(segment)));
    }
}
