//! Owned font data and CPU text rasterization using Swash/Fontations.

use std::cell::RefCell;
use std::sync::Arc;

use image::{Pixel, Rgba, RgbaImage};
use swash::scale::{Render, ScaleContext, Source};
use swash::shape::ShapeContext;
use swash::zeno::{Format, Vector};
use swash::{CacheKey, FontRef};

thread_local! {
    // Reuse font caches and scratch buffers across lines without serializing
    // independent cards rendered on different blocking-pool threads.
    static CONTEXTS: RefCell<(ShapeContext, ScaleContext)> =
        RefCell::new((ShapeContext::new(), ScaleContext::new()));
}

#[derive(Clone)]
pub struct Font {
    data: Arc<[u8]>,
    offset: u32,
    key: CacheKey,
}

impl Font {
    pub fn new(data: impl Into<Arc<[u8]>>) -> Option<Self> {
        let data = data.into();
        let font = FontRef::from_index(&data, 0)?;
        let metrics = font.metrics(&[]);
        if metrics.units_per_em == 0 || metrics.ascent + metrics.descent <= 0.0 {
            return None;
        }
        let (offset, key) = (font.offset, font.key);
        Some(Self { data, offset, key })
    }

    fn as_ref(&self) -> FontRef<'_> {
        FontRef {
            data: &self.data,
            offset: self.offset,
            key: self.key,
        }
    }

    // Card sizes historically describe ascent + descent, whereas Swash takes
    // pixels per em. Convert so existing card dimensions and wrapping stay stable.
    fn pixels_per_em(&self, size: f32) -> f32 {
        let metrics = self.as_ref().metrics(&[]);
        size * f32::from(metrics.units_per_em) / (metrics.ascent + metrics.descent)
    }

    pub fn advance(&self, c: char, size: f32) -> f32 {
        let font = self.as_ref();
        font.glyph_metrics(&[])
            .scale(self.pixels_per_em(size))
            .advance_width(font.charmap().map(c))
    }

    #[cfg(test)]
    pub fn draw(
        &self,
        image: &mut RgbaImage,
        color: Rgba<u8>,
        x: i32,
        y: i32,
        size: f32,
        text: &str,
    ) {
        self.draw_weighted(image, color, x, y, size, 0.0, text);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_weighted(
        &self,
        image: &mut RgbaImage,
        color: Rgba<u8>,
        x: i32,
        y: i32,
        size: f32,
        embolden: f32,
        text: &str,
    ) -> i32 {
        let font = self.as_ref();
        let ppem = self.pixels_per_em(size);
        let baseline = font.metrics(&[]).scale(ppem).ascent;
        CONTEXTS.with_borrow_mut(|(shapes, scales)| {
            let mut shaper = shapes
                .builder(font)
                .size(ppem)
                // Wrapping measures individual characters, so do not combine
                // them into optional ligatures when drawing a line.
                .features([("liga", 0), ("clig", 0)])
                .build();
            shaper.add_str(text);
            let mut scaler = scales.builder(font).size(ppem).hint(false).build();
            let mut pen_x = 0.0;
            shaper.shape_with(|cluster| {
                for glyph in cluster.glyphs {
                    let gx = pen_x + glyph.x;
                    let gy = baseline - glyph.y;
                    let mut render = Render::new(&[Source::Outline]);
                    render
                        .format(Format::Alpha)
                        .offset(Vector::new(gx - gx.floor(), gy.floor() - gy));
                    if let Some(bitmap) = render.render(&mut scaler, glyph.id)
                        && bitmap.placement.width > 0
                    {
                        let bw = bitmap.placement.width as usize;
                        let bh = bitmap.placement.height as usize;
                        let left = x + gx.floor() as i32 + bitmap.placement.left;
                        let top = y + gy.floor() as i32 - bitmap.placement.top;
                        if embolden <= 0.0 {
                            for (row, pixels) in bitmap.data.chunks(bw).enumerate() {
                                let py = top + row as i32;
                                if py < 0 || py >= image.height() as i32 {
                                    continue;
                                }
                                for (column, &coverage) in pixels.iter().enumerate() {
                                    if coverage == 0 {
                                        continue;
                                    }
                                    let px = left + column as i32;
                                    if px >= 0 && px < image.width() as i32 {
                                        let mut ink = color;
                                        ink[3] =
                                            (u16::from(color[3]) * u16::from(coverage) / 255) as u8;
                                        image.get_pixel_mut(px as u32, py as u32).blend(&ink);
                                    }
                                }
                            }
                        } else {
                            // Subpixel coverage emboldening avoids contour-winding erosion on CJK
                            // glyphs while adding uniform stem weight at negligible CPU cost.
                            let sx = (embolden.clamp(0.0, 1.0) * 255.0).round() as u32;
                            let sy = ((embolden * 0.35).clamp(0.0, 0.5) * 255.0).round() as u32;
                            for row in 0..=bh {
                                let py = top + row as i32;
                                if py < 0 || py >= image.height() as i32 {
                                    continue;
                                }
                                let cur_row = if row < bh {
                                    &bitmap.data[row * bw..(row + 1) * bw]
                                } else {
                                    &[]
                                };
                                let prev_row = if row > 0 {
                                    &bitmap.data[(row - 1) * bw..row * bw]
                                } else {
                                    &[]
                                };
                                let mut prev_h = 0u32;
                                for column in 0..=bw {
                                    let c = if column < cur_row.len() {
                                        u32::from(cur_row[column])
                                    } else {
                                        0
                                    };
                                    // Porter-Duff `over` with horizontally shifted glyph
                                    let h_shifted = prev_h * sx / 255;
                                    let mut cov = c + (255 - c) * h_shifted / 255;
                                    if sy > 0 && column < prev_row.len() {
                                        let v_shifted = u32::from(prev_row[column]) * sy / 255;
                                        cov = cov + (255 - cov) * v_shifted / 255;
                                    }
                                    prev_h = c;
                                    if cov == 0 {
                                        continue;
                                    }
                                    let px = left + column as i32;
                                    if px >= 0 && px < image.width() as i32 {
                                        let mut ink = color;
                                        ink[3] = (u32::from(color[3]) * cov / 255) as u8;
                                        image.get_pixel_mut(px as u32, py as u32).blend(&ink);
                                    }
                                }
                            }
                        }
                    }
                    pen_x += glyph.advance;
                }
            });
            pen_x.round() as i32
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FONT_DATA: &[u8] =
        include_bytes!("../../../../../resources/fonts/HYSongYunLangHeiW-1.ttf");

    #[test]
    fn clips_glyphs_and_blends_text_alpha() {
        let font = Font::new(FONT_DATA).unwrap();
        let ink = Rgba([40, 60, 90, 128]);
        let mut full = RgbaImage::new(200, 80);
        font.draw(&mut full, ink, 20, 20, 24.0, "中文 Agj");
        assert!(full.pixels().any(|p| p[3] > 0));
        assert!(full.pixels().all(|p| p[3] <= 128));

        let mut clipped = RgbaImage::new(40, 20);
        font.draw(&mut clipped, ink, -10, -5, 24.0, "中文 Agj");
        assert_eq!(
            clipped,
            image::imageops::crop_imm(&full, 30, 25, 40, 20).to_image()
        );

        let mut regular = RgbaImage::new(120, 60);
        let mut bold = RgbaImage::new(120, 60);
        font.draw(&mut regular, Rgba([20, 20, 20, 255]), 10, 10, 34.0, "摄影");
        font.draw_weighted(
            &mut bold,
            Rgba([20, 20, 20, 255]),
            10,
            10,
            34.0,
            0.85,
            "摄影",
        );
        let regular_alpha: u64 = regular.pixels().map(|p| u64::from(p[3])).sum();
        let bold_alpha: u64 = bold.pixels().map(|p| u64::from(p[3])).sum();
        assert!(
            bold_alpha > regular_alpha,
            "weighted text must increase ink coverage without eroding CJK glyphs"
        );
    }

    #[test]
    fn loads_font_collections_and_rejects_invalid_data() {
        assert!(Font::new(b"not a font".as_slice()).is_none());
        // Wrap the bundled face in a single-face TTC, as used by the Windows
        // and macOS fallback fonts. Table offsets are relative to the TTC.
        let mut collection = b"ttcf\x00\x01\x00\x00\x00\x00\x00\x01\x00\x00\x00\x10".to_vec();
        collection.extend_from_slice(FONT_DATA);
        let count = u16::from_be_bytes(FONT_DATA[4..6].try_into().unwrap());
        for table in 0..usize::from(count) {
            let start = 16 + 12 + table * 16 + 8;
            let offset = u32::from_be_bytes(collection[start..start + 4].try_into().unwrap());
            collection[start..start + 4].copy_from_slice(&(offset + 16).to_be_bytes());
        }
        let font = Font::new(collection).unwrap();
        assert!((font.advance('W', 24.0) - 18.95152).abs() < 0.0001);
        let mut image = RgbaImage::new(40, 40);
        font.draw(&mut image, Rgba([0, 0, 0, 255]), 0, 0, 24.0, "中");
        assert!(image.pixels().any(|p| p[3] > 0));
    }
}
