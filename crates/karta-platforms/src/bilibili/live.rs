//! Bilibili live room request flow and [`ParseResult`] construction.

use karta_core::Platform;
use karta_core::constants::DEFAULT_USER_AGENT;
use karta_core::data::{Author, Content, ImageContent, ParseResult, VideoContent};
use karta_core::error::{ParserError, Result};

use super::BilibiliParser;
use super::api::{ApiResponse, LiveAnchorInfo, LivePlayUrl, LiveRoomInfo};

const API_LIVE_INFO: &str = "https://api.live.bilibili.com/room/v1/Room/get_info";
const API_LIVE_ANCHOR_INFO: &str =
    "https://api.live.bilibili.com/live_user/v1/UserInfo/get_anchor_in_room";
/// FLV pull URLs for a live room
const API_LIVE_PLAY_URL: &str = "https://api.live.bilibili.com/room/v1/Room/playUrl";

impl BilibiliParser {
    /// Parse live room
    pub(super) async fn fetch_live(&self, room_id: &str) -> Result<ParseResult> {
        let client = self.downloader.client();

        let params = [("room_id", room_id)];

        let response: ApiResponse<LiveRoomInfo> = client
            .get(API_LIVE_INFO)
            .headers(self.get_headers().await)
            .query(&params)
            .send()
            .await?
            .json()
            .await?;

        if response.code != 0 {
            return Err(ParserError::Api {
                code: response.code,
                message: response.message.unwrap_or_default(),
            });
        }

        let room = response
            .data
            .ok_or_else(|| ParserError::ContentNotFound("Live room data not found".to_string()))?;

        // Fetch anchor info
        let anchor_response: ApiResponse<LiveAnchorInfo> = client
            .get(API_LIVE_ANCHOR_INFO)
            .headers(self.get_headers().await)
            .query(&params)
            .send()
            .await?
            .json()
            .await?;

        let mut result = build_live_result(&room, anchor_response.data.as_ref());

        // Only a running broadcast has a pullable stream.
        if self.live_record_seconds > 0 && room.live_status == 1 {
            match self.record_live_clip(room.room_id).await {
                Ok(path) => result.add_content(Content::Video(
                    VideoContent::new()
                        .with_path(path)
                        .with_duration(self.live_record_seconds),
                )),
                Err(e) => tracing::warn!(
                    room_id = room.room_id,
                    error = %e,
                    "Failed to record a live clip; sending room info only"
                ),
            }
        }

        Ok(result)
    }

    /// Record the configured number of seconds from a live room's stream.
    async fn record_live_clip(&self, room_id: i64) -> Result<std::path::PathBuf> {
        let client = self.downloader.client();

        let room = room_id.to_string();
        let params = [
            ("cid", room.as_str()),
            ("qn", "0"),
            ("platform", "web"),
            ("https_url_req", "1"),
        ];

        let response: ApiResponse<LivePlayUrl> = client
            .get(API_LIVE_PLAY_URL)
            .headers(self.get_headers().await)
            .query(&params)
            .send()
            .await?
            .json()
            .await?;

        if response.code != 0 {
            return Err(ParserError::Api {
                code: response.code,
                message: response.message.unwrap_or_default(),
            });
        }

        let stream_url = response
            .data
            .and_then(|play| play.durl.into_iter().next())
            .map(|durl| durl.url)
            .filter(|url| !url.is_empty())
            .ok_or_else(|| ParserError::ContentNotFound("Live stream URL not found".to_string()))?;

        self.downloader
            .download_stream_segment(
                &stream_url,
                self.live_record_seconds,
                Some(&[
                    ("Referer", "https://live.bilibili.com/"),
                    ("User-Agent", DEFAULT_USER_AGENT),
                ]),
            )
            .await
            .map_err(ParserError::Download)
    }
}

fn build_live_result(room: &LiveRoomInfo, anchor: Option<&LiveAnchorInfo>) -> ParseResult {
    let mut result = ParseResult::new(Platform::Bilibili)
        .with_title(format!("直播 - {}", room.title))
        .with_url(format!("https://live.bilibili.com/{}", room.room_id));

    if !room.description.is_empty() {
        result = result.with_text(&room.description);
    }

    if let Some(anchor) = anchor {
        result = result.with_author(
            Author::new(&anchor.info.uname)
                .with_uid(anchor.info.uid.to_string())
                .with_avatar(&anchor.info.face)
                .with_url(format!("https://space.bilibili.com/{}", anchor.info.uid)),
        );
    }

    result.add_extra("status", live_status_label(room.live_status));

    if let (Some(area), Some(parent_area)) = (&room.area_name, &room.parent_area_name) {
        result.add_extra("area", format!("{parent_area} / {area}"));
    }

    if let Some(tags) = room.tags.as_deref().filter(|tags| !tags.is_empty()) {
        result.add_extra("tags", tags);
    }

    result.add_content(Content::Image(
        ImageContent::new().with_url(preferred_cover(room)),
    ));

    result
}

fn live_status_label(status: i32) -> &'static str {
    match status {
        1 => "直播中",
        2 => "轮播中",
        _ => "未开播",
    }
}

fn preferred_cover(room: &LiveRoomInfo) -> &str {
    if !room.keyframe.is_empty() {
        &room.keyframe
    } else {
        &room.user_cover
    }
}
