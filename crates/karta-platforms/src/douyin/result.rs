use karta_core::Platform;
use karta_core::data::{AudioContent, Author, Content, ImageContent, ParseResult, VideoContent};

use super::api::{ApiMusic, AwemeDetail, LiveRoom, PlayAddr, SlideDetail, VideoItem};
use super::article::build_article_result;
use super::caption::{complete_desc, split_desc};

/// Author of an official API item, with the fields the card renderers use.
pub(super) fn aweme_author(item: &AwemeDetail) -> Author {
    item.author
        .as_ref()
        .map(|author| {
            let mut result = Author::new(author.nickname.as_deref().unwrap_or("抖音用户"));
            if let Some(uid) = author.uid.as_deref().filter(|uid| !uid.is_empty()) {
                result = result.with_uid(uid);
            }
            if let Some(avatar) = author
                .avatar_thumb
                .as_ref()
                .and_then(|thumb| thumb.url_list.as_ref())
                .and_then(|urls| urls.first())
            {
                result = result.with_avatar(avatar);
            }
            if let Some(signature) = author
                .signature
                .as_deref()
                .filter(|signature| !signature.is_empty())
            {
                result = result.with_description(signature);
            }
            result
        })
        .unwrap_or_else(|| Author::new("抖音用户"))
}

/// The item's share URL with its tracking query stripped.
pub(super) fn aweme_share_url(item: &AwemeDetail) -> Option<String> {
    item.share_url
        .as_deref()
        .and_then(|url| url.split('?').next())
        .filter(|url| !url.is_empty())
        .map(str::to_owned)
}

/// Builds a [`ParseResult`] from an official API item.
pub(super) fn build_aweme_result(item: &AwemeDetail) -> ParseResult {
    // Long articles reuse the aweme envelope with a placeholder video; the
    // `article_info` block is the discriminator (`aweme_type` is 0 on the
    // anonymous endpoint and 163 on the signed one).
    if let Some(info) = item.article_info.as_ref().filter(|info| info.has_content()) {
        return build_article_result(item, info);
    }

    let author = aweme_author(item);

    // Current API payloads can report `aweme_type = 0` while carrying a full
    // live-photo `images` array. Usable image media is the authoritative
    // discriminator; the numeric type is not stable enough for dispatch.
    let contents = if item
        .images
        .as_ref()
        .is_some_and(|images| !images.is_empty())
    {
        let mut contents = build_aweme_image_contents(item);
        if !contents
            .iter()
            .any(|content| matches!(content, Content::Image(_) | Content::Video(_)))
        {
            // Placeholder image entries (and their BGM) must not hide the
            // playable root video. Keep any soundtrack after the video.
            let mut video_contents = build_aweme_video_contents(item);
            video_contents.append(&mut contents);
            video_contents
        } else {
            contents
        }
    } else {
        build_aweme_video_contents(item)
    };

    let mut result = assemble_post_result(
        &complete_desc(
            item.desc.as_deref(),
            item.item_title.as_deref(),
            item.caption.as_deref(),
        ),
        item.create_time,
        author,
        contents,
    );
    if let Some(url) = aweme_share_url(item) {
        result = result.with_url(url);
    }
    result
}

/// Builds a [`ParseResult`] from fallback `_ROUTER_DATA` video data.
pub(super) fn build_router_item_result(item: &VideoItem) -> ParseResult {
    let mut author = Author::new(
        item.author
            .as_ref()
            .and_then(|author| author.nickname.as_deref())
            .unwrap_or("抖音用户"),
    );
    if let Some(avatar) = item
        .author
        .as_ref()
        .and_then(|author| author.avatar_thumb.as_ref())
        .and_then(|thumb| thumb.url_list.as_ref())
        .and_then(|urls| urls.first())
    {
        author = author.with_avatar(avatar);
    }

    assemble_post_result(
        &complete_desc(
            item.desc.as_deref(),
            item.item_title.as_deref(),
            item.caption.as_deref(),
        ),
        item.create_time,
        author,
        build_router_item_contents(item),
    )
}

