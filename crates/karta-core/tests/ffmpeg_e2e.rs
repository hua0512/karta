//! ffmpeg end to end. Fake `ffmpeg`/`ffprobe` scripts cover the failure and
//! size-cap paths in the default suite; the `real_ffmpeg_*` tests run the
//! installed binaries (`#[ignore]`, CI step `Test (external tools)`).
#![cfg(unix)]

use std::path::{Path, PathBuf};

use karta_core::cache::FileCache;
use karta_core::data::{Content, VideoContent};
use karta_core::download::StreamDownloader;
use karta_core::message::{RenderOutput, SegmentType};
use karta_core::{DownloadError, ParseResult, Platform};
use serde_json::json;
use test_harness::tools::{FakeTool, InstalledTool, OutputArg, require_tool};
use test_harness::{FakeUpstream, Reply, TempApp};

const CDN: &str = "upos.bilivideo.com";

/// Serve `video` and `audio` at `/v.m4s` and `/a.m4s`.
fn serve_streams(fake: &FakeUpstream, video: Vec<u8>, audio: Vec<u8>) {
    fake.route(CDN, "GET", "/v.m4s")
        .reply(Reply::ok().body(video));
    fake.route(CDN, "GET", "/a.m4s")
        .reply(Reply::ok().body(audio));
}

fn stream_url(path: &str) -> String {
    format!("https://{CDN}{path}")
}

/// A text-render app whose ffmpeg (and the ffprobe beside it) are `ffmpeg`.
fn app_with_ffmpeg(ffmpeg: &InstalledTool, extra: serde_json::Value) -> TempApp {
    let mut overrides = json!({
        "render_type": "text",
        "download_media": true,
        "forward_contents": false,
        "forward_text_threshold": 0,
        "forward_media_threshold": 0,
        "ffmpeg_path": ffmpeg.path_string(),
    });
    overrides
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    TempApp::new(overrides)
}

/// The single non-header segment of a text render.
fn media_segment(outputs: &[RenderOutput]) -> &karta_core::MessageSegment {
    assert_eq!(outputs.len(), 2, "{outputs:?}");
    &outputs[1].messages()[0].segments[0]
}

#[tokio::test]
async fn merged_dash_streams_are_sent_as_one_video() {
    let fake = FakeUpstream::start().await;
    serve_streams(&fake, b"video".to_vec(), b"audio".to_vec());
    let tools = tempfile::tempdir().unwrap();
    let ffmpeg = FakeTool::new()
        .writes(OutputArg::Last, "merged")
        .install(tools.path(), "ffmpeg");
    let app = app_with_ffmpeg(&ffmpeg, json!({}));
    let result = ParseResult::new(Platform::Bilibili).with_content(Content::Video(
        VideoContent::new().with_streams(stream_url("/v.m4s"), stream_url("/a.m4s")),
    ));
    let engine = app.engine(fake.downloader(app.config(), app.data_path()), vec![]);

    let outputs = engine.render(&result).await.unwrap();

    let segment = media_segment(&outputs);
    assert_eq!(segment.segment_type, SegmentType::Video);
    let merged = segment.file_path.as_ref().expect("a merged file");
    assert_eq!(std::fs::read(merged).unwrap(), b"merged");

    // ffmpeg read both downloaded streams.
    let [argv] = ffmpeg.invocations().try_into().expect("one ffmpeg run");
    let inputs: Vec<String> = argv
        .windows(2)
        .filter(|pair| pair[0] == "-i")
        .map(|pair| std::fs::read_to_string(&pair[1]).unwrap())
        .collect();
    assert_eq!(inputs, ["video", "audio"]);
}

#[tokio::test]
async fn failed_merge_degrades_to_the_cover_and_leaves_no_partial_file() {
    let fake = FakeUpstream::start().await;
    serve_streams(&fake, b"video".to_vec(), b"audio".to_vec());
    let tools = tempfile::tempdir().unwrap();
    // Writes a partial output, then fails like a crashed ffmpeg.
    let ffmpeg = FakeTool::new()
        .writes(OutputArg::Last, "partial")
        .stderr("Invalid data found when processing input")
        .exit_code(1)
        .install(tools.path(), "ffmpeg");
    let app = app_with_ffmpeg(&ffmpeg, json!({}));
    let cover = "https://i0.hdslb.com/cover.jpg";
    let result = ParseResult::new(Platform::Bilibili).with_content(Content::Video(
        VideoContent::new()
            .with_streams(stream_url("/v.m4s"), stream_url("/a.m4s"))
            .with_cover(cover),
    ));
    let downloader = fake.downloader(app.config(), app.data_path());
    let engine = app.engine(downloader.clone(), vec![]);

    let outputs = engine.render(&result).await.unwrap();

    let segment = media_segment(&outputs);
    assert_eq!(segment.segment_type, SegmentType::Image);
    assert_eq!(segment.url.as_deref(), Some(cover));
    // Only the two downloaded inputs remain: no merged file, no `.part`.
    assert_eq!(app.cache_entries().len(), 2);

    let error = downloader
        .download_and_merge(&stream_url("/v.m4s"), &stream_url("/a.m4s"), None)
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Invalid data found when processing input"),
        "the error carries ffmpeg's stderr: {error}"
    );
}

