//! Section calculation logic

use image::RgbaImage;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::data::{Content, ParseResult};

use super::constants::*;
use super::text::wrap_text;
use super::types::*;
use super::utils::*;

/// Calculate all sections for a parse result
pub fn calculate_sections(
    result: &ParseResult,
    content_width: u32,
    resources: &RenderResources,
    is_repost: bool,
) -> Vec<SectionData> {
    if is_repost {
        calculate_sections_with(
            result,
            content_width,
            &resources.repost_fonts,
            resources,
            REPOST_AVATAR_SIZE,
            true,
        )
    } else {
        calculate_sections_with(
            result,
            content_width,
            &resources.fonts,
            resources,
            AVATAR_SIZE,
            false,
        )
    }
}

fn calculate_sections_with(
    result: &ParseResult,
    content_width: u32,
    fonts: &FontSet,
    resources: &RenderResources,
    avatar_size: u32,
    is_repost: bool,
) -> Vec<SectionData> {
    let mut sections = Vec::new();

    // 1. Header section (author info)
    if let Some(header) = calculate_header_section(result, fonts, avatar_size) {
        sections.push(SectionData::Header(header));
    }

    // 2-4. Portrait SplitHero (left poster + right title/text) for vertical video covers on main cards,
    // otherwise standard vertical Title -> Cover -> Text stack.
    let has_video = result
        .contents
        .iter()
        .any(|c| matches!(c, Content::Video(_)));
    if !is_repost
        && let Some((hero, consumed_text)) =
            calculate_split_hero_section(result, content_width, fonts, has_video)
    {
        sections.push(SectionData::SplitHero(hero));
        if !consumed_text
            && let Some(text) = &result.text
            && let Some(text_section) = calculate_text_section(text, content_width, fonts)
        {
            sections.push(SectionData::Text(text_section));
        }
    } else {
        // 2. Title section
        if let Some(title) = &result.title
            && let Some(title_section) = calculate_title_section(title, content_width, fonts)
        {
            sections.push(SectionData::Title(title_section));
        }

        // 3. Cover section (shown before body text for videos; image-only posts keep text before galleries).
        let cover_section = calculate_cover_section(result, content_width, has_video);
        if let Some(cover_section) = cover_section {
            sections.push(SectionData::Cover(cover_section));
        }

        // 4. Text section
        if let Some(text) = &result.text
            && let Some(text_section) = calculate_text_section(text, content_width, fonts)
        {
            sections.push(SectionData::Text(text_section));
        }
    }

    // 5. Graphics sections (ordered text/image blocks)
    for c in &result.contents {
        if let Content::Graphics(g) = c {
            sections.extend(calculate_graphics_sections(g, content_width, fonts));
        }
    }

    // 6. Standalone images also remain visible alongside video covers.
    {
        let images: Vec<String> = result
            .contents
            .iter()
            .filter_map(|c| match c {
                Content::Image(img) => img
                    .url
                    .clone()
                    .or_else(|| img.path.as_ref().map(|p| p.to_string_lossy().to_string())),
                _ => None,
            })
            .collect();

        if !images.is_empty() {
            let image_refs: Vec<&str> = images.iter().map(|s| s.as_str()).collect();
            if let Some(grid_section) = calculate_image_grid_section(&image_refs, content_width) {
                sections.push(SectionData::ImageGrid(grid_section));
            }
        }
    }

    // 7. Extra info section
    if !result.extra.is_empty()
        && let Some(extra_section) =
            super::metadata::layout(&result.extra, content_width, &fonts.extra, &fonts.tag)
    {
        sections.push(SectionData::Extra(extra_section));
    }

    // 8. Repost section (only if not already a repost)
    if !is_repost
        && let Some(repost) = &result.repost
        && let Some(repost_section) = calculate_repost_section(repost, content_width, resources)
    {
        sections.push(SectionData::Repost(repost_section));
    }

    sections
}

