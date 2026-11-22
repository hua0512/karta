use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

use tokio::process::Command;

use crate::cache::cache_key;
use crate::error::{DownloadError, DownloadResult};

use super::{VideoInfo, YtdlpDownloader};

/// Generous upper bound for one yt-dlp invocation: fetching a
/// `max_duration`-length video at the `--max-filesize` cap over a slow link,
/// plus ffmpeg merge/extract post-processing.
const YTDLP_TIMEOUT: Duration = Duration::from_secs(15 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DownloadKind {
    Video,
    Audio,
}

impl DownloadKind {
    fn output_extension(self) -> &'static str {
        match self {
            Self::Video => "mp4",
            Self::Audio => "flac",
        }
    }

    fn template_extension(self) -> &'static str {
        match self {
            Self::Video => "mp4",
            Self::Audio => "%(ext)s",
        }
    }

    fn add_format_args(self, cmd: &mut Command) {
        match self {
            Self::Video => {
                cmd.arg("-f")
                    .arg("bv*+ba/b")
                    .arg("--merge-output-format")
                    .arg("mp4");
            }
            Self::Audio => {
                cmd.arg("-f")
                    .arg("bestaudio/best")
                    .arg("-x")
                    .arg("--audio-format")
                    .arg("flac")
                    .arg("--audio-quality")
                    .arg("0");
            }
        }
    }

    fn failure_context(self) -> &'static str {
        match self {
            Self::Video => "yt-dlp download failed",
            Self::Audio => "yt-dlp audio extraction failed",
        }
    }
}

impl YtdlpDownloader {
    fn add_cookie_proxy_args(&self, cmd: &mut Command) {
        if let Some(cookies) = &self.config.cookies_file {
            cmd.arg("--cookies").arg(cookies);
        }

        if let Some(proxy) = &self.config.proxy {
            cmd.arg("--proxy").arg(proxy);
        }
    }

    fn add_quiet_args(cmd: &mut Command) {
        cmd.arg("--no-warnings").arg("--quiet");
    }

    pub(super) fn build_info_command(&self, url: &str) -> Command {
        let mut cmd = Command::new(&self.config.ytdlp_path);
        cmd.arg("--dump-json").arg("--no-download");
        self.add_cookie_proxy_args(&mut cmd);
        Self::add_quiet_args(&mut cmd);
        // `--` so an attacker-shaped URL can never be parsed as an option
        cmd.arg("--").arg(url);
        cmd
    }

    fn output_path(&self, url: &str, kind: DownloadKind) -> PathBuf {
        self.config
            .output_dir
            .join(generate_filename(url, kind.output_extension()))
    }

    fn output_template_path(&self, url: &str, kind: DownloadKind) -> PathBuf {
        self.config
            .output_dir
            .join(generate_filename(url, kind.template_extension()))
    }

    fn build_download_command(
        &self,
        url: &str,
        kind: DownloadKind,
        output_template: &Path,
    ) -> Command {
        let mut cmd = Command::new(&self.config.ytdlp_path);
        cmd.arg("-o").arg(output_template);
        kind.add_format_args(&mut cmd);
        if self.config.max_size > 0 {
            cmd.arg("--max-filesize")
                .arg(self.config.max_size.to_string());
        }
        self.add_cookie_proxy_args(&mut cmd);
        // Not `--quiet`: it also suppresses the max-filesize skip marker,
        // the only place yt-dlp reports the oversize media's actual size.
        cmd.arg("--no-warnings").arg("--no-progress");
        // `--` so an attacker-shaped URL can never be parsed as an option
        cmd.arg("--").arg(url);
        cmd
    }

    fn check_duration_limit(&self, info: &VideoInfo) -> DownloadResult<()> {
        if self.config.max_duration > 0
            && let Some(duration) = info.duration_secs()
            && duration > self.config.max_duration
        {
            return Err(DownloadError::DurationTooLong {
                duration,
                limit: self.config.max_duration,
            });
        }
        Ok(())
    }

    pub(super) async fn run_command(
        mut cmd: Command,
        failure_context: &str,
    ) -> DownloadResult<Output> {
        // Reap yt-dlp when the timeout fires or the caller's future is dropped
        cmd.kill_on_drop(true);
        let output = tokio::time::timeout(YTDLP_TIMEOUT, cmd.output())
            .await
            .map_err(|_| {
                DownloadError::Ytdlp(format!(
                    "{}: timed out after {}s",
                    failure_context,
                    YTDLP_TIMEOUT.as_secs()
                ))
            })?
            .map_err(|e| DownloadError::Ytdlp(format!("Failed to run yt-dlp: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DownloadError::Ytdlp(format!(
                "{}: {}",
                failure_context,
                stderr.trim()
            )));
        }

