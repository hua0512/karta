use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Bilibili credential for authenticated requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiliCredential {
    /// SESSDATA cookie
    pub sessdata: String,
    /// bili_jct (CSRF token)
    pub bili_jct: String,
    /// buvid3
    pub buvid3: Option<String>,
    /// DedeUserID
    pub dedeuserid: Option<String>,
    /// ac_time_value (for refresh)
    pub ac_time_value: Option<String>,
    /// Refresh token
    pub refresh_token: Option<String>,
    /// Expiration timestamp
    pub expires_at: Option<i64>,
}

impl BiliCredential {
    /// Create a new credential from cookie string
    pub fn from_cookie_string(cookie: &str) -> Option<Self> {
        let mut sessdata = None;
        let mut bili_jct = None;
        let mut buvid3 = None;
        let mut dedeuserid = None;
        let mut ac_time_value = None;

        for part in cookie.split(';') {
            let part = part.trim();
            if let Some((key, value)) = part.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                match key {
                    "SESSDATA" => sessdata = Some(value.to_string()),
                    "bili_jct" => bili_jct = Some(value.to_string()),
                    "buvid3" => buvid3 = Some(value.to_string()),
                    "DedeUserID" => dedeuserid = Some(value.to_string()),
                    "ac_time_value" => ac_time_value = Some(value.to_string()),
                    _ => {}
                }
            }
        }

        // Bilibili's `ac_time_value` cookie is the refresh token used by the
        // web credential flow. Keep the explicit field in sync so credentials
        // loaded from config can use the same refresh path as QR credentials.
        let refresh_token = ac_time_value.clone();

        Some(Self {
            sessdata: sessdata?,
            bili_jct: bili_jct?,
            buvid3,
            dedeuserid,
            ac_time_value,
            refresh_token,
            expires_at: None,
        })
    }

    pub(super) fn from_cookie_pairs<I, K, V>(
        cookies: I,
        refresh_token: Option<String>,
    ) -> Option<Self>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: Into<String>,
    {
        let mut sessdata = None;
        let mut bili_jct = None;
        let mut buvid3 = None;
        let mut dedeuserid = None;

        for (name, value) in cookies {
            match name.as_ref() {
                "SESSDATA" => sessdata = Some(value.into()),
                "bili_jct" => bili_jct = Some(value.into()),
                "buvid3" => buvid3 = Some(value.into()),
                "DedeUserID" => dedeuserid = Some(value.into()),
                _ => {}
            }
        }

        // The web QR poll response calls this value `refresh_token`; the
        // bilibili credential/cookie model calls the same value
        // `ac_time_value`. Persist both representations consistently.
        let ac_time_value = refresh_token.clone();

        Some(Self {
            sessdata: sessdata?,
            bili_jct: bili_jct?,
            buvid3,
            dedeuserid,
            ac_time_value,
            refresh_token,
            expires_at: None,
        })
    }

    /// Convert to cookie string
    pub fn to_cookie_string(&self) -> String {
        let mut parts = vec![
            format!("SESSDATA={}", self.sessdata),
            format!("bili_jct={}", self.bili_jct),
        ];

        if let Some(buvid3) = &self.buvid3 {
            parts.push(format!("buvid3={}", buvid3));
        }
        if let Some(dedeuserid) = &self.dedeuserid {
            parts.push(format!("DedeUserID={}", dedeuserid));
        }
        if let Some(ac_time_value) = &self.ac_time_value {
            parts.push(format!("ac_time_value={}", ac_time_value));
        }

        parts.join("; ")
    }

    /// Check if credential can be refreshed
    pub fn can_refresh(&self) -> bool {
        self.ac_time_value.is_some() && self.refresh_token.is_some() && !self.bili_jct.is_empty()
    }

    /// Check if credential is expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            now >= expires_at
        } else {
            false
        }
    }
}

/// What [`CredentialManager::refresh_if_requested`](super::CredentialManager::refresh_if_requested)
/// did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshOutcome {
    /// No credential is loaded.
    NoCredential,
    /// The credential has no refresh token (`ac_time_value`), so it cannot be
    /// refreshed. Nothing was requested.
    MissingRefreshToken,
    /// Bilibili does not ask for a refresh yet.
    NotNeeded,
    /// The cookie was replaced, saved, and its old session retired.
    Refreshed,
}

/// `cookie/info` response.
#[derive(Debug, Deserialize)]
pub(super) struct CookieInfoResponse {
    pub(super) code: i64,
    #[serde(default)]
    pub(super) message: Option<String>,
    pub(super) data: Option<CookieInfoData>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CookieInfoData {
    /// Whether Bilibili wants the cookie refreshed.
    pub(super) refresh: bool,
    /// Server time in milliseconds, the input of `correspondPath`.
    pub(super) timestamp: i64,
}

/// `cookie/refresh` response.
#[derive(Debug, Deserialize)]
pub(super) struct CookieRefreshResponse {
    pub(super) code: i64,
    #[serde(default)]
    pub(super) message: Option<String>,
    pub(super) data: Option<CookieRefreshData>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CookieRefreshData {
    #[serde(default)]
    pub(super) refresh_token: Option<String>,
}

/// `confirm/refresh` response, which carries no data.
#[derive(Debug, Deserialize)]
pub(super) struct ConfirmRefreshResponse {
    pub(super) code: i64,
    #[serde(default)]
    pub(super) message: Option<String>,
}

/// QR code login state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QrLoginState {
    /// Waiting for scan
    Pending,
    /// Scanned, waiting for confirmation
    Scanned,
    /// Login successful
    Success,
    /// QR code expired
    Expired,
    /// Login failed
    Failed(String),
}

/// QR code login response
#[derive(Debug, Deserialize)]
pub(super) struct QrCodeGenerateResponse {
    pub(super) code: i64,
    pub(super) data: Option<QrCodeData>,
}

#[derive(Debug, Deserialize)]
pub(super) struct QrCodeData {
    pub(super) url: String,
    pub(super) qrcode_key: String,
}

/// QR code poll response
#[derive(Debug, Deserialize)]
pub(super) struct QrCodePollResponse {
    pub(super) code: i64,
    pub(super) data: Option<QrCodePollData>,
}

#[derive(Debug, Deserialize)]
pub(super) struct QrCodePollData {
    pub(super) code: i64,
    pub(super) message: String,
    pub(super) refresh_token: Option<String>,
}
