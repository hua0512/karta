//! A bounded sample of displayed thumbnails supplies the card's colors.

use image::{Rgba, RgbaImage};
use sha2::{Digest, Sha256};

use crate::config::ImageThemeMode;
use crate::constants::Platform;

use super::types::SectionData;

const HUE_BINS: usize = 24;

#[derive(Clone, Copy, Default)]
struct ColorBin {
    weight: f32,
    rgb: [f32; 3],
}

impl ColorBin {
    fn color(self) -> Rgba<u8> {
        Rgba([
            (self.rgb[0] / self.weight).round() as u8,
            (self.rgb[1] / self.weight).round() as u8,
            (self.rgb[2] / self.weight).round() as u8,
            255,
        ])
    }
}

pub(super) struct CardTheme {
    pub seed: u64,
    pub is_dark: bool,
    pub primary: Rgba<u8>,
    pub secondary: Rgba<u8>,
    pub page: Rgba<u8>,
    pub paper: Rgba<u8>,
    pub wash: Rgba<u8>,
    pub bottom: Rgba<u8>,
    pub shadow: Rgba<u8>,
    pub accent: Rgba<u8>,
}

pub(super) fn mix(from: Rgba<u8>, to: Rgba<u8>, amount: f32) -> Rgba<u8> {
    let mut color = from;
    for channel in 0..3 {
        color[channel] = (f32::from(from[channel]) * (1.0 - amount)
            + f32::from(to[channel]) * amount)
            .round() as u8;
    }
    color[3] = 255;
    color
}

fn collect_media_images<'a>(sections: &'a [SectionData], out: &mut Vec<&'a RgbaImage>) {
    for section in sections {
        match section {
            SectionData::Cover(cover) if !cover.is_placeholder => out.push(&cover.cover_img),
            SectionData::SplitHero(hero) if !hero.cover.is_placeholder => {
                out.push(&hero.cover.cover_img)
            }
            SectionData::ImageGrid(grid) => out.extend(
                grid.tiles
                    .iter()
                    .filter(|tile| !tile.is_placeholder)
                    .map(|tile| &tile.image),
            ),
            SectionData::Graphics(graphic) if !graphic.is_placeholder => out.push(&graphic.image),
            SectionData::Repost(repost) => collect_media_images(&repost.sections, out),
            _ => {}
        }
    }
}

pub(super) fn media_images(sections: &[SectionData]) -> std::vec::IntoIter<&RgbaImage> {
    let mut images = Vec::new();
    collect_media_images(sections, &mut images);
    images.into_iter()
}

impl CardTheme {
    pub fn from_sections(
        platform: Platform,
        sections: &[SectionData],
        mode: ImageThemeMode,
    ) -> Self {
        let seed = visual_seed(platform, sections);
        let images: Vec<_> = media_images(sections).take(9).collect();
        let (primary, secondary, mean_luma) = if images.is_empty() {
            // A complete palette even without a cover or avatar. Stable text
            // seeds vary different posts without colors jumping on each render.
            let pairs = [
                ([178, 111, 140, 255], [143, 130, 185, 255]),
                ([87, 143, 181, 255], [125, 170, 169, 255]),
                ([115, 155, 126, 255], [185, 165, 116, 255]),
                ([193, 146, 88, 255], [180, 126, 142, 255]),
                ([144, 123, 182, 255], [114, 161, 183, 255]),
                ([91, 160, 169, 255], [145, 170, 129, 255]),
            ];
            let (primary, secondary) = pairs[((seed / 3) % pairs.len() as u64) as usize];
            (Rgba(primary), Rgba(secondary), 180.0)
        } else {
            content_colors(&images)
        };
        let is_dark = match mode {
            ImageThemeMode::Light => false,
            ImageThemeMode::Dark => true,
            ImageThemeMode::Auto => !images.is_empty() && mean_luma < 118.0,
        };
        let mut theme = Self::from_colors(primary, secondary, is_dark);
        theme.seed = seed;
        theme
    }

    fn from_colors(primary: Rgba<u8>, secondary: Rgba<u8>, is_dark: bool) -> Self {
        if is_dark {
            let paper = mix(Rgba([22, 24, 33, 255]), primary, 0.08);
            let page = mix(Rgba([14, 15, 22, 255]), primary, 0.16);
            return Self {
                seed: 0,
                is_dark: true,
                primary,
                secondary,
                page,
                paper,
                wash: mix(Rgba([28, 30, 42, 255]), primary, 0.22),
                bottom: mix(paper, secondary, 0.10),
                shadow: Rgba([4, 5, 10, 255]),
                accent: mix(primary, Rgba([248, 244, 240, 255]), 0.48),
            };
        }
        let paper = mix(Rgba([254, 250, 245, 255]), primary, 0.055);
        let page = mix(Rgba([245, 239, 235, 255]), primary, 0.18);
        Self {
            seed: 0,
            is_dark: false,
            primary,
            secondary,
            page,
            paper,
            wash: mix(Rgba([251, 245, 239, 255]), primary, 0.22),
            bottom: mix(paper, secondary, 0.075),
            shadow: mix(primary, Rgba([42, 38, 43, 255]), 0.75),
            accent: mix(primary, Rgba([45, 42, 40, 255]), 0.22),
        }
    }
}

