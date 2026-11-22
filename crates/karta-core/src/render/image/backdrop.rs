//! A full-card frosted backdrop made from already prepared display images.

use image::{Rgb, RgbImage, Rgba, RgbaImage, imageops};
use imageproc::rect::Rect;

use super::constants::*;
use super::corners::CornerMask;
use super::palette::{CardTheme, media_images, mix};
use super::processing::resize;
use super::types::SectionData;

pub(super) const MUTED_TEXT: Rgba<u8> = Rgba([69, 65, 64, 255]);
pub(super) const DARK_MUTED_TEXT: Rgba<u8> = Rgba([164, 162, 176, 255]);

// One continuous frost across the paper, independent of content boundaries.
const PAPER_OPACITY: u16 = 60;
const PAPER_OPACITY_DARK: u16 = 48;
const PAGE_OPACITY: u16 = 110;
const PAGE_OPACITY_DARK: u16 = 95;

pub(super) struct Backdrop {
    // Only the horizontal dimension is enlarged. Vertical interpolation writes
    // directly into the canvas, avoiding a second full-card RGBA allocation.
    strip: RgbaImage,
    canvas_height: u32,
}

struct Sample<'a> {
    image: &'a RgbaImage,
    left: u32,
    top: u32,
    width: u32,
    height: u32,
}

impl<'a> Sample<'a> {
    fn new(image: &'a RgbaImage, aspect: f64) -> Self {
        let (width, height) = image.dimensions();
        let crop_width = (f64::from(height) * aspect).min(f64::from(width)).max(1.0) as u32;
        let crop_height = (f64::from(width) / aspect).min(f64::from(height)).max(1.0) as u32;
        Self {
            image,
            left: (width - crop_width) / 2,
            top: (height - crop_height) / 2,
            width: crop_width,
            height: crop_height,
        }
    }

    fn pixel(&self, x: u32, y: u32, width: u32, height: u32) -> Rgba<u8> {
        *self.image.get_pixel(
            self.left + x * self.width.saturating_sub(1) / (width - 1).max(1),
            self.top + y * self.height.saturating_sub(1) / (height - 1).max(1),
        )
    }
}

impl Backdrop {
    pub fn new(width: u32, height: u32, sections: &[SectionData], theme: &CardTheme) -> Self {
        const SMALL_WIDTH: u32 = 64;
        let small_height =
            (u64::from(height) * u64::from(SMALL_WIDTH) / u64::from(width)).clamp(48, 128) as u32;
        let sources: Vec<_> = media_images(sections)
            .filter(|image| !image.is_empty())
            .take(9)
            .collect();
        let samples: Vec<_> = if sources.is_empty() {
            Vec::new()
        } else {
            [0, sources.len() / 2, sources.len() - 1]
                .into_iter()
                .map(|index| Sample::new(sources[index], f64::from(width) / f64::from(height)))
                .collect()
        };
        let tiny = RgbImage::from_fn(SMALL_WIDTH, small_height, |x, y| {
            let u = x as f32 / (SMALL_WIDTH - 1) as f32;
            let v = y as f32 / (small_height - 1) as f32;
            let color = if samples.is_empty() {
                let color = mix(theme.primary, theme.secondary, u * 0.30 + v * 0.70);
                let light = (-((u - 0.38).powi(2) / 0.22 + (v - 0.42).powi(2) / 0.12)).exp();
                mix(color, theme.paper, 0.12 + light * 0.65)
            } else {
                let position = v * 2.0;
                let first = (position as usize).min(1);
                let amount = position - first as f32;
                let amount = amount * amount * (3.0 - 2.0 * amount);
                let sample = |index: usize| {
                    let pixel = samples[index].pixel(x, y, SMALL_WIDTH, small_height);
                    mix(theme.paper, pixel, f32::from(pixel[3]) / 255.0)
                };
                mix(sample(first), sample(first + 1), amount)
            };
            Rgb([color[0], color[1], color[2]])
        });
        // At most 8,192 blurred pixels, regardless of article length. There is
        // no I/O, original-resolution decode, or full-size convolution here.
        let field = image::DynamicImage::ImageRgb8(imageops::fast_blur(&tiny, 6.5)).into_rgba8();
        let strip = resize(
            &field,
            width,
            small_height,
            fast_image_resize::FilterType::Bilinear,
        );
        Self {
            strip,
            canvas_height: height,
        }
    }

    pub fn paint_page(&self, image: &mut RgbaImage, theme: &CardTheme, bounds: Rect) {
        let width = image.width() as usize;
        let opacity = if theme.is_dark {
            PAGE_OPACITY_DARK
        } else {
            PAGE_OPACITY
        };
        let mask = CornerMask::for_size(bounds.width(), bounds.height(), PAPER_RADIUS);
        for (y, row) in image.as_mut().chunks_exact_mut(width * 4).enumerate() {
            let y = y as u32;
            if y < bounds.top() as u32 || y > bounds.bottom() as u32 {
                self.paint_row(row, y, theme.page, opacity, 0, width);
                continue;
            }
            // The opaque paper replaces its interior completely. Only the
            // perimeter and partially covered corner pixels need a page layer
            // beneath the shadow and antialiased paper edges.
            let inset = mask
                .row(y - bounds.top() as u32, bounds.height(), false)
                .map_or(0, |edge| edge.inset) as usize;
            self.paint_row(
                row,
                y,
                theme.page,
                opacity,
                0,
                bounds.left() as usize + inset,
            );
            self.paint_row(
                row,
                y,
                theme.page,
                opacity,
                bounds.right() as usize + 1 - inset,
                width,
            );
        }
    }

