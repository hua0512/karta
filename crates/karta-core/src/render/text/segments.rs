use futures::future::BoxFuture;
use futures::{FutureExt, StreamExt, stream};

use crate::data::{Content, ParseResult};
use crate::download::SharedDownloader;
use crate::http;
use crate::message::MessageSegment;

use super::super::video::{CoverFallback, resolve_video_segment};
use super::super::{MAX_TEXT_SEGMENT_CHARS, RenderOptions, split_text_segments};

type OrderedSegment = (usize, MessageSegment);
type SegmentFuture = BoxFuture<'static, (usize, Option<MessageSegment>)>;

pub(super) async fn collect_ordered_segments(
    result: &ParseResult,
    downloader: &SharedDownloader,
    options: &RenderOptions,
) -> Vec<MessageSegment> {
    let mut collector = SegmentCollector::new(result, downloader, options);
    collector.collect_primary_contents(result);
    collector.collect_repost_images(result);
    collector.resolve().await
}

struct SegmentCollector<'a> {
    downloader: SharedDownloader,
    options: &'a RenderOptions,
    platform: crate::constants::Platform,
    out: Vec<OrderedSegment>,
    downloads: Vec<SegmentFuture>,
    order: usize,
}

impl<'a> SegmentCollector<'a> {
    fn new(
        result: &ParseResult,
        downloader: &SharedDownloader,
        options: &'a RenderOptions,
    ) -> Self {
        Self {
            downloader: downloader.clone(),
            options,
            platform: result.platform,
            out: Vec::new(),
            downloads: Vec::new(),
            order: 0,
        }
    }

    fn collect_primary_contents(&mut self, result: &ParseResult) {
        if !result
            .contents
            .iter()
            .any(|content| matches!(content, Content::Text(_)))
            && let Some(text) = result
                .text
                .as_deref()
                .map(str::trim)
                .filter(|text| !text.is_empty())
        {
            self.push_text(text);
        }

        for content in &result.contents {
            self.push_content(content);
        }
    }

    fn collect_repost_images(&mut self, result: &ParseResult) {
        let Some(repost) = &result.repost else {
            return;
        };

        for content in &repost.contents {
            if let Content::Image(image) = content {
                if let Some(path) = &image.path {
                    self.push_immediate(MessageSegment::image_file(path.clone()));
                } else if let Some(url) = &image.url {
                    self.push_image_url(url, repost.platform);
                }
            }
        }
    }

    fn push_content(&mut self, content: &Content) {
        match content {
            Content::Text(text) => self.push_text(text),
            Content::Video(video) => {
                if let Some(path) = &video.path {
                    self.push_immediate(MessageSegment::video_file(path.clone()));
                    return;
                }

                if !self.options.download_media {
                    if let Some(url) = &video.url {
                        self.push_immediate(MessageSegment::video_url(url));
                    } else if let Some(cover) = &video.cover {
                        self.push_immediate(MessageSegment::image_url(cover));
                    }
                    return;
                }

                // Merge / decrypt / BGM / plain-download / cover-fallback
                // decisions live in the shared resolver. The text path has no
                // card showing the cover, so failures degrade to it.
                let video = video.clone();
                let downloader = self.downloader.clone();
                let platform = self.platform;
                self.push_download(async move {
                    resolve_video_segment(video, downloader, platform, CoverFallback::Degrade).await
                });
            }
            Content::Image(image) => {
                if let Some(path) = &image.path {
                    self.push_immediate(MessageSegment::image_file(path.clone()));
                } else if let Some(url) = &image.url {
                    self.push_image_url(url, self.platform);
                }
                // A caption belongs with its image; the text path has no other
                // way to surface it.
                if let Some(alt) = image.alt.as_deref().filter(|alt| !alt.trim().is_empty()) {
                    self.push_text(alt);
                }
            }
            Content::Audio(audio) => {
                if let Some(path) = &audio.path {
                    self.push_immediate(MessageSegment::audio_file(path.clone()));
                } else if let Some(url) = audio.url.as_deref().filter(|url| !url.trim().is_empty())
                {
                    if self.options.download_media {
                        self.push_audio_download(url.to_string());
                    } else {
                        self.push_immediate(MessageSegment::audio_url(url));
                    }
                }
            }
            Content::Dynamic(dynamic) => {
                if !self.options.download_media {
                    if let Some(url) = &dynamic.video_url {
                        self.push_immediate(MessageSegment::video_url(url));
                    }
                    return;
                }

                if dynamic.should_convert_to_gif() {
                    if let Some(video_url) = &dynamic.video_url {
                        self.push_dynamic_gif_download(video_url.clone(), dynamic.gif_max_duration);
                    }
                } else if let Some(url) = &dynamic.video_url {
                    self.push_dynamic_video_download(url.clone());
                }
            }
            Content::Graphics(graphics) => {
                for image in &graphics.images {
                    if let Some(path) = &image.path {
                        self.push_immediate(MessageSegment::image_file(path.clone()));
                    } else if let Some(url) = &image.url {
                        self.push_image_url(url, self.platform);
                    }
                }
            }
        }
    }

