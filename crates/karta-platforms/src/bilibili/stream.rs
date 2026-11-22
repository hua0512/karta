//! Bilibili stream selection and [`VideoContent`] construction.

use karta_core::Platform;
use karta_core::config::{BiliCdnMode, BiliVideoCodec, BiliVideoQuality};
use karta_core::data::VideoContent;
use karta_core::http;
use wreq::header::{HeaderMap, HeaderValue, RANGE};

use super::api::{DashInfo, DashStream, StreamInfo};
use super::cdn::ordered_cdn_urls;

/// Download headers for Bilibili media CDNs.
///
/// Some CDN mirrors reject un-ranged full-object GETs with HTTP 567 while
/// serving `206 Partial Content` normally for an open-ended range from byte 0.
pub(super) fn stream_download_headers() -> HeaderMap {
    let mut headers = http::download_headers_for_platform(Platform::Bilibili);
    headers.insert(RANGE, HeaderValue::from_static("bytes=0-"));
    headers
}

fn stream_download_header_pairs() -> Vec<(String, String)> {
    stream_download_headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|v| (name.as_str().to_string(), v.to_string()))
        })
        .collect()
}

/// Stream-selection knobs shared by the video and bangumi paths.
#[derive(Debug, Clone, Copy)]
pub(super) struct StreamOptions<'a> {
    /// Requested quality ceiling.
    pub quality: BiliVideoQuality,
    /// Codec preference, applied within the chosen quality.
    pub codecs: &'a [BiliVideoCodec],
    /// How to rank the CDN hosts the API returned.
    pub cdn_mode: BiliCdnMode,
    /// Byte budget for the merged output. When set (and the duration is
    /// known), quality is stepped down until the estimate fits instead of
    /// letting the download fail on the size cap.
    pub size_budget: Option<u64>,
}

impl<'a> StreamOptions<'a> {
    /// Options that select purely by quality and codec, without a size budget.
    pub fn new(quality: BiliVideoQuality, codecs: &'a [BiliVideoCodec]) -> Self {
        Self {
            quality,
            codecs,
            cdn_mode: BiliCdnMode::default(),
            size_budget: None,
        }
    }
}

/// Build a [`VideoContent`] from Bilibili stream information.
pub(super) fn build_video_content_from_stream(
    stream: &StreamInfo,
    cover: &str,
    duration: Option<u64>,
    options: StreamOptions<'_>,
) -> VideoContent {
    let mut content = VideoContent::new().with_cover(cover);
    if let Some(duration) = duration {
        content = content.with_duration(duration);
    }

    if let Some(dash) = &stream.dash {
        let audio_stream = select_best_audio_stream(dash);
        let selected = select_video_stream(&dash.video, audio_stream, duration, &options);

        if let Some(video_stream) = selected {
            let video_urls = ordered_cdn_urls(
                &video_stream.base_url,
                &video_stream.backup_url,
                options.cdn_mode,
            );
            if let Some(audio_stream) = audio_stream {
                let audio_urls = ordered_cdn_urls(
                    &audio_stream.base_url,
                    &audio_stream.backup_url,
                    options.cdn_mode,
                );
                content = content.with_stream_candidates(video_urls, audio_urls);
            } else {
                content = content.with_url_candidates(video_urls);
            }

            if let (Some(width), Some(height)) = (video_stream.width, video_stream.height) {
                content = content.with_dimensions(width, height);
            }

            if let Some(codecs) = &video_stream.codecs {
                let label = format!(
                    "{}p {}",
                    video_stream.height.unwrap_or(0),
                    codecs.split('.').next().unwrap_or("unknown")
                );
                content = content.with_quality(label);
            }
            content = content.with_download_headers(stream_download_header_pairs());
        }
    } else if let Some(durl) = &stream.durl
        && let Some(first) = durl.first()
    {
        let urls = ordered_cdn_urls(&first.url, &first.backup_url, options.cdn_mode);
        content = content
            .with_url_candidates(urls)
            .with_size(first.size as u64)
            .with_download_headers(stream_download_header_pairs());
    }

    content
}