/// Builds a [`ParseResult`] from the fallback slides API.
pub(super) fn build_slide_result(detail: &SlideDetail) -> ParseResult {
    let mut author = Author::new(
        detail
            .author
            .as_ref()
            .and_then(|author| author.nickname.as_deref())
            .unwrap_or("抖音用户"),
    );
    if let Some(avatar) = detail
        .author
        .as_ref()
        .and_then(|author| author.avatar_thumb.as_ref())
        .and_then(|thumb| thumb.url_list.as_ref())
        .and_then(|urls| urls.first())
    {
        author = author.with_avatar(avatar);
    }

    assemble_post_result(
        &complete_desc(
            detail.desc.as_deref(),
            detail.item_title.as_deref(),
            detail.caption.as_deref(),
        ),
        detail.create_time,
        author,
        build_slide_contents(detail),
    )
}

/// Builds a [`ParseResult`] from either live response shape.
pub(super) fn build_live_result(room: &LiveRoom) -> ParseResult {
    let mut result =
        ParseResult::new(Platform::Douyin).with_title(room.title.as_deref().unwrap_or("抖音直播"));

    if let Some(cover_url) = room
        .cover
        .as_ref()
        .and_then(|cover| cover.url_list.as_ref())
        .and_then(|urls| urls.first())
    {
        result.add_content(Content::Image(ImageContent::new().with_url(cover_url)));
    }

    if let Some(count) = room.user_count {
        result.add_extra("在线观看", format!("{count}人"));
    }
    result.add_extra("类型", "直播");

    result
}

/// CDN play ratio URLs are optimistically built with; the async layer probes
/// the CDN afterwards and rewrites to a lower ratio when 1080p is not served.
pub(super) const DEFAULT_PLAY_RATIO: &str = "1080p";

/// Builds a Toutiao CDN play URL for a `video_id` (play_addr uri) at `ratio`.
pub(super) fn cdn_play_url(video_id: &str, ratio: &str) -> String {
    format!(
        "{}?video_id={}&ratio={}&line=0",
        super::CDN_VIDEO_PLAY,
        video_id,
        ratio
    )
}

fn assemble_post_result(
    desc: &str,
    create_time: Option<i64>,
    author: Author,
    contents: Vec<Content>,
) -> ParseResult {
    let (title, text) = split_desc(desc);
    let mut result = ParseResult::new(Platform::Douyin)
        .with_author(author)
        .with_contents(contents);
    // A missing or zero create_time stays unknown rather than becoming 1970.
    if let Some(create_time) = create_time.filter(|&ts| ts > 0) {
        result = result.with_timestamp(create_time);
    }
    if let Some(title) = title {
        result = result.with_title(title);
    }
    if let Some(text) = text {
        result = result.with_text(text);
    }
    result
}

fn build_aweme_image_contents(item: &AwemeDetail) -> Vec<Content> {
    let bgm_url = item.music.as_ref().and_then(aweme_music_url);

    let mut contents = Vec::new();
    if let Some(images) = &item.images {
        for image in images {
            let live_photo = !matches!(image.clip_type, None | Some(2));
            let image_video = live_photo.then_some(image.video.as_ref()).flatten();
            if let Some((image_video, video_url)) = image_video.and_then(|video| {
                video
                    .play_addr_h264
                    .as_ref()
                    .and_then(play_address_url)
                    .or_else(|| video.play_addr.as_ref().and_then(play_address_url))
                    .map(|url| (video, url))
            }) {
                let cover = image_video
                    .cover_original_scale
                    .as_ref()
                    .or(image_video.cover.as_ref())
                    .and_then(|cover| cover.url_list.as_ref())
                    .and_then(|urls| urls.first())
                    .or_else(|| image.url_list.as_ref().and_then(|urls| urls.first()));
                let mut video = VideoContent::new().with_url(&video_url);
                if let Some(cover) = cover {
                    video = video.with_cover(cover);
                }
                if let Some(duration) = image_video.duration {
                    video = video.with_duration((duration / 1000).max(0) as u64);
                }
                if let Some(bgm_url) = bgm_url.as_deref() {
                    video.audio_url = Some(bgm_url.to_owned());
                }
                contents.push(Content::Video(video));
            } else if let Some(url) = image
                .url_list
                .as_ref()
                .and_then(|urls| urls.iter().find(|url| !url.trim().is_empty()))
            {
                contents.push(Content::Image(ImageContent::new().with_url(url)));
            }
        }
    }

    if let Some(bgm_url) = bgm_url.as_deref()
        && let Some(music) = item.music.as_ref()
    {
        let title = music
            .matched_pgc_sound
            .as_ref()
            .and_then(|sound| sound.title.as_deref())
            .or(music.title.as_deref())
            .unwrap_or("抖音BGM");
        let artist = music
            .matched_pgc_sound
            .as_ref()
            .and_then(|sound| sound.author.as_deref())
            .or(music.author.as_deref());

        let mut audio = AudioContent::new().with_url(bgm_url).with_title(title);
        if let Some(artist) = artist {
            audio = audio.with_artist(artist);
        }
        if let Some(duration) = music.duration {
            audio = audio.with_duration(duration);
        }
        contents.push(Content::Audio(audio));
    }

    contents
}

