use fast_image_resize::FilterType;
use image::{Pixel, Rgb, RgbImage, Rgba, RgbaImage, imageops};
use imageproc::rect::Rect;
use std::time::Instant;

use crate::config::ImageHeaderStyle;
use crate::constants::Platform;

use super::backdrop::Backdrop;
use super::constants::*;
use super::corners::{self, CornerMask};
use super::palette::{CardTheme, media_images, mix};
use super::processing::resize;
use super::shadow;
use super::text::{EmojiImageMap, draw_single_line};
use super::types::{FontInfo, RenderResources, SectionData};
use super::utils::paste_image;

fn paper_washes(image: &mut RgbaImage, theme: &CardTheme, bounds: Rect) {
    corners::fill(image, bounds, PAPER_RADIUS, false, |y| {
        let top_amount = (1.0 - y as f32 / 205.0).clamp(0.0, 1.0);
        let bottom_amount = (1.0 - (bounds.height() - y) as f32 / 100.0).clamp(0.0, 1.0);
        let top_amount = top_amount * top_amount * (3.0 - 2.0 * top_amount);
        let color = mix(theme.paper, theme.wash, top_amount);
        mix(color, theme.bottom, bottom_amount * 0.65)
    });
}

fn mesh_glow(image: &mut RgbaImage, theme: &CardTheme, bounds: Rect) {
    let height = 230.min(bounds.height());
    if height == 0 || bounds.width() == 0 {
        return;
    }
    let primary = if theme.is_dark {
        mix(theme.primary, Rgba([245, 245, 255, 255]), 0.30)
    } else {
        mix(theme.primary, Rgba([255, 255, 255, 255]), 0.10)
    };
    let secondary = if theme.is_dark {
        mix(theme.secondary, Rgba([245, 245, 255, 255]), 0.34)
    } else {
        mix(theme.secondary, Rgba([255, 255, 255, 255]), 0.14)
    };
    let accent_hi = if theme.is_dark {
        mix(primary, Rgba([255, 255, 255, 255]), 0.35)
    } else {
        mix(secondary, Rgba([255, 255, 255, 255]), 0.35)
    };

    let tiny = RgbImage::from_fn(128, 48, |x, y| {
        let u = x as f32 / 127.0;
        let v = y as f32 / 47.0;
        let g1 = (-((u - 0.80).powi(2) / 0.16 + (v - 0.06).powi(2) / 0.38)).exp();
        let g2 = (-((u - 0.36).powi(2) / 0.20 + (v + 0.05).powi(2) / 0.32)).exp();
        let g3 = (-((u - 0.60).powi(2) / 0.09 + (v - 0.18).powi(2) / 0.22)).exp();
        let c = mix(theme.paper, primary, (g1 * 0.82).min(1.0));
        let c = mix(c, secondary, (g2 * 0.68).min(1.0));
        let c = mix(c, accent_hi, (g3 * 0.36).min(1.0));
        Rgb([c[0], c[1], c[2]])
    });
    let blurred = image::DynamicImage::ImageRgb8(imageops::fast_blur(&tiny, 4.0)).into_rgba8();
    let wash = resize(&blurred, bounds.width(), height, FilterType::Bilinear);
    let mask = CornerMask::for_size(bounds.width(), bounds.height(), PAPER_RADIUS);
    let max_alpha = if theme.is_dark { 135.0_f32 } else { 185.0_f32 };
    let power = if theme.is_dark { 1.80_f32 } else { 1.65_f32 };
    let left = bounds.left() as u32;
    let top = bounds.top() as u32;
    let max_y = height.min(image.height().saturating_sub(top));
    let max_x = bounds.width().min(image.width().saturating_sub(left));

    for y in 0..max_y {
        let fade = (1.0 - y as f32 / height as f32).powf(power);
        for x in 0..max_x {
            let cov = mask.rect_coverage(x, y, bounds.width(), bounds.height());
            let a = (fade * max_alpha * f32::from(cov) / 255.0).round() as u8;
            if a == 0 {
                continue;
            }
            let mut px = *wash.get_pixel(x, y);
            px[3] = a;
            image.get_pixel_mut(left + x, top + y).blend(&px);
        }
    }
}

