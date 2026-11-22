use serde::{Deserialize, Serialize};

/// Emoji style configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EmojiStyleConfig {
    /// Twitter's Twemoji style (default)
    #[default]
    Twemoji,
    /// Apple-style emojis
    Apple,
    /// Google Noto emojis
    Google,
    /// Facebook emojis
    Facebook,
}
