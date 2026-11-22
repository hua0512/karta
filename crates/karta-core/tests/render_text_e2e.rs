//! Text renderer end to end: a fake parser's result rendered by the engine,
//! media downloaded from a fake CDN.

use std::path::Path;
use std::sync::Arc;

use karta_core::data::{AudioContent, Author, Content, ImageContent, VideoContent};
use karta_core::message::{MessageSegment, RenderOutput, SegmentType};
use karta_core::{Config, DownloadError, ParseResult, Platform};
use serde_json::json;
use test_harness::{FakeParser, FakeUpstream, Reply, TempApp};

const LINK: &str = "https://www.bilibili.com/video/BV1xx411c7mD";
const REPOST_LINK: &str = "https://www.douyin.com/video/7300000000000000000";

/// Refuses connections: an IP literal with a port bypasses the fake
/// upstream's resolver.
const REFUSED: &str = "http://127.0.0.1:1";

fn bilibili_parser() -> FakeParser {
    FakeParser::new(Platform::Bilibili, &[("bv", r"BV[0-9A-Za-z]{10}")])
}

/// Render settings that keep every output a standalone message.
fn text_app(extra: serde_json::Value) -> TempApp {
    let mut overrides = json!({
        "render_type": "text",
        "download_media": true,
        "append_url": true,
        "forward_contents": false,
        "forward_text_threshold": 0,
        "forward_media_threshold": 0,
    });
    overrides
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    TempApp::new(overrides)
}

/// One line per segment: `<kind> <text | file body | url>`. File segments
/// must live in the app's download cache.
fn describe(segment: &MessageSegment, data_path: &Path) -> String {
    let kind = match segment.segment_type {
        SegmentType::Text => return format!("text {}", segment.text.as_deref().unwrap()),
        SegmentType::Image => "image",
        SegmentType::Video => "video",
        SegmentType::Audio => "audio",
        ref other => panic!("unexpected segment type {other:?}"),
    };
    if let Some(path) = &segment.file_path {
        assert!(
            path.starts_with(Config::cache_dir(data_path)),
            "{path:?} is outside the download cache"
        );
        let body = std::fs::read(path).unwrap();
        return format!("{kind} file {}", String::from_utf8_lossy(&body));
    }
    format!("{kind} url {}", segment.url.as_deref().unwrap())
}

/// Every output as one string; a bundle lists its messages in brackets.
fn describe_outputs(outputs: &[RenderOutput], data_path: &Path) -> Vec<String> {
    outputs
        .iter()
        .map(|output| {
            let messages: Vec<String> = output
                .messages()
                .iter()
                .map(|message| {
                    message
                        .segments
                        .iter()
                        .map(|segment| describe(segment, data_path))
                        .collect::<Vec<_>>()
                        .join(" + ")
                })
                .collect();
            match output {
                RenderOutput::Message(_) => messages.join(""),
                RenderOutput::Bundle(_) => format!("bundle [{}]", messages.join(" | ")),
            }
        })
        .collect()
}

#[tokio::test]
async fn oversized_video_notice_stays_outside_the_forward_bundle() {
    let fake = FakeUpstream::start().await;
    fake.route("cdn.example", "GET", "/large.mp4")
        .reply(Reply::ok().body(vec![0; 1024 * 1024 + 1]));
    let app = text_app(json!({ "max_size": 1, "forward_contents": true }));
    let engine = app.engine(fake.downloader(app.config(), app.data_path()), vec![]);
    let result = ParseResult::new(Platform::Douyin)
        .with_title("large video")
        .with_content(Content::Video(
            VideoContent::new().with_url("https://cdn.example/large.mp4"),
        ));

    let outputs = engine.render(&result).await.unwrap();
    assert_eq!(outputs.len(), 2);
    assert!(
        outputs
            .iter()
            .all(|output| matches!(output, RenderOutput::Message(_)))
    );
    let [notice] = outputs[1].messages()[0].segments.as_slice() else {
        panic!("expected one notice");
    };
    assert!(notice.is_failure_notice);
    assert_eq!(notice.segment_type, SegmentType::Text);
    assert!(notice.text.as_deref().unwrap().contains("上限 1 MiB"));
    fake.assert_no_unmatched();
}