#[tokio::test]
async fn merged_output_over_the_size_cap_is_rejected() {
    const MIB: usize = 1024 * 1024;
    let fake = FakeUpstream::start().await;
    serve_streams(&fake, b"video".to_vec(), b"audio".to_vec());
    let tools = tempfile::tempdir().unwrap();
    // Small inputs, but the merged file is one byte over the 1 MiB cap.
    let ffmpeg = FakeTool::new()
        .writes(OutputArg::Last, vec![0; MIB + 1])
        .install(tools.path(), "ffmpeg");
    let app = app_with_ffmpeg(&ffmpeg, json!({ "max_size": 1 }));
    let downloader = fake.downloader(app.config(), app.data_path());

    let error = downloader
        .download_and_merge(&stream_url("/v.m4s"), &stream_url("/a.m4s"), None)
        .await
        .unwrap_err();

    assert!(
        matches!(
            error,
            DownloadError::FileTooLarge { size, limit }
                if size == MIB as u64 + 1 && limit == MIB as u64
        ),
        "{error:?}"
    );
    assert_eq!(app.cache_entries().len(), 2, "inputs only");
}

// ── Real ffmpeg ──────────────────────────────────────────────────────────

fn real_tools_available() -> bool {
    require_tool("ffmpeg") && require_tool("ffprobe")
}

/// Generate a media file with `ffmpeg -f lavfi`.
fn generate(dir: &Path, name: &str, args: &[&str]) -> Vec<u8> {
    let path = dir.join(name);
    let status = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-y"])
        .args(args)
        .arg(&path)
        .status()
        .expect("run ffmpeg");
    assert!(status.success(), "generating {name}");
    std::fs::read(path).unwrap()
}

/// `(duration in seconds, has video, has audio)` per ffprobe.
fn probe(path: &Path) -> (f64, bool, bool) {
    let output = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration:stream=codec_type",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(path)
        .output()
        .expect("run ffprobe");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    let duration = lines
        .iter()
        .find_map(|line| line.parse::<f64>().ok())
        .expect("a duration");
    (duration, lines.contains(&"video"), lines.contains(&"audio"))
}

fn real_app(extra: serde_json::Value) -> TempApp {
    let mut overrides = json!({ "render_type": "text", "download_media": true });
    overrides
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    TempApp::new(overrides)
}

/// lavfi sources: a test pattern and a tone.
const PICTURE: &str = "testsrc=size=64x64:rate=10";
const TONE: &str = "sine=frequency=440";

#[tokio::test]
#[ignore = "needs ffmpeg and ffprobe; CI step Test (external tools)"]
async fn real_ffmpeg_merges_dash_streams() {
    if !real_tools_available() {
        return;
    }
    let media = tempfile::tempdir().unwrap();
    let video = generate(
        media.path(),
        "v.mp4",
        &["-f", "lavfi", "-i", PICTURE, "-t", "2", "-c:v", "mpeg4"],
    );
    let audio = generate(
        media.path(),
        "a.m4a",
        &["-f", "lavfi", "-i", TONE, "-t", "2", "-c:a", "aac"],
    );
    let fake = FakeUpstream::start().await;
    serve_streams(&fake, video, audio);
    let app = real_app(json!({}));
    let downloader = fake.downloader(app.config(), app.data_path());

    let merged = downloader
        .download_and_merge(&stream_url("/v.m4s"), &stream_url("/a.m4s"), None)
        .await
        .unwrap();

    let (duration, has_video, has_audio) = probe(&merged);
    assert!(has_video && has_audio, "{merged:?}");
    assert!((duration - 2.0).abs() < 0.5, "duration {duration}");
}