fn calculate_graphics_sections(
    graphics: &crate::data::GraphicsContent,
    content_width: u32,
    fonts: &FontSet,
) -> Vec<SectionData> {
    if graphics.images.is_empty() {
        return Vec::new();
    }

    let mut sections = Vec::new();

    // Only the first image carries the alt text (if any) to reduce verbosity.
    for (idx, img) in graphics.images.iter().enumerate() {
        let Some(img_ref) = img
            .url
            .clone()
            .or_else(|| img.path.as_ref().map(|p| p.to_string_lossy().to_string()))
        else {
            continue;
        };

        let (image, is_placeholder) =
            match load_graphics_image(std::path::Path::new(&img_ref), content_width) {
                Some(img) => (img, false),
                None => (RgbaImage::new(content_width, 200), true),
            };

        let alt_text = if idx == 0 {
            graphics.alt_text.clone()
        } else {
            None
        };

        let mut text_lines = Vec::new();
        if idx == 0
            && let Some(desc) = &graphics.description
        {
            text_lines = wrap_text(desc, content_width, &fonts.text);
        }

        // Height excludes the trailing SECTION_SPACING; draw_graphics uses ctx.advance(0).
        let mut height = image.height();
        if !text_lines.is_empty() {
            height += (text_lines.len() as u32) * fonts.text.line_height;
            height += SECTION_SPACING;
        }
        if alt_text.is_some() {
            height += SECTION_SPACING + fonts.extra.line_height;
        }

        sections.push(SectionData::Graphics(GraphicsSectionData {
            height,
            text_lines,
            image,
            is_placeholder,
            alt_text,
        }));
    }

    sections
}

/// Calculate header section
fn calculate_header_section(
    result: &ParseResult,
    fonts: &FontSet,
    avatar_size: u32,
) -> Option<HeaderSectionData> {
    let author = result.author.as_ref()?;

    // Load and process avatar
    let avatar = author
        .avatar
        .as_ref()
        .and_then(|s| load_avatar_image(std::path::Path::new(s), avatar_size));

    // Calculate text height
    let mut text_height = fonts.name.line_height;
    let time = result.timestamp.map(|ts| {
        crate::data::result::display_datetime(ts)
            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_default()
    });

    if time.is_some() {
        text_height += NAME_TIME_GAP + fonts.extra.line_height;
    }

    let height = avatar_size.max(text_height);

    Some(HeaderSectionData {
        height,
        avatar,
        name: author.name.clone(),
        time,
        text_height,
    })
}

/// Calculate title section
fn calculate_title_section(
    title: &str,
    content_width: u32,
    fonts: &FontSet,
) -> Option<TitleSectionData> {
    if title.is_empty() {
        return None;
    }

    let lines = wrap_text(title, content_width, &fonts.title);
    if lines.is_empty() {
        return None;
    }

    let height = (lines.len() as u32) * fonts.title.line_height;

    Some(TitleSectionData { height, lines })
}

/// Calculate cover section
fn calculate_cover_section(
    result: &ParseResult,
    content_width: u32,
    has_video: bool,
) -> Option<CoverSectionData> {
    // Find cover reference from video content, or from dynamic converted GIF preview.
    let cover_ref = result.contents.iter().find_map(|c| match c {
        Content::Video(v) => v.cover.as_ref(),
        Content::Dynamic(d) => d.cover_url.as_ref(),
        _ => None,
    })?;

    // Attempt to load local cover image; otherwise fallback to polarity-aware placeholder.
    let (cover_img, is_placeholder) =
        match load_cover_image(std::path::Path::new(cover_ref), content_width) {
            Some(img) => (img, false),
            None => (RgbaImage::new(content_width, 200), true),
        };

    Some(CoverSectionData {
        height: cover_img.height(),
        cover_img,
        is_placeholder,
        show_video_button: has_video,
        duration: result
            .video()
            .filter(|v| v.duration.is_some())
            .and_then(|v| v.display_duration()),
    })
}

