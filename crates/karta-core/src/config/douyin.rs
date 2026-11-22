use serde::{Deserialize, Serialize};

/// Identity from an authorized Douyin Android installation. Random identifiers
/// are not registered device identities and may produce empty API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DouyinMobileConfig {
    /// Register lazily when no explicit device/install pair is supplied.
    pub auto_register: bool,
    pub device_id: String,
    pub install_id: String,
    pub openudid: Option<String>,
    pub cdid: Option<String>,
    pub clientudid: Option<String>,
    pub device_token: Option<String>,
    pub version_code: String,
    pub version_name: String,
    pub update_version_code: String,
}

impl Default for DouyinMobileConfig {
    fn default() -> Self {
        Self {
            auto_register: false,
            openudid: None,
            cdid: None,
            clientudid: None,
            device_token: None,
            device_id: String::new(),
            install_id: String::new(),
            version_code: "400300".into(),
            version_name: "40.3.0".into(),
            update_version_code: "400300".into(),
        }
    }
}
