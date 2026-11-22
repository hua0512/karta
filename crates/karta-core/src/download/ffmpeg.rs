use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

use crate::error::DownloadError;

mod convert;
mod decrypt;
mod gif;
mod probe;

/// Generous upper bound for a single ffmpeg invocation. Sized for the worst
/// case the bot permits: re-encoding a `DEFAULT_MAX_DURATION` (480 s) video
/// with libx264 on slow hardware, or `merge_av_from_urls` reading both inputs
/// from the network (whose HTTP side is bounded by `DOWNLOAD_TIMEOUT`).
const FFMPEG_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// `ffmpeg -version` should return instantly; anything slower than this is a
/// hung binary and reads as "unavailable".
const VERSION_CHECK_TIMEOUT: Duration = Duration::from_secs(10);

/// Maximum bytes of stderr echoed into error messages.
const STDERR_TAIL_BYTES: usize = 500;

/// FFmpeg wrapper for video/audio processing
pub struct FFmpeg {
    path: PathBuf,
}

impl FFmpeg {
    /// Create a new FFmpeg instance
    pub fn new(path: Option<String>) -> Self {
        Self {
            path: PathBuf::from(path.unwrap_or_else(|| "ffmpeg".to_string())),
        }
    }

    /// Run an ffmpeg command, capturing stderr via `.output()` so the pipe
    /// buffer is fully consumed and cannot deadlock the child process.
    ///
    /// Bounded by [`FFMPEG_TIMEOUT`]; `kill_on_drop` reaps the child when the
    /// timeout fires (or the caller's future is cancelled).
    async fn exec<I, S>(&self, args: I) -> Result<std::process::Output, DownloadError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        tokio::time::timeout(
            FFMPEG_TIMEOUT,
            Command::new(&self.path)
                .args(args)
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .output(),
        )
        .await
        .map_err(|_| {
            DownloadError::FFmpeg(format!(
                "ffmpeg timed out after {}s",
                FFMPEG_TIMEOUT.as_secs()
            ))
        })?
        .map_err(|e| DownloadError::FFmpeg(e.to_string()))
    }

    /// Return `Ok(())` when the process succeeded, or a descriptive error
    /// containing the tail of stderr when it failed.
    fn check(output: &std::process::Output, operation: &str) -> Result<(), DownloadError> {
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail = stderr_tail(&stderr, STDERR_TAIL_BYTES);
        tracing::warn!(
            code = ?output.status.code(),
            stderr = %tail,
            "{} failed", operation
        );
        Err(DownloadError::FFmpeg(format!(
            "{} failed (code={:?}): {}",
            operation,
            output.status.code(),
            tail,
        )))
    }

    /// Check if FFmpeg is available
    pub async fn is_available(&self) -> bool {
        let status = tokio::time::timeout(
            VERSION_CHECK_TIMEOUT,
            Command::new(&self.path)
                .arg("-version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .kill_on_drop(true)
                .status(),
        )
        .await;
        matches!(status, Ok(Ok(s)) if s.success())
    }
}

/// Last `max_bytes` of `s`, truncated on a char boundary: ffmpeg stderr can
/// contain multi-byte UTF-8 (e.g. non-ASCII filenames) and a naive byte slice
/// would panic mid-sequence.
fn stderr_tail(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut start = s.len() - max_bytes;
    while !s.is_char_boundary(start) {
        start += 1;
    }
    &s[start..]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stderr_tail_truncates_multibyte_on_char_boundary() {
        let ascii = "a".repeat(600);
        // 800 three-byte chars = 2400 bytes; len - 500 = 1900 falls inside a
        // UTF-8 sequence, which panicked with byte-index slicing.
        let multibyte = "错误信息".repeat(200);
        // (case, stderr, expected tail length in bytes)
        let cases = [
            ("ASCII is cut to the byte", ascii.as_str(), 500),
            (
                "multi-byte is cut on a char boundary",
                multibyte.as_str(),
                498,
            ),
        ];
        for (case, stderr, len) in cases {
            let tail = stderr_tail(stderr, 500);
            assert_eq!(tail.len(), len, "{case}");
            assert!(stderr.ends_with(tail), "{case}");
        }
    }
}