/// Calculate split portrait hero for vertical video/dynamic covers (e.g. 9:16 Douyin/TikTok).
///
/// Decodes the cover directly at poster size (256px wide) after a header-only dimension probe,
/// saving ~75% decode/resize work while eliminating awkward side gutters.
fn calculate_split_hero_section(
    result: &ParseResult,
    content_width: u32,
    fonts: &FontSet,
    has_video: bool,
) -> Option<(SplitHeroSectionData, bool)> {
    const POSTER_WIDTH: u32 = 256;
    const COLUMN_GAP: u32 = 24;
    const TITLE_TEXT_GAP: u32 = 14;

    if content_width < POSTER_WIDTH + COLUMN_GAP + 240 {
        return None;
    }

    let cover_ref = result.contents.iter().find_map(|c| match c {
        Content::Video(v) => v.cover.as_ref(),
        Content::Dynamic(d) => d.cover_url.as_ref(),
        _ => None,
    })?;

    let (src_w, src_h) = super::processing::probe_dimensions(std::path::Path::new(cover_ref))?;
    // Trigger split layout only for noticeably portrait covers (aspect height/width > 1.15).
    if src_w == 0 || u64::from(src_h) * 100 <= u64::from(src_w) * 115 {
        return None;
    }

    let has_title = result.title.as_deref().is_some_and(|s| !s.is_empty());
    let has_text = result.text.as_deref().is_some_and(|s| !s.is_empty());
    if !has_title && !has_text {
        return None;
    }

    let poster_h =
        ((u64::from(POSTER_WIDTH) * u64::from(src_h)) / u64::from(src_w)).clamp(300, 380) as u32;
    let info_width = content_width - POSTER_WIDTH - COLUMN_GAP;

    let title_font = &fonts.split_title;
    let text_font = &fonts.split_text;

    let title_lines = result
        .title
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|t| wrap_text(t, info_width, title_font))
        .unwrap_or_default();
    let candidate_text = result
        .text
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|t| wrap_text(t, info_width, text_font))
        .unwrap_or_default();

    let title_h = title_lines.len() as u32 * title_font.line_height;
    // Caption-in-title platforms (TikTok, Kuaishou) can pack huge walls of text into `title`;
    // fall back to full-width stacking instead of a tall narrow right column next to the poster.
    if title_h > poster_h + 90 {
        return None;
    }

    let text_h = candidate_text.len() as u32 * text_font.line_height;
    let gap = if !title_lines.is_empty() && !candidate_text.is_empty() {
        TITLE_TEXT_GAP
    } else {
        0
    };

    let (text_lines, consumed_text) = if candidate_text.is_empty() {
        (Vec::new(), true)
    } else if title_h + gap + text_h <= poster_h + 90 {
        (candidate_text, true)
    } else if !title_lines.is_empty() {
        // Long essay body flows cleanly full-width below while title pairs beside the poster.
        (Vec::new(), false)
    } else {
        // No title and very long body: keep standard full-width stack.
        return None;
    };

    let cover_img =
        load_cropped_image(std::path::Path::new(cover_ref), POSTER_WIDTH, poster_h, 16)?;
    let right_h = title_h
        + if !title_lines.is_empty() && !text_lines.is_empty() {
            TITLE_TEXT_GAP
        } else {
            0
        }
        + text_lines.len() as u32 * text_font.line_height;

    let cover = CoverSectionData {
        height: cover_img.height(),
        cover_img,
        is_placeholder: false,
        show_video_button: has_video,
        duration: result
            .video()
            .filter(|v| v.duration.is_some())
            .and_then(|v| v.display_duration()),
    };

    Some((
        SplitHeroSectionData {
            height: poster_h.max(right_h),
            cover,
            title_lines,
            text_lines,
            text_x_offset: POSTER_WIDTH + COLUMN_GAP,
        },
        consumed_text,
    ))
}