fn color_field(sections: &[SectionData], theme: &CardTheme) -> RgbaImage {
    let sources: Vec<_> = media_images(sections)
        .filter(|image| !image.is_empty())
        .take(9)
        .collect();
    let tiny = RgbImage::from_fn(96, 40, |x, y| {
        let u = x as f32 / 95.0;
        let v = y as f32 / 39.0;
        let color = if sources.is_empty() {
            // An actual palette mesh for text-only cards, with no empty cover,
            // gray photo placeholder or dependency on a downloaded avatar.
            let primary = (-((u - 0.9).powi(2) / 0.22 + (v - 0.15).powi(2) / 0.8)).exp();
            let secondary = (-((u - 0.40).powi(2) / 0.16 + (v - 0.3).powi(2) / 0.5)).exp();
            let color = mix(theme.paper, theme.primary, primary * 0.60);
            mix(color, theme.secondary, secondary * 0.50)
        } else {
            let tile_width = 32;
            let source = sources[match x / tile_width {
                0 => 0,
                1 => sources.len() / 2,
                _ => sources.len() - 1,
            }];
            let sx = ((x % tile_width) * source.width() / tile_width).min(source.width() - 1);
            let sy = (y * source.height() / 40).min(source.height() - 1);
            let pixel = source.get_pixel(sx, sy);
            let color = mix(theme.paper, *pixel, 0.40 * f32::from(pixel[3]) / 255.0);
            let halo = (-((u - 0.75).powi(2) / 0.18 + (v - 0.1).powi(2) / 0.5)).exp();
            mix(color, mix(theme.secondary, theme.paper, 0.35), halo * 0.35)
        };
        let light = (-((u - 0.55).powi(2) / 0.025 + (v - 0.38).powi(2) / 0.10)).exp();
        let color = mix(color, theme.paper, light * 0.72);
        Rgb([color[0], color[1], color[2]])
    });
    // Blur only 3,840 opaque pixels; photographs and the full card stay sharp.
    image::DynamicImage::ImageRgb8(imageops::fast_blur(&tiny, 6.5)).into_rgba8()
}

fn blurred_header(
    image: &mut RgbaImage,
    sections: &[SectionData],
    theme: &CardTheme,
    bounds: Rect,
) {
    let field = color_field(sections, theme);
    let height = 210.min(bounds.height());
    let mut wash = resize(&field, bounds.width(), height, FilterType::Bilinear);
    for y in 0..height {
        let fade = 1.0 - y as f32 / height as f32;
        for x in 0..wash.width() {
            let right_bias = 0.40 + 0.60 * x as f32 / wash.width() as f32;
            wash.get_pixel_mut(x, y)[3] = (fade.powf(1.35) * right_bias * 245.0) as u8;
        }
    }
    round_corners(&mut wash, PAPER_RADIUS);
    paste_image(image, &wash, bounds.left() as u32, bounds.top() as u32);
}

