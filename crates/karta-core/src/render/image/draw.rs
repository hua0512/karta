//! Section drawing logic

use image::{Rgba, RgbaImage};
use imageproc::rect::Rect;

use super::constants::*;
use super::corners;
use super::palette::mix;
use super::text::{EmojiImageMap, draw_single_line, draw_text_lines};
use super::types::*;
use super::utils::*;

/// Draw all sections onto the canvas
pub fn draw_sections(
    img: &mut RgbaImage,
    sections: &[SectionData],
    ctx: &mut RenderContext,
    resources: &RenderResources,
    emoji_images: &EmojiImageMap,
) {
    let fonts = resources.fonts.with_dark(ctx.is_dark);
    draw_sections_with_fonts(img, sections, ctx, resources, &fonts, emoji_images);
}

fn draw_sections_with_fonts(
    img: &mut RgbaImage,
    sections: &[SectionData],
    ctx: &mut RenderContext,
    resources: &RenderResources,
    fonts: &FontSet,
    emoji_images: &EmojiImageMap,
) {
    for section in sections {
        match section {
            SectionData::Header(data) => draw_header(img, data, ctx, fonts, emoji_images),
            SectionData::Title(data) => draw_title(img, data, ctx, fonts, emoji_images),
            SectionData::Cover(data) => draw_cover(img, data, ctx, fonts),
            SectionData::SplitHero(data) => draw_split_hero(img, data, ctx, fonts, emoji_images),
            SectionData::Text(data) => draw_text(img, data, ctx, fonts, emoji_images),
            SectionData::Extra(data) => draw_extra(img, data, ctx, fonts, emoji_images),
            SectionData::ImageGrid(data) => draw_image_grid(img, data, ctx, fonts, emoji_images),
            SectionData::Graphics(data) => draw_graphics(img, data, ctx, fonts, emoji_images),
            SectionData::Repost(data) => {
                draw_repost(img, data, ctx, resources, fonts, emoji_images)
            }
        }
    }
}

fn draw_placeholder(
    img: &mut RgbaImage,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    radius: u32,
    is_dark: bool,
) {
    if width == 0 || height == 0 {
        return;
    }
    let bounds = Rect::at(x as i32, y as i32).of_size(width, height);
    let (fill, rim) = if is_dark {
        (Rgba([255, 255, 255, 16]), Rgba([255, 255, 255, 28]))
    } else {
        (Rgba([28, 25, 32, 16]), Rgba([28, 25, 32, 26]))
    };
    corners::fill(img, bounds, radius, false, |_| fill);
    if width >= 3 && height >= 3 {
        corners::stroke(img, bounds, radius, rim);
    }
}

/// Draw header section (avatar + name + time)
fn draw_header(
    img: &mut RgbaImage,
    data: &HeaderSectionData,
    ctx: &mut RenderContext,
    fonts: &FontSet,
    emoji_images: &EmojiImageMap,
) {
    let x = ctx.x_pos;
    let y = ctx.y_pos;
    let avatar_size = ctx.avatar_size;

    // Draw avatar + polarity-aware 1px hairline ring
    let avatar_ring = if ctx.is_dark {
        Rgba([255, 255, 255, 40])
    } else {
        Rgba([0, 0, 0, 26])
    };
    if let Some(avatar) = &data.avatar {
        paste_image(img, avatar, x, y);
        draw_circle_ring(img, x, y, avatar_size, avatar_ring);
    } else {
        let background = if ctx.is_dark {
            mix(ctx.accent, Rgba([28, 30, 42, 255]), 0.72)
        } else {
            mix(ctx.accent, Rgba([255; 4]), 0.84)
        };
        let mut initial = RgbaImage::from_pixel(avatar_size, avatar_size, background);
        super::theme::circle_corners(&mut initial, avatar_size / 2);
        let letter = data
            .name
            .chars()
            .find(|letter| !letter.is_whitespace())
            .map(|letter| letter.to_string())
            .unwrap_or_else(|| "·".to_string());
        let letter_size = (avatar_size as f32 * 0.47).round();
        let font =
            FontInfo::new(fonts.name.font.clone(), ctx.accent, letter_size).with_embolden(0.45);
        draw_single_line(
            &mut initial,
            avatar_size.saturating_sub(font.get_text_width(&letter)) / 2,
            avatar_size.saturating_sub(font.line_height) / 2,
            &letter,
            &font,
            emoji_images,
        );
        paste_image(img, &initial, x, y);
        draw_circle_ring(img, x, y, avatar_size, avatar_ring);
    }

    // Calculate text position (vertically centered with avatar)
    let text_x = x + avatar_size + AVATAR_TEXT_GAP;
    let avatar_center = y + avatar_size / 2;
    let text_start_y = avatar_center.saturating_sub(data.text_height / 2);
    let mut text_y = text_start_y;

    // Keep long author names clear of the platform logo on main cards.
    let logo_reserve = if ctx.is_main { 128 } else { 0 };
    let available = ctx
        .content_width
        .saturating_sub(avatar_size + AVATAR_TEXT_GAP + logo_reserve);
    let name = if fonts.name.get_text_width(&data.name) > available {
        let width = available
            .saturating_sub(fonts.name.get_text_width("…"))
            .max(1);
        let first = super::text::wrap_text(&data.name, width, &fonts.name)
            .into_iter()
            .next()
            .unwrap_or_default();
        format!("{first}…")
    } else {
        data.name.clone()
    };
    // Draw name
    draw_single_line(img, text_x, text_y, &name, &fonts.name, emoji_images);
    text_y += fonts.name.line_height;

    // Draw time
    if let Some(time) = &data.time {
        text_y += NAME_TIME_GAP;
        let mut font = fonts.extra.clone();
        font.fill = ctx.extra_fill;
        draw_single_line(img, text_x, text_y, time, &font, emoji_images);
    }

    ctx.advance(data.height);
}