/// Pick the video stream to download, honoring the size budget when one is set.
fn select_video_stream<'a>(
    streams: &'a [DashStream],
    audio_stream: Option<&DashStream>,
    duration: Option<u64>,
    options: &StreamOptions<'_>,
) -> Option<&'a DashStream> {
    match (options.size_budget, duration) {
        (Some(budget), Some(duration)) if duration > 0 => select_video_stream_within_budget(
            streams,
            options.codecs,
            options.quality.qn(),
            audio_stream.map_or(0, |stream| stream.bandwidth),
            duration,
            budget,
        ),
        _ => select_best_video_stream(streams, options.quality, options.codecs),
    }
}

/// Select the best video stream for the requested quality and codec order.
fn select_best_video_stream<'a>(
    streams: &'a [DashStream],
    video_quality: BiliVideoQuality,
    video_codecs: &[BiliVideoCodec],
) -> Option<&'a DashStream> {
    let selected_quality = preferred_qualities(streams, video_quality.qn())
        .into_iter()
        .next()?;

    select_at_quality(streams, video_codecs, selected_quality)
}

/// Step down through the preference order until the estimated merged size fits
/// within `budget`.
///
/// Falls back to the lowest-bitrate candidate when nothing fits, so an
/// oversized video still sends something rather than failing outright.
fn select_video_stream_within_budget<'a>(
    streams: &'a [DashStream],
    video_codecs: &[BiliVideoCodec],
    target_quality: i32,
    audio_bandwidth: i64,
    duration: u64,
    budget: u64,
) -> Option<&'a DashStream> {
    let mut smallest: Option<&DashStream> = None;

    for quality in preferred_qualities(streams, target_quality) {
        let Some(stream) = select_at_quality(streams, video_codecs, quality) else {
            continue;
        };

        if estimated_size(stream.bandwidth, audio_bandwidth, duration) <= budget {
            return Some(stream);
        }

        tracing::debug!(
            quality,
            bandwidth = stream.bandwidth,
            budget,
            "Skipping Bilibili stream whose estimated size exceeds the limit"
        );
        if smallest.is_none_or(|best| stream.bandwidth < best.bandwidth) {
            smallest = Some(stream);
        }
    }

    smallest
}

/// Estimated merged bytes for a DASH pair, from bandwidths in bits per second.
fn estimated_size(video_bandwidth: i64, audio_bandwidth: i64, duration: u64) -> u64 {
    let bits_per_second = video_bandwidth.saturating_add(audio_bandwidth).max(0) as u64;
    bits_per_second / 8 * duration
}

/// Qualities in preference order: the closest one at or below the target
/// first, then progressively lower, and finally anything above it.
fn preferred_qualities(streams: &[DashStream], target_quality: i32) -> Vec<i32> {
    let eligible = |quality: &i32| is_supported_quality(*quality, target_quality);

    let mut at_or_below: Vec<i32> = streams
        .iter()
        .map(|stream| stream.id)
        .filter(|quality| eligible(quality) && *quality <= target_quality)
        .collect();
    at_or_below.sort_unstable_by(|a, b| b.cmp(a));
    at_or_below.dedup();

    let mut above: Vec<i32> = streams
        .iter()
        .map(|stream| stream.id)
        .filter(|quality| eligible(quality) && *quality > target_quality)
        .collect();
    above.sort_unstable();
    above.dedup();

    at_or_below.extend(above);
    at_or_below
}

fn select_at_quality<'a>(
    streams: &'a [DashStream],
    video_codecs: &[BiliVideoCodec],
    quality: i32,
) -> Option<&'a DashStream> {
    for codec in video_codecs {
        if let Some(stream) = streams.iter().find(|stream| {
            stream.id == quality
                && stream
                    .codecs
                    .as_deref()
                    .is_some_and(|value| codec.matches(value))
        }) {
            return Some(stream);
        }
    }

    streams.iter().find(|stream| stream.id == quality)
}

fn is_supported_quality(quality: i32, target_quality: i32) -> bool {
    match quality {
        // HDR and Dolby Vision are only selected when explicitly requested.
        125 | 126 => quality == target_quality,
        _ => true,
    }
}