fn soft_prism(image: &mut RgbaImage, theme: &CardTheme, bounds: Rect) {
    let height = 215.min(bounds.height());
    if height == 0 || bounds.width() == 0 {
        return;
    }
    let mask = CornerMask::for_size(bounds.width(), bounds.height(), PAPER_RADIUS);
    let left = bounds.left() as u32;
    let top = bounds.top() as u32;
    let bw = bounds.width();
    let bh = bounds.height();
    let max_y = height.min(image.height().saturating_sub(top));
    let max_x = bw.min(image.width().saturating_sub(left));

    let primary = if theme.is_dark {
        mix(theme.primary, Rgba([245, 245, 255, 255]), 0.30)
    } else {
        mix(theme.primary, theme.paper, 0.20)
    };
    let secondary = if theme.is_dark {
        mix(theme.secondary, Rgba([245, 245, 255, 255]), 0.34)
    } else {
        mix(theme.secondary, theme.paper, 0.24)
    };
    let layers = [
        (
            0.92_f32,
            -0.45_f32,
            0.78_f32,
            1.32_f32,
            secondary,
            if theme.is_dark { 0.22_f32 } else { 0.30_f32 },
            0.34_f32,
        ),
        (
            1.02,
            -0.32,
            0.56,
            1.12,
            primary,
            if theme.is_dark { 0.28 } else { 0.36 },
            0.42,
        ),
        (
            1.12,
            -0.22,
            0.38,
            0.94,
            secondary,
            if theme.is_dark { 0.24 } else { 0.28 },
            0.48,
        ),
    ];

    let denom_x = bw.saturating_sub(1).max(1) as f32;
    let denom_y = height.saturating_sub(1).max(1) as f32;
    for y in 0..max_y {
        let v = y as f32 / denom_y;
        let v_fade = (1.0 - v).powf(1.25);
        for x in 0..max_x {
            let cov = mask.rect_coverage(x, y, bw, bh);
            if cov == 0 {
                continue;
            }
            let u = x as f32 / denom_x;
            let dst = image.get_pixel_mut(left + x, top + y);
            for &(cx, cy, rx, ry, ink, fill_a, rim_a) in &layers {
                if u < cx - rx * 1.03 {
                    continue;
                }
                let d = ((u - cx) / rx).hypot((v - cy) / ry);
                let inside = ((1.0 - d) / 0.012).clamp(0.0, 1.0);
                if inside > 0.0 {
                    let a = (inside * v_fade * fill_a * f32::from(cov)).round() as u8;
                    if a > 0 {
                        let mut c = ink;
                        c[3] = a;
                        dst.blend(&c);
                    }
                }
                let rim_t = (d - 0.996) / 0.0085;
                let rim_t2 = rim_t * rim_t;
                if rim_t2 < 3.92 {
                    let rim = (-rim_t2).exp();
                    let ra = (rim * v_fade * rim_a * f32::from(cov)).round() as u8;
                    if ra > 0 {
                        let rim_alpha = if theme.is_dark {
                            ra
                        } else {
                            (f32::from(ra) * 1.15).min(255.0) as u8
                        };
                        dst.blend(&Rgba([255, 255, 255, rim_alpha]));
                    }
                }
            }
        }
    }
}

pub fn frame(
    image: &mut RgbaImage,
    platform: Platform,
    sections: &[SectionData],
    resources: &RenderResources,
    emojis: &EmojiImageMap,
    theme: &CardTheme,
    style: ImageHeaderStyle,
) {
    let (width, height) = image.dimensions();
    let bounds = Rect::at(PAPER_INSET as i32, PAPER_TOP as i32)
        .of_size(width - 2 * PAPER_INSET, height - PAPER_TOP - PAPER_BOTTOM);
    if style == ImageHeaderStyle::BlurBackground {
        let background_start = Instant::now();
        let backdrop = Backdrop::new(width, height, sections, theme);
        backdrop.paint_page(image, theme, bounds);
        let page_time = background_start.elapsed();
        let shadow_start = Instant::now();
        shadow::paint(image, bounds, theme.shadow);
        tracing::debug!(
            shadow_us = shadow_start.elapsed().as_micros() as u64,
            "Card diffused shadow"
        );
        let paper_start = Instant::now();
        backdrop.paint_paper(image, theme, bounds);
        if theme.is_dark {
            corners::stroke(image, bounds, PAPER_RADIUS, Rgba([255, 255, 255, 28]));
        }
        tracing::debug!(
            background_us = (page_time + paper_start.elapsed()).as_micros() as u64,
            header_style = style.as_str(),
            "Card full blurred background"
        );
        platform_logo(image, platform, sections, resources, emojis, theme);
        return;
    }
    let shadow_start = Instant::now();
    shadow::paint(image, bounds, theme.shadow);
    tracing::debug!(
        shadow_us = shadow_start.elapsed().as_micros() as u64,
        "Card diffused shadow"
    );
    paper_washes(image, theme, bounds);
    let header_start = Instant::now();
    match style {
        ImageHeaderStyle::MeshGlow => mesh_glow(image, theme, bounds),
        ImageHeaderStyle::Blur | ImageHeaderStyle::Auto => {
            blurred_header(image, sections, theme, bounds)
        }
        ImageHeaderStyle::SoftPrism => soft_prism(image, theme, bounds),
        ImageHeaderStyle::BlurBackground => unreachable!("full background was painted above"),
    }
    if theme.is_dark {
        corners::stroke(image, bounds, PAPER_RADIUS, Rgba([255, 255, 255, 28]));
    }
    tracing::debug!(
        header_us = header_start.elapsed().as_micros() as u64,
        header_style = style.as_str(),
        "Card header decoration"
    );
    platform_logo(image, platform, sections, resources, emojis, theme);
}

