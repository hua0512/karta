use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Optional authorized ByteDance app session for resolving Hongguo motion-comic
/// videos when the anonymous multi_video_model endpoint is risk-control gated.
/// Supply a captured authorized request: endpoint url, query params (device_id,
/// iid, ...), headers (Cookie, ...), and any extra JSON body overrides.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HongguoApiAuth {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub params: HashMap<String, String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: serde_json::Value,
}
