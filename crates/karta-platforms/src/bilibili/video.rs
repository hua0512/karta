//! Bilibili video, stream, AI-summary, and audio-only request flow.

use karta_core::Platform;
use karta_core::data::{AudioContent, Author, Content, ImageContent, ParseResult, VideoContent};
use karta_core::error::{ParserError, Result};

use super::BilibiliParser;
use super::api::{AiConclusionData, ApiResponse, StreamInfo, VideoInfo};
use super::cdn::ordered_cdn_urls;
use super::format::format_stats;
use super::stream::{build_video_content_from_stream, stream_download_headers};

const API_VIDEO_INFO: &str = "https://api.bilibili.com/x/web-interface/view";
const API_VIDEO_STREAM: &str = "https://api.bilibili.com/x/player/playurl";
const API_AI_CONCLUSION: &str = "https://api.bilibili.com/x/web-interface/view/conclusion/get";

#[derive(Debug, Clone, PartialEq, Eq)]
struct SelectedVideoPage {
    title: String,
    cid: i64,
    cover: String,
    url: String,
    duration: u64,
    timestamp: i64,
}

impl BilibiliParser {
    /// Fetch video info from API with optional page number.
    pub(super) async fn fetch_video_info(
        &self,
        bvid: &str,
        aid: Option<&str>,
        page_num: u32,
    ) -> Result<ParseResult> {
        let params;
        if !bvid.is_empty() {
            params = [("bvid", bvid)];
        } else if let Some(aid) = aid {
            params = [("aid", aid)];
        } else {
            return Err(ParserError::Parse("Missing bvid/aid".to_string()));
        }

        let response: ApiResponse<VideoInfo> = self.get_wbi_signed(API_VIDEO_INFO, &params).await?;

        if response.code != 0 {
            return Err(ParserError::Api {
                code: response.code,
                message: response.message.unwrap_or_default(),
            });
        }

        let video = response
            .data
            .ok_or_else(|| ParserError::ContentNotFound("Video data not found".to_string()))?;
        let selected = select_video_page(&video, page_num);

        let mut result = ParseResult::new(Platform::Bilibili)
            .with_title(&selected.title)
            .with_url(&selected.url)
            .with_timestamp(selected.timestamp)
            .with_author(
                Author::new(&video.owner.name)
                    .with_uid(video.owner.mid.to_string())
                    .with_avatar(&video.owner.face)
                    .with_url(format!("https://space.bilibili.com/{}", video.owner.mid)),
            );

        if let Some(desc) = &video.desc
            && !desc.is_empty()
            && desc != "-"
        {
            result = result.with_text(desc);
        }

        result.add_extra("stats", format_stats(&video.stat));

        if self.cookie_string().await.is_some() && selected.cid > 0 {
            match self.fetch_ai_conclusion(&video.bvid, selected.cid).await {
                Ok(ai_summary) => result.add_extra("ai_summary", ai_summary),
                Err(e) => tracing::warn!(
                    bvid = %video.bvid,
                    cid = selected.cid,
                    error = %e,
                    "Bilibili AI summary fetch failed; continuing without it"
                ),
            }
        }

        if selected.cid > 0 && self.downloader.check_duration(selected.duration).is_ok() {
            match self
                .fetch_video_stream(
                    &video.bvid,
                    selected.cid,
                    &selected.cover,
                    selected.duration,
                )
                .await
            {
                Ok(video_content) => result.add_content(Content::Video(video_content)),
                Err(e) => {
                    tracing::warn!(
                        bvid = %video.bvid,
                        cid = selected.cid,
                        error = %e,
                        "Bilibili stream fetch failed; falling back to cover image"
                    );
                    result.add_content(Content::Image(
                        ImageContent::new().with_url(&selected.cover),
                    ));
                }
            }
        } else {
            if selected.cid > 0 {
                tracing::warn!(
                    bvid = %video.bvid,
                    duration = selected.duration,
                    limit = self.downloader.max_duration(),
                    "Bilibili video exceeds duration limit; using cover only"
                );
            }
            result.add_content(Content::Image(
                ImageContent::new().with_url(&selected.cover),
            ));
        }

        Ok(result)
    }

    /// Fetch video stream URLs with quality and codec preferences.
    async fn fetch_video_stream(
        &self,
        bvid: &str,
        cid: i64,
        cover: &str,
        duration: u64,
    ) -> Result<VideoContent> {
        let qn = self.video_quality.qn().to_string();
        let cid = cid.to_string();
        let params = [
            ("bvid", bvid),
            ("cid", cid.as_str()),
            ("qn", qn.as_str()),
            ("fnval", "4048"),
            ("fourk", "1"),
        ];

        let response: ApiResponse<StreamInfo> =
            self.get_wbi_signed(API_VIDEO_STREAM, &params).await?;

        if response.code != 0 {
            return Err(ParserError::Api {
                code: response.code,
                message: response.message.unwrap_or_default(),
            });
        }

        let stream = response
            .data
            .ok_or_else(|| ParserError::ContentNotFound("Stream data not found".to_string()))?;

        Ok(build_video_content_from_stream(
            &stream,
            cover,
            Some(duration),
            self.stream_options(),
        ))
    }

