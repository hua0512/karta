use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use kovi::{Message, Segment};
use serde_json::json;

use karta_core::ParserEngine;
use karta_core::config::{Config, RenderType};
use karta_core::constants::Platform;
use karta_platforms::CredentialManager;

use crate::state::RuntimeState;

/// Brief usage reply for recognized commands with malformed arguments.
const USAGE: &str = "Commands: /status, /login bilibili, /ban <uid>, /unban <uid>, \
/enable <platform>, /disable <platform>, /karta on|off";

/// Resolve a user-supplied platform name to a known platform.
fn resolve_platform(name: &str) -> Option<Platform> {
    let lower = name.to_lowercase();
    // Canonical slug first
    if let Some(platform) = Platform::from_slug(&lower) {
        return Some(platform);
    }
    // Common aliases
    match lower.as_str() {
        "bili" | "b站" => Some(Platform::Bilibili),
        "x" | "推特" => Some(Platform::Twitter),
        "yt" => Some(Platform::YouTube),
        "抖音" => Some(Platform::Douyin),
        "微博" => Some(Platform::Weibo),
        "小红书" | "xhs" => Some(Platform::Xiaohongshu),
        "快手" | "ks" => Some(Platform::Kuaishou),
        "小黑盒" | "xhh" | "heybox" => Some(Platform::Xiaoheihe),
        "ig" => Some(Platform::Instagram),
        "v站" => Some(Platform::V2ex),
        "红果" | "红果短剧" | "hg" => Some(Platform::Hongguo),
        "微信" | "公众号" | "微信公众号" | "wx" | "mp" => Some(Platform::Wechat),
        "视频号" | "微信视频号" | "sph" | "wxchannel" => Some(Platform::WechatChannel),
        _ => None,
    }
}

/// Concurrency guard for bilibili QR login (only one at a time).
static LOGIN_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

