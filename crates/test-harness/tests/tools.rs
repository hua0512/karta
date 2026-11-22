//! Fake tools record argv, write their scripted output and exit as told.
#![cfg(unix)]

use std::process::Command;

use test_harness::tools::{FakeTool, OutputArg, require_tool};

#[test]
fn fake_tool_records_argv_and_writes_its_output() {
    let dir = tempfile::tempdir().unwrap();
    let ffmpeg = FakeTool::new()
        .stderr("boom")
        .exit_code(3)
        .writes(OutputArg::Last, "merged")
        .install(dir.path(), "ffmpeg");
    let out = dir.path().join("out file.mp4");

    let output = Command::new(ffmpeg.path())
        .args(["-i", "in put.mp4", "-f", "mp4"])
        .arg(&out)
        .output()
        .unwrap();
    // Version checks write nothing.
    Command::new(ffmpeg.path())
        .arg("-version")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(3));
    assert_eq!(output.stderr, b"boom");
    assert_eq!(std::fs::read(&out).unwrap(), b"merged");
    assert_eq!(
        ffmpeg.invocations(),
        [
            vec![
                "-i".to_string(),
                "in put.mp4".to_string(),
                "-f".to_string(),
                "mp4".to_string(),
                out.to_string_lossy().into_owned(),
            ],
            vec!["-version".to_string()],
        ]
    );
    assert!(!dir.path().join("-version").exists());
}

#[test]
fn fake_ytdlp_expands_the_extension_placeholder() {
    let dir = tempfile::tempdir().unwrap();
    let ytdlp = FakeTool::new()
        .stdout("{}")
        .writes(OutputArg::After("-o"), "audio")
        .install(dir.path(), "yt-dlp");
    let template = dir.path().join("key.%(ext)s");

    let output = Command::new(ytdlp.path())
        .arg("-o")
        .arg(&template)
        .args([
            "-x",
            "--audio-format",
            "flac",
            "--",
            "https://example.test/v",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(output.stdout, b"{}");
    assert_eq!(
        std::fs::read(dir.path().join("key.flac")).unwrap(),
        b"audio"
    );
    assert_eq!(ytdlp.invocations().len(), 1);
}

#[test]
fn require_tool_finds_tools_on_path() {
    assert!(require_tool("sh"));
    if std::env::var_os(test_harness::tools::REQUIRE_TOOLS_ENV).is_none() {
        assert!(!require_tool("surely-not-an-installed-tool"));
    }
}
