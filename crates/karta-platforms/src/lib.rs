//! Platform-specific parsers for karta-core.
//!
//! This crate contains all platform parser implementations and a factory
//! function to instantiate them from configuration.

pub mod acfun;
pub mod bilibili;
pub mod douyin;
pub mod hongguo;
pub mod instagram;
pub mod kuaishou;
pub mod nga;
pub mod tiktok;
pub mod twitter;
pub mod v2ex;
pub mod wechat;
pub mod wechat_channel;
pub mod weibo;
pub mod xiaoheihe;
pub mod xiaohongshu;
pub mod youtube;

pub use bilibili::credential::{
    BiliCredential, CredentialManager, QrLoginState, RefreshOutcome, SharedBiliCredential,
};

use std::path::Path;
use std::sync::Arc;

use karta_core::config::Config;
use karta_core::constants::Platform;
use karta_core::download::SharedDownloader;
use karta_core::parser::Parser;

#[cfg(feature = "ytdlp")]
use karta_core::download::{YtdlpConfig, YtdlpDownloader};

/// Push the parser built by `make` unless its platform is disabled.
///
/// Taking a closure keeps disabled parsers from being constructed at all
/// (construction compiles that platform's regex patterns).
fn register<P: Parser + 'static>(
    parsers: &mut Vec<Arc<dyn Parser>>,
    config: &Config,
    platform: Platform,
    make: impl FnOnce() -> P,
) {
    if !config.is_platform_disabled(platform) {
        parsers.push(Arc::new(make()));
    }
}

/// Create all enabled platform parsers from config.
pub fn create_all_parsers(
    config: &Config,
    downloader: SharedDownloader,
    data_path: &Path,
) -> Vec<Arc<dyn Parser>> {
    create_all_parsers_inner(config, downloader, data_path, None)
}

/// Create all enabled platform parsers with a live Bilibili credential store.
pub fn create_all_parsers_with_bili_credential(
    config: &Config,
    downloader: SharedDownloader,
    data_path: &Path,
    credential: SharedBiliCredential,
) -> Vec<Arc<dyn Parser>> {
    create_all_parsers_inner(config, downloader, data_path, Some(credential))
}

fn create_all_parsers_inner(
    config: &Config,
    downloader: SharedDownloader,
    data_path: &Path,
    bili_credential: Option<SharedBiliCredential>,
) -> Vec<Arc<dyn Parser>> {
    let mut parsers: Vec<Arc<dyn Parser>> = Vec::new();

    #[cfg(feature = "ytdlp")]
    let ytdlp = make_ytdlp(config, data_path);

    register(&mut parsers, config, Platform::Bilibili, || {
        let parser = bilibili::BilibiliParser::new(downloader.clone(), config);
        if let Some(credential) = bili_credential.clone() {
            parser.with_shared_credential(credential)
        } else {
            parser
        }
    });
    register(&mut parsers, config, Platform::Douyin, || {
        douyin::DouyinParser::new(downloader.clone(), config.douyin_cookie.clone())
            .with_mobile_config(config.douyin_mobile.clone())
            .with_mobile_cache(data_path.join("douyin-device.json"))
    });
    register(&mut parsers, config, Platform::Twitter, || {
        twitter::TwitterParser::new(downloader.clone())
    });
    register(&mut parsers, config, Platform::YouTube, || {
        let yt = youtube::YouTubeParser::new(
            downloader.clone(),
            config.ytb_cookie.clone(),
            config.download_media,
            config.duration_maximum,
        );
        #[cfg(feature = "ytdlp")]
        let yt = yt.with_ytdlp(ytdlp.clone());
        yt
    });
    register(&mut parsers, config, Platform::TikTok, || {
        let tk = tiktok::TikTokParser::new(downloader.clone(), config.download_media);
        #[cfg(feature = "ytdlp")]
        let tk = tk.with_ytdlp(ytdlp.clone());
        tk
    });
    register(&mut parsers, config, Platform::Weibo, || {
        weibo::WeiboParser::new(downloader.clone())
    });
    register(&mut parsers, config, Platform::Xiaohongshu, || {
        xiaohongshu::XiaohongshuParser::new(downloader.clone(), config.xhs_cookie.clone())
    });
    register(&mut parsers, config, Platform::Kuaishou, || {
        kuaishou::KuaishouParser::new(downloader.clone())
    });
    register(&mut parsers, config, Platform::AcFun, || {
        acfun::AcfunParser::new(downloader.clone())
    });
    register(&mut parsers, config, Platform::Nga, || {
        nga::NgaParser::new(downloader.clone())
    });
    register(&mut parsers, config, Platform::V2ex, || {
        v2ex::V2exParser::new(downloader.clone())
    });
    register(&mut parsers, config, Platform::Instagram, || {
        instagram::InstagramParser::new(downloader.clone(), config.instagram_cookie.clone())
    });
    register(&mut parsers, config, Platform::Xiaoheihe, || {
        xiaoheihe::XiaoheiheParser::new(downloader.clone(), config.xhh_cookie.clone())
    });
    register(&mut parsers, config, Platform::Hongguo, || {
        hongguo::HongguoParser::new(downloader.clone(), config.hongguo_api_auth.clone())
    });
    register(&mut parsers, config, Platform::Wechat, || {
        wechat::WechatParser::new(downloader.clone())
    });
    register(&mut parsers, config, Platform::WechatChannel, || {
        wechat_channel::WechatChannelParser::new(
            downloader.clone(),
            config.wechat_channel_cookie.clone(),
        )
    });

    tracing::info!(
        registered = parsers.len(),
        disabled = ?config.disabled_platforms,
        "Platform parsers created"
    );

    parsers
}

#[cfg(feature = "ytdlp")]
fn make_ytdlp(config: &Config, data_path: &Path) -> YtdlpDownloader {
    let mut ytdlp_cfg = YtdlpConfig::default();
    if let Some(path) = &config.ytdlp_path {
        ytdlp_cfg.ytdlp_path = path.clone();
    }
    if let Some(cookies) = &config.ytdlp_cookies_file {
        ytdlp_cfg.cookies_file = Some(std::path::PathBuf::from(cookies));
    }
    ytdlp_cfg.proxy = config.proxy_url();
    ytdlp_cfg.max_duration = config.duration_maximum;
    ytdlp_cfg.max_size = config.max_size_bytes();
    ytdlp_cfg.output_dir = karta_core::config::Config::ytdlp_dir(data_path);
    YtdlpDownloader::with_config(ytdlp_cfg)
}
