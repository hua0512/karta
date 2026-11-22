use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, RwLock};
use wreq::Client;
use wreq::header::HeaderMap;

use karta_core::constants::{DEFAULT_TIMEOUT, DEFAULT_USER_AGENT, Platform};
use karta_core::error::{ParserError, Result, ResultExt};
use karta_core::http;

mod correspond;
mod model;
mod refresh;
pub use model::{BiliCredential, QrLoginState, RefreshOutcome};
use model::{
    ConfirmRefreshResponse, CookieInfoResponse, CookieRefreshResponse, QrCodeGenerateResponse,
    QrCodePollResponse,
};

const COOKIE_INFO_URL: &str = "https://passport.bilibili.com/x/passport-login/web/cookie/info";
const CORRESPOND_URL: &str = "https://www.bilibili.com/correspond/1/";
const COOKIE_REFRESH_URL: &str =
    "https://passport.bilibili.com/x/passport-login/web/cookie/refresh";
const CONFIRM_REFRESH_URL: &str =
    "https://passport.bilibili.com/x/passport-login/web/confirm/refresh";
/// Per-request bound for the refresh flow, which holds `refresh_lock`: a
/// request that never completes would otherwise block every later refresh.
const REFRESH_REQUEST_TIMEOUT: Duration = Duration::from_secs(DEFAULT_TIMEOUT);

/// Live credential store shared by the login flow and Bilibili parser.
pub type SharedBiliCredential = Arc<RwLock<Option<BiliCredential>>>;

/// Credential manager for Bilibili
pub struct CredentialManager {
    client: Client,
    credential: SharedBiliCredential,
    cookies_file: PathBuf,
    qr_key: Arc<RwLock<Option<String>>>,
    /// Serialises refreshes: each one spends the refresh token it starts from.
    refresh_lock: Mutex<()>,
}

impl CredentialManager {
    /// Create a new credential manager
    pub fn new(client: Client, data_path: &Path) -> Self {
        Self {
            client,
            credential: Arc::new(RwLock::new(None)),
            cookies_file: data_path.join("bilibili_cookies.json"),
            qr_key: Arc::new(RwLock::new(None)),
            refresh_lock: Mutex::new(()),
        }
    }

    /// Initialize credential from config or file.
    ///
    /// A valid config cookie wins and replaces the saved file. An invalid one
    /// is expected after a refresh, which retires the cookie the config still
    /// holds and saves its successor to the file, so the file is used instead.
    pub async fn init(&self, cookie_string: Option<&str>) -> Result<()> {
        let mut config_cookie_invalid = false;
        if let Some(cookie) = cookie_string
            && let Some(cred) = BiliCredential::from_cookie_string(cookie)
        {
            if self.validate_credential(&cred).await? {
                tracing::info!("Bilibili credential from config is valid");
                *self.credential.write().await = Some(cred.clone());
                self.save_credential(&cred).await?;
                return Ok(());
            }
            config_cookie_invalid = true;
        }

        if let Ok(cred) = self.load_credential().await {
            if self.validate_credential(&cred).await? {
                if config_cookie_invalid {
                    tracing::info!(
                        "Bilibili cookie in config is no longer valid; using the newer saved \
                         credential from {:?}",
                        self.cookies_file
                    );
                } else {
                    tracing::info!("Bilibili credential from file is valid");
                }
                *self.credential.write().await = Some(cred);
                return Ok(());
            }
            tracing::warn!("Bilibili credential from file is invalid");
        }

        if config_cookie_invalid {
            tracing::warn!("Bilibili credential from config is invalid");
        }
        Ok(())
    }

    /// Get the current credential
    pub async fn get_credential(&self) -> Option<BiliCredential> {
        self.credential.read().await.clone()
    }

    /// Share the live credential with parser instances.
    ///
    /// QR login and refresh replace the value in this store, so consumers see
    /// new cookies without rebuilding the parser engine.
    pub fn shared_credential(&self) -> SharedBiliCredential {
        self.credential.clone()
    }

    /// Get cookie string for requests
    pub async fn get_cookie_string(&self) -> Option<String> {
        self.credential
            .read()
            .await
            .as_ref()
            .map(|c| c.to_cookie_string())
    }

    /// Check if credential is available
    pub async fn has_credential(&self) -> bool {
        self.credential.read().await.is_some()
    }

    /// Request headers authenticated as `cred`.
    fn cookie_headers(cred: &BiliCredential) -> HeaderMap {
        http::headers_with_referer_and_cookie(
            DEFAULT_USER_AGENT,
            "https://www.bilibili.com",
            Some(&cred.to_cookie_string()),
        )
    }

    /// Validate credential by checking nav API
    async fn validate_credential(&self, cred: &BiliCredential) -> Result<bool> {
        let response: serde_json::Value = self
            .client
            .get("https://api.bilibili.com/x/web-interface/nav")
            .headers(Self::cookie_headers(cred))
            .send()
            .await?
            .json()
            .await?;

        let code = response.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
        let is_login = response
            .pointer("/data/isLogin")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        Ok(code == 0 && is_login)
    }