fn select_best_audio_stream(dash: &DashInfo) -> Option<&DashStream> {
    dash.audio
        .iter()
        .chain(dash.dolby.iter().flat_map(|dolby| dolby.audio.iter()))
        .chain(dash.flac.iter().filter_map(|flac| flac.audio.as_ref()))
        .max_by_key(|stream| stream.bandwidth)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dash_stream(id: i32, codecs: Option<&str>) -> DashStream {
        DashStream {
            id,
            base_url: format!("https://cn-jsnt-ct-01-07.bilivideo.com/{id}.m4s"),
            backup_url: Vec::new(),
            bandwidth: i64::from(id) * 1000,
            codecs: codecs.map(ToString::to_string),
            width: Some(1920),
            height: Some(id as u32),
        }
    }

    #[test]
    fn selector_prefers_quality_before_codec() {
        use BiliVideoCodec::{AV1, AVC};
        use BiliVideoQuality::{Q8K, Q720P, Q1080P, Q1080P60, QDolby};

        /// (name, (quality id, codecs) per stream, target, codec order, expected id)
        type Case<'a> = (
            &'a str,
            &'a [(i32, &'a str)],
            BiliVideoQuality,
            &'a [BiliVideoCodec],
            i32,
        );
        let cases: [Case; 5] = [
            (
                "quality wins over codec order",
                &[(116, "av01.0.08M.08"), (80, "avc1.640032")],
                Q1080P60,
                &[AVC, AV1],
                116,
            ),
            (
                "no codec match takes the best quality",
                &[(64, "unknown"), (80, "also-unknown")],
                Q1080P,
                &[AVC],
                80,
            ),
            (
                "nothing at or below the target goes above it",
                &[
                    (80, "avc1.640032"),
                    (112, "avc1.640033"),
                    (120, "avc1.640033"),
                ],
                Q720P,
                &[AVC],
                80,
            ),
            (
                "dolby when explicitly requested",
                &[(126, "avc1.dolby"), (125, "avc1.hdr"), (80, "avc1.640032")],
                QDolby,
                &[AVC],
                126,
            ),
            (
                "HDR and dolby skipped for an ordinary target",
                &[(126, "avc1.dolby"), (125, "avc1.hdr"), (120, "avc1.640033")],
                Q8K,
                &[AVC],
                120,
            ),
        ];

        for (name, streams, quality, codecs, expected) in cases {
            let streams: Vec<DashStream> = streams
                .iter()
                .map(|(id, codec)| dash_stream(*id, Some(codec)))
                .collect();
            let selected = select_best_video_stream(&streams, quality, codecs).unwrap();
            assert_eq!(selected.id, expected, "{name}");
        }
    }

    /// 1000 kbps of video + 128 kbps of audio over 100 s ≈ 14 MB.
    fn budget_streams() -> Vec<DashStream> {
        let mut high = dash_stream(120, Some("avc1.640033"));
        high.bandwidth = 8_000_000;
        let mut medium = dash_stream(80, Some("avc1.640032"));
        medium.bandwidth = 2_000_000;
        let mut low = dash_stream(64, Some("avc1.640031"));
        low.bandwidth = 500_000;
        vec![high, medium, low]
    }

    #[test]
    fn budget_selection_steps_down_to_the_best_fitting_quality() {
        const MB: u64 = 1024 * 1024;
        let streams = budget_streams();
        let codecs = [BiliVideoCodec::AVC];
        let mut audio = dash_stream(30280, Some("mp4a.40.2"));
        audio.bandwidth = 4_000_000;

        // At 100 s: 8 Mbps ≈ 100 MB, 2 Mbps ≈ 25 MB, 0.5 Mbps ≈ 6 MB.
        let cases = [
            ("best fitting quality", 30 * MB, None, Some(100), 80),
            (
                "audio counts towards the budget",
                60 * MB,
                Some(&audio),
                Some(100),
                64,
            ),
            ("nothing fits: smallest stream", 1024, None, Some(100), 64),
            ("unknown duration: budget ignored", 1024, None, None, 120),
        ];

        for (name, budget, audio, duration, expected) in cases {
            let options = StreamOptions {
                size_budget: Some(budget),
                ..StreamOptions::new(BiliVideoQuality::Q4K, &codecs)
            };
            let selected = select_video_stream(&streams, audio, duration, &options).unwrap();
            assert_eq!(selected.id, expected, "{name}");
        }
    }
}
