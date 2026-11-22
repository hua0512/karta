use karta_core::Platform;
use karta_core::data::{Author, Content, ImageContent, ParseResult, VideoContent};

use super::game::GameType;
use super::media::{dedupe_images, extract_text_and_media_from_entities, optimize_image_url};

pub(super) fn build_bbs_result(link: &serde_json::Value, link_id: &str) -> ParseResult {
    let title = link
        .get("title")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();

    let username = link
        .pointer("/user/username")
        .and_then(|value| value.as_str())
        .unwrap_or("小黑盒用户");
    let mut author = Author::new(username);
    if let Some(avatar) = user_avatar(link).filter(|avatar| !avatar.is_empty()) {
        author = author.with_avatar(avatar);
    }

    let description = link
        .get("description")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();

    let mut contents = Vec::new();
    let thumb = link
        .get("video_thumb")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .or_else(|| link.get("thumb").and_then(|value| value.as_str()));

    if let Some(thumb_url) = thumb {
        contents.push(Content::Image(
            ImageContent::new().with_url(optimize_image_url(thumb_url)),
        ));
    }

    let mut full_text = None;
    if let Some(text_val) = link.get("text").and_then(|value| value.as_str()) {
        full_text = extract_text_and_media_from_entities(text_val, &mut contents);
    }

    if link.get("has_video").and_then(|value| value.as_i64()) == Some(1)
        && let Some(video_url) = link.get("video_url").and_then(|value| value.as_str())
    {
        let mut video = VideoContent::new().with_url(video_url);
        if let Some(cover) = thumb {
            video = video.with_cover(optimize_image_url(cover));
        }
        contents.push(Content::Video(video));
    }

    dedupe_images(&mut contents);

    let mut result = ParseResult::new(Platform::Xiaoheihe)
        .with_title(title)
        .with_author(author)
        .with_url(format!("https://xiaoheihe.cn/bbs/link/{link_id}"))
        .with_contents(contents);

    if let Some(text) = full_text {
        result = result.with_text(text);
    } else if !description.is_empty() {
        result = result.with_text(description);
    }

    if let Some(hashtags) = link.get("hashtags").and_then(|value| value.as_array()) {
        let tags: Vec<String> = hashtags
            .iter()
            .filter_map(|hashtag| hashtag.get("name").and_then(|value| value.as_str()))
            .map(|name| format!("#{name}"))
            .collect();
        if !tags.is_empty() {
            result.add_extra("标签", tags.join(" "));
        }
    }

    result
}

pub(super) fn build_game_result(game: &serde_json::Value, game_type: GameType) -> ParseResult {
    let name = game
        .get("name")
        .and_then(|value| value.as_str())
        .unwrap_or("未知游戏");
    let name_en = game
        .get("name_en")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let title = if name_en.is_empty() {
        name.to_string()
    } else {
        format!("{name} ({name_en})")
    };

    let mut contents = Vec::new();
    if let Some(image_url) = game.get("image").and_then(|value| value.as_str()) {
        contents.push(Content::Image(
            ImageContent::new().with_url(optimize_image_url(image_url)),
        ));
    }

    let about = game
        .get("about_the_game")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let text = if about.chars().count() > 300 {
        format!("{}...", about.chars().take(300).collect::<String>())
    } else {
        about.to_string()
    };

    let mut result = ParseResult::new(Platform::Xiaoheihe)
        .with_title(title)
        .with_contents(contents);

    if !text.is_empty() {
        result = result.with_text(text);
    }

    if let Some(score) = game.get("score").and_then(|value| value.as_f64())
        && score > 0.0
    {
        result.add_extra("评分", format!("{score:.1}"));
    }

    if let Some(count) = game
        .pointer("/comment_stats/score_comment")
        .and_then(|value| value.as_i64())
        && count > 0
    {
        result.add_extra("评价数", count.to_string());
    }

    add_price_metadata(&mut result, game, game_type);
    add_menu_metadata(&mut result, game);

    result
}

fn user_avatar(link: &serde_json::Value) -> Option<&str> {
    link.pointer("/user/avartar")
        .or_else(|| link.pointer("/user/avatar"))
        .and_then(|value| value.as_str())
}

fn add_price_metadata(result: &mut ParseResult, game: &serde_json::Value, game_type: GameType) {
    match game_type {
        GameType::Pc => {
            if let Some(price) = game.pointer("/price/current") {
                let price_str = if let Some(price) = price.as_f64() {
                    if price > 0.0 {
                        format!("¥{price:.0}")
                    } else {
                        "免费".to_string()
                    }
                } else {
                    "免费".to_string()
                };
                result.add_extra("价格", price_str);

                if let Some(discount) = game.pointer("/price/discount").and_then(|v| v.as_i64())
                    && discount > 0
                {
                    result.add_extra("折扣", format!("-{discount}%"));
                }
            }
        }
        GameType::Console | GameType::Mobile => {
            if let Some(region) = game
                .get("region_prices")
                .and_then(|value| value.as_array())
                .and_then(|regions| regions.first())
            {
                if let Some(amount) = region.get("final_amount").and_then(|value| value.as_str()) {
                    result.add_extra("价格", amount);
                }
                if let Some(discount) = region.get("discount").and_then(|value| value.as_i64())
                    && discount > 0
                {
                    result.add_extra("折扣", format!("-{discount}%"));
                }
            }
        }
    }
}

fn add_menu_metadata(result: &mut ParseResult, game: &serde_json::Value) {
    if let Some(menu) = game.get("menu_v2").and_then(|value| value.as_array()) {
        for item in menu {
            let item_type = item
                .get("type")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let value = item
                .get("value")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            if value.is_empty() {
                continue;
            }

            match item_type {
                "release_date" => result.add_extra("发售日期", value),
                "developer" => result.add_extra("开发商", value),
                "publisher" => result.add_extra("发行商", value),
                _ => {}
            }
        }
    }
}
