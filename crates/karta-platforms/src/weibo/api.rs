#![allow(dead_code)]

use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(super) struct WeiboResponse {
    pub(super) ok: Option<i32>,
    pub(super) data: Option<WeiboData>,
}

#[derive(Debug, Deserialize)]
pub(super) struct WeiboData {
    pub(super) text: Option<String>,
    #[serde(rename = "created_at")]
    pub(super) created_at: Option<String>,
    pub(super) user: Option<WeiboUser>,
    pub(super) page_info: Option<PageInfo>,
    pub(super) pics: Option<Vec<WeiboPic>>,
    pub(super) retweeted_status: Option<Box<WeiboData>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct WeiboUser {
    pub(super) screen_name: Option<String>,
    pub(super) profile_image_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct PageInfo {
    #[serde(rename = "type")]
    pub(super) page_type: Option<String>,
    pub(super) media_info: Option<MediaInfo>,
    pub(super) page_pic: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct MediaInfo {
    pub(super) stream_url: Option<String>,
    pub(super) stream_url_hd: Option<String>,
    pub(super) mp4_hd_url: Option<String>,
    pub(super) mp4_sd_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct WeiboPic {
    pub(super) large: Option<PicInfo>,
}

#[derive(Debug, Deserialize)]
pub(super) struct PicInfo {
    pub(super) url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ArticleResponse {
    pub(super) code: Option<String>,
    pub(super) msg: Option<String>,
    pub(super) data: Option<ArticleData>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ArticleData {
    pub(super) url: Option<String>,
    pub(super) title: Option<String>,
    pub(super) content: Option<String>,
    pub(super) userinfo: Option<ArticleUserInfo>,
    pub(super) create_at_unix: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ArticleUserInfo {
    pub(super) screen_name: Option<String>,
    pub(super) profile_image_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FidResponse {
    pub(super) data: Option<FidData>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FidData {
    #[serde(rename = "Component_Play_Playinfo")]
    pub(super) play_info: Option<FidPlayInfo>,
    #[serde(rename = "Component_Reward")]
    pub(super) reward: Option<FidReward>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FidPlayInfo {
    pub(super) title: Option<String>,
    pub(super) text: Option<String>,
    pub(super) cover_image: Option<String>,
    pub(super) stream_url: Option<String>,
    pub(super) real_date: Option<i64>,
    pub(super) urls: Option<HashMap<String, String>>,
    pub(super) reward: Option<FidReward>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FidReward {
    pub(super) user: Option<FidUser>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FidUser {
    pub(super) name: Option<String>,
    pub(super) screen_name: Option<String>,
    pub(super) profile_image_url: Option<String>,
    pub(super) description: Option<String>,
}
