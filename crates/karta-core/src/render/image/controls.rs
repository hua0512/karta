//! Compact, antialiased video controls with a frosted cover backdrop.

use image::{Pixel, Rgba, RgbaImage, imageops};
use imageproc::rect::Rect;

use super::types::{CoverSectionData, FontInfo};
use super::utils::paste_image;

const INSET: u32 = 16;
const BADGE_HEIGHT: u32 = 32;
const WHITE: Rgba<u8> = Rgba([255, 255, 255, 245]);

pub(super) fn draw(
    image: &mut RgbaImage,
    data: &CoverSectionData,
    x: u32,
    y: u32,
    font: &FontInfo,
) {
    let cover = &data.cover_img;
    let badge = data
        .duration
        .as_deref()
        .and_then(|duration| duration_badge(cover, duration, font));

    let side = cover.width().min(cover.height());
    let diameter = (side / 6).clamp(44, 68);
    if data.show_video_button && side >= diameter + 2 * INSET {
        let bounds = Rect::at(
            ((cover.width() - diameter) / 2) as i32,
            ((cover.height() - diameter) / 2) as i32,
        )
        .of_size(diameter, diameter);
        // On very small covers, keep the duration readable without overlap.
        if badge
            .as_ref()
            .is_none_or(|(badge_bounds, _)| bounds.intersect(*badge_bounds).is_none())
        {
            let mut button = surface(cover, bounds, 120);
            draw_play_icon(&mut button);
            paste_image(
                image,
                &button,
                x + bounds.left() as u32,
                y + bounds.top() as u32,
            );
        }
    }
    if let Some((bounds, badge)) = badge {
        paste_image(
            image,
            &badge,
            x + bounds.left() as u32,
            y + bounds.top() as u32,
        );
    }
}

/// Draw a compact frosted-glass corner badge (e.g. "共 6 图") on the bottom-right of a media tile.
pub(super) fn draw_corner_badge(
    image: &mut RgbaImage,
    tile: &RgbaImage,
    x: u32,
    y: u32,
    label: &str,
    badge_font: &FontInfo,
) {
    if let Some((bounds, badge)) = pill_badge(tile, label, badge_font, 12, 28, 22) {
        paste_image(
            image,
            &badge,
            x + bounds.left() as u32,
            y + bounds.top() as u32,
        );
    }
}

fn duration_badge(cover: &RgbaImage, duration: &str, font: &FontInfo) -> Option<(Rect, RgbaImage)> {
    pill_badge(cover, duration, font, INSET, BADGE_HEIGHT, 28)
}

fn pill_badge(
    cover: &RgbaImage,
    label: &str,
    font: &FontInfo,
    inset: u32,
    badge_height: u32,
    pad_x: u32,
) -> Option<(Rect, RgbaImage)> {
    // Center the visible glyphs rather than the font's ascent/descent box.
    let mut text = RgbaImage::new(font.get_text_width(label) + 8, font.line_height + 4);
    font.font
        .draw_weighted(&mut text, WHITE, 0, 0, font.size, font.embolden, label);
    let mut left = text.width();
    let mut top = text.height();
    let (mut right, mut bottom) = (0, 0);
    for (x, y, pixel) in text.enumerate_pixels() {
        if pixel[3] > 0 {
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + 1);
            bottom = bottom.max(y + 1);
        }
    }
    if left >= right || top >= bottom {
        return None;
    }
    let text = imageops::crop_imm(&text, left, top, right - left, bottom - top).to_image();
    let width = (text.width() + pad_x).max(52);
    if cover.width() < width + 2 * inset || cover.height() < badge_height + 2 * inset {
        return None;
    }
    let bounds = Rect::at(
        (cover.width() - width - inset) as i32,
        (cover.height() - badge_height - inset) as i32,
    )
    .of_size(width, badge_height);
    let mut badge = surface(cover, bounds, 175);
    paste_image(
        &mut badge,
        &text,
        (width - text.width()) / 2,
        badge_height.saturating_sub(text.height()) / 2,
    );
    Some((bounds, badge))
}

fn surface(cover: &RgbaImage, bounds: Rect, tint: u8) -> RgbaImage {
    let source = imageops::crop_imm(
        cover,
        bounds.left() as u32,
        bounds.top() as u32,
        bounds.width(),
        bounds.height(),
    )
    .to_image();
    // Blur only the few thousand pixels underneath each control.
    let mut image = imageops::fast_blur(&source, 4.0);
    let half_width = bounds.width() as f32 / 2.0;
    let radius = bounds.height() as f32 / 2.0;
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let dx = ((x as f32 + 0.5 - half_width).abs() - (half_width - radius)).max(0.0);
        let dy = y as f32 + 0.5 - radius;
        let distance = dx.hypot(dy) - radius;
        let coverage = (0.5 - distance).clamp(0.0, 1.0);
        pixel.blend(&Rgba([20, 23, 29, tint]));
        // A restrained one-pixel rim separates glass from both light and dark media.
        let rim = (distance + 1.5).clamp(0.0, 1.0);
        pixel.blend(&Rgba([255, 255, 255, (rim * 48.0) as u8]));
        pixel[3] = (f32::from(pixel[3]) * coverage) as u8;
    }
    image
}

fn draw_play_icon(image: &mut RgbaImage) {
    let size = image.width() as f32;
    // Shift the triangle slightly right so its visual center matches the circle.
    let points = [(0.42, 0.34), (0.42, 0.66), (0.68, 0.50)].map(|(x, y)| (x * size, y * size));
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let (x, y) = (x as f32 + 0.5, y as f32 + 0.5);
        let mut distance = f32::MAX;
        let mut inside = true;
        for index in 0..3 {
            let (ax, ay) = points[index];
            let (bx, by) = points[(index + 1) % 3];
            let (vx, vy) = (bx - ax, by - ay);
            let t = (((x - ax) * vx + (y - ay) * vy) / (vx * vx + vy * vy)).clamp(0.0, 1.0);
            distance = distance.min((x - ax - t * vx).hypot(y - ay - t * vy));
            inside &= vx * (y - ay) - vy * (x - ax) <= 0.0;
        }
        let signed_distance = if inside { -distance } else { distance };
        let coverage = (1.5 - signed_distance).clamp(0.0, 1.0);
        pixel.blend(&Rgba([255, 255, 255, (coverage * 245.0) as u8]));
    }
}