    /// Save credential to file
    async fn save_credential(&self, cred: &BiliCredential) -> Result<()> {
        if let Some(parent) = self.cookies_file.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let json = serde_json::to_string_pretty(cred)?;
        tokio::fs::write(&self.cookies_file, json).await?;
        tracing::info!("Saved Bilibili credential to {:?}", self.cookies_file);
        Ok(())
    }

    /// Load credential from file
    async fn load_credential(&self) -> Result<BiliCredential> {
        let content = tokio::fs::read_to_string(&self.cookies_file).await?;
        let cred: BiliCredential = serde_json::from_str(&content)?;
        Ok(cred)
    }

    /// Generate QR code for login
    /// Returns the QR code URL that should be converted to an image
    pub async fn generate_qr_code(&self) -> Result<String> {
        let headers = http::headers_with_referer(DEFAULT_USER_AGENT, "https://www.bilibili.com");

        let response: QrCodeGenerateResponse = self
            .client
            .get("https://passport.bilibili.com/x/passport-login/web/qrcode/generate")
            .headers(headers)
            .send()
            .await?
            .json()
            .await?;

        if response.code != 0 {
            return Err(ParserError::Api {
                code: response.code,
                message: "Failed to generate QR code".to_string(),
            });
        }

        let data = response
            .data
            .ok_or_else(|| ParserError::ContentNotFound("QR code data not found".to_string()))?;

        // Store the QR key for polling
        *self.qr_key.write().await = Some(data.qrcode_key);

        Ok(data.url)
    }

    /// Poll QR code login state
    pub async fn poll_qr_state(&self) -> Result<QrLoginState> {
        let qr_key = self
            .qr_key
            .read()
            .await
            .clone()
            .ok_or_else(|| ParserError::Parse("No QR code generated".to_string()))?;

        let headers = http::headers_with_referer(DEFAULT_USER_AGENT, "https://www.bilibili.com");

        let response = self
            .client
            .get("https://passport.bilibili.com/x/passport-login/web/qrcode/poll")
            .headers(headers)
            .query(&[("qrcode_key", &qr_key)])
            .send()
            .await?;

        // Extract cookies from response headers before consuming response
        let cookies: Vec<(String, String)> = response
            .cookies()
            .map(|c| (c.name().to_string(), c.value().to_string()))
            .collect();

        let json: QrCodePollResponse = response.json().await?;

        if json.code != 0 {
            return Err(ParserError::Api {
                code: json.code,
                message: "Failed to poll QR code".to_string(),
            });
        }

        let data = json
            .data
            .ok_or_else(|| ParserError::ContentNotFound("QR poll data not found".to_string()))?;

        match data.code {
            0 => {
                if let Some(cred) = BiliCredential::from_cookie_pairs(cookies, data.refresh_token) {
                    *self.credential.write().await = Some(cred.clone());
                    self.save_credential(&cred).await?;
                    *self.qr_key.write().await = None;

                    Ok(QrLoginState::Success)
                } else {
                    Ok(QrLoginState::Failed(
                        "Failed to extract cookies".to_string(),
                    ))
                }
            }
            86038 => Ok(QrLoginState::Expired),
            86090 => Ok(QrLoginState::Scanned),
            86101 => Ok(QrLoginState::Pending),
            _ => Ok(QrLoginState::Failed(data.message)),
        }
    }

    /// Wait for QR code login with timeout
    pub async fn wait_for_qr_login(&self, timeout: Duration) -> Result<QrLoginState> {
        let start = std::time::Instant::now();
        let poll_interval = Duration::from_secs(2);

        loop {
            if start.elapsed() > timeout {
                return Ok(QrLoginState::Expired);
            }

            let state = self.poll_qr_state().await?;

            match state {
                QrLoginState::Success | QrLoginState::Expired | QrLoginState::Failed(_) => {
                    return Ok(state);
                }
                QrLoginState::Pending | QrLoginState::Scanned => {
                    tokio::time::sleep(poll_interval).await;
                }
            }
        }
    }

    /// Refresh the cookie when Bilibili asks for it (web flow from
    /// bilibili-API-collect `docs/login/cookie_refresh.md`):
    ///
    /// 1. `cookie/info` says whether a refresh is wanted, and the server time;
    /// 2. the `correspond/1/{correspondPath}` page yields `refresh_csrf`;
    /// 3. `cookie/refresh` spends the refresh token for new cookies and a new
    ///    refresh token, which replace the credential here, in the shared
    ///    store and in `bilibili_cookies.json`;
    /// 4. `confirm/refresh` retires the old session, which logs out wherever
    ///    else the old cookie is in use.
    ///
    /// Any failure before step 3 succeeds leaves the credential untouched.
    /// After it, the old refresh token is spent, so the new credential is
    /// kept even when saving or confirming fails; the error is still returned.
    pub async fn refresh_if_requested(&self) -> Result<RefreshOutcome> {
        let _refreshing = self.refresh_lock.lock().await;
        let Some(old) = self.get_credential().await else {
            return Ok(RefreshOutcome::NoCredential);
        };
        if refresh::refresh_token(&old).is_none() {
            return Ok(RefreshOutcome::MissingRefreshToken);
        }

        let Some(timestamp) = self
            .refresh_timestamp(&old)
            .await
            .with_context(Platform::Bilibili, COOKIE_INFO_URL)?
        else {
            return Ok(RefreshOutcome::NotNeeded);
        };
        let refresh_csrf = self
            .fetch_refresh_csrf(&old, timestamp)
            .await
            .with_context(Platform::Bilibili, CORRESPOND_URL)?;
        let new = self
            .refresh_cookie(&old, &refresh_csrf)
            .await
            .with_context(Platform::Bilibili, COOKIE_REFRESH_URL)?;

        *self.credential.write().await = Some(new.clone());
        // The old cookie stays valid until the confirmation, so it is only
        // retired once its successor is on disk.
        self.save_credential(&new).await?;
        self.confirm_refresh(&old, &new)
            .await
            .with_context(Platform::Bilibili, CONFIRM_REFRESH_URL)?;
        Ok(RefreshOutcome::Refreshed)
    }

