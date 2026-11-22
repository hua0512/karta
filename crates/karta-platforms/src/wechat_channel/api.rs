//! Endpoint calls for the WeChat video-channel two-step resolution flow:
//!
//! 1. The Tencent Yuanbao parse API resolves a share link into a
//!    `playable_url` whose query carries `token` (generalToken) and `eid`
//!    (exportId) — one-shot feed-preview credentials.
//! 2. The channels feed-preview API exchanges those credentials for the video
//!    URL, author info, and interaction stats. No cookie is needed here.

use serde::Deserialize;
use serde_json::json;
use wreq::Client;
use wreq::header::{HeaderMap, HeaderValue};

use karta_core::error::{ParserError, Result};
use karta_core::http;

/// Step-1 endpoint: Tencent Yuanbao share-link parse API.
const YUANBAO_PARSE_URL: &str = "https://yuanbao.tencent.com/api/weixin/get_parse_result";

/// Step-2 endpoint: WeChat channel feed-preview detail API.
const FEED_INFO_URL: &str = "https://channels.weixin.qq.com/finder-preview/api/feed/get_feed_info";

/// Feed-preview page URL, used for step-2's `_pageUrl` param and `Referer`.
const FEED_PAGE_URL: &str = "https://channels.weixin.qq.com/finder-preview/pages/feed";