fn visual_seed(platform: Platform, sections: &[SectionData]) -> u64 {
    let mut hash = Sha256::new();
    hash.update(platform.slug());
    for section in sections {
        match section {
            SectionData::Header(header) => {
                hash.update(header.name.as_bytes());
                hash.update([0]);
            }
            SectionData::Title(title) => {
                for line in &title.lines {
                    hash.update(line.as_bytes());
                    hash.update([0]);
                }
            }
            SectionData::SplitHero(hero) => {
                for line in hero.title_lines.iter().chain(hero.text_lines.iter()) {
                    hash.update(line.as_bytes());
                    hash.update([0]);
                }
            }
            SectionData::Text(text) => {
                for line in &text.lines {
                    hash.update(line.as_bytes());
                    hash.update([0]);
                }
            }
            SectionData::Graphics(graphic) => {
                for line in &graphic.text_lines {
                    hash.update(line.as_bytes());
                    hash.update([0]);
                }
            }
            _ => {}
        }
    }
    // Paths, signed URLs, timestamps and engagement counters can change between
    // requests. Use pixels instead, keeping auto stable across fresh downloads.
    for image in media_images(sections)
        .take(3)
        .filter(|image| !image.is_empty())
    {
        for fraction in [1, 2, 3] {
            hash.update(
                image
                    .get_pixel(image.width() * fraction / 4, image.height() / 2)
                    .0,
            );
        }
    }
    u64::from_le_bytes(hash.finalize()[..8].try_into().expect("eight hash bytes"))
}

fn hue_bin(rgb: [f32; 3], chroma: f32) -> usize {
    let [r, g, b] = rgb;
    let hue = if r >= g && r >= b {
        (g - b) / chroma
    } else if g >= b {
        (b - r) / chroma + 2.0
    } else {
        (r - g) / chroma + 4.0
    };
    ((hue.rem_euclid(6.0) * HUE_BINS as f32 / 6.0) as usize).min(HUE_BINS - 1)
}

fn content_colors(images: &[&RgbaImage]) -> (Rgba<u8>, Rgba<u8>, f32) {
    let mut bins = [ColorBin::default(); HUE_BINS];
    let mut luma_sum = 0.0_f64;
    let mut luma_count = 0_u64;
    for image in images {
        // Every image has the same sample budget, regardless of its dimensions.
        // Ignore transparent corners, near-black/white backgrounds and gray UI.
        let cols = image.width().min(24);
        let rows = image.height().min(24);
        for row in 0..rows {
            for col in 0..cols {
                let pixel = image.get_pixel(
                    ((2 * col + 1) * image.width() / (2 * cols)).min(image.width() - 1),
                    ((2 * row + 1) * image.height() / (2 * rows)).min(image.height() - 1),
                );
                if pixel[3] < 224 {
                    continue;
                }
                let rgb = [
                    f32::from(pixel[0]),
                    f32::from(pixel[1]),
                    f32::from(pixel[2]),
                ];
                luma_sum += 0.299 * f64::from(rgb[0])
                    + 0.587 * f64::from(rgb[1])
                    + 0.114 * f64::from(rgb[2]);
                luma_count += 1;
                let max = rgb.into_iter().fold(0.0_f32, f32::max);
                let min = rgb.into_iter().fold(255.0_f32, f32::min);
                let chroma = max - min;
                if chroma < 18.0 || max < 35.0 || min > 235.0 {
                    continue;
                }
                let weight = (chroma / 255.0).sqrt() / (cols * rows) as f32;
                let bin = &mut bins[hue_bin(rgb, chroma)];
                bin.weight += weight;
                for (sum, component) in bin.rgb.iter_mut().zip(rgb) {
                    *sum += component * weight;
                }
            }
        }
    }
    let mean_luma = if luma_count > 0 {
        (luma_sum / luma_count as f64) as f32
    } else {
        180.0
    };
    // Merge neighboring hue buckets so tiny color shifts do not flip a theme.
    let merged: [ColorBin; HUE_BINS] = std::array::from_fn(|index| {
        let mut merged = ColorBin::default();
        for bin in [
            bins[(index + HUE_BINS - 1) % HUE_BINS],
            bins[index],
            bins[(index + 1) % HUE_BINS],
        ] {
            merged.weight += bin.weight;
            for (sum, component) in merged.rgb.iter_mut().zip(bin.rgb) {
                *sum += component;
            }
        }
        merged
    });
    let (primary_index, primary_bin) = merged
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.weight.total_cmp(&b.weight))
        .unwrap();
    if primary_bin.weight < 0.01 {
        // Monochrome artwork stays neutral; do not borrow unrelated avatar hues.
        return (
            Rgba([142, 140, 137, 255]),
            Rgba([163, 164, 167, 255]),
            mean_luma,
        );
    }
    let primary = primary_bin.color();
    let secondary = merged
        .iter()
        .enumerate()
        .filter(|(index, bin)| {
            let distance = index.abs_diff(primary_index);
            distance.min(HUE_BINS - distance) >= 3 && bin.weight >= primary_bin.weight * 0.04
        })
        .max_by(|(_, a), (_, b)| a.weight.total_cmp(&b.weight))
        .map(|(_, bin)| bin.color())
        .unwrap_or_else(|| mix(primary, Rgba([168, 158, 148, 255]), 0.65));
    (primary, secondary, mean_luma)
}
