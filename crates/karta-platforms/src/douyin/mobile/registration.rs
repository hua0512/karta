use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use karta_core::config::DouyinMobileConfig;
use karta_core::error::{ParserError, Result};
use karta_core::http;
use serde_json::json;
use tokio::io::AsyncWriteExt;
use wreq::header::{HeaderMap, HeaderValue};

use super::{DouyinParser, signing};

#[derive(Default)]
pub(in crate::douyin) struct RegistrationState {
    identity: Option<DouyinMobileConfig>,
    loaded: bool,
    retry_after: Option<Instant>,
}

pub(super) fn positive_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value.bytes().all(|b| b.is_ascii_digit())
        && value.bytes().any(|b| b != b'0')
}

fn valid_cached(value: &DouyinMobileConfig, profile: &DouyinMobileConfig) -> bool {
    positive_id(&value.device_id)
        && positive_id(&value.install_id)
        && value.version_code == profile.version_code
        && value.version_name == profile.version_name
        && value.update_version_code == profile.update_version_code
        && value
            .openudid
            .as_ref()
            .is_some_and(|v| v.len() == 16 && v.bytes().all(|b| b.is_ascii_hexdigit()))
        && value
            .cdid
            .as_ref()
            .is_some_and(|v| uuid::Uuid::parse_str(v).is_ok())
        && value
            .clientudid
            .as_ref()
            .is_some_and(|v| uuid::Uuid::parse_str(v).is_ok())
}

