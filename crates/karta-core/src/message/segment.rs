use std::path::PathBuf;

use base64::{Engine, engine::general_purpose::STANDARD as BASE64_STANDARD};

/// Segment type enumeration
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SegmentType {
    #[default]
    Text,
    Image,
    Video,
    Audio,
    At,
    Reply,
    Face,
    Custom(String),
}

/// A single message segment
#[derive(Debug, Clone, Default)]
pub struct MessageSegment {
    /// Type of the segment
    pub segment_type: SegmentType,
    /// Text content (for text segments)
    pub text: Option<String>,
    /// File path (for media segments)
    pub file_path: Option<PathBuf>,
    /// URL (for media segments)
    pub url: Option<String>,
    /// Base64 encoded data (for media segments)
    pub base64_data: Option<String>,
    /// Target ID (for at/reply segments)
    pub target_id: Option<String>,
    /// Additional data as key-value pairs
    pub extra: Vec<(String, String)>,
    /// This segment explains a failed or skipped media attachment.
    pub is_failure_notice: bool,
}

impl MessageSegment {
    /// Create a text segment
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            segment_type: SegmentType::Text,
            text: Some(content.into()),
            ..Self::default()
        }
    }

    /// Text to deliver without marking the overall request successful.
    pub fn failure_notice(content: impl Into<String>) -> Self {
        Self {
            is_failure_notice: true,
            ..Self::text(content)
        }
    }

    /// Create an image segment from URL
    pub fn image_url(url: impl Into<String>) -> Self {
        Self {
            segment_type: SegmentType::Image,
            url: Some(url.into()),
            ..Self::default()
        }
    }

    /// Create an image segment from file path
    pub fn image_file(path: PathBuf) -> Self {
        Self {
            segment_type: SegmentType::Image,
            file_path: Some(path),
            ..Self::default()
        }
    }

    /// Create an image segment from base64 data
    pub fn image_base64(data: impl Into<String>) -> Self {
        Self {
            segment_type: SegmentType::Image,
            base64_data: Some(data.into()),
            ..Self::default()
        }
    }

    /// Create a video segment from URL
    pub fn video_url(url: impl Into<String>) -> Self {
        Self {
            segment_type: SegmentType::Video,
            url: Some(url.into()),
            ..Self::default()
        }
    }

    /// Create a video segment from file path
    pub fn video_file(path: PathBuf) -> Self {
        Self {
            segment_type: SegmentType::Video,
            file_path: Some(path),
            ..Self::default()
        }
    }

    /// Create a video segment from base64 data
    pub fn video_base64(data: impl Into<String>) -> Self {
        Self {
            segment_type: SegmentType::Video,
            base64_data: Some(data.into()),
            ..Self::default()
        }
    }

    /// Create an audio segment from URL
    pub fn audio_url(url: impl Into<String>) -> Self {
        Self {
            segment_type: SegmentType::Audio,
            url: Some(url.into()),
            ..Self::default()
        }
    }

    /// Create an audio segment from file path
    pub fn audio_file(path: PathBuf) -> Self {
        Self {
            segment_type: SegmentType::Audio,
            file_path: Some(path),
            ..Self::default()
        }
    }

    /// Create an audio segment from base64 data
    pub fn audio_base64(data: impl Into<String>) -> Self {
        Self {
            segment_type: SegmentType::Audio,
            base64_data: Some(data.into()),
            ..Self::default()
        }
    }

    /// Create an at segment
    pub fn at(target_id: impl Into<String>) -> Self {
        Self {
            segment_type: SegmentType::At,
            target_id: Some(target_id.into()),
            ..Self::default()
        }
    }

    /// Create a reply segment
    pub fn reply(message_id: impl Into<String>) -> Self {
        Self {
            segment_type: SegmentType::Reply,
            target_id: Some(message_id.into()),
            ..Self::default()
        }
    }

    /// Add extra data
    pub fn with_extra(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra.push((key.into(), value.into()));
        self
    }

    /// Create a custom segment with key-value extras.
    pub fn custom(type_name: impl Into<String>) -> Self {
        Self {
            segment_type: SegmentType::Custom(type_name.into()),
            ..Self::default()
        }
    }

    /// Get the file reference (either path, URL, or base64)
    pub fn file_ref(&self) -> Option<String> {
        if let Some(base64) = &self.base64_data {
            Some(format!("base64://{}", base64))
        } else if let Some(path) = &self.file_path {
            // Absolute paths already start with '/', so a naive
            // `file:///{path}` would produce four slashes; `from_file_path`
            // also percent-encodes what needs it. It only accepts absolute
            // paths, so fall back to the naive form for relative ones.
            Some(
                url::Url::from_file_path(path)
                    .map(|u| u.to_string())
                    .unwrap_or_else(|_| format!("file:///{}", path.display())),
            )
        } else {
            self.url.clone()
        }
    }

    /// Check if this segment has base64 data
    pub fn is_base64(&self) -> bool {
        self.base64_data.is_some()
    }

    /// Convert file path to base64 (reads file and encodes)
    pub async fn to_base64(&mut self) -> Result<(), std::io::Error> {
        if let Some(path) = &self.file_path {
            let data = tokio::fs::read(path).await?;
            self.base64_data = Some(BASE64_STANDARD.encode(&data));
            self.file_path = None;
        }
        Ok(())
    }
}