/// Draw title section
fn draw_title(
    img: &mut RgbaImage,
    data: &TitleSectionData,
    ctx: &mut RenderContext,
    fonts: &FontSet,
    emoji_images: &EmojiImageMap,
) {
    draw_text_lines(
        img,
        ctx.x_pos,
        ctx.y_pos,
        &data.lines,
        &fonts.title,
        emoji_images,
    );
    ctx.advance(data.height);
}

/// Draw cover section with optional video button
fn draw_cover(
    img: &mut RgbaImage,
    data: &CoverSectionData,
    ctx: &mut RenderContext,
    fonts: &FontSet,
) {
    let (w, h) = data.cover_img.dimensions();
    let x = ctx.x_pos + ctx.content_width.saturating_sub(w) / 2;
    let y = ctx.y_pos;

    if data.is_placeholder {
        draw_placeholder(img, x, y, w, h, 20, ctx.is_dark);
    } else {
        paste_image(img, &data.cover_img, x, y);
        if ctx.is_dark && w >= 3 && h >= 3 {
            corners::stroke(
                img,
                Rect::at(x as i32, y as i32).of_size(w, h),
                20,
                Rgba([255, 255, 255, 28]),
            );
        }
    }
    super::controls::draw(img, data, x, y, &fonts.extra);

    ctx.advance(data.height);
}

/// Draw split portrait hero (left poster + right title & text)
fn draw_split_hero(
    img: &mut RgbaImage,
    data: &SplitHeroSectionData,
    ctx: &mut RenderContext,
    fonts: &FontSet,
    emoji_images: &EmojiImageMap,
) {
    let x = ctx.x_pos;
    let y = ctx.y_pos;
    let (w, h) = data.cover.cover_img.dimensions();

    paste_image(img, &data.cover.cover_img, x, y);
    if w >= 3 && h >= 3 {
        let rim = if ctx.is_dark {
            Rgba([255, 255, 255, 32])
        } else {
            Rgba([28, 25, 32, 22])
        };
        corners::stroke(img, Rect::at(x as i32, y as i32).of_size(w, h), 16, rim);
    }
    super::controls::draw(img, &data.cover, x, y, &fonts.extra);

    let title_font = &fonts.split_title;
    let text_font = &fonts.split_text;

    let title_h = data.title_lines.len() as u32 * title_font.line_height;
    let text_h = data.text_lines.len() as u32 * text_font.line_height;
    let gap = if !data.title_lines.is_empty() && !data.text_lines.is_empty() {
        14
    } else {
        0
    };
    let right_h = title_h + gap + text_h;

    let text_x = x + data.text_x_offset;
    // Gently center short right-hand blocks vertically against the poster; top-align taller blocks.
    let mut text_y = if right_h + 24 < data.cover.height {
        y + (data.cover.height - right_h) / 3
    } else {
        y
    };

    if !data.title_lines.is_empty() {
        draw_text_lines(
            img,
            text_x,
            text_y,
            &data.title_lines,
            title_font,
            emoji_images,
        );
        text_y += title_h + gap;
    }
    if !data.text_lines.is_empty() {
        draw_text_lines(
            img,
            text_x,
            text_y,
            &data.text_lines,
            text_font,
            emoji_images,
        );
    }

    ctx.advance(data.height);
}

