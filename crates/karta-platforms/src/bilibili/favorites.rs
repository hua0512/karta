//! Bilibili favorites list request flow and [`ParseResult`] construction.

use karta_core::Platform;
use karta_core::data::{Author, Content, GraphicsContent, ImageContent, ParseResult};
use karta_core::error::{ParserError, Result};

use super::BilibiliParser;
use super::api::{ApiResponse, FavoritesInfo};

const API_FAVORITE_INFO: &str = "https://api.bilibili.com/x/v3/fav/resource/list";

impl BilibiliParser {
    /// Parse favorites list
    pub(super) async fn fetch_favorites(&self, fav_id: &str) -> Result<ParseResult> {
        let client = self.downloader.client();

        let params = [("media_id", fav_id), ("ps", "20"), ("pn", "1")];

        let response: ApiResponse<FavoritesInfo> = client
            .get(API_FAVORITE_INFO)
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

        let fav_data = response
            .data
            .ok_or_else(|| ParserError::ContentNotFound("Favorites data not found".to_string()))?;

        Ok(build_favorites_result(&fav_data, fav_id))
    }
}

fn build_favorites_result(data: &FavoritesInfo, fav_id: &str) -> ParseResult {
    let info = &data.info;
    let mut result = ParseResult::new(Platform::Bilibili)
        .with_title(format!("收藏夹 - {}", info.title))
        .with_text(&info.intro)
        .with_url(format!(
            "https://space.bilibili.com/{}/favlist?fid={fav_id}",
            info.upper.mid
        ))
        .with_timestamp(info.ctime)
        .with_author(
            Author::new(&info.upper.name)
                .with_uid(info.upper.mid.to_string())
                .with_avatar(&info.upper.face)
                .with_url(format!("https://space.bilibili.com/{}", info.upper.mid)),
        );

    result.add_extra("media_count", info.media_count.to_string());

    if let Some(medias) = &data.medias {
        for media in medias.iter().take(20) {
            let graphics = GraphicsContent::new()
                .with_images(vec![ImageContent::new().with_url(&media.cover)])
                .with_description(media_description(media));
            result.add_content(Content::Graphics(graphics));
        }
    }

    result
}

fn media_description(media: &super::api::FavMedia) -> String {
    let url = media
        .bvid
        .as_ref()
        .map(|bvid| format!("https://www.bilibili.com/video/{bvid}"))
        .unwrap_or_else(|| format!("https://www.bilibili.com/video/av{}", media.id));

    format!(
        "标题: {}\n简介: {}\n链接: {}",
        media.title, media.intro, url
    )
}