/// Adaptive hole-free Bento tile geometry `(x, y, width, height)` for 2..=9 images.
pub(super) fn bento_rects(count: usize, width: u32) -> Vec<(u32, u32, u32, u32)> {
    let g = IMAGE_GRID_SPACING;
    let w2 = (width.saturating_sub(g) / 2).max(1);
    let w2_r = width.saturating_sub(g + w2).max(1);
    let w3 = (width.saturating_sub(2 * g) / 3).max(1);
    let w3_r = width.saturating_sub(2 * g + 2 * w3).max(1);

    match count {
        0 => Vec::new(),
        1 => vec![(0, 0, width.max(1), width.max(1))],
        2 => vec![(0, 0, w2, 370), (w2 + g, 0, w2_r, 370)],
        3 => {
            let left_w = (width.saturating_sub(g) * 60 / 100).max(1);
            let right_w = width.saturating_sub(g + left_w).max(1);
            let total_h = 376;
            let top_h = (total_h - g) / 2;
            let bot_h = total_h - g - top_h;
            vec![
                (0, 0, left_w, total_h),
                (left_w + g, 0, right_w, top_h),
                (left_w + g, top_h + g, right_w, bot_h),
            ]
        }
        4 => {
            let h = 256;
            vec![
                (0, 0, w2, h),
                (w2 + g, 0, w2_r, h),
                (0, h + g, w2, h),
                (w2 + g, h + g, w2_r, h),
            ]
        }
        5 => {
            let h1 = 248;
            let h2 = 204;
            let y2 = h1 + g;
            vec![
                (0, 0, w2, h1),
                (w2 + g, 0, w2_r, h1),
                (0, y2, w3, h2),
                (w3 + g, y2, w3, h2),
                (2 * (w3 + g), y2, w3_r, h2),
            ]
        }
        6 => {
            let h = w3;
            let y2 = h + g;
            vec![
                (0, 0, w3, h),
                (w3 + g, 0, w3, h),
                (2 * (w3 + g), 0, w3_r, h),
                (0, y2, w3, h),
                (w3 + g, y2, w3, h),
                (2 * (w3 + g), y2, w3_r, h),
            ]
        }
        7 => {
            let h1 = 264;
            let h2 = 196;
            let y2 = h1 + g;
            let y3 = y2 + h2 + g;
            vec![
                (0, 0, width.max(1), h1),
                (0, y2, w3, h2),
                (w3 + g, y2, w3, h2),
                (2 * (w3 + g), y2, w3_r, h2),
                (0, y3, w3, h2),
                (w3 + g, y3, w3, h2),
                (2 * (w3 + g), y3, w3_r, h2),
            ]
        }
        8 => {
            let h1 = 228;
            let h2 = 196;
            let y2 = h1 + g;
            let y3 = y2 + h2 + g;
            vec![
                (0, 0, w2, h1),
                (w2 + g, 0, w2_r, h1),
                (0, y2, w3, h2),
                (w3 + g, y2, w3, h2),
                (2 * (w3 + g), y2, w3_r, h2),
                (0, y3, w3, h2),
                (w3 + g, y3, w3, h2),
                (2 * (w3 + g), y3, w3_r, h2),
            ]
        }
        _ => {
            let h = w3;
            (0..9)
                .map(|idx| {
                    let row = idx as u32 / 3;
                    let col = idx as u32 % 3;
                    let tw = if col == 2 { w3_r } else { w3 };
                    (col * (w3 + g), row * (h + g), tw, h)
                })
                .collect()
        }
    }
}

/// Calculate adaptive Bento image grid section
fn calculate_image_grid_section(
    image_urls: &[&str],
    content_width: u32,
) -> Option<ImageGridSectionData> {
    if image_urls.is_empty() {
        return None;
    }

    let total_count = image_urls.len();
    let display_count = total_count.min(MAX_IMAGES_DISPLAY);
    let has_more = total_count > MAX_IMAGES_DISPLAY;
    let remaining_count = total_count.saturating_sub(MAX_IMAGES_DISPLAY);

    let tiles: Vec<GridTile> = if display_count == 1 {
        let (image, is_placeholder) =
            match load_grid_image(std::path::Path::new(image_urls[0]), content_width, false) {
                Some(img) => (img, false),
                None => (RgbaImage::new(content_width, 200), true),
            };
        let x = content_width.saturating_sub(image.width()) / 2;
        vec![GridTile {
            image,
            x,
            y: 0,
            is_placeholder,
        }]
    } else {
        let rects = bento_rects(display_count, content_width);
        let load = |idx: usize| {
            let (x, y, tw, th) = rects[idx];
            let (image, is_placeholder) =
                match load_cropped_image(std::path::Path::new(image_urls[idx]), tw, th, 14) {
                    Some(img) => (img, false),
                    None => (RgbaImage::new(tw, th), true),
                };
            GridTile {
                image,
                x,
                y,
                is_placeholder,
            }
        };
        let next = AtomicUsize::new(0);
        let workers = super::processing::grid_decode_workers(&image_urls[..display_count]);
        std::thread::scope(|scope| {
            let next = &next;
            let handles: Vec<_> = (0..workers)
                .map(|_| {
                    scope.spawn(move || {
                        let mut batch = Vec::new();
                        loop {
                            let index = next.fetch_add(1, Ordering::Relaxed);
                            if index >= display_count {
                                break;
                            }
                            batch.push((index, load(index)));
                        }
                        batch
                    })
                })
                .collect();
            let mut indexed: Vec<_> = handles
                .into_iter()
                .flat_map(|handle| {
                    handle
                        .join()
                        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
                })
                .collect();
            indexed.sort_unstable_by_key(|(index, _)| *index);
            indexed.into_iter().map(|(_, tile)| tile).collect()
        })
    };

    let height = tiles
        .iter()
        .map(|tile| tile.y + tile.image.height())
        .max()
        .unwrap_or(0);

    Some(ImageGridSectionData {
        height,
        tiles,
        total_count,
        has_more,
        remaining_count,
    })
}