/// Select a playable non-original soundtrack URL from the current API shape.
/// Douyin sometimes leaves `play_url.uri` empty and stores the signed URL in
/// the JSON-encoded `extra.original_song_url` field instead.
fn aweme_music_url(music: &ApiMusic) -> Option<String> {
    if music.is_original_sound.unwrap_or(false) {
        return None;
    }

    let direct = music.play_url.as_ref().and_then(|play| {
        play.url_list
            .as_ref()
            .and_then(|urls| urls.iter().find(|url| !url.trim().is_empty()))
            .or_else(|| play.uri.as_ref().filter(|uri| !uri.trim().is_empty()))
    });
    direct
        .cloned()
        .or_else(|| {
            let extra = music.extra.as_deref()?;
            serde_json::from_str::<serde_json::Value>(extra)
                .ok()?
                .get("original_song_url")?
                .as_str()
                .map(str::to_owned)
        })
        .filter(|url| url.starts_with("http://") || url.starts_with("https://"))
}

fn play_address_url(play: &PlayAddr) -> Option<String> {
    if let Some(uri) = play
        .uri
        .as_deref()
        .map(str::trim)
        .filter(|uri| !uri.is_empty())
    {
        return Some(cdn_play_url(uri, DEFAULT_PLAY_RATIO));
    }
    play.url_list.as_ref()?.iter().find_map(|value| {
        let value = value.trim();
        let url = url::Url::parse(value).ok()?;
        (matches!(url.scheme(), "http" | "https") && url.has_host()).then(|| value.to_owned())
    })
}

fn build_aweme_video_contents(item: &AwemeDetail) -> Vec<Content> {
    let mut contents = Vec::new();
    if let Some(video) = &item.video
        && let Some(video_url) = video.play_addr.as_ref().and_then(play_address_url)
    {
        let cover = video
            .cover_original_scale
            .as_ref()
            .or(video.cover.as_ref())
            .and_then(|cover| cover.url_list.as_ref())
            .and_then(|urls| urls.first());
        let duration = video.duration.map(|duration| (duration / 1000) as u64);

        let mut video = VideoContent::new().with_url(&video_url);
        if let Some(cover) = cover {
            video = video.with_cover(cover);
        }
        if let Some(duration) = duration {
            video = video.with_duration(duration);
        }
        contents.push(Content::Video(video));
    }
    contents
}

fn build_router_item_contents(item: &VideoItem) -> Vec<Content> {
    let mut contents = Vec::new();

    if let Some(images) = &item.images {
        contents.extend(
            images
                .iter()
                .filter_map(|image| image.url_list.as_ref().and_then(|urls| urls.first()))
                .map(|url| Content::Image(ImageContent::new().with_url(url))),
        );
    }

    if contents.is_empty()
        && let Some(video_url) = item
            .video
            .as_ref()
            .and_then(|video| video.play_addr.as_ref())
            .and_then(|play| play.url_list.as_ref())
            .and_then(|urls| urls.first())
    {
        let video_url = video_url.replace("playwm", "play");
        let cover_url = item
            .video
            .as_ref()
            .and_then(|video| video.cover.as_ref())
            .and_then(|cover| cover.url_list.as_ref())
            .and_then(|urls| urls.first());
        let duration = item
            .video
            .as_ref()
            .and_then(|video| video.duration)
            .map(|duration| (duration / 1000) as u64);

        let mut video = VideoContent::new().with_url(video_url);
        if let Some(cover) = cover_url {
            video = video.with_cover(cover);
        }
        if let Some(duration) = duration {
            video = video.with_duration(duration);
        }
        contents.push(Content::Video(video));
    }

    contents
}

