use std::path::Path;

use crate::error::DownloadError;

use super::FFmpeg;

/// A CENC-AES-128-CTR key is exactly 16 bytes, i.e. 32 hex characters.
fn is_valid_cenc_key(key: &str) -> bool {
    key.len() == 32 && key.bytes().all(|b| b.is_ascii_hexdigit())
}

impl FFmpeg {
    /// Decrypt a CENC-AES-CTR encrypted media file, remuxing without re-encoding.
    ///
    /// `decryption_key` must be the 32 lowercase hex characters of the AES-128 key.
    pub async fn decrypt(
        &self,
        input_path: &Path,
        output_path: &Path,
        decryption_key: &str,
    ) -> Result<(), DownloadError> {
        if !is_valid_cenc_key(decryption_key) {
            return Err(DownloadError::FFmpeg(
                "invalid CENC decryption key: expected 32 hex characters".to_string(),
            ));
        }

        tracing::debug!(
            input = ?input_path,
            output = ?output_path,
            "FFmpeg decrypting CENC-AES-CTR media"
        );
        let input_path = input_path
            .to_str()
            .ok_or_else(|| DownloadError::FFmpeg("Invalid input path".to_string()))?;
        let output_path = output_path
            .to_str()
            .ok_or_else(|| DownloadError::FFmpeg("Invalid output path".to_string()))?;

        // The key rides on the ffmpeg command line and is therefore visible in
        // /proc/<pid>/cmdline for the process lifetime. Accepted: ffmpeg has no
        // stdin/file alternative for -decryption_key, and the key only guards
        // already-public short-drama media.
        let output = self
            .exec([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-decryption_key",
                decryption_key,
                "-i",
                input_path,
                "-map",
                "0",
                "-c",
                "copy",
                // Explicit container: the output may be a `.part` staging path,
                // so ffmpeg cannot infer the format from the extension.
                "-f",
                "mp4",
                output_path,
            ])
            .await?;

        Self::check(&output, "FFmpeg decrypt")
    }
}
