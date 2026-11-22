//! Cached, antialiased continuous corners for the native card.
//!
//! A cubic superellipse extends the transition over 1.5 radii. Its curvature
//! approaches zero at each straight edge, approximating Apple's continuous
//! corner treatment without depending on a platform-specific graphics API.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use image::{Pixel, Rgba, RgbaImage};
use imageproc::rect::Rect;
use parking_lot::Mutex;

pub(super) struct CornerRow {
    /// First fully covered pixel, measured inward from the rectangle's edge.
    pub inset: u32,
    start: u32,
    coverage: Vec<u8>,
}

impl CornerRow {
    /// Composite only the partially covered pixels at the two outside edges.
    pub fn paint_edges(
        &self,
        image: &mut RgbaImage,
        bounds: Rect,
        y: u32,
        mut color_at: impl FnMut(u32) -> Rgba<u8>,
    ) {
        for (index, &coverage) in self.coverage.iter().enumerate() {
            let offset = self.start + index as u32;
            for x in [
                bounds.left() as u32 + offset,
                bounds.right() as u32 - offset,
            ] {
                if x >= image.width() || y >= image.height() {
                    continue;
                }
                let mut color = color_at(x);
                color[3] = (u16::from(color[3]) * u16::from(coverage) / 255) as u8;
                image.get_pixel_mut(x, y).blend(&color);
            }
        }
    }
}

pub(super) struct CornerMask {
    extent: u32,
    rows: Vec<CornerRow>,
}

impl CornerMask {
    pub fn for_size(width: u32, height: u32, radius: u32) -> Arc<Self> {
        let extent = radius
            .saturating_add(radius.div_ceil(2))
            .min(width / 2)
            .min(height / 2);
        static MASKS: OnceLock<Mutex<HashMap<u32, Arc<CornerMask>>>> = OnceLock::new();
        MASKS
            .get_or_init(Default::default)
            .lock()
            .entry(extent)
            .or_insert_with(|| Arc::new(Self::new(extent)))
            .clone()
    }

    fn new(extent: u32) -> Self {
        const SAMPLES: u32 = 8;
        let distances: Vec<_> = (0..extent * SAMPLES)
            .map(|index| {
                let distance = 1.0 - (f64::from(index) + 0.5) / f64::from(extent * SAMPLES);
                distance * distance * distance
            })
            .collect();
        let rows = (0..extent)
            .map(|y| {
                let coverage: Vec<_> = (0..extent)
                    .map(|x| {
                        let mut inside = 0;
                        for sy in 0..SAMPLES {
                            for sx in 0..SAMPLES {
                                inside += u32::from(
                                    distances[(y * SAMPLES + sy) as usize]
                                        + distances[(x * SAMPLES + sx) as usize]
                                        <= 1.0,
                                );
                            }
                        }
                        ((inside * 255 + SAMPLES * SAMPLES / 2) / (SAMPLES * SAMPLES)) as u8
                    })
                    .collect();
                let start = coverage
                    .iter()
                    .position(|&alpha| alpha != 0)
                    .unwrap_or(extent as usize);
                let inset = coverage
                    .iter()
                    .position(|&alpha| alpha == 255)
                    .unwrap_or(extent as usize);
                CornerRow {
                    inset: inset as u32,
                    start: start as u32,
                    coverage: coverage[start..inset].to_vec(),
                }
            })
            .collect();
        Self { extent, rows }
    }

    pub fn extent(&self) -> u32 {
        self.extent
    }

    pub fn row(&self, y: u32, height: u32, top_only: bool) -> Option<&CornerRow> {
        let edge = if top_only { y } else { y.min(height - 1 - y) };
        self.rows.get(edge as usize)
    }

    /// Antialiased coverage (0..=255) for a point `(x, y)` inside a `width × height` rectangle.
    pub fn rect_coverage(&self, x: u32, y: u32, width: u32, height: u32) -> u8 {
        if x >= width || y >= height {
            return 0;
        }
        let ex = x.min(width - 1 - x);
        let ey = y.min(height - 1 - y);
        self.coverage_at(ex, ey)
    }

    fn coverage_at(&self, x: u32, y: u32) -> u8 {
        let Some(row) = self.rows.get(y as usize) else {
            return 255;
        };
        if x >= row.inset {
            255
        } else if x < row.start {
            0
        } else {
            row.coverage[(x - row.start) as usize]
        }
    }
}

