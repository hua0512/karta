#![allow(dead_code)]

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(super) struct EmbedContextWrapper {
    pub(super) context: EmbedContext,
}

#[derive(Debug, Deserialize)]
pub(super) struct EmbedContext {
    #[serde(default)]
    pub(super) media: Option<MediaNode>,
    #[serde(default)]
    pub(super) gql_data: Option<GraphqlData>,
}

#[derive(Debug, Deserialize)]
pub(super) struct MediaNode {
    pub(super) __typename: Option<String>,
    #[serde(default)]
    pub(super) display_url: Option<String>,
    #[serde(default)]
    pub(super) display_resources: Vec<DisplayResource>,
    #[serde(default)]
    pub(super) is_video: bool,
    #[serde(default)]
    pub(super) video_url: Option<String>,
    #[serde(default)]
    pub(super) video_duration: Option<f64>,
    #[serde(default)]
    pub(super) taken_at_timestamp: Option<i64>,
    #[serde(default)]
    pub(super) owner: Option<OwnerNode>,
    #[serde(default)]
    pub(super) edge_media_to_caption: Option<EdgeContainer<CaptionNode>>,
    #[serde(default)]
    pub(super) edge_media_preview_like: Option<CountNode>,
    #[serde(default)]
    pub(super) edge_sidecar_to_children: Option<EdgeContainer<MediaNode>>,
    #[serde(default)]
    pub(super) commenter_count: Option<i64>,
    #[serde(default)]
    pub(super) accessibility_caption: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct DisplayResource {
    #[serde(default)]
    pub(super) config_width: u32,
    pub(super) src: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct OwnerNode {
    #[serde(default)]
    pub(super) username: Option<String>,
    #[serde(default)]
    pub(super) full_name: Option<String>,
    #[serde(default)]
    pub(super) profile_pic_url: Option<String>,
    #[serde(default)]
    pub(super) is_verified: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(bound(deserialize = "T: serde::de::DeserializeOwned"))]
pub(super) struct EdgeContainer<T> {
    #[serde(default)]
    pub(super) edges: Vec<EdgeNode<T>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct EdgeNode<T> {
    pub(super) node: T,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct CaptionNode {
    #[serde(default)]
    pub(super) text: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CountNode {
    #[serde(default)]
    pub(super) count: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(super) struct GraphqlResponse {
    pub(super) data: Option<GraphqlData>,
}

#[derive(Debug, Deserialize)]
pub(super) struct GraphqlData {
    #[serde(alias = "xdt_shortcode_media")]
    pub(super) shortcode_media: Option<MediaNode>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ApiResponse {
    #[serde(default)]
    pub(super) items: Vec<ApiMediaItem>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ApiMediaItem {
    #[serde(default)]
    pub(super) media_type: Option<i32>,
    #[serde(default)]
    pub(super) taken_at: Option<i64>,
    #[serde(default)]
    pub(super) caption: Option<ApiCaption>,
    #[serde(default)]
    pub(super) user: Option<ApiUser>,
    #[serde(default)]
    pub(super) image_versions2: Option<ImageVersions>,
    #[serde(default)]
    pub(super) video_versions: Option<Vec<VideoVersion>>,
    #[serde(default)]
    pub(super) video_duration: Option<f64>,
    #[serde(default)]
    pub(super) like_count: Option<i64>,
    #[serde(default)]
    pub(super) comment_count: Option<i64>,
    #[serde(default)]
    pub(super) carousel_media: Option<Vec<ApiMediaItem>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ApiCaption {
    #[serde(default)]
    pub(super) text: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ApiUser {
    #[serde(default)]
    pub(super) username: Option<String>,
    #[serde(default)]
    pub(super) full_name: Option<String>,
    #[serde(default)]
    pub(super) profile_pic_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct StoriesResponse {
    pub(super) reels_media: Vec<ReelMedia>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ReelMedia {
    #[serde(default)]
    pub(super) items: Vec<StoryItem>,
    #[serde(default)]
    pub(super) user: Option<StoryUser>,
}

#[derive(Debug, Deserialize)]
pub(super) struct StoryUser {
    #[serde(default)]
    pub(super) username: Option<String>,
    #[serde(default)]
    pub(super) full_name: Option<String>,
    #[serde(default)]
    pub(super) profile_pic_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct StoryItem {
    #[serde(default)]
    pub(super) pk: Option<String>,
    #[serde(default)]
    pub(super) id: Option<String>,
    #[serde(default)]
    pub(super) taken_at: Option<i64>,
    #[serde(default)]
    pub(super) media_type: Option<i32>,
    #[serde(default)]
    pub(super) image_versions2: Option<ImageVersions>,
    #[serde(default)]
    pub(super) video_versions: Option<Vec<VideoVersion>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ImageVersions {
    #[serde(default)]
    pub(super) candidates: Vec<ImageCandidate>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ImageCandidate {
    #[serde(default)]
    pub(super) width: u32,
    pub(super) url: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct VideoVersion {
    #[serde(default)]
    pub(super) width: u32,
    pub(super) url: String,
}