/// Adapt dark-ink logos (e.g. X, Douyin, TikTok, YouTube wordmark) for dark glass surfaces
/// by smoothly lifting low-chroma dark strokes to crisp light ink while keeping colored accents
/// and filled dark badge icons (such as `xiaoheihe.png`, which already contain a bright foreground
/// glyph inside an opaque dark tile) intact.
///
/// Returns `true` when the logo is a filled dark badge that benefits from a subtle light rim.
pub(super) fn adapt_logo_for_dark(logo: &mut RgbaImage) -> bool {
    let total_pixels = u64::from(logo.width()) * u64::from(logo.height());
    if total_pixels == 0 {
        return false;
    }
    let mut luma_sum = 0.0_f64;
    let mut opaque_count = 0_u64;
    let mut bright_count = 0_u64;
    for pixel in logo.pixels() {
        if pixel[3] > 128 {
            let luma = 0.299 * f64::from(pixel[0])
                + 0.587 * f64::from(pixel[1])
                + 0.114 * f64::from(pixel[2]);
            luma_sum += luma;
            opaque_count += 1;
            if luma > 195.0 {
                bright_count += 1;
            }
        }
    }
    if opaque_count == 0 || (luma_sum / opaque_count as f64) >= 92.0 {
        return false;
    }
    // Filled dark badge (e.g. black rounded square with white inner mark): keep original
    // artwork intact instead of inverting the background into a white square.
    if opaque_count * 100 >= total_pixels * 55 && bright_count * 100 >= opaque_count * 5 {
        return true;
    }
    for pixel in logo.pixels_mut() {
        if pixel[3] == 0 {
            continue;
        }
        let max = f32::from(pixel[0].max(pixel[1]).max(pixel[2]));
        let min = f32::from(pixel[0].min(pixel[1]).min(pixel[2]));
        let chroma = max - min;
        let luma =
            0.299 * f32::from(pixel[0]) + 0.587 * f32::from(pixel[1]) + 0.114 * f32::from(pixel[2]);
        // Smooth ramps across chroma [18..52] and luma [70..160] prevent hard gray edge seams.
        let chroma_w = ((52.0 - chroma) / 34.0).clamp(0.0, 1.0);
        let luma_w = ((160.0 - luma) / 90.0).clamp(0.0, 1.0);
        let lift = chroma_w * luma_w;
        if lift > 0.0 {
            let target = [242.0_f32, 242.0, 248.0];
            for ch in 0..3 {
                let src = f32::from(pixel[ch]);
                pixel[ch] = (src * (1.0 - lift) + target[ch] * lift).round() as u8;
            }
        }
    }
    false
}