        Ok(output)
    }

    pub(super) async fn download_media(
        &self,
        url: &str,
        kind: DownloadKind,
    ) -> DownloadResult<PathBuf> {
        let info = self.extract_info(url).await?;
        self.check_duration_limit(&info)?;

        let output_path = self.output_path(url, kind);
        if output_path.exists() {
            return Ok(output_path);
        }

        let output_template = self.output_template_path(url, kind);
        let cmd = self.build_download_command(url, kind, &output_template);
        let output = Self::run_command(cmd, kind.failure_context()).await?;

        if !output_path.exists() {
            return Err(missing_output_error(
                self.config.max_size,
                &output.stdout,
                &output.stderr,
            ));
        }

        Ok(output_path)
    }
}

fn generate_filename(url: &str, ext: &str) -> String {
    format!("{}.{}", cache_key(url), ext)
}

/// Marker yt-dlp prints after skipping an oversize download (suppressed by
/// `--quiet`): `[download] File is larger than max-filesize (N bytes > M bytes)`.
const MAX_FILESIZE_MARKER: &str = "File is larger than max-filesize";

/// Classify a successful yt-dlp exit that left no output file behind.
///
/// yt-dlp SKIPS media exceeding `--max-filesize` with exit code 0, so
/// `run_command` returns `Ok` and the only evidence is the missing file
/// (plus the stdout marker carrying the media size). Map that case to
/// the same size-limit error the stream downloader raises instead of a
/// misleading "output file not found".
fn missing_output_error(max_size: u64, stdout: &[u8], stderr: &[u8]) -> DownloadError {
    if max_size > 0 {
        let combined = format!(
            "{}\n{}",
            String::from_utf8_lossy(stdout),
            String::from_utf8_lossy(stderr)
        );
        if let Some(size) = parse_skipped_size(&combined) {
            return DownloadError::FileTooLarge {
                size,
                limit: max_size,
            };
        }
        // Without the marker (no size to report), a clean exit producing
        // no file with a cap set still means yt-dlp refused the media.
        return DownloadError::Ytdlp(format!(
            "yt-dlp produced no output file: media exceeds the --max-filesize cap of {} bytes",
            max_size
        ));
    }
    DownloadError::Ytdlp("yt-dlp completed but output file not found".to_string())
}

/// Extract the actual media size from the max-filesize skip marker, if present.
fn parse_skipped_size(text: &str) -> Option<u64> {
    let rest = &text[text.find(MAX_FILESIZE_MARKER)? + MAX_FILESIZE_MARKER.len()..];
    let digits: String = rest
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::download::ytdlp::YtdlpConfig;

    /// Write an executable stand-in for yt-dlp that answers `--dump-json`
    /// with minimal metadata and, for downloads, behaves like yt-dlp skipping
    /// oversize media: exit 0, no output file, and the skip marker on stdout
    /// unless `--quiet` is passed.
    fn fake_ytdlp(dir: &Path) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let path = dir.join("fake-yt-dlp");
        std::fs::write(
            &path,
            r#"#!/bin/sh
quiet=0
for arg in "$@"; do
    case "$arg" in
        --dump-json) echo '{"title":"t","duration":5}'; exit 0 ;;
        --quiet) quiet=1 ;;
    esac
done
case " $* " in
    *" --max-filesize "*)
        if [ "$quiet" = 0 ]; then
            printf '\r[download] File is larger than max-filesize (2000 bytes > 1000 bytes). Aborting.\n'
        fi ;;
esac
exit 0
"#,
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();

        // A process forked by a concurrent test while the script was open
        // for writing keeps it busy (ETXTBSY) until that child execs. Once
        // one exec succeeds no such descriptor remains.
        for _ in 0..1000 {
            match std::process::Command::new(&path).output() {
                Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                    std::thread::yield_now();
                }
                result => {
                    result.unwrap();
                    break;
                }
            }
        }
        path
    }

    #[tokio::test]
    async fn oversize_download_reports_file_too_large() {
        let dir = tempfile::tempdir().unwrap();
        let config = YtdlpConfig {
            ytdlp_path: fake_ytdlp(dir.path()).to_string_lossy().into_owned(),
            max_size: 1000,
            output_dir: dir.path().join("out"),
            ..YtdlpConfig::default()
        };
        let downloader = YtdlpDownloader::with_config(config);

        for kind in [DownloadKind::Video, DownloadKind::Audio] {
            let err = downloader
                .download_media("https://example.test/video", kind)
                .await
                .expect_err("oversize media must not download");
            assert!(
                matches!(
                    err,
                    DownloadError::FileTooLarge {
                        size: 2000,
                        limit: 1000
                    }
                ),
                "{kind:?}: expected FileTooLarge, got: {err:?}"
            );
        }
    }
}
