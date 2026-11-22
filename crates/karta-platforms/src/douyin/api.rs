#![allow(dead_code)]

use serde::Deserialize;

// --- Official API response ---

#[derive(Debug, Deserialize)]
pub(super) struct ApiResponse {
    pub(super) status_code: Option<i64>,
    pub(super) status_msg: Option<String>,
    pub(super) filter_list: Option<Vec<super::filters::ContentFilter>>,
    pub(super) aweme_detail: Option<AwemeDetail>,
}

#[derive(Debug, Deserialize)]
pub(super) struct AwemeDetail {
    pub(super) aweme_id: Option<String>,
    pub(super) aweme_type: Option<u32>,
    pub(super) desc: Option<String>,
    pub(super) item_title: Option<String>,
    pub(super) caption: Option<String>,
    pub(super) create_time: Option<i64>,
    pub(super) share_url: Option<String>,
    pub(super) author: Option<ApiAuthor>,
    pub(super) video: Option<ApiVideo>,
    pub(super) images: Option<Vec<ApiImage>>,
    pub(super) music: Option<ApiMusic>,
    pub(super) statistics: Option<ApiStatistics>,
    /// Present only for long articles (长文章); see `article.rs`.
    pub(super) article_info: Option<ApiArticleInfo>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ApiStatistics {
    pub(super) digg_count: Option<i64>,
    pub(super) comment_count: Option<i64>,
    pub(super) collect_count: Option<i64>,
    pub(super) share_count: Option<i64>,
}

/// Long-article payload. `article_content` and `fe_data` are JSON-encoded
/// strings (decoded in `article.rs`); the aweme's `video` block is a
/// zero-duration placeholder when this is present.
#[derive(Debug, Deserialize)]
pub(super) struct ApiArticleInfo {
    pub(super) article_id: Option<String>,
    pub(super) article_title: Option<String>,
    pub(super) article_content: Option<String>,
    pub(super) fe_data: Option<String>,
    /// Estimated reading time in minutes.
    pub(super) read_time: Option<u64>,
    pub(super) has_more: Option<bool>,
}

impl ApiArticleInfo {
    /// Whether the block carries anything renderable. Some payloads ship an
    /// empty `article_info` object on regular videos; those keep the normal
    /// video/image path.
    pub(super) fn has_content(&self) -> bool {
        let non_empty =
            |value: &Option<String>| value.as_deref().is_some_and(|v| !v.trim().is_empty());
        non_empty(&self.article_title) || non_empty(&self.article_content)
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct ApiAuthor {
    pub(super) uid: Option<String>,
    pub(super) nickname: Option<String>,
    pub(super) avatar_thumb: Option<UrlList>,
    pub(super) signature: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UrlList {
    pub(super) url_list: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ApiVideo {
    pub(super) play_addr: Option<PlayAddr>,
    pub(super) cover: Option<UrlList>,
    pub(super) cover_original_scale: Option<UrlList>,
    pub(super) duration: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(super) struct PlayAddr {
    pub(super) uri: Option<String>,
    pub(super) url_list: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ApiImage {
    pub(super) url_list: Option<Vec<String>>,
    pub(super) clip_type: Option<i32>,
    pub(super) video: Option<ImageVideo>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ImageVideo {
    pub(super) play_addr: Option<PlayAddr>,
    pub(super) play_addr_h264: Option<PlayAddr>,
    pub(super) cover: Option<UrlList>,
    pub(super) cover_original_scale: Option<UrlList>,
    pub(super) duration: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ApiMusic {
    pub(super) title: Option<String>,
    pub(super) author: Option<String>,
    pub(super) play_url: Option<MusicPlayUrl>,
    pub(super) matched_pgc_sound: Option<PgcSound>,
    pub(super) duration: Option<u64>,
    pub(super) extra: Option<String>,
    pub(super) is_original_sound: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(super) struct MusicPlayUrl {
    pub(super) uri: Option<String>,
    pub(super) url_list: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct PgcSound {
    pub(super) title: Option<String>,
    pub(super) author: Option<String>,
}

// --- Share-carrier resolution (口令 -> item schema) ---

#[derive(Debug, Deserialize)]
pub(super) struct CarrierParseResponse {
    pub(super) status_code: Option<i64>,
    pub(super) data: Option<CarrierParseData>,
}

/// On success `schema` is an app deep link such as
/// `aweme://article/detail?item_id=…`; on rejection only `message`
/// (`check_no_pass`, `safety verification not pass`) is set.
#[derive(Debug, Deserialize)]
pub(super) struct CarrierParseData {
    pub(super) schema: Option<String>,
    pub(super) share_scene: Option<String>,
    pub(super) message: Option<String>,
}

// --- Live stream response ---

#[derive(Debug, Deserialize)]
pub(super) struct LiveResponse {
    pub(super) data: Option<LiveData>,
}

#[derive(Debug, Deserialize)]
pub(super) struct LiveData {
    pub(super) data: Option<Vec<LiveRoom>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct LiveRoom {
    pub(super) title: Option<String>,
    pub(super) cover: Option<UrlList>,
    pub(super) user_count: Option<u64>,
}

// --- Live stream type 2 response (webcast.amemv.com) ---

#[derive(Debug, Deserialize)]
pub(super) struct LiveReflowResponse {
    pub(super) data: Option<LiveReflowData>,
}

#[derive(Debug, Deserialize)]
pub(super) struct LiveReflowData {
    pub(super) room: Option<LiveRoom>,
}

// --- Fallback: _ROUTER_DATA types ---

#[derive(Debug, Deserialize)]
pub(super) struct RouterData {
    #[serde(rename = "loaderData")]
    pub(super) loader_data: Option<LoaderData>,
}

#[derive(Debug, Deserialize)]
pub(super) struct LoaderData {
    #[serde(rename = "video_(id)/page")]
    pub(super) video_page: Option<VideoPage>,

    #[serde(rename = "note_(id)/page")]
    pub(super) note_page: Option<VideoPage>,
}

#[derive(Debug, Deserialize)]
pub(super) struct VideoPage {
    #[serde(rename = "videoInfoRes")]
    pub(super) video_info_res: Option<VideoInfoRes>,
}

#[derive(Debug, Deserialize)]
pub(super) struct VideoInfoRes {
    pub(super) item_list: Option<Vec<VideoItem>>,
    pub(super) filter_list: Option<Vec<super::filters::ContentFilter>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct VideoItem {
    pub(super) aweme_id: Option<String>,
    pub(super) desc: Option<String>,
    pub(super) item_title: Option<String>,
    pub(super) caption: Option<String>,
    pub(super) create_time: Option<i64>,
    pub(super) author: Option<VideoAuthor>,
    pub(super) video: Option<VideoInfo>,
    pub(super) images: Option<Vec<ImageInfo>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct VideoAuthor {
    pub(super) nickname: Option<String>,
    pub(super) avatar_thumb: Option<AvatarInfo>,
}

#[derive(Debug, Deserialize)]
pub(super) struct AvatarInfo {
    pub(super) url_list: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct VideoInfo {
    pub(super) play_addr: Option<FallbackPlayAddr>,
    pub(super) cover: Option<CoverInfo>,
    pub(super) duration: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FallbackPlayAddr {
    pub(super) url_list: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CoverInfo {
    pub(super) url_list: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ImageInfo {
    pub(super) url_list: Option<Vec<String>>,
}

// --- Fallback: slides API types ---

#[derive(Debug, Deserialize)]
pub(super) struct SlidesResponse {
    pub(super) aweme_details: Option<Vec<SlideDetail>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SlideDetail {
    pub(super) desc: Option<String>,
    pub(super) item_title: Option<String>,
    pub(super) caption: Option<String>,
    pub(super) create_time: Option<i64>,
    pub(super) author: Option<VideoAuthor>,
    pub(super) images: Option<Vec<SlideImage>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SlideImage {
    pub(super) url_list: Option<Vec<String>>,
    pub(super) video: Option<SlideVideo>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SlideVideo {
    pub(super) play_addr: Option<FallbackPlayAddr>,
}
