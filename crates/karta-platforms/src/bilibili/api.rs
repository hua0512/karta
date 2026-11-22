//! Bilibili API response DTOs.

#![allow(dead_code)]

use std::collections::HashMap;

use serde::Deserialize;
use serde::de::Error as _;
use serde_json::Value;

/// Bilibili API response wrapper.
#[derive(Debug, Deserialize)]
pub(super) struct ApiResponse<T> {
    pub(super) code: i64,
    pub(super) message: Option<String>,
    pub(super) data: Option<T>,
}

/// Video info from API.
#[derive(Debug, Deserialize)]
pub struct VideoInfo {
    pub bvid: String,
    pub aid: i64,
    pub title: String,
    pub desc: Option<String>,
    #[serde(rename = "pic")]
    pub cover: String,
    #[serde(rename = "pubdate")]
    pub pub_date: i64,
    pub duration: i64,
    pub owner: VideoOwner,
    pub stat: VideoStat,
    pub pages: Option<Vec<VideoPage>>,
}

#[derive(Debug, Deserialize)]
pub struct VideoOwner {
    pub mid: i64,
    pub name: String,
    pub face: String,
}

#[derive(Debug, Deserialize)]
pub struct VideoStat {
    pub view: i64,
    pub danmaku: i64,
    pub reply: i64,
    pub favorite: i64,
    pub coin: i64,
    pub share: i64,
    pub like: i64,
}

#[derive(Debug, Deserialize)]
pub struct VideoPage {
    pub cid: i64,
    pub page: i32,
    pub part: String,
    pub duration: i64,
    #[serde(default)]
    pub ctime: Option<i64>,
    #[serde(default)]
    pub first_frame: Option<String>,
}

/// Video stream info.
#[derive(Debug, Deserialize)]
pub struct StreamInfo {
    pub dash: Option<DashInfo>,
    pub durl: Option<Vec<DurlInfo>>,
}

#[derive(Debug, Deserialize)]
pub struct DashInfo {
    #[serde(default, deserialize_with = "deserialize_vec_or_default")]
    pub video: Vec<DashStream>,
    #[serde(default, deserialize_with = "deserialize_vec_or_default")]
    pub audio: Vec<DashStream>,
    #[serde(default)]
    pub dolby: Option<DashDolby>,
    #[serde(default)]
    pub flac: Option<DashFlac>,
}

#[derive(Debug, Deserialize)]
pub struct DashDolby {
    #[serde(default, deserialize_with = "deserialize_vec_or_default")]
    pub audio: Vec<DashStream>,
}

#[derive(Debug, Deserialize)]
pub struct DashFlac {
    pub audio: Option<DashStream>,
}

#[derive(Debug)]
pub struct DashStream {
    pub id: i32,
    pub base_url: String,
    pub backup_url: Vec<String>,
    pub bandwidth: i64,
    pub codecs: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

impl<'de> Deserialize<'de> for DashStream {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawDashStream {
            id: i32,
            bandwidth: i64,
            #[serde(default)]
            codecs: Option<String>,
            #[serde(default)]
            width: Option<u32>,
            #[serde(default)]
            height: Option<u32>,
            #[serde(flatten)]
            extra: HashMap<String, Value>,
        }

        let mut raw = RawDashStream::deserialize(deserializer)?;
        // Both spellings have appeared in Bilibili payloads, and some proxy
        // layers emit both at once. Prefer the current snake_case spelling
        // deterministically instead of using `alias`, which rejects duplicates.
        let base_url = raw
            .extra
            .remove("base_url")
            .or_else(|| raw.extra.remove("baseUrl"))
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
            .ok_or_else(|| D::Error::missing_field("base_url/baseUrl"))?;
        let backup_url = raw
            .extra
            .remove("backup_url")
            .or_else(|| raw.extra.remove("backupUrl"))
            .filter(|value| !value.is_null())
            .map(serde_json::from_value)
            .transpose()
            .map_err(D::Error::custom)?
            .unwrap_or_default();