    /// Fetch AI conclusion/summary for a video.
    async fn fetch_ai_conclusion(&self, bvid: &str, cid: i64) -> Result<String> {
        let cid = cid.to_string();
        let params = [("bvid", bvid), ("cid", cid.as_str()), ("up_mid", "0")];

        let response: ApiResponse<AiConclusionData> =
            self.get_wbi_signed(API_AI_CONCLUSION, &params).await?;

        if response.code != 0 {
            return Err(ParserError::Api {
                code: response.code,
                message: response.message.unwrap_or_default(),
            });
        }

        let data = response.data.ok_or_else(|| {
            ParserError::ContentNotFound("AI conclusion data not found".to_string())
        })?;

        Ok(data.summary())
    }

    /// Parse BV audio only (for `bm` command).
    ///
    /// Fetches video info to get the DASH audio stream URL, then returns it as
    /// an [`AudioContent`].
    pub(super) async fn parse_audio(&self, bvid: &str, page_num: u32) -> Result<ParseResult> {
        let params = [("bvid", bvid)];

        let response: ApiResponse<VideoInfo> = self.get_wbi_signed(API_VIDEO_INFO, &params).await?;

        if response.code != 0 {
            return Err(ParserError::Api {
                code: response.code,
                message: response.message.unwrap_or_default(),
            });
        }

        let video = response
            .data
            .ok_or_else(|| ParserError::ContentNotFound("Video data not found".to_string()))?;
        let selected = select_video_page(&video, page_num);
        if selected.cid <= 0 {
            return Err(ParserError::ContentNotFound("No pages found".to_string()));
        }
        self.downloader.check_duration(selected.duration)?;

        let qn = self.video_quality.qn().to_string();
        let cid_str = selected.cid.to_string();
        let stream_params = [
            ("bvid", bvid),
            ("cid", cid_str.as_str()),
            ("qn", qn.as_str()),
            ("fnval", "4048"),
            ("fourk", "1"),
        ];

        let stream_response: ApiResponse<StreamInfo> = self
            .get_wbi_signed(API_VIDEO_STREAM, &stream_params)
            .await?;

        if stream_response.code != 0 {
            return Err(ParserError::Api {
                code: stream_response.code,
                message: stream_response.message.unwrap_or_default(),
            });
        }

        let stream = stream_response
            .data
            .ok_or_else(|| ParserError::ContentNotFound("Stream data not found".to_string()))?;
        let audio_stream = stream
            .dash
            .as_ref()
            .and_then(|dash| dash.audio.iter().max_by_key(|audio| audio.bandwidth))
            .ok_or_else(|| ParserError::ContentNotFound("Audio stream not found".to_string()))?;
        let audio_urls = ordered_cdn_urls(
            &audio_stream.base_url,
            &audio_stream.backup_url,
            self.cdn_mode,
        );

        let headers = stream_download_headers();
        let audio_path = self
            .downloader
            .download_audio_candidates(&audio_urls, Some(headers))
            .await
            .map_err(ParserError::Download)?;

        let title = selected.title;

        let mut result = ParseResult::new(Platform::Bilibili)
            .with_title(&title)
            .with_author(Author::new(&video.owner.name).with_avatar(&video.owner.face));

        let audio = AudioContent::new().with_path(audio_path).with_title(&title);
        result.add_content(Content::Audio(audio));

        Ok(result)
    }
}

fn select_video_page(video: &VideoInfo, page_num: u32) -> SelectedVideoPage {
    let requested_index = page_num.saturating_sub(1) as usize;
    let selected_page = video.pages.as_deref().and_then(|pages| {
        if pages.is_empty() {
            None
        } else {
            pages.get(requested_index % pages.len())
        }
    });
    let selected_page_num = selected_page
        .map(|page| page.page.max(1) as u32)
        .unwrap_or(1);
    let multipart = video.pages.as_ref().is_some_and(|pages| pages.len() > 1);

    let title = selected_page
        .filter(|_| multipart)
        .map(|page| format!("{} | 分集 - {}", video.title, page.part))
        .unwrap_or_else(|| video.title.clone());
    let cid = selected_page.map_or(0, |page| page.cid);
    let duration = selected_page
        .map(|page| page.duration)
        .filter(|duration| *duration > 0)
        .unwrap_or(video.duration)
        .max(0) as u64;
    let timestamp = selected_page
        .and_then(|page| page.ctime)
        .unwrap_or(video.pub_date);
    let cover = selected_page
        .and_then(|page| page.first_frame.as_deref())
        .filter(|cover| !cover.is_empty())
        .unwrap_or(&video.cover)
        .to_string();

    let url = if selected_page_num > 1 {
        format!(
            "https://www.bilibili.com/video/{}?p={}",
            video.bvid, selected_page_num
        )
    } else {
        format!("https://www.bilibili.com/video/{}", video.bvid)
    };

    SelectedVideoPage {
        title,
        cid,
        cover,
        url,
        duration,
        timestamp,
    }
}
