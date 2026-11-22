//! Image processing utilities

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::SystemTime;

use fast_image_resize::FilterType;
use image::{GrayImage, Luma, Rgba, RgbaImage, imageops};
use imageproc::drawing::draw_filled_circle_mut;
use imageproc::rect::Rect;

use super::constants::*;
use super::processing::{self, Thumbnail};

#[derive(Clone, PartialEq, Eq, Hash)]
struct ProcessedImageKey {
    path: PathBuf,
    modified: Option<SystemTime>,
    bytes: u64,
    kind: u8,
    width: u32,
    height: u32,
    radius: u32,
    square: bool,
}

// Cache only bounded, display-sized pixels, not multi-megapixel source images.
static PROCESSED_IMAGES: OnceLock<crate::cache::Cache<ProcessedImageKey, Arc<RgbaImage>>> =
    OnceLock::new();

#[allow(clippy::too_many_arguments)]
fn processed_image(
    path: &Path,
    kind: u8,
    width: u32,
    height: u32,
    radius: u32,
    square: bool,
    process: impl FnOnce() -> Option<RgbaImage>,
) -> Option<RgbaImage> {
    let metadata = std::fs::metadata(path).ok()?;
    let key = ProcessedImageKey {
        path: path.to_path_buf(),
        modified: metadata.modified().ok(),
        bytes: metadata.len(),
        kind,
        width,
        height,
        radius,
        square,
    };
    let cache = PROCESSED_IMAGES.get_or_init(|| crate::cache::Cache::new(32));
    if let Some(image) = cache.get_cloned(&key) {
        return Some(image.as_ref().clone());
    }
    let image = Arc::new(process()?);
    cache.insert(key, image.clone());
    Some(image.as_ref().clone())
}

pub fn load_grid_image(path: &Path, size: u32, square: bool) -> Option<RgbaImage> {
    processed_image(path, 0, size, size, 16, square, || {
        let mut image = processing::load_thumbnail(path, Thumbnail::Grid { size, square })?;
        super::corners::clip(&mut image, 16);
        Some(image)
    })
}

pub fn load_cropped_image(path: &Path, width: u32, height: u32, radius: u32) -> Option<RgbaImage> {
    processed_image(path, 4, width, height, radius, false, || {
        let mut image = processing::load_thumbnail(path, Thumbnail::Crop { width, height })?;
        super::corners::clip(&mut image, radius);
        Some(image)
    })
}

pub fn load_cover_image(path: &Path, width: u32) -> Option<RgbaImage> {
    processed_image(path, 1, width, 0, 20, false, || {
        let mut image = processing::load_thumbnail(path, Thumbnail::Cover { width })?;
        super::corners::clip(&mut image, 20);
        Some(image)
    })
}

pub fn load_avatar_image(path: &Path, size: u32) -> Option<RgbaImage> {
    processed_image(path, 2, size, size, size / 2, false, || {
        let source = processing::load_thumbnail(
            path,
            Thumbnail::Grid {
                size: size * AVATAR_UPSCALE_FACTOR,
                square: true,
            },
        )?;
        Some(process_avatar(source, size))
    })
}

pub fn load_graphics_image(path: &Path, width: u32) -> Option<RgbaImage> {
    processed_image(path, 3, width, 0, 0, false, || {
        processing::load_thumbnail(
            path,
            Thumbnail::Grid {
                size: width,
                square: false,
            },
        )
    })
}

/// Load an image from a path
pub fn load_image(path: &Path) -> Option<RgbaImage> {
    image::open(path).ok().map(|img| img.into_rgba8())
}

/// Process an avatar image into a circular shape with anti-aliasing.
///
/// Uses 2x supersampling for smooth edges; polarity-aware rim is drawn at composite time.
pub fn process_avatar(img: RgbaImage, size: u32) -> RgbaImage {
    let upscale_size = size * AVATAR_UPSCALE_FACTOR;

    // Resize to upscaled size
    let resized = processing::resize(&img, upscale_size, upscale_size, FilterType::Lanczos3);

    // Create circular mask
    let mut mask = GrayImage::new(upscale_size, upscale_size);
    let center = (upscale_size / 2) as i32;
    let radius = (upscale_size / 2) as i32 - 1;

    // Draw filled circle on mask
    draw_filled_circle_mut(&mut mask, (center, center), radius, Luma([255]));

    // Apply mask to create circular avatar
    let mut output = RgbaImage::new(upscale_size, upscale_size);
    for (x, y, pixel) in resized.enumerate_pixels() {
        let mask_value = mask.get_pixel(x, y).0[0];
        if mask_value > 0 {
            output.put_pixel(x, y, *pixel);
        } else {
            output.put_pixel(x, y, TRANSPARENT);
        }
    }

    // Downscale to final size with anti-aliasing
    processing::resize(&output, size, size, FilterType::Lanczos3)
}

/// Draw a subtle 1px antialiased circular rim around an avatar (polarity-aware).
pub fn draw_circle_ring(img: &mut RgbaImage, x: u32, y: u32, size: u32, color: Rgba<u8>) {
    use image::Pixel;

    if size < 4 || color[3] == 0 {
        return;
    }
    let center = size as f32 / 2.0;
    let outer_r = center - 0.5;
    let inner_r = outer_r - 1.0;
    for dy in 0..size {
        let py = y + dy;
        if py >= img.height() {
            break;
        }
        let fy = dy as f32 + 0.5 - center;
        for dx in 0..size {
            let px = x + dx;
            if px >= img.width() {
                break;
            }
            let dist = (dx as f32 + 0.5 - center).hypot(fy);
            let outer_cov = (outer_r + 0.5 - dist).clamp(0.0, 1.0);
            let inner_cov = (inner_r + 0.5 - dist).clamp(0.0, 1.0);
            let cov = (outer_cov - inner_cov).clamp(0.0, 1.0);
            if cov > 0.0 {
                let mut ink = color;
                ink[3] = (f32::from(color[3]) * cov).round() as u8;
                img.get_pixel_mut(px, py).blend(&ink);
            }
        }
    }
}

/// Draw a filled rounded rectangle
pub fn draw_rounded_rect(
    img: &mut RgbaImage,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    radius: u32,
    color: Rgba<u8>,
) {
    if width == 0 || height == 0 {
        return;
    }
    super::corners::fill(
        img,
        Rect::at(x as i32, y as i32).of_size(width, height),
        radius,
        false,
        |_| color,
    );
}

/// Paste one image onto another at the specified position
pub fn paste_image(dest: &mut RgbaImage, src: &RgbaImage, x: u32, y: u32) {
    // Fast path: `overlay` handles alpha compositing internally.
    imageops::overlay(dest, src, x as i64, y as i64);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn processed_image_cache_observes_replaced_files_and_target_sizes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("image.png");
        RgbaImage::from_pixel(80, 80, Rgba([255, 0, 0, 255]))
            .save(&path)
            .unwrap();
        let red = load_grid_image(&path, 32, true).unwrap();
        assert_eq!(red.dimensions(), (32, 32));
        assert_eq!(red.get_pixel(16, 16).0, [255, 0, 0, 255]);
        assert_eq!(
            load_grid_image(&path, 24, true).unwrap().dimensions(),
            (24, 24)
        );
        RgbaImage::from_pixel(120, 60, Rgba([0, 255, 0, 255]))
            .save(&path)
            .unwrap();
        let green = load_grid_image(&path, 32, true).unwrap();
        assert_eq!(green.get_pixel(16, 16).0, [0, 255, 0, 255]);
    }
}