#[tokio::test]
#[ignore = "needs ffmpeg and ffprobe; CI step Test (external tools)"]
async fn real_ffmpeg_loops_a_short_bgm_over_the_whole_video() {
    if !real_tools_available() {
        return;
    }
    let media = tempfile::tempdir().unwrap();
    let cases = [
        (
            "video with its own audio",
            generate(
                media.path(),
                "loud.mp4",
                &[
                    "-f", "lavfi", "-i", PICTURE, "-f", "lavfi", "-i", TONE, "-t", "4", "-c:v",
                    "mpeg4", "-c:a", "aac",
                ],
            ),
        ),
        (
            "silent video",
            generate(
                media.path(),
                "silent.mp4",
                &["-f", "lavfi", "-i", PICTURE, "-t", "4", "-c:v", "mpeg4"],
            ),
        ),
    ];
    let bgm = generate(
        media.path(),
        "bgm.m4a",
        &["-f", "lavfi", "-i", TONE, "-t", "1", "-c:a", "aac"],
    );

    for (case, video) in cases {
        let fake = FakeUpstream::start().await;
        fake.route(CDN, "GET", "/clip.mp4")
            .reply(Reply::ok().body(video));
        fake.route(CDN, "GET", "/bgm.m4a")
            .reply(Reply::ok().body(bgm.clone()));
        let app = real_app(json!({}));
        let downloader = fake.downloader(app.config(), app.data_path());

        let merged = downloader
            .download_and_merge_bgm(&stream_url("/clip.mp4"), &stream_url("/bgm.m4a"), None)
            .await
            .unwrap_or_else(|e| panic!("{case}: {e}"));

        // The 1 s BGM loops: the output keeps the video's 4 s, with audio.
        let (duration, has_video, has_audio) = probe(&merged);
        assert!(has_video && has_audio, "{case}");
        assert!((duration - 4.0).abs() < 0.5, "{case}: duration {duration}");
    }
}

#[tokio::test]
#[ignore = "needs ffmpeg and ffprobe; CI step Test (external tools)"]
async fn real_ffmpeg_gif_conversion_honours_the_duration_cap() {
    if !real_tools_available() {
        return;
    }
    let media = tempfile::tempdir().unwrap();
    let clip = generate(
        media.path(),
        "clip.mp4",
        &["-f", "lavfi", "-i", PICTURE, "-t", "3", "-c:v", "mpeg4"],
    );
    let fake = FakeUpstream::start().await;
    fake.route(CDN, "GET", "/clip.mp4")
        .reply(Reply::ok().body(clip));
    let app = real_app(json!({}));
    let downloader = fake.downloader(app.config(), app.data_path());
    let url = stream_url("/clip.mp4");

    let error = downloader
        .download_as_gif(&url, None, Some(1), None, None)
        .await
        .unwrap_err();
    assert!(
        matches!(
            error,
            DownloadError::DurationTooLong {
                duration: 3,
                limit: 1
            }
        ),
        "{error:?}"
    );

    let gif: PathBuf = downloader
        .download_as_gif(&url, None, Some(10), None, None)
        .await
        .unwrap();
    assert!(std::fs::read(&gif).unwrap().starts_with(b"GIF8"));
    // No palette or `.part` file is left next to the cached GIF.
    let leftovers: Vec<String> = app
        .cache_entries()
        .into_iter()
        .filter(|name| name.ends_with(".png") || name.ends_with(".part"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

fn seed_download(downloader: &StreamDownloader, url: &str, extension: &str) -> PathBuf {
    let path = downloader
        .file_cache()
        .get_path(&FileCache::key_from_url(url), extension);
    std::fs::write(&path, url).unwrap();
    path
}

#[tokio::test]
async fn ffmpeg_inputs_stay_cached_for_other_requests() {
    let temp_dir = tempfile::tempdir().unwrap();
    let ffmpeg = FakeTool::new()
        .writes(OutputArg::Last, "converted")
        .requires_files_after("-i")
        .install(temp_dir.path(), "ffmpeg");
    let app = app_with_ffmpeg(&ffmpeg, json!({}));
    let downloader =
        StreamDownloader::with_client(app.config(), app.data_path(), wreq::Client::new()).unwrap();

    // Port 1 refuses connections: a removed input cannot be re-downloaded.
    let video = "http://127.0.0.1:1/video.m4s";
    let video_input = seed_download(&downloader, video, "m4v");
    let audio_inputs = ["http://127.0.0.1:1/hi.m4a", "http://127.0.0.1:1/lo.m4a"]
        .map(|audio| (audio, seed_download(&downloader, audio, "m4a")));

    for (audio, audio_input) in &audio_inputs {
        let merged = downloader
            .download_and_merge(video, audio, None)
            .await
            .expect("merge should find both inputs");
        assert_eq!(std::fs::read(&merged).unwrap(), b"converted");
        assert!(video_input.exists(), "merge removed the shared video input");
        assert!(audio_input.exists(), "merge removed the audio input");
    }

    // GIF conversion and BGM mixing download the clip under the same key.
    let clip = "http://127.0.0.1:1/clip.mp4";
    let clip_input = seed_download(&downloader, clip, "mp4");
    let bgm = "http://127.0.0.1:1/bgm.mp3";
    let bgm_input = seed_download(&downloader, bgm, "mp3");

    downloader
        .download_as_gif(clip, None, None, None, None)
        .await
        .expect("GIF conversion should find its input");
    assert!(
        clip_input.exists(),
        "GIF conversion removed the shared clip"
    );

    downloader
        .download_and_merge_bgm(clip, bgm, None)
        .await
        .expect("BGM mixing should still find the clip");
    assert!(clip_input.exists(), "BGM mixing removed the shared clip");
    assert!(bgm_input.exists(), "BGM mixing removed the BGM input");
}
