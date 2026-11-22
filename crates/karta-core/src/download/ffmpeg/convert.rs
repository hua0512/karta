use std::path::Path;

use crate::error::DownloadError;

use super::FFmpeg;

/// Number of *extra* BGM repetitions (`-stream_loop` value) needed so the
/// looped BGM covers the whole video. Zero when either duration is unknown
/// (`<= 0`) or the BGM is already at least as long as the video.
///
/// Example: 10 s video with a 3 s BGM -> `ceil(10 / 3) - 1 = 3` extra loops,
/// i.e. 4 plays (12 s), fully covering the video before `-shortest` clamps
/// the output back to 10 s.
fn bgm_loop_count(video_duration: f64, bgm_duration: f64) -> u32 {
    if video_duration <= 0.0 || bgm_duration <= 0.0 || video_duration <= bgm_duration {
        return 0;
    }
    ((video_duration / bgm_duration).ceil() as u32).saturating_sub(1)
}

/// Build the ffmpeg argument list for a BGM merge.
///
/// `loop_count` extra repetitions are applied to the BGM input via
/// `-stream_loop` (which must precede its `-i`). When the video has its own
/// audio the two tracks are blended with `amix` using `duration=longest` so
/// the mix is not cut to the shorter track; otherwise the BGM is mapped
/// directly as the sole audio track. `-shortest` clamps the output to the
/// video length in both cases.
fn bgm_merge_args(
    video: &str,
    audio: &str,
    output: &str,
    video_has_audio: bool,
    loop_count: u32,
) -> Vec<String> {
    let mut args: Vec<String> = vec!["-y".into(), "-i".into(), video.into()];
    if loop_count > 0 {
        args.push("-stream_loop".into());
        args.push(loop_count.to_string());
    }
    args.push("-i".into());
    args.push(audio.into());
    if video_has_audio {
        args.extend([
            "-filter_complex".into(),
            "[0:a][1:a]amix=inputs=2:duration=longest:dropout_transition=3[aout]".into(),
            "-map".into(),
            "0:v".into(),
            "-map".into(),
            "[aout]".into(),
        ]);
    } else {
        args.extend(["-map".into(), "0:v".into(), "-map".into(), "1:a".into()]);
    }
    args.extend([
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "fast".into(),
        "-c:a".into(),
        "aac".into(),
        "-b:a".into(),
        "192k".into(),
        "-shortest".into(),
        // Explicit container: the output may be a `.part` staging path,
        // so ffmpeg cannot infer the format from the extension.
        "-f".into(),
        "mp4".into(),
        output.into(),
    ]);
    args
}

impl FFmpeg {
    /// Merge video and audio streams into a single file.
    pub async fn merge_av(
        &self,
        video_path: &Path,
        audio_path: &Path,
        output_path: &Path,
    ) -> Result<(), DownloadError> {
        tracing::debug!(
            video = ?video_path,
            audio = ?audio_path,
            output = ?output_path,
            "FFmpeg merging A/V"
        );
        let video_path = video_path
            .to_str()
            .ok_or_else(|| DownloadError::FFmpeg("Invalid video path".to_string()))?;
        let audio_path = audio_path
            .to_str()
            .ok_or_else(|| DownloadError::FFmpeg("Invalid audio path".to_string()))?;
        let output_path = output_path
            .to_str()
            .ok_or_else(|| DownloadError::FFmpeg("Invalid output path".to_string()))?;

        let output = self
            .exec([
                "-y",
                "-i",
                video_path,
                "-i",
                audio_path,
                "-c:v",
                "copy",
                "-c:a",
                "aac",
                "-strict",
                "experimental",
                // Explicit container: the output may be a `.part` staging path,
                // so ffmpeg cannot infer the format from the extension.
                "-f",
                "mp4",
                output_path,
            ])
            .await?;

        Self::check(&output, "FFmpeg merge")
    }