#[tokio::test]
async fn text_render_downloads_media_and_degrades_failures() {
    let fake = FakeUpstream::start().await;
    let cdn = "i0.hdslb.com";
    let image = fake
        .route(cdn, "GET", "/bfs/a.jpg")
        .reply(Reply::ok().content_type("image/jpeg").body("image-a"));
    fake.route(cdn, "GET", "/bfs/gone.jpg")
        .reply(Reply::status(404));
    fake.route(cdn, "GET", "/bfs/empty.jpg").reply(Reply::ok());
    fake.route(cdn, "GET", "/bfs/cut.jpg")
        .reply(Reply::ok().body("abc").truncated(10));
    fake.route("upos.bilivideo.com", "GET", "/audio.m4a")
        .reply(Reply::ok().body("audio"));
    let backup = fake
        .route("upos-backup.bilivideo.com", "GET", "/video.mp4")
        .reply(Reply::ok().body("video"));
    fake.route("upos.bilivideo.com", "GET", "/dead.mp4")
        .reply(Reply::status(503));
    let repost_image = fake
        .route("p3.douyinpic.com", "GET", "/r.jpg")
        .reply(Reply::ok().body("image-r"));

    let app = text_app(json!({}));
    let result = ParseResult::new(Platform::Bilibili)
        .with_author(Author::new("UP主"))
        .with_title("标题")
        .with_text("正文")
        .with_url(LINK)
        .with_extra("播放", "100")
        .with_content(Content::Image(
            ImageContent::new().with_url(format!("https://{cdn}/bfs/a.jpg")),
        ))
        .with_content(Content::Image(
            ImageContent::new().with_url(format!("https://{cdn}/bfs/gone.jpg")),
        ))
        .with_content(Content::Image(
            ImageContent::new().with_url(format!("https://{cdn}/bfs/empty.jpg")),
        ))
        .with_content(Content::Image(
            ImageContent::new().with_url(format!("https://{cdn}/bfs/cut.jpg")),
        ))
        .with_content(Content::Audio(
            AudioContent::new().with_url("https://upos.bilivideo.com/audio.m4a"),
        ))
        .with_content(Content::Video(
            VideoContent::new()
                .with_url_candidates(vec![
                    format!("{REFUSED}/video.mp4"),
                    "https://upos-backup.bilivideo.com/video.mp4".to_string(),
                ])
                .with_download_headers(vec![("User-Agent".into(), "app-agent/1.0".into())]),
        ))
        .with_content(Content::Video(
            VideoContent::new()
                .with_url_candidates(vec![
                    format!("{REFUSED}/dead.mp4"),
                    "https://upos.bilivideo.com/dead.mp4".to_string(),
                ])
                .with_cover(format!("https://{cdn}/bfs/cover.jpg")),
        ))
        .with_repost(
            ParseResult::new(Platform::Douyin)
                .with_author(Author::new("原作者"))
                .with_title("原标题")
                .with_url(REPOST_LINK)
                .with_content(Content::Image(
                    ImageContent::new().with_url("https://p3.douyinpic.com/r.jpg"),
                )),
        );
    let parser = Arc::new(bilibili_parser().returning(result));
    let engine = app.engine(fake.downloader(app.config(), app.data_path()), vec![parser]);

    let outputs = engine
        .parse_and_render(&format!("看看 {LINK}"))
        .await
        .unwrap()
        .expect("link matches the fake parser");

    let header = format!(
        "text [Bilibili] @UP主 | 标题\n播放: 100\n链接: {LINK}\n\n转发自:\n  [抖音] @原作者 | 原标题\n  链接: {REPOST_LINK}"
    );
    assert_eq!(
        describe_outputs(&outputs, app.data_path()),
        [
            header.as_str(),
            // Top-level text goes first when no content carries text.
            "text 正文",
            "image file image-a",
            // Failed image downloads degrade to the remote URL.
            &format!("image url https://{cdn}/bfs/gone.jpg"),
            &format!("image url https://{cdn}/bfs/empty.jpg"),
            &format!("image url https://{cdn}/bfs/cut.jpg"),
            "audio file audio",
            // The refused primary falls back to the backup URL.
            "video file video",
            // No candidate works: the text path shows the cover instead.
            &format!("image url https://{cdn}/bfs/cover.jpg"),
            // Repost images follow the main contents.
            "image file image-r",
        ]
    );

    // Downloads carry the platform's Referer, or the video's own headers.
    assert_eq!(
        image.last_request().header("referer"),
        Some("https://www.bilibili.com")
    );
    assert_eq!(
        repost_image.last_request().header("referer"),
        Some("https://www.douyin.com/")
    );
    assert_eq!(
        backup.last_request().header("user-agent"),
        Some("app-agent/1.0")
    );
    fake.assert_no_unmatched();

    // Only complete downloads are cached; failed ones leave no file and no
    // `.part` behind.
    let cached = app.cache_entries();
    assert_eq!(cached.len(), 4, "{cached:?}");
    assert!(
        cached.iter().all(|name| !name.ends_with(".part")),
        "{cached:?}"
    );
}

#[tokio::test]
async fn concurrent_renders_fetch_each_url_once() {
    let fake = FakeUpstream::start().await;
    let image = fake
        .route("i0.hdslb.com", "GET", "/bfs/a.jpg")
        .reply(Reply::ok().body("image-a"));
    let app = text_app(json!({}));
    let result = ParseResult::new(Platform::Bilibili).with_content(Content::Image(
        ImageContent::new().with_url("https://i0.hdslb.com/bfs/a.jpg"),
    ));
    let engine = app.engine(fake.downloader(app.config(), app.data_path()), vec![]);

    let (first, second) = tokio::join!(engine.render(&result), engine.render(&result));

    let first = describe_outputs(&first.unwrap(), app.data_path());
    let second = describe_outputs(&second.unwrap(), app.data_path());
    assert_eq!(first, second);
    assert_eq!(first[1], "image file image-a");
    assert_eq!(image.hits(), 1);
}

