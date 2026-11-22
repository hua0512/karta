use karta_core::Platform;
use karta_core::data::{Author, Content, ImageContent, ParseResult, VideoContent};
use karta_core::error::{ParserError, Result};
use karta_core::html;

use super::api::{ArticleData, FidData, FidPlayInfo, FidReward, WeiboData};

pub(super) fn build_status_result(data: &WeiboData) -> ParseResult {
    let mut result = ParseResult::new(Platform::Weibo);

    if let Some(text) = data.text.as_ref().map(|text| html::html_to_text(text)) {
        result = result.with_text(text);
    }
    if let Some(author) = data.user.as_ref().map(|user| {
        let mut author = Author::new(user.screen_name.as_deref().unwrap_or("微博用户"));
        if let Some(avatar) = &user.profile_image_url {
            author = author.with_avatar(avatar);
        }
        author
    }) {
        result = result.with_author(author);
    }

    result = result.with_contents(collect_status_contents(data));

    if let Some(repost_data) = &data.retweeted_status {
        result = result.with_repost(build_status_result(repost_data));
    }

    result
}

pub(super) fn build_fid_result(data: &FidData) -> Result<ParseResult> {
    let play_info = data
        .play_info
        .as_ref()
        .ok_or_else(|| ParserError::ContentNotFound("Play info not found".to_string()))?;

    let mut result = ParseResult::new(Platform::Weibo);

    if let Some(title) = &play_info.title {
        result = result.with_title(title);
    }
    if let Some(text) = &play_info.text {
        result = result.with_text(text);
    }
    if let Some(timestamp) = play_info.real_date {
        result = result.with_timestamp(timestamp);
    }
    if let Some(author) = data
        .reward
        .as_ref()
        .or(play_info.reward.as_ref())
        .and_then(build_fid_author)
    {
        result = result.with_author(author);
    }
    if let Some(video) = build_fid_video(play_info) {
        result.add_content(Content::Video(video));
    }

    Ok(result)
}

pub(super) fn build_article_result(data: &ArticleData) -> ParseResult {
    let mut result = ParseResult::new(Platform::Weibo);

    if let Some(title) = &data.title {
        result = result.with_title(title);
    }
    if let Some(url) = &data.url {
        result = result.with_url(url);
    }
    if let Some(timestamp) = data.create_at_unix {
        result = result.with_timestamp(timestamp);
    }
    if let Some(userinfo) = &data.userinfo {
        let mut author = Author::new(userinfo.screen_name.as_deref().unwrap_or("微博用户"));
        if let Some(avatar) = &userinfo.profile_image_url {
            author = author.with_avatar(avatar);
        }
        result = result.with_author(author);
    }

    if let Some(content) = &data.content {
        for url in html::extract_img_urls(content)
            .into_iter()
            .filter(|url| !url.contains("face/") && !url.contains("emoticon/"))
            .take(9)
        {
            result.add_content(Content::Image(ImageContent::new().with_url(&url)));
        }

        let text = html::html_to_text(content);
        if !text.is_empty() {
            result = result.with_text(text);
        }
    }

    result
}

fn collect_status_contents(data: &WeiboData) -> Vec<Content> {
    let mut contents = Vec::new();

    if let Some(page_info) = &data.page_info
        && let Some(media) = &page_info.media_info
    {
        let video_url = media
            .mp4_hd_url
            .as_ref()
            .or(media.stream_url_hd.as_ref())
            .or(media.stream_url.as_ref())
            .or(media.mp4_sd_url.as_ref());

        if let Some(url) = video_url {
            let cover = page_info.page_pic.as_deref().unwrap_or("");
            contents.push(Content::Video(
                VideoContent::new().with_url(url).with_cover(cover),
            ));
        }
    }

    if let Some(pics) = &data.pics {
        contents.extend(pics.iter().filter_map(|pic| {
            pic.large
                .as_ref()
                .and_then(|large| large.url.as_deref())
                .map(|url| Content::Image(ImageContent::new().with_url(url)))
        }));
    }

    contents
}

fn build_fid_author(reward: &FidReward) -> Option<Author> {
    let user = reward.user.as_ref()?;
    let name = user
        .name
        .as_deref()
        .or(user.screen_name.as_deref())
        .unwrap_or("微博用户");
    let mut author = Author::new(name);
    if let Some(avatar) = &user.profile_image_url {
        author = author.with_avatar(avatar);
    }
    if let Some(description) = user.description.as_deref().filter(|desc| !desc.is_empty()) {
        author = author.with_description(description);
    }
    Some(author)
}

fn build_fid_video(play_info: &FidPlayInfo) -> Option<VideoContent> {
    let url = best_fid_video_url(play_info)?;
    let mut video = VideoContent::new().with_url(url);
    if let Some(cover) = play_info.cover_image.as_deref().map(normalize_weibo_url) {
        video = video.with_cover(cover);
    }
    Some(video)
}

fn best_fid_video_url(play_info: &FidPlayInfo) -> Option<String> {
    const PREFERRED_KEYS: &[&str] = &[
        "mp4_hd_url",
        "mp4_720p_mp4",
        "mp4_sd_url",
        "stream_url_hd",
        "stream_url",
    ];

    if let Some(urls) = &play_info.urls {
        for key in PREFERRED_KEYS {
            if let Some(url) = urls.get(*key) {
                return Some(normalize_weibo_url(url));
            }
        }
        if let Some(url) = urls.values().next() {
            return Some(normalize_weibo_url(url));
        }
    }

    play_info.stream_url.as_deref().map(normalize_weibo_url)
}

fn normalize_weibo_url(url: &str) -> String {
    if url.starts_with("http") {
        url.to_string()
    } else {
        format!("https:{url}")
    }
}