    pub fn paint_paper(&self, image: &mut RgbaImage, theme: &CardTheme, bounds: Rect) {
        let mask = CornerMask::for_size(bounds.width(), bounds.height(), PAPER_RADIUS);
        let opacity = if theme.is_dark {
            PAPER_OPACITY_DARK
        } else {
            PAPER_OPACITY
        };
        let width = image.width() as usize;
        let top = bounds.top() as u32;
        let bottom = bounds.bottom() as u32;
        for y in top..=bottom {
            let edge = mask.row(y - top, bounds.height(), false);
            let inset = edge.map_or(0, |row| row.inset) as usize;
            // Layout must not create pale text bands or dark photo panels.
            // The same blurred field and frost continue beneath every section;
            // photographs are pasted sharply later.
            let row = &mut image.as_mut()[y as usize * width * 4..(y as usize + 1) * width * 4];
            self.paint_row(
                row,
                y,
                theme.paper,
                opacity,
                bounds.left() as usize + inset,
                bounds.right() as usize + 1 - inset,
            );
            if let Some(edge) = edge {
                edge.paint_edges(image, bounds, y, |x| self.pixel(x, y, theme.paper, opacity));
            }
        }
    }

    fn pixel(&self, x: u32, y: u32, base: Rgba<u8>, opacity: u16) -> Rgba<u8> {
        let position = u64::from(y) * u64::from(self.strip.height() - 1) * 256
            / u64::from(self.canvas_height.saturating_sub(1).max(1));
        let above = position as u32 / 256;
        let below = (above + 1).min(self.strip.height() - 1);
        let fraction = (position % 256) as u16;
        let first = self.strip.get_pixel(x, above);
        let second = self.strip.get_pixel(x, below);
        let mut color = base;
        for channel in 0..3 {
            let value = (u16::from(first[channel]) * (256 - fraction)
                + u16::from(second[channel]) * fraction)
                / 256;
            color[channel] =
                ((value * opacity + u16::from(base[channel]) * (255 - opacity) + 127) / 255) as u8;
        }
        color
    }

    fn paint_row(
        &self,
        row: &mut [u8],
        y: u32,
        base: Rgba<u8>,
        opacity: u16,
        left: usize,
        right: usize,
    ) {
        let stride = self.strip.width() as usize * 4;
        let position = u64::from(y) * u64::from(self.strip.height() - 1) * 256
            / u64::from(self.canvas_height.saturating_sub(1).max(1));
        let above = position as usize / 256;
        let below = (above + 1).min(self.strip.height() as usize - 1);
        let fraction = (position % 256) as u16;
        let source = self.strip.as_raw();
        let first = &source[above * stride + left * 4..above * stride + right * 4];
        let second = &source[below * stride + left * 4..below * stride + right * 4];
        for ((pixel, first), second) in row[left * 4..right * 4]
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(first.as_chunks::<4>().0)
            .zip(second.as_chunks::<4>().0)
        {
            for channel in 0..3 {
                let color = (u16::from(first[channel]) * (256 - fraction)
                    + u16::from(second[channel]) * fraction)
                    / 256;
                pixel[channel] =
                    ((color * opacity + u16::from(base[channel]) * (255 - opacity) + 127) / 255)
                        as u8;
            }
            pixel[3] = 255;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::Platform;

    #[test]
    fn skipping_covered_page_pixels_preserves_shadow_and_antialiased_paper() {
        use crate::config::ImageThemeMode;

        for mode in [ImageThemeMode::Light, ImageThemeMode::Dark] {
            let theme = CardTheme::from_sections(Platform::Douyin, &[], mode);
            let page_opacity = if theme.is_dark {
                PAGE_OPACITY_DARK
            } else {
                PAGE_OPACITY
            };
            for (width, height) in [(200, 180), (800, 1100)] {
                let bounds = Rect::at(PAPER_INSET as i32, PAPER_TOP as i32)
                    .of_size(width - 2 * PAPER_INSET, height - PAPER_TOP - PAPER_BOTTOM);
                let backdrop = Backdrop::new(width, height, &[], &theme);
                let mut reference = RgbaImage::from_pixel(width, height, theme.page);
                for (y, row) in reference
                    .as_mut()
                    .chunks_exact_mut(width as usize * 4)
                    .enumerate()
                {
                    backdrop.paint_row(row, y as u32, theme.page, page_opacity, 0, width as usize);
                }
                let mut skipped = RgbaImage::from_pixel(width, height, theme.page);
                backdrop.paint_page(&mut skipped, &theme, bounds);
                for image in [&mut reference, &mut skipped] {
                    super::super::shadow::paint(image, bounds, theme.shadow);
                    backdrop.paint_paper(image, &theme, bounds);
                }
                assert!(
                    skipped == reference,
                    "page composition differs at {width}×{height} ({mode:?})"
                );
            }
        }
    }
}
