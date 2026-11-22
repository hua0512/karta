use std::path::Path;

use crate::error::DownloadError;

use super::FFmpeg;

impl FFmpeg {
    /// Convert video to GIF.
    pub async fn video_to_gif(
        &self,
        input_path: &Path,
        output_path: &Path,
        fps: u32,
        width: u32,
    ) -> Result<(), DownloadError> {
        let palette_path = input_path.with_extension("palette.png");
        let input_path_str = input_path
            .to_str()
            .ok_or_else(|| DownloadError::FFmpeg("Invalid input path".to_string()))?;
        let palette_path_str = palette_path
            .to_str()
            .ok_or_else(|| DownloadError::FFmpeg("Invalid palette path".to_string()))?;
        let output_path_str = output_path
            .to_str()
            .ok_or_else(|| DownloadError::FFmpeg("Invalid output path".to_string()))?;

        let palette_filter = format!(
            "fps={},scale={}:-1:flags=lanczos,palettegen=stats_mode=diff",
            fps, width
        );

        let result = self
            .exec([
                "-y",
                "-i",
                input_path_str,
                "-vf",
                &palette_filter,
                palette_path_str,
            ])
            .await?;

        Self::check(&result, "FFmpeg palette generation")?;

        let gif_filter = format!(
            "fps={},scale={}:-1:flags=lanczos[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle",
            fps, width
        );

        let result = self
            .exec([
                "-y",
                "-i",
                input_path_str,
                "-i",
                palette_path_str,
                "-lavfi",
                &gif_filter,
                // Explicit container: the output may be a `.part` staging path,
                // so ffmpeg cannot infer the format from the extension.
                "-f",
                "gif",
                output_path_str,
            ])
            .await?;

        let _ = tokio::fs::remove_file(&palette_path).await;

        Self::check(&result, "FFmpeg GIF generation")
    }

    /// Convert video to GIF with simple single-pass encoding.
    pub async fn video_to_gif_simple(
        &self,
        input_path: &Path,
        output_path: &Path,
        fps: u32,
        width: u32,
    ) -> Result<(), DownloadError> {
        let input_path = input_path
            .to_str()
            .ok_or_else(|| DownloadError::FFmpeg("Invalid input path".to_string()))?;
        let output_path = output_path
            .to_str()
            .ok_or_else(|| DownloadError::FFmpeg("Invalid output path".to_string()))?;
        let filter = format!("fps={},scale={}:-1:flags=lanczos", fps, width);

        let output = self
            .exec([
                "-y",
                "-i",
                input_path,
                "-vf",
                &filter,
                "-loop",
                "0",
                "-f",
                "gif",
                output_path,
            ])
            .await?;

        Self::check(&output, "FFmpeg simple GIF generation")
    }
}