    fn push_text(&mut self, text: &str) {
        for chunk in split_text_segments(text, MAX_TEXT_SEGMENT_CHARS) {
            self.push_immediate(MessageSegment::text(chunk));
        }
    }

    fn push_image_url(&mut self, url: &str, platform: crate::constants::Platform) {
        if !self.options.download_media {
            self.push_immediate(MessageSegment::image_url(url));
            return;
        }

        let url = url.to_string();
        let downloader = self.downloader.clone();
        self.push_download(async move {
            let headers = http::download_headers_for_platform(platform);
            match downloader.download_image(&url, Some(headers)).await {
                Ok(path) => Some(MessageSegment::image_file(path)),
                Err(e) => {
                    tracing::warn!("Failed to download image: {e}");
                    Some(MessageSegment::image_url(&url))
                }
            }
        });
    }

    fn push_audio_download(&mut self, url: String) {
        let downloader = self.downloader.clone();
        let platform = self.platform;
        self.push_download(async move {
            let headers = http::download_headers_for_platform(platform);
            match downloader.download_audio(&url, Some(headers)).await {
                Ok(path) => Some(MessageSegment::audio_file(path)),
                Err(e) => {
                    tracing::warn!("Failed to download audio: {e}");
                    Some(MessageSegment::audio_url(&url))
                }
            }
        });
    }

    fn push_dynamic_gif_download(&mut self, video_url: String, max_duration: Option<u64>) {
        let downloader = self.downloader.clone();
        let platform = self.platform;
        self.push_download(async move {
            let headers = http::download_headers_for_platform(platform);
            match downloader
                .download_as_gif(&video_url, Some(headers), max_duration, None, None)
                .await
            {
                Ok(path) => Some(MessageSegment::image_file(path)),
                Err(e) => {
                    tracing::warn!("Failed to convert dynamic to GIF: {e}");
                    Some(MessageSegment::video_url(&video_url))
                }
            }
        });
    }

    fn push_dynamic_video_download(&mut self, url: String) {
        let downloader = self.downloader.clone();
        let platform = self.platform;
        self.push_download(async move {
            let headers = http::download_headers_for_platform(platform);
            match downloader.download_video(&url, Some(headers)).await {
                Ok(path) => Some(MessageSegment::video_file(path)),
                Err(e) => {
                    tracing::warn!("Failed to download dynamic video: {e}");
                    Some(MessageSegment::video_url(&url))
                }
            }
        });
    }

    fn push_immediate(&mut self, segment: MessageSegment) {
        self.out.push((self.order, segment));
        self.order += 1;
    }

    fn push_download(
        &mut self,
        future: impl std::future::Future<Output = Option<MessageSegment>> + Send + 'static,
    ) {
        let order = self.order;
        self.order += 1;
        self.downloads
            .push(async move { (order, future.await) }.boxed());
    }

    async fn resolve(mut self) -> Vec<MessageSegment> {
        let concurrency = self.options.download_concurrency.max(1);
        let results: Vec<(usize, Option<MessageSegment>)> = stream::iter(self.downloads)
            .buffer_unordered(concurrency)
            .collect()
            .await;

        self.out.extend(
            results
                .into_iter()
                .filter_map(|(order, segment)| segment.map(|segment| (order, segment))),
        );
        self.out.sort_by_key(|(order, _)| *order);
        self.out.into_iter().map(|(_, segment)| segment).collect()
    }
}