/// Yuanbao request headers, including the required device/session fingerprint
/// `x-*` headers.
const PARSE_HEADER_PAIRS: &[(&str, &str)] = &[
    ("accept", "application/json, text/plain, */*"),
    ("accept-language", "zh-CN,zh;q=0.9,en;q=0.8"),
    ("content-type", "application/json"),
    ("origin", "https://yuanbao.tencent.com"),
    (
        "referer",
        "https://yuanbao.tencent.com/chat/naQivTmsDa/cf4d0079-ed1b-4c55-a3f3-2ca1379727d1",
    ),
    (
        "user-agent",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36",
    ),
    (
        "sec-ch-ua",
        r#""Chromium";v="148", "Google Chrome";v="148", "Not/A)Brand";v="99""#,
    ),
    ("sec-ch-ua-mobile", "?0"),
    ("sec-ch-ua-platform", r#""macOS""#),
    ("sec-fetch-dest", "empty"),
    ("sec-fetch-mode", "cors"),
    ("sec-fetch-site", "same-origin"),
    ("t-userid", "b9575f6b0a8c4a55a08096904a5ef20a"),
    (
        "x-agentid",
        "naQivTmsDa/cf4d0079-ed1b-4c55-a3f3-2ca1379727d1",
    ),
    ("x-commit-tag", "72282a0d"),
    ("x-device-id", "1921b001708100d7fa31002b9646bd0cc15a3e2e1f"),
    ("x-hy106", ""),
    ("x-hy92", "e963067ffa31002b9646bd0c03000008b1951a"),
    ("x-hy93", "1921b001708100d7fa31002b9646bd0cc15a3e2e1f"),
    ("x-id", "b9575f6b0a8c4a55a08096904a5ef20a"),
    ("x-instance-id", "5"),
    ("x-language", "zh-CN"),
    ("x-os_version", "Mac OS(10.15.7)-Blink"),
    ("x-platform", "mac"),
    ("x-requested-with", "XMLHttpRequest"),
    ("x-source", "web"),
    ("x-web-third-source", "main"),
    ("x-webdriver", "0"),
    ("x-webversion", "2.69.0"),
    ("x-ybuitest", "0"),
];

/// Feed detail request headers (`Referer` is appended per-request in
/// [`feed_info_headers`]).
const FEED_INFO_HEADER_PAIRS: &[(&str, &str)] = &[
    ("accept", "application/json, text/plain, */*"),
    ("accept-language", "zh-CN,zh;q=0.9,en;q=0.8"),
    ("connection", "keep-alive"),
    ("content-type", "application/json"),
    ("origin", "https://channels.weixin.qq.com"),
    ("sec-fetch-dest", "empty"),
    ("sec-fetch-mode", "cors"),
    ("sec-fetch-site", "same-origin"),
    (
        "user-agent",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36",
    ),
    (
        "sec-ch-ua",
        r#""Chromium";v="148", "Google Chrome";v="148", "Not/A)Brand";v="99""#,
    ),
    ("sec-ch-ua-mobile", "?0"),
    ("sec-ch-ua-platform", r#""macOS""#),
];

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

/// Step-1 (Yuanbao parse API) response envelope.
#[derive(Debug, Deserialize)]
pub(super) struct YuanbaoParseResponse {
    pub(super) data: Option<YuanbaoParseData>,
}

/// Step-1 payload: share-link metadata plus the `playable_url` carrying the
/// feed-preview credentials. The `cover_url` / `author` / `desc` fields serve
/// as fallbacks when step 2 omits the corresponding values.
#[derive(Debug, Deserialize)]
pub(super) struct YuanbaoParseData {
    /// Success marker; also the share link's *original* export id — NOT the
    /// `eid` needed by step 2 (see [`extract_playable_params`]).
    pub(super) wx_export_id: Option<String>,
    pub(super) playable_url: Option<String>,
    pub(super) cover_url: Option<String>,
    pub(super) author: Option<String>,
    pub(super) author_icon: Option<String>,
    pub(super) desc: Option<String>,
}

/// Step-2 (feed detail API) response envelope.
#[derive(Debug, Deserialize)]
pub(super) struct FeedInfoResponse {
    /// May be absent entirely on success; only a present non-zero value is an
    /// error.
    #[serde(rename = "errCode")]
    pub(super) err_code: Option<i64>,
    #[serde(rename = "errMsg")]
    pub(super) err_msg: Option<String>,
    pub(super) data: Option<FeedData>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FeedData {
    #[serde(rename = "feedInfo")]
    pub(super) feed_info: Option<FeedInfo>,
    #[serde(rename = "authorInfo")]
    pub(super) author_info: Option<AuthorInfo>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct FeedInfo {
    /// Original video URL with all CDN params (preferred download link).
    #[serde(rename = "videoUrl")]
    pub(super) video_url: Option<String>,
    /// H.264-encoded variant, used as the fallback video URL.
    #[serde(rename = "h264VideoInfo")]
    pub(super) h264_video_info: Option<H264VideoInfo>,
    #[serde(rename = "coverUrl")]
    pub(super) cover_url: Option<String>,
    pub(super) description: Option<String>,
    /// Publish time in unix seconds.
    pub(super) createtime: Option<i64>,
    /// Pre-formatted 红心 (like) count — despite the name, this is what the
    /// 视频号 App shows as ❤️.
    #[serde(rename = "likeCountFmt")]
    pub(super) like_count_fmt: Option<String>,
    /// Pre-formatted 喜欢 count — shown as 👍 (see [`FeedInfo::like_count_fmt`]).
    #[serde(rename = "favCountFmt")]
    pub(super) fav_count_fmt: Option<String>,
    #[serde(rename = "forwardCountFmt")]
    pub(super) forward_count_fmt: Option<String>,
    #[serde(rename = "commentCountFmt")]
    pub(super) comment_count_fmt: Option<String>,
    /// Feed media type; kept for the no-video error message.
    #[serde(rename = "mediaType")]
    pub(super) media_type: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(super) struct H264VideoInfo {
    #[serde(rename = "videoUrl")]
    pub(super) video_url: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct AuthorInfo {
    pub(super) nickname: Option<String>,
    #[serde(rename = "headImgUrl")]
    pub(super) head_img_url: Option<String>,
}

// ---------------------------------------------------------------------------
// Request helpers
// ---------------------------------------------------------------------------

/// Build the step-1 header set: fingerprint headers plus the Yuanbao cookie.
fn parse_headers(cookie: &str) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    for (name, value) in PARSE_HEADER_PAIRS {
        headers.insert(*name, HeaderValue::from_static(value));
    }
    let cookie = HeaderValue::from_str(cookie).map_err(|_| {
        ParserError::Parse(
            "wechat_channel_cookie 含有非法字符（如换行），无法作为 Cookie 请求头发送".to_string(),
        )
    })?;
    headers.insert("cookie", cookie);
    Ok(headers)
}

/// Build the step-2 header set with the per-request `Referer`. No cookie.
fn feed_info_headers(referer: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    for (name, value) in FEED_INFO_HEADER_PAIRS {
        headers.insert(*name, HeaderValue::from_static(value));
    }
    if let Ok(value) = HeaderValue::from_str(referer) {
        headers.insert("referer", value);
    }
    headers
}

/// Generate the `_rid` query value: unix seconds in lowercase hex, a dash,
/// then 8 random lowercase hex chars.
fn generate_rid() -> String {
    use rand::RngExt;

    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut rng = rand::rng();
    let suffix: String = (0..8)
        .map(|_| char::from_digit(rng.random_range(0..16u32), 16).expect("value below 16"))
        .collect();

    format!("{secs:x}-{suffix}")
}

/// Build the step-2 URL: `_rid` plus the percent-encoded feed page URL.
fn build_feed_info_url(rid: &str) -> String {
    let mut url = url::Url::parse(FEED_INFO_URL).expect("static endpoint URL is valid");
    url.query_pairs_mut()
        .append_pair("_rid", rid)
        .append_pair("_pageUrl", FEED_PAGE_URL);
    url.to_string()
}

/// Build the step-2 `Referer`: the feed page URL with fixed scene params plus
/// the percent-encoded token and eid.
fn build_feed_referer(general_token: &str, export_id: &str) -> String {
    let mut url = url::Url::parse(FEED_PAGE_URL).expect("static page URL is valid");
    url.query_pairs_mut()
        .append_pair("entry_card_type", "48")
        .append_pair("comment_scene", "39")
        .append_pair("appid", "0")
        .append_pair("token", general_token)
        .append_pair("entry_scene", "0")
        .append_pair("eid", export_id);
    url.to_string()
}

/// Extract `token` (generalToken) and `eid` (exportId) from the step-1
/// `playable_url` query.
///
/// Both are one-shot feed-preview credentials issued by Yuanbao and both are
/// required. `eid` must NOT fall back to the step-1 `wx_export_id`: that is
/// the share link's original export id — a different value — and mixing them
/// makes step 2 return empty data.
pub(super) fn extract_playable_params(playable_url: &str) -> Result<(String, String)> {
    let parsed = url::Url::parse(playable_url).ok();
    let param = |name: &str| -> Option<String> {
        parsed
            .as_ref()?
            .query_pairs()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.into_owned())
            .filter(|value| !value.is_empty())
    };

    match (param("token"), param("eid")) {
        (Some(token), Some(eid)) => Ok((token, eid)),
        _ => Err(ParserError::Parse(
            "元宝接口返回的 playable_url 缺少 token 或 eid，无法解析视频号详情".to_string(),
        )),
    }
}

/// Validate the step-1 response: `wx_export_id` is the success marker, and
/// its absence means the cookie is stale or the share link is invalid.
fn validate_parse_response(response: YuanbaoParseResponse) -> Result<YuanbaoParseData> {
    match response.data {
        Some(data)
            if data
                .wx_export_id
                .as_deref()
                .is_some_and(|id| !id.is_empty()) =>
        {
            Ok(data)
        }
        _ => Err(ParserError::Parse(
            "元宝接口未返回 wx_export_id，可能是 Cookie 失效或分享链接无效".to_string(),
        )),
    }
}

// ---------------------------------------------------------------------------
// Endpoint calls
// ---------------------------------------------------------------------------

/// Step 1: resolve a share link through the Yuanbao parse API.
pub(super) async fn parse_share_url(
    client: &Client,
    share_url: &str,
    cookie: &str,
) -> Result<YuanbaoParseData> {
    let payload = json!({
        "type": "video_channel_url",
        "url": share_url,
        "scene": 1,
    });

    let response = client
        .post(YUANBAO_PARSE_URL)
        .headers(parse_headers(cookie)?)
        .json(&payload)
        .send()
        .await?;
    http::check_status(&response)?;

    validate_parse_response(response.json().await?)
}

/// Step 2: exchange the feed-preview credentials for the feed detail.
pub(super) async fn get_feed_info(
    client: &Client,
    export_id: &str,
    general_token: &str,
) -> Result<FeedInfoResponse> {
    let api_url = build_feed_info_url(&generate_rid());
    let referer = build_feed_referer(general_token, export_id);
    let payload = json!({
        "baseReq": { "generalToken": general_token },
        "exportId": export_id,
    });

    let response = client
        .post(&api_url)
        .headers(feed_info_headers(&referer))
        .json(&payload)
        .send()
        .await?;
    http::check_status(&response)?;

    Ok(response.json().await?)
}