    /// `cookie/info`: the server timestamp when a refresh is wanted.
    async fn refresh_timestamp(&self, cred: &BiliCredential) -> Result<Option<i64>> {
        let response: CookieInfoResponse = self
            .client
            .get(COOKIE_INFO_URL)
            .timeout(REFRESH_REQUEST_TIMEOUT)
            .headers(Self::cookie_headers(cred))
            .query(&[("csrf", cred.bili_jct.as_str())])
            .send()
            .await
            .map_err(without_url)?
            .json()
            .await
            .map_err(without_url)?;
        if response.code != 0 {
            return Err(api_error(response.code, response.message));
        }
        let data = response
            .data
            .ok_or_else(|| ParserError::ContentNotFound("cookie/info data".to_string()))?;
        Ok(data.refresh.then_some(data.timestamp))
    }

    /// `refresh_csrf` from the correspond page for `timestamp`.
    async fn fetch_refresh_csrf(&self, cred: &BiliCredential, timestamp: i64) -> Result<String> {
        let path = correspond::correspond_path(timestamp)?;
        let html = self
            .client
            .get(format!("{CORRESPOND_URL}{path}"))
            .timeout(REFRESH_REQUEST_TIMEOUT)
            .headers(Self::cookie_headers(cred))
            .send()
            .await
            .and_then(wreq::Response::error_for_status)
            .map_err(without_url)?
            .text()
            .await
            .map_err(without_url)?;
        refresh::extract_refresh_csrf(&html)
            .ok_or_else(|| ParserError::ContentNotFound("refresh_csrf".to_string()))
    }

    /// `cookie/refresh`: the credential built from the new cookies and token.
    async fn refresh_cookie(
        &self,
        old: &BiliCredential,
        refresh_csrf: &str,
    ) -> Result<BiliCredential> {
        let response = self
            .client
            .post(COOKIE_REFRESH_URL)
            .timeout(REFRESH_REQUEST_TIMEOUT)
            .headers(Self::cookie_headers(old))
            .form(&refresh::refresh_form(old, refresh_csrf))
            .send()
            .await
            .map_err(without_url)?;
        let cookies: Vec<(String, String)> = response
            .cookies()
            .map(|c| (c.name().to_string(), c.value().to_string()))
            .collect();
        let json: CookieRefreshResponse = response.json().await.map_err(without_url)?;
        if json.code != 0 {
            return Err(api_error(json.code, json.message));
        }
        let new_token = json
            .data
            .and_then(|data| data.refresh_token)
            .filter(|token| !token.is_empty())
            .ok_or_else(|| ParserError::ContentNotFound("new refresh_token".to_string()))?;
        refresh::refreshed_credential(old, cookies, new_token).ok_or_else(|| {
            ParserError::ContentNotFound("new SESSDATA and bili_jct cookies".to_string())
        })
    }

    /// `confirm/refresh`, sent as the new credential.
    async fn confirm_refresh(&self, old: &BiliCredential, new: &BiliCredential) -> Result<()> {
        let json: ConfirmRefreshResponse = self
            .client
            .post(CONFIRM_REFRESH_URL)
            .timeout(REFRESH_REQUEST_TIMEOUT)
            .headers(Self::cookie_headers(new))
            .form(&refresh::confirm_form(old, new))
            .send()
            .await
            .map_err(without_url)?
            .json()
            .await
            .map_err(without_url)?;
        if json.code != 0 {
            return Err(api_error(json.code, json.message));
        }
        Ok(())
    }

    /// Clear credential
    pub async fn clear(&self) {
        *self.credential.write().await = None;
        let _ = tokio::fs::remove_file(&self.cookies_file).await;
    }
}

/// A transport error without its URL, which can carry the csrf token
/// (`cookie/info` sends it in the query string).
fn without_url(error: wreq::Error) -> ParserError {
    ParserError::Http(error.without_uri())
}

fn api_error(code: i64, message: Option<String>) -> ParserError {
    ParserError::Api {
        code,
        message: message.unwrap_or_default(),
    }
}