/// Draw text section
fn draw_text(
    img: &mut RgbaImage,
    data: &TextSectionData,
    ctx: &mut RenderContext,
    fonts: &FontSet,
    emoji_images: &EmojiImageMap,
) {
    draw_text_lines(
        img,
        ctx.x_pos,
        ctx.y_pos,
        &data.lines,
        &fonts.text,
        emoji_images,
    );
    ctx.advance(data.height);
}

/// Draw extra info section (tag pills, hairline divider, metrics, detail lines)
fn draw_extra(
    img: &mut RgbaImage,
    data: &ExtraSectionData,
    ctx: &mut RenderContext,
    fonts: &FontSet,
    emoji_images: &EmojiImageMap,
) {
    let mut font = fonts.extra.clone();
    font.fill = ctx.extra_fill;
    super::metadata::draw(
        img,
        data,
        ctx.x_pos,
        ctx.y_pos,
        ctx.content_width,
        &font,
        &fonts.tag,
        ctx.accent,
        ctx.is_dark,
        emoji_images,
    );
    ctx.advance(data.height);
}

/// Draw adaptive Bento image grid section
fn draw_image_grid(
    img: &mut RgbaImage,
    data: &ImageGridSectionData,
    ctx: &mut RenderContext,
    fonts: &FontSet,
    emoji_images: &EmojiImageMap,
) {
    if data.tiles.is_empty() {
        return;
    }

    let last_idx = data.tiles.len() - 1;
    let tile_radius = if data.tiles.len() == 1 { 16 } else { 14 };
    for (idx, tile) in data.tiles.iter().enumerate() {
        let tx = ctx.x_pos + tile.x;
        let ty = ctx.y_pos + tile.y;
        let (tw, th) = tile.image.dimensions();
        let remaining = (data.has_more && idx == last_idx).then_some(data.remaining_count);
        if tile.is_placeholder {
            draw_placeholder(img, tx, ty, tw, th, tile_radius, ctx.is_dark);
            if let Some(remaining) = remaining.filter(|count| *count > 0) {
                draw_remaining_label(img, tx, ty, tw, th, remaining, fonts, emoji_images);
            }
        } else {
            draw_gallery_tile(img, &tile.image, tx, ty, remaining, fonts, emoji_images);
            if ctx.is_dark && tw >= 3 && th >= 3 {
                corners::stroke(
                    img,
                    Rect::at(tx as i32, ty as i32).of_size(tw, th),
                    tile_radius,
                    Rgba([255, 255, 255, 24]),
                );
            }
        }
        if idx == last_idx && !data.has_more && data.total_count > 1 && !tile.is_placeholder {
            let label = format!("共 {} 图", data.total_count);
            super::controls::draw_corner_badge(img, &tile.image, tx, ty, &label, &fonts.badge);
        }
    }

    ctx.advance(data.height);
}

#[allow(clippy::too_many_arguments)]
fn draw_remaining_label(
    canvas: &mut RgbaImage,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    remaining: usize,
    fonts: &FontSet,
    emojis: &EmojiImageMap,
) {
    let label = format!("+{remaining}");
    let base = &fonts.indicator;
    let available = width.saturating_sub(24).max(1);
    let scale = (available as f32 / base.get_text_width(&label).max(1) as f32).min(1.0);
    let font = FontInfo::new(base.font.clone(), COLOR_INDICATOR, base.size * scale)
        .with_embolden(base.embolden * scale);
    let text_x = x + width.saturating_sub(font.get_text_width(&label)) / 2;
    let text_y = y + height.saturating_sub(font.line_height) / 2;
    draw_single_line(canvas, text_x, text_y, &label, &font, emojis);
}

/// Draw a gallery tile, overlaying a dimmed "+N" badge when `remaining` is set.
fn draw_gallery_tile(
    canvas: &mut RgbaImage,
    image: &RgbaImage,
    x: u32,
    y: u32,
    remaining: Option<usize>,
    fonts: &FontSet,
    emojis: &EmojiImageMap,
) {
    let Some(remaining) = remaining.filter(|count| *count > 0) else {
        paste_image(canvas, image, x, y);
        return;
    };
    let mut shaded = image.clone();
    for pixel in shaded.pixels_mut() {
        // Preserve the rounded mask and source alpha instead of laying an
        // opaque rectangular overlay across the clipped image's corners.
        for channel in &mut pixel.0[..3] {
            *channel = (u16::from(*channel) * 140 / 255) as u8;
        }
    }
    paste_image(canvas, &shaded, x, y);
    draw_remaining_label(
        canvas,
        x,
        y,
        image.width(),
        image.height(),
        remaining,
        fonts,
        emojis,
    );
}

