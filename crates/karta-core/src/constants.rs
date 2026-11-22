use std::fmt;

/// Platform enumeration for all supported platforms
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Bilibili,
    Douyin,
    Twitter,
    YouTube,
    TikTok,
    Weibo,
    Xiaohongshu,
    Kuaishou,
    AcFun,
    Nga,
    Xiaoheihe,
    Instagram,
    V2ex,
    Hongguo,
    Wechat,
    WechatChannel,
}

impl Platform {
    /// Every supported platform, in declaration order.
    pub const ALL: &'static [Platform] = &[
        Platform::Bilibili,
        Platform::Douyin,
        Platform::Twitter,
        Platform::YouTube,
        Platform::TikTok,
        Platform::Weibo,
        Platform::Xiaohongshu,
        Platform::Kuaishou,
        Platform::AcFun,
        Platform::Nga,
        Platform::Xiaoheihe,
        Platform::Instagram,
        Platform::V2ex,
        Platform::Hongguo,
        Platform::Wechat,
        Platform::WechatChannel,
    ];

    /// Canonical lowercase identifier, identical to the serde wire format
    /// (`#[serde(rename_all = "lowercase")]`).
    pub const fn slug(&self) -> &'static str {
        match self {
            Platform::Bilibili => "bilibili",
            Platform::Douyin => "douyin",
            Platform::Twitter => "twitter",
            Platform::YouTube => "youtube",
            Platform::TikTok => "tiktok",
            Platform::Weibo => "weibo",
            Platform::Xiaohongshu => "xiaohongshu",
            Platform::Kuaishou => "kuaishou",
            Platform::AcFun => "acfun",
            Platform::Nga => "nga",
            Platform::Xiaoheihe => "xiaoheihe",
            Platform::Instagram => "instagram",
            Platform::V2ex => "v2ex",
            Platform::Hongguo => "hongguo",
            Platform::Wechat => "wechat",
            Platform::WechatChannel => "wechatchannel",
        }
    }

    /// Resolve a canonical slug back to its platform.
    pub fn from_slug(s: &str) -> Option<Platform> {
        Platform::ALL.iter().copied().find(|p| p.slug() == s)
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Platform::Bilibili => write!(f, "Bilibili"),
            Platform::Douyin => write!(f, "抖音"),
            Platform::Twitter => write!(f, "Twitter/X"),
            Platform::YouTube => write!(f, "YouTube"),
            Platform::TikTok => write!(f, "TikTok"),
            Platform::Weibo => write!(f, "微博"),
            Platform::Xiaohongshu => write!(f, "小红书"),
            Platform::Kuaishou => write!(f, "快手"),
            Platform::AcFun => write!(f, "AcFun"),
            Platform::Nga => write!(f, "NGA"),
            Platform::Xiaoheihe => write!(f, "小黑盒"),
            Platform::Instagram => write!(f, "Instagram"),
            Platform::V2ex => write!(f, "V2EX"),
            Platform::Hongguo => write!(f, "红果短剧"),
            Platform::Wechat => write!(f, "微信公众号"),
            Platform::WechatChannel => write!(f, "微信视频号"),
        }
    }
}

/// Default User-Agent for HTTP requests
pub const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// Mobile User-Agent for platforms that require it
pub const MOBILE_USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1";

/// Default request timeout in seconds
pub const DEFAULT_TIMEOUT: u64 = 30;

/// Default per-parse timeout in seconds (covers the full parse operation)
pub const DEFAULT_PARSE_TIMEOUT: u64 = 60;

/// Download timeout in seconds
pub const DOWNLOAD_TIMEOUT: u64 = 300;

/// Default maximum file size in MB
pub const DEFAULT_MAX_SIZE_MB: u64 = 90;

/// Default maximum duration in seconds
pub const DEFAULT_MAX_DURATION: u64 = 480;

/// Default cache size (number of entries)
pub const DEFAULT_CACHE_SIZE: usize = 50;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_slug_round_trips_serde_and_from_slug() {
        for &platform in Platform::ALL {
            let json = serde_json::to_string(&platform).expect("platform serializes");
            assert_eq!(json, format!("\"{}\"", platform.slug()));

            assert_eq!(Platform::from_slug(platform.slug()), Some(platform));
        }
    }
}
