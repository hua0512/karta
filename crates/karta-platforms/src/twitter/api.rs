use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(super) struct VxMediaElement {
    #[serde(rename = "type")]
    pub(super) media_type: String,
    pub(super) url: String,
    #[serde(default)]
    pub(super) thumbnail_url: Option<String>,
    #[serde(default)]
    pub(super) duration_millis: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub(super) struct VxTwitterResponse {
    pub(super) text: String,
    pub(super) user_name: String,
    pub(super) user_screen_name: String,
    pub(super) user_profile_image_url: String,
    pub(super) date_epoch: i64,
    #[serde(default)]
    pub(super) likes: Option<i64>,
    #[serde(default)]
    pub(super) media_extended: Vec<VxMediaElement>,
    #[serde(default)]
    pub(super) qrt: Option<Box<VxTwitterResponse>>,
    #[serde(default)]
    #[serde(rename = "qrtURL")]
    pub(super) qrt_url: Option<String>,
    #[serde(default)]
    article: Option<serde_json::Value>,
}

impl VxTwitterResponse {
    pub(super) fn article_title(&self) -> Option<String> {
        self.article.as_ref().and_then(|value| {
            value.as_str().map(ToOwned::to_owned).or_else(|| {
                value
                    .get("title")
                    .and_then(|title| title.as_str())
                    .map(ToOwned::to_owned)
            })
        })
    }
}
