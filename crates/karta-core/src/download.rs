pub mod ffmpeg;
pub mod stream;

#[cfg(feature = "ytdlp")]
pub mod ytdlp;

pub use ffmpeg::FFmpeg;
pub use stream::{SharedDownloader, StreamDownloader};

#[cfg(feature = "ytdlp")]
pub use ytdlp::{VideoInfo as YtdlpVideoInfo, YtdlpConfig, YtdlpDownloader};