impl DouyinParser {
    pub(in crate::douyin) async fn mobile_identity(&self) -> Result<DouyinMobileConfig> {
        let profile = self
            .mobile
            .as_ref()
            .ok_or_else(|| ParserError::Parse("douyin_mobile is not configured".into()))?;
        if positive_id(&profile.device_id) && positive_id(&profile.install_id) {
            return Ok(profile.clone());
        }
        if !profile.auto_register || !profile.device_id.is_empty() || !profile.install_id.is_empty()
        {
            return Err(ParserError::Parse(
                "Set both mobile device/install IDs or enable auto_register with both IDs empty"
                    .into(),
            ));
        }
        let mut state = self.mobile_registration.lock().await;
        if !state.loaded {
            state.loaded = true;
            if let Some(path) = self.mobile_cache_path.as_deref() {
                match tokio::fs::metadata(path).await {
                    Ok(meta) if meta.len() <= 65536 => {
                        if let Ok(bytes) = tokio::fs::read(path).await
                            && let Ok(cached) = serde_json::from_slice::<DouyinMobileConfig>(&bytes)
                            && valid_cached(&cached, profile)
                        {
                            state.identity = Some(cached);
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    _ => tracing::warn!("Douyin mobile identity cache could not be loaded"),
                }
            }
        }
        if let Some(identity) = state.identity.as_ref() {
            return Ok(identity.clone());
        }
        if state
            .retry_after
            .is_some_and(|deadline| Instant::now() < deadline)
        {
            return Err(ParserError::Parse(
                "Douyin device registration failed recently; retry later".into(),
            ));
        }
        // Set before awaiting: cancellation must not cause a registration storm.
        state.retry_after = Some(Instant::now() + Duration::from_secs(60));
        let identity = self.register_mobile(profile).await?;
        if let Some(path) = self.mobile_cache_path.as_deref()
            && let Err(error) = persist(path, &identity).await
        {
            tracing::warn!(%error, "Douyin identity is usable in memory but could not be persisted");
        }
        state.identity = Some(identity.clone());
        state.retry_after = None;
        Ok(identity)
    }

    async fn register_mobile(&self, profile: &DouyinMobileConfig) -> Result<DouyinMobileConfig> {
        let version: u32 = profile
            .version_code
            .parse()
            .map_err(|_| ParserError::Parse("Invalid mobile version_code".into()))?;
        let update: u32 = profile
            .update_version_code
            .parse()
            .map_err(|_| ParserError::Parse("Invalid mobile update_version_code".into()))?;
        if version == 0
            || update == 0
            || profile.version_name.is_empty()
            || profile.version_name.len() > 32
            || !profile
                .version_name
                .bytes()
                .all(|b| b.is_ascii_digit() || b == b'.')
        {
            return Err(ParserError::Parse("Invalid mobile version profile".into()));
        }
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| ParserError::Parse(e.to_string()))?
            .as_secs();
        let time = u32::try_from(time).map_err(|e| ParserError::Parse(e.to_string()))?;
        let openudid =
            format!("{:x}", md5::compute(uuid::Uuid::new_v4().to_string()))[..16].to_owned();
        let cdid = uuid::Uuid::new_v4().to_string();
        let clientudid = uuid::Uuid::new_v4().to_string();
        let query = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs([
                ("device_id", "0"),
                ("aid", "1128"),
                ("app_name", "aweme"),
                ("version_code", profile.version_code.as_str()),
                ("version_name", profile.version_name.as_str()),
                ("device_platform", "android"),
                ("os_version", "15"),
                ("os_api", "35"),
                ("device_type", "SM-A5560"),
                ("device_brand", "samsung"),
                ("channel", "channel_aweme"),
                ("openudid", openudid.as_str()),
                ("cdid", cdid.as_str()),
                ("clientudid", clientudid.as_str()),
                ("ts", &time.to_string()),
            ])
            .finish();
        let body = serde_json::to_vec(&json!({
            "magic_tag": "ss_app_log", "_gen_time": u64::from(time) * 1000,
            "header": {
                "display_name": "抖音", "update_version_code": update, "manifest_version_code": version,
                "aid": 1128, "channel": "channel_aweme", "app_name": "aweme", "package": "com.ss.android.ugc.aweme",
                "app_version": profile.version_name, "version_code": version, "os": "Android", "os_version": "15", "os_api": 35,
                "device_model": "SM-A5560", "device_brand": "samsung", "device_manufacturer": "samsung", "cpu_abi": "arm64-v8a",
                "resolution": "2400x1080", "density_dpi": 420, "language": "zh", "timezone": 8, "access": "wifi",
                "openudid": openudid, "cdid": cdid, "clientudid": clientudid,
            }
        }))?;
        let mut headers = HeaderMap::new();
        let ua = format!(
            "com.ss.android.ugc.aweme/{} (Linux; U; Android 15; zh_CN; SM-A5560; Build/AP3A.240905.015.A2)",
            profile.version_code
        );
        headers.insert(
            "user-agent",
            HeaderValue::from_str(&ua).map_err(|e| ParserError::Parse(e.to_string()))?,
        );
        headers.insert(
            "content-type",
            HeaderValue::from_static("application/json; charset=utf-8"),
        );
        headers.insert("accept", HeaderValue::from_static("application/json"));
        headers.insert("x-tt-request-tag", HeaderValue::from_static("t=0"));
        for (name, value) in signing::sign(&query, "0", &profile.version_name, time, Some(&body)) {
            headers.insert(
                name,
                HeaderValue::from_str(&value).map_err(|e| ParserError::Parse(e.to_string()))?,
            );
        }
        let mut last = ParserError::Parse("Device registration returned no valid identity".into());
        for host in ["https://log.snssdk.com", "https://applog.snssdk.com"] {
            let attempt = async {
                let response = self
                    .downloader
                    .client()
                    .post(format!("{host}/service/2/device_register/?{query}"))
                    .headers(headers.clone())
                    .body(body.clone())
                    .timeout(Duration::from_secs(5))
                    .send()
                    .await?;
                http::check_status(&response)?;
                let data: serde_json::Value = response.json().await?;
                let id = |name: &str| -> Option<String> {
                    [format!("{name}_str"), name.to_owned()]
                        .iter()
                        .find_map(|key| {
                            let value = data.get(key)?;
                            let value = value
                                .as_str()
                                .map(str::to_owned)
                                .or_else(|| value.as_u64().map(|v| v.to_string()))?;
                            positive_id(&value).then_some(value)
                        })
                };
                let device_id = id("device_id").ok_or_else(|| {
                    ParserError::Parse("Registration returned invalid device_id".into())
                })?;
                let install_id = id("install_id").ok_or_else(|| {
                    ParserError::Parse("Registration returned invalid install_id".into())
                })?;
                Ok(DouyinMobileConfig {
                    device_id,
                    install_id,
                    openudid: Some(openudid.clone()),
                    cdid: Some(cdid.clone()),
                    clientudid: Some(clientudid.clone()),
                    device_token: data
                        .get("device_token")
                        .and_then(|v| v.as_str())
                        .map(str::to_owned),
                    ..profile.clone()
                })
            }
            .await;
            match attempt {
                Ok(identity) => return Ok(identity),
                Err(error) => last = error,
            }
        }
        Err(last)
    }
}

async fn persist(path: &Path, identity: &DouyinMobileConfig) -> Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let result = async {
        let mut file = options.open(&temp).await?;
        file.write_all(&serde_json::to_vec(identity)?).await?;
        file.sync_all().await?;
        drop(file);
        tokio::fs::rename(&temp, path).await?;
        Ok(())
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&temp).await;
    }
    result
}
