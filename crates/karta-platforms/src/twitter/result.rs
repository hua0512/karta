use karta_core::Platform;
use karta_core::data::{Author, Content, DynamicContent, ImageContent, ParseResult, VideoContent};
use karta_core::error::Result;

use super::api::VxTwitterResponse;
use super::syndication::{best_video_variant, parse_twitter_timestamp, twitter_original_image_url};

pub(super) fn build_vxtwitter_result(data: &VxTwitterResponse, url: &str) -> ParseResult {
    let author = Author::new(&data.user_name)
        .with_avatar(&data.user_profile_image_url)
        .with_url(format!("https://x.com/{}", data.user_screen_name));

    let mut result = ParseResult::new(Platform::Twitter)
        .with_author(author)
        .with_text(&data.text)
        .with_timestamp(data.date_epoch)
        .with_url(url);

    if let Some(title) = data.article_title() {
        result = result.with_title(title);
    }

    if let Some(likes) = data.likes {
        result = result.with_extra("likes", likes.to_string());
    }

    for media in &data.media_extended {
        match media.media_type.as_str() {
            "video" => {
                let mut video = VideoContent::new().with_url(&media.url);
                if let Some(thumb) = &media.thumbnail_url {
                    video = video.with_cover(thumb);
                }
                if let Some(ms) = media.duration_millis {
                    video = video.with_duration(ms / 1000);
                }
                result = result.with_content(Content::Video(video));
            }
            "image" => {
                result = result.with_content(Content::Image(
                    ImageContent::new().with_url(twitter_original_image_url(&media.url)),
                ));
            }
            "gif" => {
                let idx = result.contents.len();
                let mut dynamic = DynamicContent::new(format!("twitter_gif_{idx}"), "GIF");
                dynamic = dynamic.with_gif_conversion(&media.url, 30);
                if let Some(thumb) = &media.thumbnail_url {
                    dynamic = dynamic.with_cover(thumb);
                }
                result = result.with_content(Content::Dynamic(dynamic));
            }
            _ => {}
        }
    }

    if let Some(quote) = &data.qrt {
        let quote_url = data.qrt_url.as_deref().unwrap_or("");
        result = result.with_repost(build_vxtwitter_result(quote, quote_url));
    }

    result
}

pub(super) fn build_syndication_result(data: &serde_json::Value, url: &str) -> Result<ParseResult> {
    let text = data
        .get("text")
        .and_then(|value| value.as_str())
        .unwrap_or("");

    let user = data.get("user");
    let author_name = user
        .and_then(|user| user.get("name"))
        .and_then(|value| value.as_str())
        .unwrap_or("Twitter User");
    let avatar = user
        .and_then(|user| user.get("profile_image_url_https"))
        .and_then(|value| value.as_str());

    let mut author = Author::new(author_name);
    if let Some(avatar_url) = avatar {
        author = author.with_avatar(avatar_url);
    }
    if let Some(screen_name) = user
        .and_then(|user| user.get("screen_name"))
        .and_then(|value| value.as_str())
    {
        author = author.with_url(format!("https://x.com/{screen_name}"));
    }

    let timestamp = data
        .get("created_at")
        .and_then(|value| value.as_str())
        .and_then(parse_twitter_timestamp);

    let mut contents = Vec::new();

    if let Some(media_array) = data.get("mediaDetails").and_then(|value| value.as_array()) {
        for media in media_array {
            let media_type = media
                .get("type")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            match media_type {
                "photo" => {
                    if let Some(media_url) = media
                        .get("media_url_https")
                        .and_then(|value| value.as_str())
                    {
                        contents.push(Content::Image(
                            ImageContent::new().with_url(twitter_original_image_url(media_url)),
                        ));
                    }
                }
                "video" => {
                    if let Some(best) = best_video_variant(media) {
                        let mut video = VideoContent::new().with_url(best);
                        if let Some(thumb) = media
                            .get("media_url_https")
                            .and_then(|value| value.as_str())
                        {
                            video = video.with_cover(thumb);
                        }
                        contents.push(Content::Video(video));
                    } else if let Some(thumb) = media
                        .get("media_url_https")
                        .and_then(|value| value.as_str())
                    {
                        contents.push(Content::Image(ImageContent::new().with_url(thumb)));
                    }
                }
                "animated_gif" => {
                    if let Some(gif_url) = best_video_variant(media) {
                        let idx = contents.len();
                        let mut dynamic = DynamicContent::new(format!("twitter_gif_{idx}"), "GIF");
                        dynamic = dynamic.with_gif_conversion(gif_url, 30);
                        if let Some(thumb) = media
                            .get("media_url_https")
                            .and_then(|value| value.as_str())
                        {
                            dynamic = dynamic.with_cover(thumb);
                        }
                        contents.push(Content::Dynamic(dynamic));
                    }
                }
                _ => {}
            }
        }
    }

    let mut result = ParseResult::new(Platform::Twitter)
        .with_author(author)
        .with_text(text)
        .with_url(url)
        .with_contents(contents);

    if let Some(timestamp) = timestamp {
        result = result.with_timestamp(timestamp);
    }

    Ok(result)
}
