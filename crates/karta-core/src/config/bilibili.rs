use serde::{Deserialize, Serialize};

/// Bilibili video quality levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BiliVideoQuality {
    /// 240P
    #[serde(rename = "240p")]
    Q240P = 6,
    /// 360P
    #[serde(rename = "360p")]
    Q360P = 16,
    /// 480P
    #[serde(rename = "480p")]
    Q480P = 32,
    /// 720P
    #[serde(rename = "720p")]
    Q720P = 64,
    /// 720P60
    #[serde(rename = "720p60")]
    Q720P60 = 74,
    /// 1080P
    #[serde(rename = "1080p")]
    #[default]
    Q1080P = 80,
    /// 1080P+
    #[serde(rename = "1080p_plus")]
    Q1080PPlus = 112,
    /// 1080P60
    #[serde(rename = "1080p60")]
    Q1080P60 = 116,
    /// 4K
    #[serde(rename = "4k")]
    Q4K = 120,
    /// HDR
    #[serde(rename = "hdr")]
    QHDR = 125,
    /// Dolby Vision
    #[serde(rename = "dolby")]
    QDolby = 126,
    /// 8K
    #[serde(rename = "8k")]
    Q8K = 127,
}

impl BiliVideoQuality {
    /// Get the quality number (qn parameter)
    pub fn qn(&self) -> i32 {
        *self as i32
    }
}

/// Bilibili video codec
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BiliVideoCodec {
    /// AVC/H.264
    #[serde(rename = "avc")]
    #[default]
    AVC,
    /// HEVC/H.265
    #[serde(rename = "hevc")]
    HEVC,
    /// AV1
    #[serde(rename = "av1")]
    AV1,
}

impl BiliVideoCodec {
    /// Get the codec ID used in Bilibili API
    pub fn codec_id(&self) -> i32 {
        match self {
            Self::AVC => 7,
            Self::HEVC => 12,
            Self::AV1 => 13,
        }
    }

    /// Check if a codec string matches this codec
    pub fn matches(&self, codecs: &str) -> bool {
        match self {
            Self::AVC => codecs.starts_with("avc"),
            Self::HEVC => codecs.starts_with("hev") || codecs.starts_with("hvc"),
            // Bilibili reports AV1 streams as either "av01.*" or "av1"; "av01"
            // does not start with "av1", so both prefixes must be checked.
            Self::AV1 => codecs.starts_with("av01") || codecs.starts_with("av1"),
        }
    }
}

/// How to pick between the CDN hosts Bilibili returns for a stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BiliCdnMode {
    /// Rank the API's URLs, preferring fast CDNs and avoiding slow `mcdn`
    /// nodes, and rewrite a host that is slow on every candidate.
    #[default]
    Auto,
    /// Use the API's primary URL untouched. Useful when a network reaches the
    /// assigned CDN faster than any mirror.
    Original,
    /// Always rewrite to a known CDN mirror when one is mapped for the host.
    Mirror,
}
