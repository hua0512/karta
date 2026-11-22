//! Bilibili dynamic request flow and result construction.

use karta_core::Platform;
use karta_core::data::{Author, Content, GraphicsContent, ImageContent, ParseResult};
use karta_core::error::{ParserError, Result};

use super::BilibiliParser;
use super::api::{ApiResponse, DynamicDetail, DynamicItem, DynamicMajor};

const API_DYNAMIC_INFO: &str = "https://api.bilibili.com/x/polymer/web-dynamic/v1/detail";

impl BilibiliParser {
    /// Parse dynamic
    pub(super) async fn fetch_dynamic(&self, dynamic_id: &str) -> Result<ParseResult> {
        let client = self.downloader.client();

        let params = [("id", dynamic_id)];

        let response: ApiResponse<DynamicDetail> = client
            .get(API_DYNAMIC_INFO)
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

        let detail = response
            .data
            .ok_or_else(|| ParserError::ContentNotFound("Dynamic data not found".to_string()))?;

        // An article posted as a dynamic keeps its text/image order only on
        // the opus endpoint; the dynamic payload has already flattened it.
        if is_opus_like(&detail.item) {
            match self.fetch_opus(dynamic_id).await {
                Ok(result) => return Ok(result),
                Err(e) => tracing::warn!(
                    dynamic_id,
                    error = %e,
                    "Opus detail failed for an article dynamic; using the dynamic payload"
                ),
            }
        }

        self.build_dynamic(&detail.item).await
    }

    /// Build a dynamic's result, downloading a shared video when there is one.
    ///
    /// Boxed because reposts recurse: a forwarded dynamic may itself share a
    /// video that needs the same treatment.
    fn build_dynamic<'a>(
        &'a self,
        item: &'a DynamicItem,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ParseResult>> + Send + 'a>> {
        Box::pin(async move {
            if let Some(bvid) = archive_bvid(item) {
                match self.fetch_video_info(bvid, None, 1).await {
                    Ok(mut result) => {
                        // Keep the video's own metadata, but show what the
                        // poster actually wrote above it.
                        if let Some(text) = dynamic_text(item) {
                            result.text = Some(text.to_string());
                        }
                        result.add_extra("content_type", "动态");
                        return Ok(result);
                    }
                    Err(e) => tracing::warn!(
                        bvid,
                        error = %e,
                        "Failed to fetch the video shared by a dynamic; using its cover"
                    ),
                }
            }

            let mut result = build_dynamic_result_shallow(item);
            if let Some(orig) = &item.orig {
                result = result.with_repost(self.build_dynamic(orig).await?);
            }
            Ok(result)
        })
    }
}

/// Build a dynamic's own [`ParseResult`], leaving its repost unresolved.
///
/// The parser recurses over reposts itself so a reposted video can be fetched
/// (an async step this pure builder cannot take).
fn build_dynamic_result_shallow(item: &DynamicItem) -> ParseResult {
    let author = &item.modules.module_author;
    let dynamic = &item.modules.module_dynamic;

    let mut result = ParseResult::new(Platform::Bilibili)
        .with_url(format!("https://t.bilibili.com/{}", item.id))
        .with_timestamp(author.pub_ts)
        .with_author(
            Author::new(&author.name)
                .with_uid(author.mid.to_string())
                .with_avatar(&author.face)
                .with_url(format!("https://space.bilibili.com/{}", author.mid)),
        );

    if let Some(desc) = &dynamic.desc {
        result = result.with_text(&desc.text);
    }

    if let Some(major) = &dynamic.major {
        add_major_content(&mut result, major);
    }

    result
}

/// The BV id of the video a dynamic shares, when it is a video share.
///
/// Callers fetch it so a shared video is downloaded like a direct BV link,
/// rather than degrading to its cover image.
fn archive_bvid(item: &DynamicItem) -> Option<&str> {
    let major = item.modules.module_dynamic.major.as_ref()?;
    if major.major_type != "MAJOR_TYPE_ARCHIVE" {
        return None;
    }
    let bvid = major.archive.as_ref()?.bvid.as_str();
    (!bvid.is_empty()).then_some(bvid)
}

/// Whether this dynamic is an article whose body only the opus endpoint keeps
/// in its original text/image order.
fn is_opus_like(item: &DynamicItem) -> bool {
    if item.dynamic_type == "DYNAMIC_TYPE_ARTICLE" {
        return true;
    }
    item.modules
        .module_dynamic
        .major
        .as_ref()
        .is_some_and(|major| {
            matches!(
                major.major_type.as_str(),
                "MAJOR_TYPE_OPUS" | "MAJOR_TYPE_ARTICLE"
            )
        })
}

/// The dynamic's own text, used to caption a video fetched for it.
fn dynamic_text(item: &DynamicItem) -> Option<&str> {
    item.modules
        .module_dynamic
        .desc
        .as_ref()
        .map(|desc| desc.text.as_str())
        .filter(|text| !text.is_empty())
}

fn add_major_content(result: &mut ParseResult, major: &DynamicMajor) {
    match major.major_type.as_str() {
        "MAJOR_TYPE_ARCHIVE" => {
            if let Some(archive) = &major.archive {
                result.title = Some(archive.title.clone());
                result.add_content(Content::Image(ImageContent::new().with_url(&archive.cover)));
                result.add_extra("duration", &archive.duration_text);
            }
        }
        "MAJOR_TYPE_DRAW" => {
            if let Some(draw) = &major.draw {
                let images: Vec<ImageContent> = draw
                    .items
                    .iter()
                    .map(|it| {
                        ImageContent::new()
                            .with_url(&it.src)
                            .with_dimensions(it.width, it.height)
                    })
                    .collect();

                if !images.is_empty() {
                    result.add_content(Content::Graphics(
                        GraphicsContent::new().with_images(images),
                    ));
                }
            }
        }
        _ => {}
    }
}
