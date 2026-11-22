//! Type definitions for the image renderer

use std::collections::HashMap;
use std::sync::Arc;

use image::{Rgba, RgbaImage};
use parking_lot::Mutex;

use crate::constants::Platform;

use super::constants::*;
use super::font::Font;

// ============================================================================
// Font Types
// ============================================================================

/// Font information for text rendering
#[derive(Clone)]
pub struct FontInfo {
    /// The font
    pub font: Font,
    /// Text color
    pub fill: Rgba<u8>,
    /// Font size
    pub size: f32,
    /// Outline embolden strength in pixels (0.0 for regular weight)
    pub embolden: f32,
    /// Line height in pixels
    pub line_height: u32,
    /// Width of CJK characters (approximation)
    pub cjk_width: u32,
    /// Memoized non-CJK glyph advance widths (shared across clones)
    width_cache: Arc<Mutex<HashMap<char, u32>>>,
}

impl FontInfo {
    /// Create a new FontInfo
    pub fn new(font: Font, fill: Rgba<u8>, size: f32) -> Self {
        let line_height = (size * 1.2) as u32;
        let advance = font.advance('中', size);
        let cjk_width = if advance > 0.0 {
            advance.ceil().max(1.0) as u32
        } else {
            size.ceil().max(1.0) as u32
        };
        Self {
            font,
            fill,
            size,
            embolden: 0.0,
            line_height,
            cjk_width,
            width_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn with_embolden(mut self, embolden: f32) -> Self {
        self.embolden = embolden;
        self
    }

    /// Check if a character is CJK
    pub fn is_cjk(c: char) -> bool {
        ('\u{4e00}'..='\u{9fff}').contains(&c)
            || ('\u{3400}'..='\u{4dbf}').contains(&c) // CJK Extension A
            || ('\u{f900}'..='\u{faff}').contains(&c) // CJK Compatibility
            || ('\u{3000}'..='\u{303f}').contains(&c) // CJK Punctuation
    }

    /// Get the width of a character: fixed-width fast path for CJK, real
    /// (memoized) glyph advance for everything else so wide glyphs like
    /// 'W'/'M' don't overflow the card edge.
    pub fn get_char_width(&self, c: char) -> u32 {
        if Self::is_cjk(c) {
            return self.cjk_width;
        }

        if let Some(width) = self.width_cache.lock().get(&c) {
            return *width;
        }

        let advance = self.font.advance(c, self.size);
        let width = if advance > 0.0 {
            advance.ceil() as u32
        } else {
            // Glyph missing from the font: keep the old approximation.
            (self.size * 0.6) as u32
        };
        self.width_cache.lock().insert(c, width);
        width
    }

    /// Calculate the width of a text string, accounting for inline emoji segments.
    pub fn get_text_width(&self, text: &str) -> u32 {
        use super::emoji::{TextSegment, segment_text};

        let mut width = 0u32;
        for segment in segment_text(text) {
            match segment {
                TextSegment::Text(part) => {
                    for c in part.chars() {
                        width += self.get_char_width(c);
                    }
                }
                TextSegment::Emoji { .. } => {
                    width += self.size as u32;
                }
            }
        }
        width
    }
}

/// A set of fonts for different text types
#[derive(Clone)]
pub struct FontSet {
    /// Font for author name
    pub name: FontInfo,
    /// Font for title
    pub title: FontInfo,
    /// Font for SplitHero right-column title
    pub split_title: FontInfo,
    /// Font for SplitHero right-column body
    pub split_text: FontInfo,
    /// Font for body text (dark gray)
    pub text: FontInfo,
    /// Font for extra info (medium gray)
    pub extra: FontInfo,
    /// Font for tag pills
    pub tag: FontInfo,
    /// Font for frosted corner media count badges
    pub badge: FontInfo,
    /// Font for repost eyebrow label
    pub eyebrow: FontInfo,
    /// Font for "+N" indicator (white, large)
    pub indicator: FontInfo,
}

impl FontSet {
    /// Create a new FontSet from a font
    pub fn new(font: Font) -> Self {
        let mut fonts = Self {
            name: FontInfo::new(font.clone(), COLOR_NAME, FONT_SIZE_NAME).with_embolden(0.65),
            title: FontInfo::new(font.clone(), COLOR_TITLE, FONT_SIZE_TITLE).with_embolden(0.90),
            split_title: FontInfo::new(font.clone(), COLOR_TITLE, 30.0).with_embolden(0.90),
            split_text: FontInfo::new(font.clone(), COLOR_TEXT, 22.0),
            text: FontInfo::new(font.clone(), COLOR_TEXT, FONT_SIZE_TEXT),
            extra: FontInfo::new(font.clone(), COLOR_EXTRA, FONT_SIZE_EXTRA),
            tag: FontInfo::new(font.clone(), COLOR_EXTRA, FONT_SIZE_TAG).with_embolden(0.35),
            badge: FontInfo::new(font.clone(), COLOR_INDICATOR, 15.0).with_embolden(0.35),
            eyebrow: FontInfo::new(font.clone(), COLOR_EXTRA, 16.0).with_embolden(0.45),
            indicator: FontInfo::new(font, COLOR_INDICATOR, FONT_SIZE_INDICATOR).with_embolden(1.0),
        };
        fonts.name.line_height = 34;
        fonts.title.line_height = 46;
        fonts.split_title.line_height = 41;
        fonts.split_text.line_height = 34;
        fonts.text.line_height = 38;
        fonts.extra.line_height = CAPTION_HEIGHT;
        fonts.tag.line_height = (FONT_SIZE_TAG * 1.12).round() as u32;
        fonts.eyebrow.line_height = 18;
        fonts
    }

    /// Compact crisp FontSet for natively laid out repost sections (no bitmap downscaling)
    pub fn repost(&self) -> Self {
        let font = self.text.font.clone();
        let mut fonts = Self {
            name: FontInfo::new(font.clone(), self.name.fill, 21.0).with_embolden(0.55),
            title: FontInfo::new(font.clone(), self.title.fill, 26.0).with_embolden(0.75),
            split_title: FontInfo::new(font.clone(), self.title.fill, 24.0).with_embolden(0.70),
            split_text: FontInfo::new(font.clone(), self.text.fill, 19.0),
            text: FontInfo::new(font.clone(), self.text.fill, 21.0),
            extra: FontInfo::new(font.clone(), self.extra.fill, 16.0),
            tag: FontInfo::new(font.clone(), self.extra.fill, 15.0).with_embolden(0.35),
            badge: self.badge.clone(),
            eyebrow: self.eyebrow.clone(),
            indicator: FontInfo::new(font, COLOR_INDICATOR, 48.0).with_embolden(0.9),
        };
        fonts.name.line_height = 28;
        fonts.title.line_height = 36;
        fonts.split_title.line_height = 34;
        fonts.split_text.line_height = 28;
        fonts.text.line_height = 32;
        fonts.extra.line_height = 24;
        fonts.tag.line_height = (15.0_f32 * 1.12).round() as u32;
        fonts
    }

    /// Return a copy of this FontSet tinted for light or dark polarity, preserving glyph caches.
    pub fn with_dark(&self, is_dark: bool) -> Self {
        let mut fonts = self.clone();
        if is_dark {
            fonts.name.fill = Rgba([244, 242, 240, 255]);
            fonts.title.fill = Rgba([252, 250, 248, 255]);
            fonts.split_title.fill = Rgba([252, 250, 248, 255]);
            fonts.split_text.fill = Rgba([212, 210, 220, 255]);
            fonts.text.fill = Rgba([212, 210, 220, 255]);
            fonts.extra.fill = Rgba([164, 162, 176, 255]);
        }
        fonts
    }
}

// ============================================================================
// Section Data Types
// ============================================================================

/// Header section data (avatar + name + time)
pub struct HeaderSectionData {
    /// Section height
    pub height: u32,
    /// Processed avatar image (circular)
    pub avatar: Option<RgbaImage>,
    /// Author name
    pub name: String,
    /// Formatted timestamp
    pub time: Option<String>,
    /// Height of text content
    pub text_height: u32,
}

/// Title section data
pub struct TitleSectionData {
    /// Section height
    pub height: u32,
    /// Wrapped title lines
    pub lines: Vec<String>,
}

/// Cover section data (cover image with optional video button)
pub struct CoverSectionData {
    /// Section height
    pub height: u32,
    /// Resized cover image
    pub cover_img: RgbaImage,
    /// Whether this cover is an unloaded placeholder (excluded from theme sampling)
    pub is_placeholder: bool,
    /// Whether to show video button
    pub show_video_button: bool,
    pub duration: Option<String>,
}

/// Text section data (body text)
pub struct TextSectionData {
    /// Section height
    pub height: u32,
    /// Wrapped text lines
    pub lines: Vec<String>,
}

/// Extra info section data
pub struct ExtraSectionData {
    /// Section height
    pub height: u32,
    /// Wrapped rows of tag pills
    pub tag_rows: Vec<Vec<TagData>>,
    /// Wrapped rows of interaction icons and their values.
    pub metric_rows: Vec<Vec<MetricData>>,
    /// Wrapped extra info lines
    pub lines: Vec<String>,
}

pub struct TagData {
    pub text: String,
    pub width: u32,
}

pub struct MetricData {
    pub icon: super::metadata::MetricIcon,
    pub value: String,
    pub width: u32,
}

/// Split portrait hero section: vertical video poster on the left, title & text on the right.
pub struct SplitHeroSectionData {
    /// Section height
    pub height: u32,
    /// Compact portrait cover on the left
    pub cover: CoverSectionData,
    /// Wrapped title lines on the right
    pub title_lines: Vec<String>,
    /// Wrapped body lines on the right (empty when long body flows below full-width)
    pub text_lines: Vec<String>,
    /// Horizontal offset from section left edge where the right text column starts
    pub text_x_offset: u32,
}

/// Positioned tile in an adaptive Bento image gallery.
pub struct GridTile {
    pub image: RgbaImage,
    pub x: u32,
    pub y: u32,
    pub is_placeholder: bool,
}

/// Image grid section data
pub struct ImageGridSectionData {
    /// Section height
    pub height: u32,
    /// Processed and positioned Bento grid tiles
    pub tiles: Vec<GridTile>,
    /// Total number of images in the post (including hidden ones past MAX_IMAGES_DISPLAY)
    pub total_count: usize,
    /// Whether there are more images
    pub has_more: bool,
    /// Number of remaining images not shown
    pub remaining_count: usize,
}

/// Graphics section data (text + image pairs)
pub struct GraphicsSectionData {
    /// Section height
    pub height: u32,
    /// Text lines before image
    pub text_lines: Vec<String>,
    /// The image
    pub image: RgbaImage,
    /// Whether this image is an unloaded placeholder
    pub is_placeholder: bool,
    /// Alt text for image
    pub alt_text: Option<String>,
}

/// Repost section data (natively laid out nested sections)
pub struct RepostSectionData {
    /// Section height
    pub height: u32,
    /// Inner content width for nested sections
    pub content_width: u32,
    /// Natively laid out sections inside the repost quote container
    pub sections: Vec<SectionData>,
}

/// Enum for all section types
pub enum SectionData {
    Header(HeaderSectionData),
    Title(TitleSectionData),
    Cover(CoverSectionData),
    SplitHero(SplitHeroSectionData),
    Text(TextSectionData),
    Extra(ExtraSectionData),
    ImageGrid(ImageGridSectionData),
    Graphics(GraphicsSectionData),
    Repost(RepostSectionData),
}

impl SectionData {
    /// Get the height of this section
    pub fn height(&self) -> u32 {
        match self {
            SectionData::Header(s) => s.height,
            SectionData::Title(s) => s.height,
            SectionData::Cover(s) => s.height,
            SectionData::SplitHero(s) => s.height,
            SectionData::Text(s) => s.height,
            SectionData::Extra(s) => s.height,
            SectionData::ImageGrid(s) => s.height,
            SectionData::Graphics(s) => s.height,
            SectionData::Repost(s) => s.height,
        }
    }
}

// ============================================================================
// Render Context
// ============================================================================

/// Context for rendering operations
pub struct RenderContext {
    /// Left X origin for section content
    pub x_pos: u32,
    /// Content width (card_width - 2 * padding for main card, narrower for repost)
    pub content_width: u32,
    /// Current Y position for drawing
    pub y_pos: u32,
    /// Whether this is the main card (not a repost)
    pub is_main: bool,
    /// Whether the card uses the dark frosted glass polarity
    pub is_dark: bool,
    /// Vertical spacing between sections
    pub section_spacing: u32,
    /// Avatar diameter for header sections
    pub avatar_size: u32,
    pub accent: Rgba<u8>,
    pub extra_fill: Rgba<u8>,
}

impl RenderContext {
    /// Create a new render context
    pub fn new(card_width: u32, is_main: bool) -> Self {
        Self {
            x_pos: PADDING,
            content_width: card_width - 2 * PADDING,
            y_pos: PADDING,
            is_main,
            is_dark: false,
            section_spacing: SECTION_SPACING,
            avatar_size: AVATAR_SIZE,
            accent: Rgba([251, 71, 128, 255]),
            extra_fill: COLOR_EXTRA,
        }
    }

    /// Advance Y position by the given amount plus section spacing
    pub fn advance(&mut self, height: u32) {
        self.y_pos += height + self.section_spacing;
    }
}

// ============================================================================
// Resources
// ============================================================================

/// Loaded resources for rendering
pub struct RenderResources {
    /// Main card font set
    pub fonts: FontSet,
    /// Prebuilt compact font set for nested repost sections (shares cached advances)
    pub repost_fonts: FontSet,
    /// Platform logos (pre-scaled to fit the header bounding box)
    pub logos: HashMap<Platform, RgbaImage>,
}