        Ok(Self {
            id: raw.id,
            base_url,
            backup_url,
            bandwidth: raw.bandwidth,
            codecs: raw.codecs,
            width: raw.width,
            height: raw.height,
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct DurlInfo {
    pub url: String,
    #[serde(default, deserialize_with = "deserialize_vec_or_default")]
    pub backup_url: Vec<String>,
    pub size: i64,
}

fn deserialize_vec_or_default<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(deserializer)?.unwrap_or_default())
}

/// AI conclusion response.
#[derive(Debug, Deserialize)]
pub struct AiConclusionData {
    pub model_result: Option<AiModelResult>,
}

#[derive(Debug, Deserialize)]
pub struct AiModelResult {
    pub summary: Option<String>,
}

impl AiConclusionData {
    /// Get the AI summary text.
    pub fn summary(&self) -> String {
        self.model_result
            .as_ref()
            .and_then(|r| r.summary.as_ref())
            .filter(|s| !s.is_empty())
            .map(|s| format!("AI总结: {}", s))
            .unwrap_or_else(|| "该视频暂不支持AI总结".to_string())
    }
}

/// Dynamic info.
#[derive(Debug, Deserialize)]
pub struct DynamicDetail {
    pub item: DynamicItem,
}

#[derive(Debug, Deserialize)]
pub struct DynamicItem {
    #[serde(rename = "id_str")]
    pub id: String,
    pub modules: DynamicModules,
    #[serde(rename = "type")]
    pub dynamic_type: String,
    pub orig: Option<Box<DynamicItem>>,
}

#[derive(Debug, Deserialize)]
pub struct DynamicModules {
    pub module_author: ModuleAuthor,
    pub module_dynamic: ModuleDynamic,
}

#[derive(Debug, Deserialize)]
pub struct ModuleAuthor {
    pub name: String,
    pub mid: i64,
    pub face: String,
    #[serde(rename = "pub_ts", deserialize_with = "deserialize_i64_or_string")]
    pub pub_ts: i64,
}

/// An `i64` that Bilibili's API may serialize as either a JSON number or a
/// numeric string.
#[derive(Deserialize)]
#[serde(untagged)]
enum I64OrString {
    Int(i64),
    Str(String),
}

/// Deserialize an `i64` that Bilibili's API may serialize as either a JSON
/// number or a numeric string.
///
/// Bilibili already flipped `pub_ts` to a string on the opus detail endpoint;
/// tolerate the same flip on the dynamic detail endpoint instead of failing
/// every dynamic/opus parse at deserialization.
fn deserialize_i64_or_string<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match I64OrString::deserialize(deserializer)? {
        I64OrString::Int(value) => Ok(value),
        I64OrString::Str(value) => value.parse::<i64>().map_err(serde::de::Error::custom),
    }
}

/// Same as [`deserialize_i64_or_string`], but tolerant of a missing or
/// unparseable value: an opus without a timestamp is still worth rendering.
fn deserialize_opt_i64_or_string<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(match Option::<I64OrString>::deserialize(deserializer)? {
        Some(I64OrString::Int(value)) => Some(value),
        Some(I64OrString::Str(value)) => value.parse::<i64>().ok(),
        None => None,
    })
}

#[derive(Debug, Deserialize)]
pub struct ModuleDynamic {
    pub desc: Option<DynamicDesc>,
    pub major: Option<DynamicMajor>,
}

