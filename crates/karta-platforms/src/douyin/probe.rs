use karta_core::data::{Content, ParseResult};
use wreq::header::{HeaderMap, HeaderValue};
use wreq::redirect::Policy;

use super::result::{DEFAULT_PLAY_RATIO, cdn_play_url};
use super::{CDN_VIDEO_PLAY, DouyinParser};

/// CDN play ratios in preference order; probing stops at the first ratio the
/// CDN actually serves.
const PLAY_RATIOS: [&str; 4] = ["1080p", "720p", "540p", "360p"];

impl DouyinParser {
    /// Rewrite each CDN play URL in `result` to the highest ratio the CDN
    /// actually serves.
    ///
    /// `result.rs` builds URLs with the optimistic [`DEFAULT_PLAY_RATIO`];
    /// videos not available at 1080p get an empty response from the CDN
    /// instead of an error, so we probe before handing the URL on. On total
    /// probe failure the 1080p URL is kept.
    pub(super) async fn probe_cdn_ratios(&self, result: &mut ParseResult) {
        for content in &mut result.contents {
            let Content::Video(video) = content else {
                continue;
            };
            let Some(url) = video.url.clone() else {
                continue;
            };
            if !url.starts_with(CDN_VIDEO_PLAY) {
                continue;
            }
            let Some(video_id) = cdn_video_id(&url) else {
                continue;
            };
            match self.probe_play_ratio(video_id).await {
                Some(ratio) if ratio != DEFAULT_PLAY_RATIO => {
                    tracing::debug!(video_id, ratio, "Douyin: probed CDN play ratio");
                    video.url = Some(cdn_play_url(video_id, ratio));
                }
                Some(_) => {}
                None => {
                    tracing::debug!(video_id, "Douyin: CDN ratio probe failed, keeping 1080p");
                }
            }
        }
    }

    /// Probe [`PLAY_RATIOS`] in order and return the first ratio the CDN
    /// serves content for.
    ///
    /// Sends a `Range: bytes=0-1` GET and accepts only responses that advertise
    /// a real content size (`Content-Range` total, or a `Content-Length` larger
    /// than the 2 requested bytes).
    async fn probe_play_ratio(&self, video_id: &str) -> Option<&'static str> {
        let client = self.downloader.client();
        for ratio in PLAY_RATIOS {
            let url = cdn_play_url(video_id, ratio);
            let mut headers = self.mobile_headers();
            headers.insert(wreq::header::RANGE, HeaderValue::from_static("bytes=0-1"));

            let response = match client
                .get(&url)
                .headers(headers)
                .redirect(Policy::limited(5))
                .send()
                .await
            {
                Ok(response) => response,
                Err(e) => {
                    tracing::debug!(video_id, ratio, "Douyin: ratio probe request failed: {}", e);
                    continue;
                }
            };

            if response.status().is_success() && probed_content_size(response.headers()).is_some() {
                return Some(ratio);
            }
        }
        None
    }
}

/// Extract the `video_id` query parameter from a CDN play URL.
fn cdn_video_id(url: &str) -> Option<&str> {
    let start = url.find("video_id=")? + "video_id=".len();
    let rest = &url[start..];
    let id = rest.split('&').next().unwrap_or(rest);
    (!id.is_empty()).then_some(id)
}

/// Total content size advertised by a ratio-probe response.
///
/// Prefers the `Content-Range` total (`bytes 0-1/{total}`); falls back to a
/// `Content-Length` greater than the 2 requested bytes.
fn probed_content_size(headers: &HeaderMap) -> Option<u64> {
    let range_total = headers
        .get(wreq::header::CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.rsplit('/').next())
        .and_then(|total| total.parse::<u64>().ok())
        .filter(|total| *total > 0);
    if range_total.is_some() {
        return range_total;
    }

    headers
        .get(wreq::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|size| *size > 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_probed_content_size() {
        fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
            let mut headers = HeaderMap::new();
            for (name, value) in pairs {
                headers.insert(
                    wreq::header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                    value.parse().unwrap(),
                );
            }
            headers
        }

        // Unknown total ('*') falls back to Content-Length, which must be
        // larger than the 2 requested bytes.
        assert_eq!(
            probed_content_size(&headers(&[
                ("content-range", "bytes 0-1/*"),
                ("content-length", "2"),
            ])),
            None
        );
        assert_eq!(
            probed_content_size(&headers(&[("content-length", "2")])),
            None
        );
        assert_eq!(
            probed_content_size(&headers(&[("content-length", "1048576")])),
            Some(1048576)
        );
    }
}