/// Draw a 1px antialiased continuous-corner outline matching `fill`.
pub(super) fn stroke(image: &mut RgbaImage, bounds: Rect, radius: u32, color: Rgba<u8>) {
    let (w, h) = (bounds.width(), bounds.height());
    if w < 3 || h < 3 || color[3] == 0 {
        return;
    }
    let outer = CornerMask::for_size(w, h, radius);
    let inner = CornerMask::for_size(w - 2, h - 2, radius.saturating_sub(1));
    let extent = outer.extent();
    let left = bounds.left() as u32;
    let top = bounds.top() as u32;
    let right = bounds.right() as u32;
    let bottom = bounds.bottom() as u32;

    let mut blend_at = |px: u32, py: u32, cov: u8| {
        if cov == 0 || px >= image.width() || py >= image.height() {
            return;
        }
        let mut ink = color;
        ink[3] = (u16::from(color[3]) * u16::from(cov) / 255) as u8;
        image.get_pixel_mut(px, py).blend(&ink);
    };

    // Straight top & bottom runs
    for x in extent..w.saturating_sub(extent) {
        blend_at(left + x, top, 255);
        blend_at(left + x, bottom, 255);
    }
    // Straight left & right runs
    for y in extent..h.saturating_sub(extent) {
        blend_at(left, top + y, 255);
        blend_at(right, top + y, 255);
    }
    // Four antialiased superellipse corners
    for y in 0..extent {
        for x in 0..extent {
            let out_cov = outer.coverage_at(x, y);
            let in_cov = if x == 0 || y == 0 {
                0
            } else {
                inner.coverage_at(x - 1, y - 1)
            };
            let cov = out_cov.saturating_sub(in_cov);
            if cov == 0 {
                continue;
            }
            blend_at(left + x, top + y, cov);
            blend_at(right - x, top + y, cov);
            blend_at(left + x, bottom - y, cov);
            blend_at(right - x, bottom - y, cov);
        }
    }
}

pub(super) fn clip(image: &mut RgbaImage, radius: u32) {
    let mask = CornerMask::for_size(image.width(), image.height(), radius);
    for (y, row) in mask.rows.iter().enumerate() {
        for x in 0..row.inset {
            let coverage = if x < row.start {
                0
            } else {
                row.coverage[(x - row.start) as usize]
            };
            for (px, py) in [
                (x, y as u32),
                (image.width() - 1 - x, y as u32),
                (x, image.height() - 1 - y as u32),
                (image.width() - 1 - x, image.height() - 1 - y as u32),
            ] {
                let pixel = image.get_pixel_mut(px, py);
                pixel[3] = (u16::from(pixel[3]) * u16::from(coverage) / 255) as u8;
            }
        }
    }
}

/// Fill a rectangle using the same coverage as image clipping and shadow masks.
pub(super) fn fill(
    image: &mut RgbaImage,
    bounds: Rect,
    radius: u32,
    top_only: bool,
    mut color_at: impl FnMut(u32) -> Rgba<u8>,
) {
    let mask = CornerMask::for_size(bounds.width(), bounds.height(), radius);
    let stride = image.width() as usize * 4;
    for relative_y in 0..bounds.height() {
        let y = bounds.top() as u32 + relative_y;
        if y >= image.height() {
            break;
        }
        let edge = mask.row(relative_y, bounds.height(), top_only);
        let inset = edge.map_or(0, |row| row.inset);
        let left = (bounds.left() as u32 + inset).min(image.width()) as usize;
        let right = (bounds.right() as u32 + 1 - inset).min(image.width()) as usize;
        let color = color_at(relative_y);
        let pixels =
            &mut image.as_mut()[y as usize * stride + left * 4..y as usize * stride + right * 4];
        let pixels = pixels.as_chunks_mut::<4>().0;
        if color[3] == 255 {
            pixels.fill(color.0);
        } else {
            for pixel in pixels {
                let mut bottom = Rgba(*pixel);
                bottom.blend(&color);
                *pixel = bottom.0;
            }
        }
        if let Some(edge) = edge {
            edge.paint_edges(image, bounds, y, |_| color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_clipping_and_fill_share_symmetric_alpha_without_raising_source_alpha() {
        for (width, height, radius) in [(120, 80, 20), (4, 40, 2), (1, 1, 20)] {
            let mut filled = RgbaImage::new(width, height);
            fill(
                &mut filled,
                Rect::at(0, 0).of_size(width, height),
                radius,
                false,
                |_| Rgba([90, 120, 150, 255]),
            );
            let mut clipped = RgbaImage::from_pixel(width, height, Rgba([90, 120, 150, 255]));
            clip(&mut clipped, radius);
            let mut translucent = RgbaImage::from_pixel(width, height, Rgba([90, 120, 150, 96]));
            clip(&mut translucent, radius);
            for (x, y, pixel) in clipped.enumerate_pixels() {
                assert_eq!(pixel[3], filled.get_pixel(x, y)[3]);
                assert_eq!(pixel[3], clipped.get_pixel(width - 1 - x, y)[3]);
                assert_eq!(pixel[3], clipped.get_pixel(x, height - 1 - y)[3]);
                assert_eq!(
                    translucent.get_pixel(x, y)[3],
                    (u16::from(pixel[3]) * 96 / 255) as u8
                );
            }
            assert_eq!(clipped.get_pixel(width / 2, height / 2)[3], 255);
        }
        let mut empty = RgbaImage::new(0, 0);
        clip(&mut empty, 20);

        let mut outlined = RgbaImage::from_pixel(80, 60, Rgba([255, 255, 255, 255]));
        stroke(
            &mut outlined,
            Rect::at(4, 4).of_size(72, 52),
            12,
            Rgba([20, 20, 20, 128]),
        );
        // Straight edge pixel is blended, interior pixel is untouched.
        assert!(outlined.get_pixel(40, 4)[0] < 200);
        assert_eq!(outlined.get_pixel(40, 30).0, [255, 255, 255, 255]);
    }
}
