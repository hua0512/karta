use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::process::Command;

use crate::error::DownloadError;

use super::FFmpeg;

/// ffprobe reads a local file and prints one number; anything slower than
/// this is a hung process.
const FFPROBE_TIMEOUT: Duration = Duration::from_secs(30);

impl FFmpeg {
    fn ffprobe_path(&self) -> PathBuf {
        let probe_name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| name.ends_with(".exe"))
            .map(|_| "ffprobe.exe")
            .unwrap_or("ffprobe");

        match self.path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.join(probe_name),
            _ => PathBuf::from(probe_name),
        }
    }

    /// Run ffprobe with the given args, returning stdout on success.
    async fn ffprobe(&self, args: &[&str]) -> Result<String, DownloadError> {
        let ffprobe_path = self.ffprobe_path();
        let output = tokio::time::timeout(
            FFPROBE_TIMEOUT,
            Command::new(&ffprobe_path)
                .args(args)
                // Reap the child when the timeout fires or the caller cancels.
                .kill_on_drop(true)
                .output(),
        )
        .await
        .map_err(|_| {
            DownloadError::FFmpeg(format!(
                "ffprobe timed out after {}s",
                FFPROBE_TIMEOUT.as_secs()
            ))
        })?
        .map_err(|e| DownloadError::FFmpeg(format!("Failed to run {:?}: {}", ffprobe_path, e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DownloadError::FFmpeg(format!(
                "ffprobe failed: {}",
                stderr.trim()
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// Get media duration in fractional seconds.
    pub async fn duration_secs(&self, input_path: &Path) -> Result<f64, DownloadError> {
        let input_path = input_path
            .to_str()
            .ok_or_else(|| DownloadError::FFmpeg("Invalid input path".to_string()))?;
        let stdout = self
            .ffprobe(&[
                "-v",
                "error",
                "-show_entries",
                "format=duration",
                "-of",
                "default=noprint_wrappers=1:nokey=1",
                input_path,
            ])
            .await?;

        stdout
            .trim()
            .parse()
            .map_err(|e| DownloadError::FFmpeg(format!("Failed to parse duration: {}", e)))
    }

    /// Get video duration in seconds (rounded up).
    pub async fn get_duration(&self, input_path: &Path) -> Result<u64, DownloadError> {
        Ok(self.duration_secs(input_path).await?.ceil() as u64)
    }

    /// Whether the file contains at least one audio stream.
    pub async fn has_audio_stream(&self, input_path: &Path) -> Result<bool, DownloadError> {
        let input_path = input_path
            .to_str()
            .ok_or_else(|| DownloadError::FFmpeg("Invalid input path".to_string()))?;
        // Prints one `audio` line per audio stream; empty output means none.
        let stdout = self
            .ffprobe(&[
                "-v",
                "error",
                "-select_streams",
                "a",
                "-show_entries",
                "stream=codec_type",
                "-of",
                "csv=p=0",
                input_path,
            ])
            .await?;

        Ok(!stdout.trim().is_empty())
    }
}
