use serde::{Deserialize, Serialize};

/// Access control mode for the plugin
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccessMode {
    /// Blacklist mode - listed IDs are denied (default)
    #[default]
    Blacklist,
    /// Whitelist mode - only listed IDs are allowed
    Whitelist,
}

/// Access control configuration
///
/// Uses Kovi's framework-level access control to block/allow users and groups
/// before plugin listeners execute.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AccessControlConfig {
    /// Whether access control is enabled
    #[serde(default)]
    pub enabled: bool,

    /// Mode: "blacklist" or "whitelist" (default: blacklist)
    #[serde(default)]
    pub mode: AccessMode,

    /// User IDs in the access list
    #[serde(default)]
    pub users: Vec<i64>,

    /// Group IDs in the access list
    #[serde(default)]
    pub groups: Vec<i64>,
}