/// A recognized admin command, split into its dispatch target.
enum Command<'a> {
    Status,
    LoginBilibili,
    Ban(&'a str),
    Unban(&'a str),
    Enable(&'a str),
    Disable(&'a str),
    KartaToggle(bool),
    /// Recognized command word, but the arguments are malformed.
    Malformed,
}

/// Match `text` against this plugin's command words.
///
/// Returns `None` for command words this plugin does not own: QQ groups run
/// multiple bots, so an unrecognized `/`-prefixed message is most likely
/// another bot's command and must be ignored silently. The usage reply is
/// reserved for commands clearly addressed to this plugin (`Malformed`).
fn parse_command(text: &str) -> Option<Command<'_>> {
    let text = text.trim();
    let (word, rest) = match text.split_once(char::is_whitespace) {
        Some((word, rest)) => (word, rest.trim()),
        None => (text, ""),
    };

    match word {
        "/status" if rest.is_empty() => Some(Command::Status),
        "/status" => Some(Command::Malformed),
        "/login" => match rest {
            "bilibili" | "bili" => Some(Command::LoginBilibili),
            _ => Some(Command::Malformed),
        },
        "/ban" => Some(Command::Ban(rest)),
        "/unban" => Some(Command::Unban(rest)),
        "/enable" => Some(Command::Enable(rest)),
        "/disable" => Some(Command::Disable(rest)),
        "/karta" => match rest {
            "on" => Some(Command::KartaToggle(true)),
            "off" => Some(Command::KartaToggle(false)),
            _ => Some(Command::Malformed),
        },
        _ => None,
    }
}

/// Entry point for admin command handling.
pub async fn handle_command(
    text: &str,
    group_id: Option<i64>,
    engine: &ParserEngine,
    credential_manager: &CredentialManager,
    state: &RuntimeState,
    event: &impl Reply,
) {
    let Some(command) = parse_command(text) else {
        // Not our command word: silently ignore (likely another bot's).
        return;
    };

    match command {
        Command::Status => handle_status(engine, credential_manager, state, event).await,
        Command::LoginBilibili => handle_login_bilibili(credential_manager, event).await,
        Command::Ban(rest) => handle_ban(rest, state, event).await,
        Command::Unban(rest) => handle_unban(rest, state, event).await,
        Command::Enable(rest) => {
            handle_enable_platform(rest, group_id, engine.config(), state, event).await
        }
        Command::Disable(rest) => handle_disable_platform(rest, group_id, state, event).await,
        Command::KartaToggle(on) => {
            let blacklist_mode = engine.config().group_blacklist_enabled;
            handle_karta_toggle(on, group_id, blacklist_mode, state, event).await
        }
        Command::Malformed => event.reply_text(USAGE),
    }
}

/// Trait to abstract reply capability for both AdminMsgEvent and testing.
pub trait Reply {
    fn reply_text(&self, text: &str);
    fn reply_image_base64(&self, base64: &str);
}

impl Reply for kovi_onebot::AdminMsgEvent {
    fn reply_text(&self, text: &str) {
        self.reply(text);
    }

    fn reply_image_base64(&self, b64: &str) {
        let mut msg = Message::new();
        msg.push(Segment::new(
            "image",
            json!({"file": format!("base64://{}", b64)}),
        ));
        self.reply(msg);
    }
}

async fn handle_status(
    engine: &ParserEngine,
    credential_manager: &CredentialManager,
    state: &RuntimeState,
    event: &impl Reply,
) {
    let config = engine.config();
    let mut lines = Vec::new();

    lines.push("=== Karta Status ===".to_string());

    // Render type
    let render_str = match config.render_type {
        RenderType::Text => "Text",
        RenderType::Image => "Image",
    };
    lines.push(format!("Render: {}", render_str));

    // Platform status with cookie info
    lines.push("--- Platforms ---".to_string());
    for &platform in Platform::ALL {
        let disabled_global = config.is_platform_disabled(platform);
        let cookie_status = match platform {
            Platform::Bilibili => {
                if credential_manager.has_credential().await {
                    " (credential: ok)"
                } else if config.bili_cookie.is_some() {
                    " (cookie: configured)"
                } else {
                    ""
                }
            }
            Platform::YouTube if config.ytb_cookie.is_some() => " (cookie: configured)",
            Platform::Xiaohongshu if config.xhs_cookie.is_some() => " (cookie: configured)",
            Platform::Twitter if config.twitter_cookie.is_some() => " (cookie: configured)",
            Platform::Weibo if config.weibo_cookie.is_some() => " (cookie: configured)",
            Platform::Xiaoheihe if config.xhh_cookie.is_some() => " (cookie: configured)",
            Platform::Douyin if config.douyin_cookie.is_some() => " (cookie: configured)",
            _ => "",
        };

        let status = if disabled_global {
            "disabled (global)"
        } else {
            "enabled"
        };
        lines.push(format!(
            "  {}: {}{}",
            platform.slug(),
            status,
            cookie_status
        ));
    }

    // Tools
    lines.push("--- Tools ---".to_string());
    let ffmpeg = engine.is_ffmpeg_available().await;
    lines.push(format!(
        "  FFmpeg: {}",
        if ffmpeg { "available" } else { "not found" }
    ));

    let ytdlp_status = if config.ytdlp_path.is_some() || config.ytdlp_cookies_file.is_some() {
        "configured"
    } else {
        "not configured"
    };
    lines.push(format!("  yt-dlp: {}", ytdlp_status));

    // Access control
    if config.access_control.enabled {
        lines.push(format!(
            "Access control: {:?}, {} users, {} groups",
            config.access_control.mode,
            config.access_control.users.len(),
            config.access_control.groups.len(),
        ));
    }

    // Runtime state
    lines.push("--- Runtime State ---".to_string());
    lines.push(format!("  Banned users: {}", state.banned_user_count()));
    // The same persisted set is a blacklist or a whitelist depending on mode.
    let group_label = if config.group_blacklist_enabled {
        "Disabled groups"
    } else {
        "Enabled groups (whitelist)"
    };
    lines.push(format!("  {}: {}", group_label, state.group_set_count()));

    event.reply_text(&lines.join("\n"));
}

async fn handle_login_bilibili(credential_manager: &CredentialManager, event: &impl Reply) {
    // Concurrency guard
    if LOGIN_IN_PROGRESS
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        event.reply_text("A Bilibili login is already in progress.");
        return;
    }

    // Ensure guard is released on all exit paths
    struct LoginGuard;
    impl Drop for LoginGuard {
        fn drop(&mut self) {
            LOGIN_IN_PROGRESS.store(false, Ordering::SeqCst);
        }
    }
    let _guard = LoginGuard;

    // Generate QR code
    let qr_url = match credential_manager.generate_qr_code().await {
        Ok(url) => url,
        Err(e) => {
            event.reply_text(&format!("Failed to generate QR code: {}", e));
            return;
        }
    };

    // Render QR code to PNG and send as base64
    match render_qr_code(&qr_url) {
        Ok(b64) => {
            event.reply_image_base64(&b64);
            event
                .reply_text("Please scan the QR code with the Bilibili app. Waiting up to 120s...");
        }
        Err(e) => {
            event.reply_text(&format!("Failed to render QR code: {}", e));
            return;
        }
    }

    // Poll for login result
    match credential_manager
        .wait_for_qr_login(Duration::from_secs(120))
        .await
    {
        Ok(karta_platforms::QrLoginState::Success) => {
            event.reply_text("Bilibili login successful!");
        }
        Ok(karta_platforms::QrLoginState::Expired) => {
            event.reply_text("QR code expired. Please try /login bilibili again.");
        }
        Ok(karta_platforms::QrLoginState::Failed(msg)) => {
            event.reply_text(&format!("Bilibili login failed: {}", msg));
        }
        Ok(_) => {
            event.reply_text("Bilibili login timed out.");
        }
        Err(e) => {
            event.reply_text(&format!("Bilibili login error: {}", e));
        }
    }
    // _guard drops here, releasing LOGIN_IN_PROGRESS
}

/// Render a URL as a QR code PNG and return as base64.
fn render_qr_code(url: &str) -> Result<String, String> {
    use base64::Engine as _;
    use image::Luma;
    use qrcode::QrCode;

    let code = QrCode::new(url.as_bytes()).map_err(|e| format!("QR encode error: {}", e))?;
    let img = code.render::<Luma<u8>>().quiet_zone(true).build();

    // Encode to PNG in memory
    let mut buf = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageLuma8(img)
        .write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| format!("PNG encode error: {}", e))?;

    Ok(base64::engine::general_purpose::STANDARD.encode(buf.into_inner()))
}