/// Calculate text section
fn calculate_text_section(
    text: &str,
    content_width: u32,
    fonts: &FontSet,
) -> Option<TextSectionData> {
    if text.is_empty() {
        return None;
    }

    let lines = wrap_text(text, content_width, &fonts.text);
    if lines.is_empty() {
        return None;
    }

    let height = (lines.len() as u32) * fonts.text.line_height;

    Some(TextSectionData { height, lines })
}

/// Calculate repost section natively at its actual nested width
fn calculate_repost_section(
    repost: &ParseResult,
    content_width: u32,
    resources: &RenderResources,
) -> Option<RepostSectionData> {
    let repost_width = content_width.saturating_sub(REPOST_PADDING_X * 2).max(1);
    let sections = calculate_sections_with(
        repost,
        repost_width,
        &resources.repost_fonts,
        resources,
        REPOST_AVATAR_SIZE,
        true,
    );
    if sections.is_empty() {
        return None;
    }

    let inner_height: u32 = sections.iter().map(|s| s.height()).sum::<u32>()
        + REPOST_SECTION_SPACING * (sections.len().saturating_sub(1) as u32);
    let height = REPOST_PADDING_Y * 2 + REPOST_EYEBROW_HEIGHT + REPOST_EYEBROW_GAP + inner_height;

    Some(RepostSectionData {
        height,
        content_width: repost_width,
        sections,
    })
}

/// Calculate total card height from sections
pub fn calculate_total_height(sections: &[SectionData]) -> u32 {
    let content_height: u32 = sections.iter().map(|s| s.height()).sum();
    let spacing = SECTION_SPACING * (sections.len().saturating_sub(1) as u32);
    PADDING * 2 + content_height + spacing
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parallel_gallery_keeps_source_order() {
        let directory = tempfile::tempdir().unwrap();
        let paths: Vec<_> = (0..9)
            .map(|index| {
                let path = directory.path().join(format!("{index}.png"));
                // Different sizes/processing times encourage workers to finish out of order.
                RgbaImage::from_pixel(
                    90 + index * 20,
                    90,
                    image::Rgba([index as u8 * 25, 100, 200, 255]),
                )
                .save(&path)
                .unwrap();
                path.to_string_lossy().into_owned()
            })
            .collect();
        let refs: Vec<_> = paths.iter().map(String::as_str).collect();
        let grid = calculate_image_grid_section(&refs, 704).unwrap();
        for (index, tile) in grid.tiles.iter().enumerate() {
            assert_eq!(tile.image.get_pixel(40, 40)[0], index as u8 * 25);
        }
    }

    #[test]
    fn bento_layouts_fill_full_width_without_trailing_holes() {
        for count in 2..=9 {
            let rects = bento_rects(count, 704);
            assert_eq!(rects.len(), count);
            let max_right = rects.iter().map(|(x, _, w, _)| x + w).max().unwrap();
            assert_eq!(max_right, 704, "count={count}");
        }
    }
}