fn platform_logo(
    image: &mut RgbaImage,
    platform: Platform,
    sections: &[SectionData],
    resources: &RenderResources,
    emojis: &EmojiImageMap,
    theme: &CardTheme,
) {
    let header_height = sections
        .iter()
        .find_map(|section| match section {
            SectionData::Header(header) => Some(header.height),
            _ => None,
        })
        .unwrap_or(BRAND_HEADER_HEIGHT);
    let max_width = 116;
    let max_height = 30;
    let right = image.width() - PADDING;
    if let Some(logo) = resources.logos.get(&platform) {
        // Draw the transparent source logo directly, retaining its aspect ratio.
        // Never upscale small crisp logos past 1:1.
        let scale = (max_width as f64 / f64::from(logo.width().max(1)))
            .min(max_height as f64 / f64::from(logo.height().max(1)))
            .min(1.0);
        let width = (f64::from(logo.width()) * scale).round().max(1.0) as u32;
        let height = (f64::from(logo.height()) * scale).round().max(1.0) as u32;
        let mut logo = resize(logo, width, height, FilterType::Bilinear);
        let needs_badge_rim = theme.is_dark && adapt_logo_for_dark(&mut logo);
        let lx = right - width;
        let ly = PADDING + header_height.saturating_sub(height) / 2;
        paste_image(image, &logo, lx, ly);
        if needs_badge_rim && width >= 6 && height >= 6 {
            corners::stroke(
                image,
                Rect::at(lx as i32, ly as i32).of_size(width, height),
                (width.min(height) / 5).max(2),
                Rgba([255, 255, 255, 42]),
            );
        }
    } else {
        let label = platform.to_string();
        let base_size = 21.0;
        let probe = FontInfo::new(resources.fonts.name.font.clone(), theme.accent, base_size);
        let measured = probe.get_text_width(&label).max(1);
        let size = if measured > max_width {
            (base_size * max_width as f32 / measured as f32).max(14.0)
        } else {
            base_size
        };
        let font = FontInfo::new(resources.fonts.name.font.clone(), theme.accent, size)
            .with_embolden(0.50);
        let width = font.get_text_width(&label).min(max_width);
        draw_single_line(
            image,
            right - width,
            PADDING + header_height.saturating_sub(font.line_height) / 2,
            &label,
            &font,
            emojis,
        );
    }
}

pub fn round_corners(image: &mut RgbaImage, radius: u32) {
    corners::clip(image, radius);
}

/// Avatar placeholders keep the same circular silhouette as photo avatars.
pub fn circle_corners(image: &mut RgbaImage, radius: u32) {
    let radius = radius.min(image.width() / 2).min(image.height() / 2);
    for y in 0..radius {
        for x in 0..radius {
            let distance = ((radius - x) as f32 - 0.5).hypot((radius - y) as f32 - 0.5);
            let coverage = (radius as f32 - distance + 0.5).clamp(0.0, 1.0);
            for (px, py) in [
                (x, y),
                (image.width() - 1 - x, y),
                (x, image.height() - 1 - y),
                (image.width() - 1 - x, image.height() - 1 - y),
            ] {
                let pixel = image.get_pixel_mut(px, py);
                pixel[3] = (f32::from(pixel[3]) * coverage) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_logo_adaptation_preserves_filled_badges_and_lifts_dark_wordmarks() {
        let mut xhh = image::load_from_memory(include_bytes!(
            "../../../../../resources/logos/xiaoheihe.png"
        ))
        .unwrap()
        .into_rgba8();
        assert!(
            adapt_logo_for_dark(&mut xhh),
            "filled dark badge should request rim rather than background inversion"
        );
        // Dark badge interior stays dark instead of turning into a white square
        let bg = xhh.get_pixel(12, 12);
        assert!(bg[0] < 60 && bg[1] < 60 && bg[2] < 60, "{bg:?}");

        let mut twitter =
            image::load_from_memory(include_bytes!("../../../../../resources/logos/twitter.png"))
                .unwrap()
                .into_rgba8();
        assert!(!adapt_logo_for_dark(&mut twitter));
        let opaque_max = twitter
            .pixels()
            .filter(|p| p[3] > 200)
            .map(|p| p[0])
            .max()
            .unwrap_or(0);
        assert!(opaque_max > 220, "twitter mark should lift to light ink");
    }
}
