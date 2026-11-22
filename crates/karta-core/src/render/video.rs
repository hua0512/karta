//! Shared video-resolution logic for renderers.
//!
//! Both the text renderer ([`super::text`]) and the media appender
//! ([`super::media`]) need to turn a [`VideoContent`] into a message segment.
//! The decision tree (merge streams / decrypt / BGM merge / plain download /
//! cover fallback) lives here so the two paths cannot drift.

use std::path::PathBuf;

use crate::constants::Platform;
use crate::data::VideoContent;
use crate::download::SharedDownloader;
use crate::error::DownloadError;
use crate::http;
use crate::message::MessageSegment;

/// The download strategy chosen for a [`VideoContent`], decided before any I/O.
///
/// URL fields are ranked candidate lists for the same media: the primary URL
/// followed by any backups the parser collected (e.g. a Bilibili CDN and its
/// mirrors). The downloader walks them in order until one succeeds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum VideoAction {
    /// Use the already-downloaded local file.
    LocalFile(PathBuf),
    /// DASH streams: download video + audio and merge.
    Merge {
        video_urls: Vec<String>,
        audio_urls: Vec<String>,
    },
    /// Encrypted single stream: download and decrypt.
    Decrypt { url: String, key: String },
    /// Single stream with a separate BGM track: merge, falling back to a
    /// plain download of the video stream.
    DownloadWithBgm { urls: Vec<String>, bgm_url: String },
    /// Plain single-stream download.
    Download { urls: Vec<String> },
    /// No downloadable stream: show the cover if present.
    CoverOnly,
}

/// How to represent a video whose download failed (or that never had a
/// stream URL).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CoverFallback {
    /// Emit the cover image in place of the video. Used by the text renderer,
    /// where nothing else shows the cover.
    Degrade,
    /// Emit nothing. Used by the card paths ([`super::media`]): the card
    /// image already displays the cover, so a fallback would duplicate it.
    Skip,
}

/// Pick the download strategy for a video. Pure so branch selection is
/// unit-testable without network access.
pub(crate) fn plan_video_action(video: &VideoContent) -> VideoAction {
    if let Some(path) = &video.path {
        return VideoAction::LocalFile(path.clone());
    }

    // Separate DASH streams take precedence over a combined URL, matching the
    // historical behavior of both render paths.
    if video.video_url.is_some() && video.audio_url.is_some() {
        return VideoAction::Merge {
            video_urls: video.video_url_candidates(),
            audio_urls: video.audio_url_candidates(),
        };
    }

    if let Some(url) = &video.url {
        if let Some(key) = &video.decryption_key {
            return VideoAction::Decrypt {
                url: url.clone(),
                key: key.clone(),
            };
        }
        // audio_url without video_url: a BGM track to merge into `url`.
        if let Some(bgm_url) = &video.audio_url {
            return VideoAction::DownloadWithBgm {
                urls: video.url_candidates(),
                bgm_url: bgm_url.clone(),
            };
        }
        return VideoAction::Download {
            urls: video.url_candidates(),
        };
    }

    VideoAction::CoverOnly
}

/// Resolve a video into a message segment, downloading as needed.
///
/// Configured size/duration rejections produce a failure notice. For other
/// failures or missing streams, [`CoverFallback::Degrade`] emits the cover
/// image and [`CoverFallback::Skip`] emits nothing.
pub(crate) async fn resolve_video_segment(
    video: VideoContent,
    downloader: SharedDownloader,
    platform: Platform,
    fallback: CoverFallback,
) -> Option<MessageSegment> {
    let headers = video_download_headers(&video, platform);
    match plan_video_action(&video) {
        VideoAction::LocalFile(path) => Some(MessageSegment::video_file(path)),
        VideoAction::Merge {
            video_urls,
            audio_urls,
        } => {
            match downloader
                .download_and_merge_candidates(&video_urls, &audio_urls, Some(headers))
                .await
            {
                Ok(path) => Some(MessageSegment::video_file(path)),
                Err(e) => {
                    tracing::warn!("Failed to download video: {e}");
                    download_failure(&e, &video, fallback)
                }
            }
        }
        VideoAction::Decrypt { url, key } => {
            match downloader
                .download_video_decrypt(&url, &key, Some(headers))
                .await
            {
                Ok(path) => Some(MessageSegment::video_file(path)),
                Err(e) => {
                    tracing::warn!("Failed to decrypt video: {e}");
                    download_failure(&e, &video, fallback)
                }
            }
        }
        VideoAction::DownloadWithBgm { urls, bgm_url } => {
            let primary = urls.first().cloned().unwrap_or_default();
            let result = match downloader
                .download_and_merge_bgm(&primary, &bgm_url, Some(headers.clone()))
                .await
            {
                Ok(path) => Ok(path),
                Err(e) => {
                    tracing::warn!("Failed to merge BGM into video: {e}");
                    downloader
                        .download_video_candidates(&urls, Some(headers))
                        .await
                }
            };
            match result {
                Ok(path) => Some(MessageSegment::video_file(path)),
                Err(e) => {
                    tracing::warn!("Failed to download video: {e}");
                    download_failure(&e, &video, fallback)
                }
            }
        }
        VideoAction::Download { urls } => {
            match downloader
                .download_video_candidates(&urls, Some(headers))
                .await
            {
                Ok(path) => Some(MessageSegment::video_file(path)),
                Err(e) => {
                    tracing::warn!("Failed to download video: {e}");
                    download_failure(&e, &video, fallback)
                }
            }
        }
        VideoAction::CoverOnly => fallback_cover(&video, fallback),
    }
}

fn download_failure(
    error: &DownloadError,
    video: &VideoContent,
    fallback: CoverFallback,
) -> Option<MessageSegment> {
    match error.limit_notice() {
        Some(notice) => Some(MessageSegment::failure_notice(notice)),
        None => fallback_cover(video, fallback),
    }
}

/// Apply the caller's [`CoverFallback`] policy for a failed/stream-less video.
fn fallback_cover(video: &VideoContent, fallback: CoverFallback) -> Option<MessageSegment> {
    match fallback {
        CoverFallback::Degrade => cover_segment(video.cover.as_deref()),
        CoverFallback::Skip => None,
    }
}

/// Build an image segment for a cover reference: a local file after
/// hydration, a remote URL otherwise.
fn cover_segment(cover: Option<&str>) -> Option<MessageSegment> {
    let cover = cover?;
    let path = std::path::Path::new(cover);
    if path.is_file() {
        Some(MessageSegment::image_file(path.to_path_buf()))
    } else {
        Some(MessageSegment::image_url(cover))
    }
}

/// Headers for downloading a video: the video's own `download_headers` when set
/// (e.g. an app `User-Agent` for a CDN that rejects browser agents), otherwise
/// the platform default.
fn video_download_headers(video: &VideoContent, platform: Platform) -> wreq::header::HeaderMap {
    match video.download_headers.as_deref() {
        Some(pairs) if !pairs.is_empty() => http::headers_from_pairs(pairs),
        _ => http::download_headers_for_platform(platform),
    }
}