fn build_slide_contents(detail: &SlideDetail) -> Vec<Content> {
    let mut contents = Vec::new();

    if let Some(images) = &detail.images {
        for slide in images {
            if let Some(video) = &slide.video
                && let Some(play_url) = video
                    .play_addr
                    .as_ref()
                    .and_then(|play| play.url_list.as_ref())
                    .and_then(|urls| urls.first())
            {
                let cover_url = slide.url_list.as_ref().and_then(|urls| urls.first());
                let mut video = VideoContent::new().with_url(play_url);
                if let Some(cover) = cover_url {
                    video = video.with_cover(cover);
                }
                contents.push(Content::Video(video));
                continue;
            }

            if let Some(url) = slide.url_list.as_ref().and_then(|urls| urls.first()) {
                contents.push(Content::Image(ImageContent::new().with_url(url)));
            }
        }
    }

    contents
}

#[cfg(test)]
mod tests {
    use super::super::api::ApiResponse;
    use super::*;

    #[test]
    fn aweme_image_result_keeps_static_and_animated_slides_plus_bgm() {
        let json = r#"{
            "aweme_detail": {
                "aweme_type": 0,
                "desc": "标题\n正文",
                "create_time": 123,
                "share_url": "https://www.douyin.com/note/123?from=share",
                "author": {
                    "uid": "author-id",
                    "nickname": "作者",
                    "signature": "作者签名",
                    "avatar_thumb": {"url_list": ["avatar.jpg"]}
                },
                "images": [
                    {"clip_type": 2, "url_list": ["image-1.jpg"]},
                    {
                        "clip_type": 4,
                        "url_list": ["cover.jpg"],
                        "video": {
                            "play_addr": {"uri": "slide-video-uri"},
                            "cover": {"url_list": ["video-cover.jpg"]},
                            "duration": 9000
                        }
                    },
                    {
                        "clip_type": 4,
                        "url_list": ["h264-cover.jpg"],
                        "video": {"play_addr_h264": {"uri": "h264-only-uri"}}
                    }
                ],
                "music": {
                    "title": "BGM",
                    "author": "歌手",
                    "duration": 60,
                    "is_original_sound": false,
                    "play_url": {"uri": ""},
                    "extra": "{\"original_song_url\":\"https://audio.example/bgm.mp3\"}"
                }
            }
        }"#;
        let data: ApiResponse = serde_json::from_str(json).unwrap();
        let detail = data.aweme_detail.unwrap();

        let result = build_aweme_result(&detail);
        assert_eq!(result.contents.len(), 4);
        assert!(matches!(result.contents[0], Content::Image(_)));

        let Content::Video(video) = &result.contents[1] else {
            panic!("expected animated slide video");
        };
        assert!(video.url.as_deref().unwrap().contains("slide-video-uri"));
        assert_eq!(video.cover.as_deref(), Some("video-cover.jpg"));
        assert_eq!(video.duration, Some(9));
        assert_eq!(
            video.audio_url.as_deref(),
            Some("https://audio.example/bgm.mp3")
        );

        // A live photo with only an H.264 stream, and the image as its cover.
        let Content::Video(h264) = &result.contents[2] else {
            panic!("expected the H.264-only live photo");
        };
        assert!(h264.url.as_deref().unwrap().contains("h264-only-uri"));
        assert_eq!(h264.cover.as_deref(), Some("h264-cover.jpg"));

        let Content::Audio(audio) = &result.contents[3] else {
            panic!("expected bgm audio");
        };
        assert_eq!(audio.title.as_deref(), Some("BGM"));
        assert_eq!(audio.artist.as_deref(), Some("歌手"));
        assert_eq!(audio.duration, Some(60));
    }
}
