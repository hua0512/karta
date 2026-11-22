//! Constants for the image renderer

use image::Rgba;

// ============================================================================
// Card Layout Constants
// ============================================================================

/// Default card width in pixels
pub const CARD_WIDTH: u32 = 800;

/// Inner padding around card edges
pub const PADDING: u32 = 48;

pub const BRAND_HEADER_HEIGHT: u32 = 56;
pub const CAPTION_HEIGHT: u32 = 27;

/// Outer paper bounds leave space for a diffused shadow.
pub const PAPER_INSET: u32 = 24;
pub const PAPER_TOP: u32 = 20;
pub const PAPER_BOTTOM: u32 = 46;
pub const PAPER_RADIUS: u32 = 32;
pub const SHADOW_BOTTOM_PADDING: u32 = 24;

/// Avatar dimensions (square)
pub const AVATAR_SIZE: u32 = 64;

/// Space between avatar and text
pub const AVATAR_TEXT_GAP: u32 = 15;

/// Space between sections
pub const SECTION_SPACING: u32 = 20;

/// Space between name and timestamp
pub const NAME_TIME_GAP: u32 = 5;

/// Avatar supersampling factor for anti-aliasing
pub const AVATAR_UPSCALE_FACTOR: u32 = 2;

// ============================================================================
// Image Processing Constants
// ============================================================================

/// Maximum cover image height
pub const MAX_COVER_HEIGHT: u32 = 800;

/// Space between grid images
pub const IMAGE_GRID_SPACING: u32 = 10;

/// Maximum number of images to display in grid
pub const MAX_IMAGES_DISPLAY: usize = 9;

// ============================================================================
// Repost Constants
// ============================================================================

/// Horizontal padding inside repost container
pub const REPOST_PADDING_X: u32 = 22;

/// Vertical padding inside repost container
pub const REPOST_PADDING_Y: u32 = 18;

/// Border radius for repost container
pub const REPOST_BORDER_RADIUS: u32 = 16;

/// Height of the inline repost eyebrow row (icon + "转发")
pub const REPOST_EYEBROW_HEIGHT: u32 = 22;

/// Space between the repost eyebrow and nested sections
pub const REPOST_EYEBROW_GAP: u32 = 12;

/// Vertical spacing between sections inside a repost
pub const REPOST_SECTION_SPACING: u32 = 14;

/// Avatar size inside a repost
pub const REPOST_AVATAR_SIZE: u32 = 44;

/// Repost header text
pub const REPOST_HEADER_TEXT: &str = "转发";

// ============================================================================
// Font Sizes
// ============================================================================

/// Font size for author name
pub const FONT_SIZE_NAME: f32 = 25.0;

/// Font size for title
pub const FONT_SIZE_TITLE: f32 = 34.0;

/// Font size for body text
pub const FONT_SIZE_TEXT: f32 = 24.0;

/// Font size for extra info
pub const FONT_SIZE_EXTRA: f32 = 18.0;

/// Font size for tag pills
pub const FONT_SIZE_TAG: f32 = 16.0;

/// Font size for "+N" indicator
pub const FONT_SIZE_INDICATOR: f32 = 60.0;

// ============================================================================
// Colors
// ============================================================================

/// Light gray placeholder background for unloaded media
pub const REPOST_BG_COLOR: Rgba<u8> = Rgba([247, 247, 247, 255]);

/// Author name color
pub const COLOR_NAME: Rgba<u8> = Rgba([22, 25, 31, 255]);

/// Title color
pub const COLOR_TITLE: Rgba<u8> = Rgba([18, 21, 26, 255]);

/// Dark gray color for body text
pub const COLOR_TEXT: Rgba<u8> = Rgba([58, 60, 66, 255]);

/// Medium gray color for extra info
pub const COLOR_EXTRA: Rgba<u8> = Rgba([112, 102, 108, 255]);

/// White color for indicator text
pub const COLOR_INDICATOR: Rgba<u8> = Rgba([255, 255, 255, 255]);

/// Transparent color
pub const TRANSPARENT: Rgba<u8> = Rgba([0, 0, 0, 0]);

// ============================================================================
// Resource Paths
// ============================================================================

/// Default font filename
pub const DEFAULT_FONT_FILENAME: &str = "HYSongYunLangHeiW-1.ttf";