    /// Merge a video with background music, mixing both audio tracks.
    ///
    /// Uses `amix` to overlay the BGM onto the video's existing audio. If the
    /// video has no audio stream, falls back to adding the BGM as the sole audio
    /// track. A BGM shorter than the video is looped (`-stream_loop`) to cover
    /// the full video length; `-shortest` then clamps the output back to the
    /// video, so a short BGM no longer truncates it (douyin live-photos).
    pub async fn merge_with_bgm(
        &self,
        video_path: &Path,
        audio_path: &Path,
        output_path: &Path,
    ) -> Result<(), DownloadError> {
        let video = video_path
            .to_str()
            .ok_or_else(|| DownloadError::FFmpeg("Invalid video path".to_string()))?;
        let audio = audio_path
            .to_str()
            .ok_or_else(|| DownloadError::FFmpeg("Invalid audio path".to_string()))?;
        let output = output_path
            .to_str()
            .ok_or_else(|| DownloadError::FFmpeg("Invalid output path".to_string()))?;

        // Probe both inputs; on failure fall back to defaults that reproduce
        // the pre-probe behavior (no loop, amix first with direct-map retry).
        let video_duration = match self.duration_secs(video_path).await {
            Ok(duration) => duration,
            Err(e) => {
                tracing::warn!(error = %e, "ffprobe failed for video duration, skipping BGM loop");
                0.0
            }
        };
        let bgm_duration = match self.duration_secs(audio_path).await {
            Ok(duration) => duration,
            Err(e) => {
                tracing::warn!(error = %e, "ffprobe failed for BGM duration, skipping BGM loop");
                0.0
            }
        };
        let video_has_audio = match self.has_audio_stream(video_path).await {
            Ok(has_audio) => has_audio,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "ffprobe audio-stream check failed, assuming video has audio"
                );
                true
            }
        };
        let loop_count = bgm_loop_count(video_duration, bgm_duration);

        tracing::debug!(
            video = ?video_path,
            audio = ?audio_path,
            output = ?output_path,
            video_duration,
            bgm_duration,
            video_has_audio,
            loop_count,
            "FFmpeg merging video with BGM"
        );

        let result = self
            .exec(bgm_merge_args(
                video,
                audio,
                output,
                video_has_audio,
                loop_count,
            ))
            .await?;

        if result.status.success() {
            return Ok(());
        }
        if !video_has_audio {
            return Self::check(&result, "FFmpeg BGM merge");
        }

        // The probe can misreport an audio stream with no decodable frames;
        // retry with the BGM mapped as the sole audio track.
        let stderr = String::from_utf8_lossy(&result.stderr);
        tracing::debug!(
            stderr = %stderr,
            "amix failed, falling back to simple merge"
        );

        let result = self
            .exec(bgm_merge_args(video, audio, output, false, loop_count))
            .await?;

        Self::check(&result, "FFmpeg BGM merge")
    }

    /// Merge video and audio from URLs directly.
    pub async fn merge_av_from_urls(
        &self,
        video_url: &str,
        audio_url: &str,
        output_path: &Path,
        headers: Option<&[(&str, &str)]>,
    ) -> Result<(), DownloadError> {
        let output_path = output_path
            .to_str()
            .ok_or_else(|| DownloadError::FFmpeg("Invalid output path".to_string()))?
            .to_string();
        let mut args = vec!["-y".to_string()];

        if let Some(hdrs) = headers {
            for (key, value) in hdrs {
                args.push("-headers".to_string());
                args.push(format!("{}: {}", key, value));
            }
        }

        args.extend([
            "-i".to_string(),
            video_url.to_string(),
            "-i".to_string(),
            audio_url.to_string(),
            "-c:v".to_string(),
            "copy".to_string(),
            "-c:a".to_string(),
            "aac".to_string(),
            "-strict".to_string(),
            "experimental".to_string(),
            output_path,
        ]);

        let output = self.exec(&args).await?;
        Self::check(&output, "FFmpeg merge from URLs")
    }

    /// Record the first `seconds` of a live stream into a playable file.
    ///
    /// A live URL never ends on its own, so `-t` is what bounds the transfer:
    /// without it ffmpeg would run until the global `FFMPEG_TIMEOUT`. Streams
    /// are copied, not re-encoded, so the clip stays cheap to produce.
    pub async fn record_stream(
        &self,
        stream_url: &str,
        output_path: &Path,
        seconds: u64,
        headers: Option<&[(&str, &str)]>,
    ) -> Result<(), DownloadError> {
        let output_path = output_path
            .to_str()
            .ok_or_else(|| DownloadError::FFmpeg("Invalid output path".to_string()))?
            .to_string();

        let mut args = vec!["-y".to_string()];
        if let Some(headers) = headers {
            for (key, value) in headers {
                args.push("-headers".to_string());
                args.push(format!("{}: {}", key, value));
            }
        }

        args.extend([
            "-i".to_string(),
            stream_url.to_string(),
            "-t".to_string(),
            seconds.to_string(),
            "-c".to_string(),
            "copy".to_string(),
            // The output may be a `.part` staging path, so the container
            // cannot be inferred from the extension.
            "-f".to_string(),
            "mp4".to_string(),
            // FLV pulled from a live edge has no seekable index; without this
            // the moov atom cannot be written at the end of a copy remux.
            "-movflags".to_string(),
            "frag_keyframe+empty_moov".to_string(),
            output_path,
        ]);

        let output = self.exec(&args).await?;
        Self::check(&output, "FFmpeg stream recording")
    }

    /// Re-encode video to H.264 for compatibility.
    pub async fn encode_to_h264(
        &self,
        input_path: &Path,
        output_path: &Path,
    ) -> Result<(), DownloadError> {
        let input_path = input_path
            .to_str()
            .ok_or_else(|| DownloadError::FFmpeg("Invalid input path".to_string()))?;
        let output_path = output_path
            .to_str()
            .ok_or_else(|| DownloadError::FFmpeg("Invalid output path".to_string()))?;

        let output = self
            .exec([
                "-y",
                "-i",
                input_path,
                "-c:v",
                "libx264",
                "-preset",
                "fast",
                "-crf",
                "23",
                "-c:a",
                "aac",
                "-b:a",
                "128k",
                output_path,
            ])
            .await?;

        Self::check(&output, "FFmpeg encode")
    }

    /// Extract audio from video.
    pub async fn extract_audio(
        &self,
        input_path: &Path,
        output_path: &Path,
    ) -> Result<(), DownloadError> {
        let input_path = input_path
            .to_str()
            .ok_or_else(|| DownloadError::FFmpeg("Invalid input path".to_string()))?;
        let output_path = output_path
            .to_str()
            .ok_or_else(|| DownloadError::FFmpeg("Invalid output path".to_string()))?;

        let output = self
            .exec([
                "-y",
                "-i",
                input_path,
                "-vn",
                "-c:a",
                "libmp3lame",
                "-b:a",
                "192k",
                output_path,
            ])
            .await?;

        Self::check(&output, "FFmpeg audio extraction")
    }
}