/// Draw graphics section (text + image)
fn draw_graphics(
    img: &mut RgbaImage,
    data: &GraphicsSectionData,
    ctx: &mut RenderContext,
    fonts: &FontSet,
    emoji_images: &EmojiImageMap,
) {
    // Draw text lines if any
    if !data.text_lines.is_empty() {
        draw_text_lines(
            img,
            ctx.x_pos,
            ctx.y_pos,
            &data.text_lines,
            &fonts.text,
            emoji_images,
        );
        ctx.y_pos += (data.text_lines.len() as u32) * fonts.text.line_height;
        ctx.y_pos += SECTION_SPACING;
    }

    // Draw image (centered)
    let (w, h) = data.image.dimensions();
    let img_x = ctx.x_pos + (ctx.content_width.saturating_sub(w)) / 2;
    if data.is_placeholder {
        draw_placeholder(img, img_x, ctx.y_pos, w, h, 14, ctx.is_dark);
    } else {
        paste_image(img, &data.image, img_x, ctx.y_pos);
    }
    ctx.y_pos += h;

    // Draw alt text if any
    if let Some(alt) = &data.alt_text {
        ctx.y_pos += SECTION_SPACING;
        let mut font = fonts.extra.clone();
        font.fill = ctx.extra_fill;
        let text_width = fonts.extra.get_text_width(alt);
        let text_x = ctx.x_pos + (ctx.content_width.saturating_sub(text_width)) / 2;
        draw_single_line(img, text_x, ctx.y_pos, alt, &font, emoji_images);
        ctx.y_pos += fonts.extra.line_height;
    }

    ctx.advance(0);
}

/// Draw repost section as a translucent theme-tinted quote block rendered natively
fn draw_repost(
    img: &mut RgbaImage,
    data: &RepostSectionData,
    ctx: &mut RenderContext,
    resources: &RenderResources,
    fonts: &FontSet,
    emoji_images: &EmojiImageMap,
) {
    let x = ctx.x_pos;
    let y = ctx.y_pos;
    let width = ctx.content_width;
    let height = data.height;
    let bounds = Rect::at(x as i32, y as i32).of_size(width, height);

    // Soft translucent surface + 1px hairline border that harmonizes with light and dark backdrops
    let (surface_fill, border_color, eyebrow_ink) = if ctx.is_dark {
        (
            Rgba([255, 255, 255, 14]),
            Rgba([255, 255, 255, 28]),
            mix(ctx.accent, Rgba([255, 255, 255, 255]), 0.25),
        )
    } else {
        let tint = mix(ctx.accent, Rgba([38, 34, 42, 255]), 0.45);
        (
            Rgba([tint[0], tint[1], tint[2], 18]),
            Rgba([tint[0], tint[1], tint[2], 36]),
            mix(ctx.accent, Rgba([42, 38, 46, 255]), 0.35),
        )
    };
    corners::fill(img, bounds, REPOST_BORDER_RADIUS, false, |_| surface_fill);
    if width >= 3 && height >= 3 {
        corners::stroke(img, bounds, REPOST_BORDER_RADIUS, border_color);
    }

    // Compact inline eyebrow: vector repost icon + "转发"
    let eyebrow_x = x + REPOST_PADDING_X;
    let eyebrow_y = y + REPOST_PADDING_Y;
    super::metadata::draw_repost_icon(
        img,
        eyebrow_x,
        eyebrow_y + REPOST_EYEBROW_HEIGHT.saturating_sub(18) / 2,
        Rgba([eyebrow_ink[0], eyebrow_ink[1], eyebrow_ink[2], 225]),
    );
    let mut eyebrow_font = fonts.eyebrow.clone();
    eyebrow_font.fill = eyebrow_ink;
    draw_single_line(
        img,
        eyebrow_x + 24,
        eyebrow_y + REPOST_EYEBROW_HEIGHT.saturating_sub(eyebrow_font.line_height) / 2,
        REPOST_HEADER_TEXT,
        &eyebrow_font,
        emoji_images,
    );

    // Draw nested sections natively at full resolution using prebuilt repost FontSet
    let repost_fonts = resources.repost_fonts.with_dark(ctx.is_dark);
    let mut nested_ctx = RenderContext {
        x_pos: x + REPOST_PADDING_X,
        content_width: data.content_width,
        y_pos: eyebrow_y + REPOST_EYEBROW_HEIGHT + REPOST_EYEBROW_GAP,
        is_main: false,
        is_dark: ctx.is_dark,
        section_spacing: REPOST_SECTION_SPACING,
        avatar_size: REPOST_AVATAR_SIZE,
        accent: ctx.accent,
        extra_fill: ctx.extra_fill,
    };
    draw_sections_with_fonts(
        img,
        &data.sections,
        &mut nested_ctx,
        resources,
        &repost_fonts,
        emoji_images,
    );

    ctx.advance(height);
}