async fn handle_ban(rest: &str, state: &RuntimeState, event: &impl Reply) {
    let uid: i64 = match rest.parse() {
        Ok(v) => v,
        Err(_) => {
            event.reply_text("Usage: /ban <uid> (numeric user ID)");
            return;
        }
    };

    if state.ban_user(uid).await {
        event.reply_text(&format!("Banned user {}", uid));
    } else {
        event.reply_text(&format!("User {} is already banned", uid));
    }
}

async fn handle_unban(rest: &str, state: &RuntimeState, event: &impl Reply) {
    let uid: i64 = match rest.parse() {
        Ok(v) => v,
        Err(_) => {
            event.reply_text("Usage: /unban <uid> (numeric user ID)");
            return;
        }
    };

    if state.unban_user(uid).await {
        event.reply_text(&format!("Unbanned user {}", uid));
    } else {
        event.reply_text(&format!("User {} was not banned", uid));
    }
}

async fn handle_enable_platform(
    rest: &str,
    group_id: Option<i64>,
    config: &Config,
    state: &RuntimeState,
    event: &impl Reply,
) {
    let Some(gid) = group_id else {
        event.reply_text("This command can only be used in a group.");
        return;
    };

    let Some(platform) = resolve_platform(rest) else {
        let names: Vec<&str> = Platform::ALL.iter().map(|p| p.slug()).collect();
        event.reply_text(&format!(
            "Unknown platform '{}'. Available: {}",
            rest,
            names.join(", ")
        ));
        return;
    };

    // No parser is registered for a globally disabled platform, so a group
    // toggle could never take effect.
    if config.is_platform_disabled(platform) {
        event.reply_text(&format!(
            "{} is disabled globally in config; it cannot be enabled per group",
            platform.slug()
        ));
        return;
    }

    state.set_platform_in_group(gid, platform, true).await;
    event.reply_text(&format!("Enabled {} in this group", platform.slug()));
}

async fn handle_disable_platform(
    rest: &str,
    group_id: Option<i64>,
    state: &RuntimeState,
    event: &impl Reply,
) {
    let Some(gid) = group_id else {
        event.reply_text("This command can only be used in a group.");
        return;
    };

    let Some(platform) = resolve_platform(rest) else {
        let names: Vec<&str> = Platform::ALL.iter().map(|p| p.slug()).collect();
        event.reply_text(&format!(
            "Unknown platform '{}'. Available: {}",
            rest,
            names.join(", ")
        ));
        return;
    };

    state.set_platform_in_group(gid, platform, false).await;
    event.reply_text(&format!("Disabled {} in this group", platform.slug()));
}

async fn handle_karta_toggle(
    on: bool,
    group_id: Option<i64>,
    blacklist_mode: bool,
    state: &RuntimeState,
    event: &impl Reply,
) {
    let Some(gid) = group_id else {
        event.reply_text("This command can only be used in a group.");
        return;
    };

    state.set_group_enabled(gid, on, blacklist_mode).await;
    let status = if on { "enabled" } else { "disabled" };
    event.reply_text(&format!("Karta {} in this group", status));
}
