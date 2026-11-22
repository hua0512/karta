//! yt-dlp end to end against a fake `yt-dlp` script that answers
//! `--dump-json` and writes the `-o` target like the real tool.
#![cfg(all(feature = "ytdlp", unix))]

use std::path::Path;

use karta_core::download::{YtdlpConfig, YtdlpDownloader};
use test_harness::tools::{FakeTool, InstalledTool, OutputArg};

/// Starts with `-`: only the `--` separator keeps it from being an option.
const URL: &str = "--exec=touch pwned";

fn fake_ytdlp(dir: &Path) -> InstalledTool {
    FakeTool::new()
        .stdout(r#"{"title":"t","duration":5}"#)
        .writes(OutputArg::After("-o"), "media")
        .install(dir, "yt-dlp")
}

fn value_after<'a>(argv: &'a [String], flag: &str) -> Option<&'a str> {
    argv.windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].as_str())
}

#[tokio::test]
async fn downloads_pass_the_config_and_keep_the_url_out_of_option_parsing() {
    let dir = tempfile::tempdir().unwrap();
    let ytdlp = fake_ytdlp(dir.path());
    let cookies = dir.path().join("cookies.txt");
    let downloader = YtdlpDownloader::with_config(YtdlpConfig {
        ytdlp_path: ytdlp.path_string(),
        cookies_file: Some(cookies.clone()),
        proxy: Some("http://127.0.0.1:8080".to_string()),
        max_size: 1000,
        output_dir: dir.path().join("out"),
        ..YtdlpConfig::default()
    });

    let audio = downloader.download_audio(URL).await.unwrap();
    let video = downloader.download_video(URL).await.unwrap();
    // Already downloaded: no further yt-dlp run.
    assert_eq!(downloader.download_audio(URL).await.unwrap(), audio);

    assert_eq!(audio.extension().unwrap(), "flac");
    assert_eq!(video.extension().unwrap(), "mp4");
    for path in [&audio, &video] {
        assert!(path.starts_with(dir.path().join("out")), "{path:?}");
        assert_eq!(std::fs::read(path).unwrap(), b"media");
    }

    let runs = ytdlp.invocations();
    // One metadata lookup (cached per URL), then one run per download.
    assert_eq!(runs.len(), 3, "{runs:?}");
    assert!(runs[0].contains(&"--dump-json".to_string()));
    for argv in &runs {
        assert_eq!(&argv[argv.len() - 2..], ["--", URL], "{argv:?}");
        assert_eq!(value_after(argv, "--cookies"), cookies.to_str());
        assert_eq!(value_after(argv, "--proxy"), Some("http://127.0.0.1:8080"));
    }
    for argv in &runs[1..] {
        assert_eq!(value_after(argv, "--max-filesize"), Some("1000"));
    }
    assert_eq!(value_after(&runs[1], "--audio-format"), Some("flac"));
    assert_eq!(value_after(&runs[2], "--merge-output-format"), Some("mp4"));
}

#[tokio::test]
async fn unlimited_size_sends_no_filesize_cap() {
    let dir = tempfile::tempdir().unwrap();
    let ytdlp = fake_ytdlp(dir.path());
    let downloader = YtdlpDownloader::with_config(YtdlpConfig {
        ytdlp_path: ytdlp.path_string(),
        max_size: 0,
        output_dir: dir.path().join("out"),
        ..YtdlpConfig::default()
    });

    downloader.download_video(URL).await.unwrap();

    let runs = ytdlp.invocations();
    assert_eq!(runs.len(), 2);
    assert!(
        !runs[1].contains(&"--max-filesize".to_string()),
        "{:?}",
        runs[1]
    );
}
