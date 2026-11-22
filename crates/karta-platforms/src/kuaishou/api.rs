use serde::Deserialize;

/// A photo payload inside the `window.INIT_STATE` blob.
///
/// The state object maps request keys to heterogeneous values (strings,
/// objects), so the payload is located dynamically and only this node is
/// deserialized — see `result::build_state_result`.
#[derive(Debug, Deserialize)]
pub(super) struct PhotoInfo {
    pub(super) caption: Option<String>,
    pub(super) timestamp: Option<i64>,
    #[serde(rename = "headUrl")]
    pub(super) head_url: Option<String>,
    #[serde(rename = "userName")]
    pub(super) user_name: Option<String>,
    #[serde(rename = "mainMvUrls")]
    pub(super) main_mv_urls: Option<Vec<UrlInfo>>,
    #[serde(rename = "coverUrl")]
    pub(super) cover_url: Option<String>,
    pub(super) duration: Option<i64>,
    pub(super) ext_params: Option<ExtParams>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UrlInfo {
    pub(super) url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ExtParams {
    pub(super) photo: Option<Vec<PhotoImage>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct PhotoImage {
    pub(super) url: Option<String>,
}
