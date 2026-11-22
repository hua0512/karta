use serde::Deserialize;

use karta_core::error::{ParserError, Result};
use karta_core::extract;

/// Marker preceding the embedded video info object in AcFun pages.
const VIDEO_INFO_MARKER: &str = "window.videoInfo";

/// AcFun video info (`window.videoInfo`)
#[derive(Debug, Deserialize)]
pub(super) struct VideoInfo {
    pub(super) title: Option<String>,
    pub(super) description: Option<String>,
    #[serde(rename = "createTimeMillis")]
    pub(super) create_time_millis: Option<i64>,
    #[serde(rename = "coverUrl")]
    pub(super) cover_url: Option<String>,
    pub(super) user: Option<AcfunUser>,
    #[serde(rename = "currentVideoInfo")]
    pub(super) current_video_info: Option<CurrentVideoInfo>,
}

#[derive(Debug, Deserialize)]
pub(super) struct AcfunUser {
    pub(super) name: Option<String>,
    #[serde(rename = "headUrl")]
    pub(super) head_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CurrentVideoInfo {
    #[serde(rename = "durationMillis")]
    pub(super) duration_millis: Option<i64>,
    /// Nested JSON document serialized as a string.
    #[serde(rename = "ksPlayJson")]
    pub(super) ks_play_json: Option<String>,
}

/// Payload of the `ksPlayJson` string field.
#[derive(Debug, Deserialize)]
pub(super) struct KsPlayInfo {
    #[serde(rename = "adaptationSet")]
    pub(super) adaptation_set: Option<Vec<AdaptationSet>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct AdaptationSet {
    pub(super) representation: Option<Vec<Representation>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Representation {
    pub(super) url: Option<String>,
}

/// One chunk of an `ajaxpipe=1` streaming response; only `html` matters.
#[derive(Debug, Deserialize)]
struct AjaxpipeChunk {
    html: Option<String>,
}

/// Extract `window.videoInfo` from an AcFun page body.
///
/// The `ajaxpipe=1` endpoint returns JSON chunks (`{"html":"..."}` followed by
/// a `/*<!-- fetch-stream -->*/` comment) whose `html` string embeds the
/// videoInfo script one JSON-string escape level deep. Deserializing `html`
/// through serde removes that level, after which the balanced-brace scanner
/// slices the plain object. Plain HTML bodies are scanned as-is.
pub(super) fn extract_video_info(body: &str) -> Result<VideoInfo> {
    let html = ajaxpipe_html(body);
    let html = html.as_deref().unwrap_or(body);

    let raw = extract::extract_json_after(html, VIDEO_INFO_MARKER)
        .ok_or_else(|| ParserError::Parse("Cannot find videoInfo".to_string()))?;

    serde_json::from_str(raw)
        .map_err(|e| ParserError::Parse(format!("Failed to parse AcFun data: {}", e)))
}

/// `html` fragment of the first ajaxpipe chunk containing the videoInfo
/// script, or `None` when the body is not an ajaxpipe response.
fn ajaxpipe_html(body: &str) -> Option<String> {
    serde_json::Deserializer::from_str(body)
        .into_iter::<AjaxpipeChunk>()
        .map_while(std::result::Result::ok)
        .filter_map(|chunk| chunk.html)
        .find(|html| html.contains(VIDEO_INFO_MARKER))
}