#[tokio::test]
async fn bundled_text_render_keeps_audio_and_video_standalone() {
    // Downloads off: media stays remote, so no CDN is involved.
    let app = text_app(json!({
        "download_media": false,
        "forward_contents": true,
        "append_url": false,
    }));
    let result = ParseResult::new(Platform::Bilibili)
        .with_title("标题")
        .with_url(LINK)
        // Ignored: the contents carry their own text.
        .with_text("摘要")
        .with_content(Content::Image(
            ImageContent::new().with_url("https://i0.hdslb.com/a.jpg"),
        ))
        .with_content(Content::Audio(
            AudioContent::new().with_url("https://upos.bilivideo.com/a.m4a"),
        ))
        .with_content(Content::Video(
            VideoContent::new().with_url("https://upos.bilivideo.com/v.mp4"),
        ))
        .with_content(Content::Text("正文".to_string()));
    let downloader = Arc::new(
        karta_core::download::StreamDownloader::new(app.config(), app.data_path()).unwrap(),
    );
    let engine = app.engine(downloader, vec![]);

    let outputs = engine.render(&result).await.unwrap();

    assert_eq!(
        describe_outputs(&outputs, app.data_path()),
        [
            "bundle [text [Bilibili] | 标题 | image url https://i0.hdslb.com/a.jpg | text 正文]",
            "audio url https://upos.bilivideo.com/a.m4a",
            "video url https://upos.bilivideo.com/v.mp4",
        ]
    );
}

#[tokio::test]
async fn partial_206_responses_validate_completeness_and_fall_back() {
    const MIB: usize = 1024 * 1024;
    let fake = FakeUpstream::start().await;
    let cdn = "upos.bilivideo.com";

    fake.route(cdn, "GET", "/complete.mp4").reply(
        Reply::status(206)
            .header("content-range", "bytes 0-4/5")
            .body("video"),
    );
    fake.route(cdn, "GET", "/partial.mp4").reply(
        Reply::status(206)
            .header("content-range", "bytes 0-1/5")
            .body("vi"),
    );
    fake.route(cdn, "GET", "/no-range.mp4")
        .reply(Reply::status(206).body("video"));
    fake.route(cdn, "GET", "/chunked-short.mp4").reply(
        Reply::status(206)
            .header("content-range", "bytes 0-9/10")
            .body("short")
            .chunked(),
    );
    fake.route(cdn, "GET", "/oversized.mp4").reply(
        Reply::status(206)
            .header("content-range", &format!("bytes 0-{MIB}/{}", MIB + 1))
            .body(vec![b'x'; MIB + 1]),
    );
    let oversized_backup = fake
        .route(cdn, "GET", "/oversized-backup.mp4")
        .reply(Reply::ok().body("unused"));
    fake.route(cdn, "GET", "/backup.mp4").reply(
        Reply::status(206)
            .header("content-range", "bytes 0-5/6")
            .body("backup"),
    );
    fake.route(cdn, "GET", "/gzip.mp4")
        .reply(Reply::ok().body("gzipped").gzip());

    let app = text_app(json!({ "max_size": 1 }));
    let downloader = fake.downloader(app.config(), app.data_path());

    for path in ["/partial.mp4", "/no-range.mp4", "/chunked-short.mp4"] {
        let err = downloader
            .download_video(&format!("https://{cdn}{path}"), None)
            .await
            .unwrap_err();
        assert!(
            matches!(err, DownloadError::Incomplete(_)),
            "{path}: expected Incomplete, got {err:?}"
        );
    }
    assert!(app.cache_entries().is_empty(), "{:?}", app.cache_entries());

    // Size rejections on 206 headers must not retry backup candidates.
    let err = downloader
        .download_video_candidates(
            &[
                format!("https://{cdn}/oversized.mp4"),
                format!("https://{cdn}/oversized-backup.mp4"),
            ],
            None,
        )
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            DownloadError::FileTooLarge { size, limit }
                if size == MIB as u64 + 1 && limit == MIB as u64
        ),
        "{err:?}"
    );
    assert_eq!(oversized_backup.hits(), 0);
    assert!(app.cache_entries().is_empty(), "{:?}", app.cache_entries());

    let complete = downloader
        .download_video(&format!("https://{cdn}/complete.mp4"), None)
        .await
        .unwrap();
    assert_eq!(std::fs::read(&complete).unwrap(), b"video");

    let recovered = downloader
        .download_video_candidates(
            &[
                format!("https://{cdn}/partial.mp4"),
                format!("https://{cdn}/backup.mp4"),
            ],
            None,
        )
        .await
        .unwrap();
    assert_eq!(std::fs::read(&recovered).unwrap(), b"backup");

    let gzipped = downloader
        .download_video(&format!("https://{cdn}/gzip.mp4"), None)
        .await
        .unwrap();
    assert_eq!(std::fs::read(&gzipped).unwrap(), b"gzipped");

    let cached = app.cache_entries();
    assert_eq!(cached.len(), 3, "{cached:?}");
    assert!(
        cached.iter().all(|name| !name.ends_with(".part")),
        "{cached:?}"
    );
    fake.assert_no_unmatched();
}
