//! Bilibili Bangumi request flow, episode selection, and [`ParseResult`] construction.

use karta_core::Platform;
use karta_core::data::{Content, ImageContent, ParseResult};
use karta_core::error::{ParserError, Result};

use super::BilibiliParser;
use super::api::{BangumiEpisode, BangumiResponse, BangumiSeason, StreamInfo};
use super::stream::{StreamOptions, build_video_content_from_stream};

/// Bangumi season info (PGC)
const API_BANGUMI_INFO: &str = "https://api.bilibili.com/pgc/view/web/season";
/// Bangumi stream URL (PGC)
const API_BANGUMI_STREAM: &str = "https://api.bilibili.com/pgc/player/web/playurl";

impl BilibiliParser {
    /// Parse Bangumi/PGC episode
    pub(super) async fn fetch_bangumi(&self, ep_id: &str) -> Result<ParseResult> {
        let client = self.downloader.client();

        // 1. Fetch season info to get episode metadata
        let params = [("ep_id", ep_id)];
        let season_resp: BangumiResponse<BangumiSeason> = client
            .get(API_BANGUMI_INFO)
            .headers(self.get_headers().await)
            .query(&params)
            .send()
            .await?
            .json()
            .await?;

        if season_resp.code != 0 {
            return Err(ParserError::Api {
                code: season_resp.code,
                message: season_resp.message.unwrap_or_default(),
            });
        }

        let season = season_resp
            .result
            .ok_or_else(|| ParserError::ContentNotFound("Bangumi season not found".to_string()))?;

        let episode = select_episode(&season, ep_id)?;

        if let Some(duration) = episode.duration_secs()
            && self.downloader.check_duration(duration).is_err()
        {
            tracing::warn!(
                ep_id,
                duration,
                limit = self.downloader.max_duration(),
                "Bilibili bangumi episode exceeds duration limit; using cover only"
            );
            return Ok(build_bangumi_result(
                &season,
                &episode,
                ep_id,
                None,
                self.stream_options(),
            ));
        }

        // 3. Fetch stream URL via PGC API
        let qn = self.video_quality.qn().to_string();
        let cid = episode.cid.to_string();
        let stream_params = [
            ("ep_id", ep_id),
            ("cid", cid.as_str()),
            ("qn", qn.as_str()),
            ("fnval", "4048"),
            ("fourk", "1"),
        ];

        let mut stream_resp: serde_json::Value = client
            .get(API_BANGUMI_STREAM)
            .headers(self.get_headers().await)
            .query(&stream_params)
            .send()
            .await?
            .json()
            .await?;

        // PGC stream response uses "result" key
        let stream_data = match stream_resp.get_mut("result") {
            Some(result) => result.take(),
            None => stream_resp,
        };

        // Try to parse DASH streams from the result
        let stream = serde_json::from_value::<StreamInfo>(stream_data).ok();
        Ok(build_bangumi_result(
            &season,
            &episode,
            ep_id,
            stream.as_ref(),
            self.stream_options(),
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SelectedEpisode {
    cid: i64,
    cover: String,
    title: String,
    duration_ms: Option<i64>,
    pub_time: Option<i64>,
}

impl SelectedEpisode {
    fn duration_secs(&self) -> Option<u64> {
        self.duration_ms
            .filter(|duration| *duration >= 0)
            .map(|duration| duration as u64 / 1000)
    }
}

fn select_episode(season: &BangumiSeason, ep_id: &str) -> Result<SelectedEpisode> {
    let ep_id = ep_id
        .parse::<i64>()
        .map_err(|_| ParserError::Parse("Invalid ep_id".to_string()))?;

    find_episode(season, ep_id)
        .or_else(|| first_episode(season))
        .map(SelectedEpisode::from)
        .ok_or_else(|| ParserError::ContentNotFound("No episodes found in season".to_string()))
}

fn build_bangumi_result(
    season: &BangumiSeason,
    episode: &SelectedEpisode,
    ep_id: &str,
    stream: Option<&StreamInfo>,
    options: StreamOptions<'_>,
) -> ParseResult {
    let mut result = ParseResult::new(Platform::Bilibili)
        .with_title(format!("{} - {}", season.title, episode.title))
        .with_url(format!("https://www.bilibili.com/bangumi/play/ep{ep_id}"));

    if let Some(evaluate) = &season.evaluate {
        result = result.with_text(evaluate);
    }

    if let Some(ts) = episode.pub_time {
        result = result.with_timestamp(ts);
    }

    if let Some(stream) = stream {
        let duration_secs = episode.duration_secs();
        result.add_content(Content::Video(build_video_content_from_stream(
            stream,
            &episode.cover,
            duration_secs,
            options,
        )));
    } else {
        result.add_content(Content::Image(ImageContent::new().with_url(&episode.cover)));
    }

    result
}

fn find_episode(season: &BangumiSeason, ep_id: i64) -> Option<&BangumiEpisode> {
    season
        .episodes
        .as_deref()
        .and_then(|episodes| {
            episodes
                .iter()
                .find(|episode| matches_episode(episode, ep_id))
        })
        .or_else(|| {
            season
                .section
                .as_deref()
                .into_iter()
                .flat_map(|sections| sections.iter())
                .filter_map(|section| section.episodes.as_deref())
                .flat_map(|episodes| episodes.iter())
                .find(|episode| matches_episode(episode, ep_id))
        })
        .or_else(|| {
            season
                .main_section
                .as_ref()
                .and_then(|section| section.episodes.as_deref())
                .and_then(|episodes| {
                    episodes
                        .iter()
                        .find(|episode| matches_episode(episode, ep_id))
                })
        })
}

fn first_episode(season: &BangumiSeason) -> Option<&BangumiEpisode> {
    season
        .episodes
        .as_deref()
        .and_then(|episodes| episodes.first())
        .or_else(|| {
            season
                .section
                .as_deref()
                .into_iter()
                .flat_map(|sections| sections.iter())
                .filter_map(|section| section.episodes.as_deref())
                .find_map(|episodes| episodes.first())
        })
        .or_else(|| {
            season
                .main_section
                .as_ref()
                .and_then(|section| section.episodes.as_deref())
                .and_then(|episodes| episodes.first())
        })
}

fn matches_episode(episode: &BangumiEpisode, ep_id: i64) -> bool {
    episode.id == ep_id || episode.ep_id == Some(ep_id)
}

impl From<&BangumiEpisode> for SelectedEpisode {
    fn from(episode: &BangumiEpisode) -> Self {
        Self {
            cid: episode.cid,
            cover: episode.cover.clone(),
            title: episode.display_title().to_string(),
            duration_ms: episode.duration,
            pub_time: episode.pub_time,
        }
    }
}