#[derive(Debug, Deserialize)]
pub struct DynamicDesc {
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct DynamicMajor {
    #[serde(rename = "type")]
    pub major_type: String,
    pub archive: Option<DynamicArchive>,
    pub draw: Option<DynamicDraw>,
}

#[derive(Debug, Deserialize)]
pub struct DynamicArchive {
    pub bvid: String,
    pub title: String,
    pub cover: String,
    pub desc: String,
    pub duration_text: String,
}

#[derive(Debug, Deserialize)]
pub struct DynamicDraw {
    pub items: Vec<DrawItem>,
}

#[derive(Debug, Deserialize)]
pub struct DrawItem {
    pub src: String,
    pub width: u32,
    pub height: u32,
}

/// Opus (图文动态) detail.
///
/// The opus endpoint returns the body as an ordered list of paragraphs, which
/// is the only place the original text/image interleaving survives — the
/// dynamic endpoint flattens it into a summary plus a picture array.
#[derive(Debug, Deserialize)]
pub struct OpusDetail {
    pub item: OpusItem,
}

#[derive(Debug, Deserialize)]
pub struct OpusItem {
    #[serde(default)]
    pub basic: Option<OpusBasic>,
    #[serde(default)]
    pub modules: Vec<OpusModule>,
}

#[derive(Debug, Deserialize)]
pub struct OpusBasic {
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OpusModule {
    pub module_type: String,
    #[serde(default)]
    pub module_author: Option<OpusAuthor>,
    #[serde(default)]
    pub module_content: Option<OpusContent>,
}

#[derive(Debug, Deserialize)]
pub struct OpusAuthor {
    pub name: String,
    #[serde(default)]
    pub face: String,
    #[serde(default)]
    pub mid: i64,
    #[serde(default, deserialize_with = "deserialize_opt_i64_or_string")]
    pub pub_ts: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct OpusContent {
    #[serde(default)]
    pub paragraphs: Vec<OpusParagraph>,
}

#[derive(Debug, Deserialize)]
pub struct OpusParagraph {
    #[serde(default)]
    pub text: Option<OpusText>,
    #[serde(default)]
    pub pic: Option<OpusPic>,
}

#[derive(Debug, Deserialize)]
pub struct OpusText {
    #[serde(default)]
    pub nodes: Vec<OpusTextNode>,
}

#[derive(Debug, Deserialize)]
pub struct OpusTextNode {
    #[serde(rename = "type", default)]
    pub node_type: Option<String>,
    #[serde(default)]
    pub word: Option<OpusWord>,
}

#[derive(Debug, Deserialize)]
pub struct OpusWord {
    #[serde(default)]
    pub words: String,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OpusPic {
    #[serde(default)]
    pub pics: Vec<OpusPicture>,
}

#[derive(Debug, Deserialize)]
pub struct OpusPicture {
    pub url: String,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
}

/// Live room info.
#[derive(Debug, Deserialize)]
pub struct LiveRoomInfo {
    pub room_id: i64,
    pub title: String,
    pub description: String,
    pub live_status: i32,
    pub user_cover: String,
    pub keyframe: String,
    pub uid: i64,
    pub area_name: Option<String>,
    pub parent_area_name: Option<String>,
    pub tags: Option<String>,
}

/// Live room pull URLs.
#[derive(Debug, Deserialize)]
pub struct LivePlayUrl {
    #[serde(default)]
    pub durl: Vec<LivePlayDurl>,
}

#[derive(Debug, Deserialize)]
pub struct LivePlayDurl {
    pub url: String,
}

/// Live anchor info.
#[derive(Debug, Deserialize)]
pub struct LiveAnchorInfo {
    pub info: AnchorBaseInfo,
}

#[derive(Debug, Deserialize)]
pub struct AnchorBaseInfo {
    pub uid: i64,
    pub uname: String,
    pub face: String,
}

/// Article info from API.
#[derive(Debug, Deserialize)]
pub struct ArticleInfo {
    #[serde(rename = "type")]
    pub article_type: Option<String>,
    pub meta: Option<ArticleMeta>,
    pub children: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
pub struct ArticleMeta {
    pub id: i64,
    pub title: String,
    pub summary: String,
    pub publish_time: i64,
    pub author: ArticleAuthor,
    pub stats: ArticleStats,
    pub words: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ArticleAuthor {
    pub mid: i64,
    pub name: String,
    pub face: String,
}

#[derive(Debug, Deserialize)]
pub struct ArticleStats {
    pub view: i64,
    pub favorite: i64,
    pub like: i64,
    pub reply: i64,
    pub share: i64,
    pub coin: i64,
}

/// Favorites list info.
#[derive(Debug, Deserialize)]
pub struct FavoritesInfo {
    pub info: FavInfo,
    pub medias: Option<Vec<FavMedia>>,
}

#[derive(Debug, Deserialize)]
pub struct FavInfo {
    pub title: String,
    pub cover: String,
    pub upper: FavUpper,
    pub ctime: i64,
    pub media_count: i64,
    pub intro: String,
}

#[derive(Debug, Deserialize)]
pub struct FavUpper {
    pub mid: i64,
    pub name: String,
    pub face: String,
}

#[derive(Debug, Deserialize)]
pub struct FavMedia {
    pub title: String,
    pub cover: String,
    pub intro: String,
    pub bvid: Option<String>,
    pub id: i64,
}

/// Bangumi API response wrapper.
///
/// PGC endpoints use `result` instead of `data`.
#[derive(Debug, Deserialize)]
pub struct BangumiResponse<T> {
    pub code: i64,
    pub message: Option<String>,
    pub result: Option<T>,
}

/// Bangumi season info.
#[derive(Debug, Deserialize)]
pub struct BangumiSeason {
    pub title: String,
    pub cover: String,
    pub evaluate: Option<String>,
    pub episodes: Option<Vec<BangumiEpisode>>,
    pub section: Option<Vec<BangumiSection>>,
    /// Alternate path: main_section.episodes.
    pub main_section: Option<BangumiSection>,
}

#[derive(Debug, Deserialize)]
pub struct BangumiSection {
    pub episodes: Option<Vec<BangumiEpisode>>,
}

#[derive(Debug, Deserialize)]
pub struct BangumiEpisode {
    pub id: i64,
    #[serde(default)]
    pub ep_id: Option<i64>,
    pub bvid: String,
    pub cid: i64,
    /// Episode title, sometimes called `long_title`.
    #[serde(default)]
    pub share_copy: Option<String>,
    #[serde(default)]
    pub long_title: Option<String>,
    pub cover: String,
    pub duration: Option<i64>,
    pub pub_time: Option<i64>,
}

impl BangumiEpisode {
    /// Get the best available title for this episode.
    pub fn display_title(&self) -> &str {
        self.share_copy
            .as_deref()
            .or(self.long_title.as_deref())
            .unwrap_or("未知")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_stream_with_null_backup_url() {
        let stream: DashStream = serde_json::from_str(
            r#"{
                "id": 80,
                "base_url": "https://example.com/video.m4s",
                "backup_url": null,
                "bandwidth": 1000
            }"#,
        )
        .unwrap();

        assert_eq!(stream.base_url, "https://example.com/video.m4s");
        assert!(stream.backup_url.is_empty());
    }
}
