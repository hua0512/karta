use serde::{Deserialize, Serialize};

/// Render type for output
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RenderType {
    #[default]
    Text,
    Image,
}

/// Visual treatment for native image cards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageHeaderStyle {
    /// Pick a stable treatment from the post's text and display-sized media.
    Auto,
    /// Multi-lobe ambient mesh glow across the top of the card.
    MeshGlow,
    /// A softly blurred field of image colors, or a generated palette for text.
    Blur,
    /// Layered translucent glass arcs with crisp specular rim highlights.
    SoftPrism,
    /// A full-card blurred backdrop with a readable frosted surface and no header art.
    #[default]
    BlurBackground,
}

/// Color polarity for native image cards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageThemeMode {
    /// Warm frosted light paper.
    Light,
    /// Deep obsidian frosted glass.
    Dark,
    /// Automatically choose dark glass for moody/dark media and light paper otherwise.
    #[default]
    Auto,
}

impl ImageThemeMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::Auto => "auto",
        }
    }
}

impl ImageHeaderStyle {
    /// Configuration value and preview filename component.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::MeshGlow => "mesh_glow",
            Self::Blur => "blur",
            Self::SoftPrism => "soft_prism",
            Self::BlurBackground => "blur_background",
        }
    }

    pub(crate) fn resolve(self, seed: u64) -> Self {
        if self != Self::Auto {
            return self;
        }
        match seed % 3 {
            0 => Self::MeshGlow,
            1 => Self::Blur,
            _ => Self::SoftPrism,
        }
    }
}
