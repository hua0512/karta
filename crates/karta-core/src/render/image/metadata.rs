//! Compact interaction metrics, tag pills, and separate detail lines with cached vector icons.

use std::collections::HashMap;
use std::sync::OnceLock;

use image::{Pixel, Rgba, RgbaImage};
use imageproc::rect::Rect;
use swash::zeno::{Cap, Join, Mask, Stroke, Transform};

use super::corners;
use super::palette::mix;
use super::text::{EmojiImageMap, draw_single_line, draw_text_lines, wrap_text};
use super::types::{ExtraSectionData, FontInfo, MetricData, TagData};

const ICON_SIZE: u32 = 22;
const SMALL_ICON_SIZE: u32 = 18;
const ICON_GAP: u32 = 8;
const ITEM_GAP: u32 = 24;
const ROW_HEIGHT: u32 = 30;
const ROW_GAP: u32 = 8;
const DETAIL_GAP: u32 = 12;

const TAG_HEIGHT: u32 = 28;
const TAG_PAD_X: u32 = 11;
const TAG_GAP_X: u32 = 8;
const TAG_GAP_Y: u32 = 8;
const TAG_RADIUS: u32 = TAG_HEIGHT / 2;
const TAG_DIVIDER_GAP: u32 = 18;
const HAIRLINE_TOP_GAP: u32 = 10;
const MAX_TAGS_DISPLAY: usize = 12;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MetricIcon {
    Play,
    View,
    Like,
    Comment,
    Star,
    Repost,
    Share,
    Coin,
    Danmaku,
}

impl MetricIcon {
    fn from_key(key: &str) -> Option<Self> {
        Some(match key {
            "播放" => Self::Play,
            "浏览" | "阅读" | "views" => Self::View,
            "点赞" | "likes" => Self::Like,
            "评论" | "回复" | "comments" => Self::Comment,
            "收藏" | "favorites" | "bookmarks" => Self::Star,
            "转发" | "reposts" | "retweets" => Self::Repost,
            "分享" | "shares" => Self::Share,
            "投币" | "coins" => Self::Coin,
            "弹幕" | "danmaku" => Self::Danmaku,
            _ => return None,
        })
    }

    fn path(self) -> &'static str {
        match self {
            Self::Play => "M9 5L19 12L9 19Z",
            Self::View => {
                "M2 12C6 3 18 3 22 12C18 21 6 21 2 12Z \
                 M15 12A3 3 0 1 1 9 12A3 3 0 1 1 15 12Z"
            }
            Self::Like => {
                "M20 5.5C18 3.5 14.5 4 12 6.5C9.5 4 6 3.5 4 5.5\
                 C1.5 8 2.5 11.5 5 14L12 21L19 14C21.5 11.5 22.5 8 20 5.5Z"
            }
            Self::Comment => {
                "M21 11.5A8.5 8.5 0 0 1 12.5 20H4L5.6 15.8\
                 A8.5 8.5 0 1 1 21 11.5Z"
            }
            Self::Star => {
                "M12 3L14.8 8.7L21.1 9.6L16.5 14L17.6 20.3\
                 L12 17.3L6.4 20.3L7.5 14L2.9 9.6L9.2 8.7Z"
            }
            Self::Repost => "M4 7H19L15 3M19 7L15 11M20 17H5L9 21M5 17L9 13",
            Self::Share => "M14 4L21 11L14 18V13C8 13 4.5 15 3 20V15C3 10 7 7 14 7Z",
            Self::Coin => {
                "M21 12A9 9 0 1 1 3 12A9 9 0 1 1 21 12Z \
                 M8 7L12 11L16 7M8 12H16M8 15H16M12 11V18"
            }
            Self::Danmaku => {
                "M5 4H19Q21 4 21 6V16Q21 18 19 18H9L4 21V18\
                 Q3 18 3 16V6Q3 4 5 4Z M7 9H17M7 13H13"
            }
        }
    }

    fn mask(self) -> &'static [u8] {
        static MASKS: OnceLock<[Vec<u8>; 9]> = OnceLock::new();
        &MASKS.get_or_init(|| {
            [
                Self::Play,
                Self::View,
                Self::Like,
                Self::Comment,
                Self::Star,
                Self::Repost,
                Self::Share,
                Self::Coin,
                Self::Danmaku,
            ]
            .map(|icon| {
                let scale = ICON_SIZE as f32 / 24.0;
                Mask::new(icon.path())
                    .size(ICON_SIZE, ICON_SIZE)
                    .transform(Some(Transform::scale(scale, scale)))
                    .style(Stroke::new(1.8).cap(Cap::Round).join(Join::Round))
                    .render()
                    .0
            })
        })[self as usize]
    }

    fn draw(self, image: &mut RgbaImage, x: u32, y: u32, color: Rgba<u8>) {
        draw_mask(image, self.mask(), ICON_SIZE, x, y, color);
    }
}

fn draw_mask(image: &mut RgbaImage, mask: &[u8], size: u32, x: u32, y: u32, color: Rgba<u8>) {
    for (index, &coverage) in mask.iter().enumerate() {
        let px = x + index as u32 % size;
        let py = y + index as u32 / size;
        if coverage != 0 && px < image.width() && py < image.height() {
            let mut ink = color;
            ink[3] = (u16::from(ink[3]) * u16::from(coverage) / 255) as u8;
            image.get_pixel_mut(px, py).blend(&ink);
        }
    }
}

pub(super) fn draw_repost_icon(image: &mut RgbaImage, x: u32, y: u32, color: Rgba<u8>) {
    static MASK: OnceLock<Vec<u8>> = OnceLock::new();
    let mask = MASK.get_or_init(|| {
        let scale = SMALL_ICON_SIZE as f32 / 24.0;
        Mask::new(MetricIcon::Repost.path())
            .size(SMALL_ICON_SIZE, SMALL_ICON_SIZE)
            .transform(Some(Transform::scale(scale, scale)))
            .style(Stroke::new(2.0).cap(Cap::Round).join(Join::Round))
            .render()
            .0
    });
    draw_mask(image, mask, SMALL_ICON_SIZE, x, y, color);
}

fn parse_tags(raw: &str) -> Vec<String> {
    // Hashtag-prefixed lists (e.g. Xiaohongshu / Xiaoheihe) are whitespace-separated even if an
    // individual tag contains a comma; bare lists (e.g. Bilibili live "虚拟,直播") split on commas.
    let tokens: Vec<&str> = if raw.contains('#') {
        raw.split_whitespace()
            .map(|token| token.trim_matches([',', '，']))
            .collect()
    } else if raw.contains(',') || raw.contains('，') {
        raw.split([',', '，']).collect()
    } else {
        raw.split_whitespace().collect()
    };
    let mut tags = Vec::new();
    for token in tokens {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }
        let normalized = if trimmed.starts_with('#') {
            trimmed.to_string()
        } else {
            format!("#{trimmed}")
        };
        if !tags.contains(&normalized) {
            tags.push(normalized);
        }
    }
    tags
}

fn layout_tags(
    raw_tags: &[String],
    content_width: u32,
    tag_font: &FontInfo,
) -> (Vec<Vec<TagData>>, Vec<String>) {
    let mut overflow_lines = Vec::new();
    let mut items: Vec<TagData> = Vec::new();
    let visible_count = raw_tags.len().min(MAX_TAGS_DISPLAY);
    for tag in &raw_tags[..visible_count] {
        let pill_width = tag_font.get_text_width(tag) + TAG_PAD_X * 2;
        if pill_width <= content_width {
            items.push(TagData {
                text: tag.clone(),
                width: pill_width,
            });
        } else {
            overflow_lines.push(tag.clone());
        }
    }
    if raw_tags.len() > MAX_TAGS_DISPLAY {
        let more = format!("+{}", raw_tags.len() - MAX_TAGS_DISPLAY);
        let pill_width = tag_font.get_text_width(&more) + TAG_PAD_X * 2;
        if pill_width <= content_width {
            items.push(TagData {
                text: more,
                width: pill_width,
            });
        }
    }

    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut used = 0;
    for tag in items {
        let gap = if row.is_empty() { 0 } else { TAG_GAP_X };
        if !row.is_empty() && used + gap + tag.width > content_width {
            rows.push(std::mem::take(&mut row));
            used = 0;
        }
        used += if row.is_empty() { 0 } else { TAG_GAP_X };
        used += tag.width;
        row.push(tag);
    }
    if !row.is_empty() {
        rows.push(row);
    }
    (rows, overflow_lines)
}

fn split_emoji_metric(token: &str) -> Option<(MetricIcon, &str)> {
    const PREFIXES: [(&str, MetricIcon); 9] = [
        ("▶", MetricIcon::Play),
        ("👀", MetricIcon::View),
        ("👍", MetricIcon::Like),
        ("❤", MetricIcon::Like),
        ("💬", MetricIcon::Comment),
        ("⭐", MetricIcon::Star),
        ("🔄", MetricIcon::Repost),
        ("↩", MetricIcon::Share),
        ("🪙", MetricIcon::Coin),
    ];
    if let Some(rest) = token.strip_prefix("💭") {
        return Some((MetricIcon::Danmaku, rest.trim_start_matches('\u{FE0F}')));
    }
    for (prefix, icon) in PREFIXES {
        if let Some(rest) = token.strip_prefix(prefix) {
            return Some((icon, rest.trim_start_matches('\u{FE0F}')));
        }
    }
    None
}

/// Parse packed emoji stats strings (e.g. `"👍 11.9万 🪙 4670 ..."`) into vector metric entries.
/// Returns `None` if any token is unrecognized or maps to a duplicate icon so callers can fall
/// back to rendering the raw string without losing distinct emoji semantics.
fn parse_stats_metrics(raw: &str) -> Option<Vec<(MetricIcon, String)>> {
    let mut parsed = Vec::new();
    let mut tokens = raw.split_whitespace();
    while let Some(token) = tokens.next() {
        let (icon, rest) = split_emoji_metric(token)?;
        let value = if rest.is_empty() {
            let next = tokens.next()?;
            if split_emoji_metric(next).is_some() {
                return None;
            }
            next
        } else {
            rest
        };
        if value.is_empty() || parsed.iter().any(|(existing, _)| *existing == icon) {
            return None;
        }
        parsed.push((icon, value.to_string()));
    }
    (!parsed.is_empty()).then_some(parsed)
}

pub(super) fn parses_as_metrics(raw: &str) -> bool {
    parse_stats_metrics(raw).is_some()
}

pub(super) fn layout(
    extra: &HashMap<String, String>,
    content_width: u32,
    font: &FontInfo,
    tag_font: &FontInfo,
) -> Option<ExtraSectionData> {
    let mut fields: Vec<_> = extra.iter().collect();
    fields.sort_by_key(|(key, _)| *key);
    let mut raw_tags = Vec::new();
    let mut metrics = Vec::new();
    let mut lines = Vec::new();
    for (key, value) in fields {
        if let Some(icon) = MetricIcon::from_key(key) {
            if value.trim().is_empty() {
                continue;
            }
            let width = ICON_SIZE + ICON_GAP + font.get_text_width(value);
            if width <= content_width {
                metrics.push(MetricData {
                    icon,
                    value: value.clone(),
                    width,
                });
                continue;
            }
        }
        if key == "stats"
            && let Some(parsed) = parse_stats_metrics(value)
        {
            let candidates: Vec<MetricData> = parsed
                .into_iter()
                .map(|(icon, val)| {
                    let width = ICON_SIZE + ICON_GAP + font.get_text_width(&val);
                    MetricData {
                        icon,
                        value: val,
                        width,
                    }
                })
                .collect();
            if candidates.iter().all(|m| m.width <= content_width) {
                metrics.extend(candidates);
                continue;
            }
        }
        if matches!(key.as_str(), "标签" | "tags") {
            raw_tags.extend(parse_tags(value));
            continue;
        }
        if let Some(detail) = crate::render::extra::format_extra_entry(key, value, " ") {
            lines.extend(wrap_text(&detail, content_width, font));
        }
    }

    let (tag_rows, wide_tags) = layout_tags(&raw_tags, content_width, tag_font);
    for tag in wide_tags {
        lines.extend(wrap_text(&tag, content_width, font));
    }

    metrics.sort_by_key(|metric| metric.icon);
    let mut metric_rows = Vec::new();
    let mut row = Vec::new();
    let mut used = 0;
    for metric in metrics {
        let gap = if row.is_empty() { 0 } else { ITEM_GAP };
        if !row.is_empty() && used + gap + metric.width > content_width {
            metric_rows.push(std::mem::take(&mut row));
            used = 0;
        }
        used += if row.is_empty() { 0 } else { ITEM_GAP };
        used += metric.width;
        row.push(metric);
    }
    if !row.is_empty() {
        metric_rows.push(row);
    }
    if tag_rows.is_empty() && metric_rows.is_empty() && lines.is_empty() {
        return None;
    }

    let tags_h = tags_height(tag_rows.len());
    let has_footer = !metric_rows.is_empty() || !lines.is_empty();
    let divider_gap = if !has_footer {
        0
    } else if !tag_rows.is_empty() {
        TAG_DIVIDER_GAP
    } else {
        HAIRLINE_TOP_GAP
    };
    let footer_h = metrics_height(metric_rows.len())
        + if !metric_rows.is_empty() && !lines.is_empty() {
            DETAIL_GAP
        } else {
            0
        }
        + lines.len() as u32 * font.line_height;
    let height = tags_h + divider_gap + footer_h;
    Some(ExtraSectionData {
        height,
        tag_rows,
        metric_rows,
        lines,
    })
}

fn tags_height(rows: usize) -> u32 {
    rows as u32 * TAG_HEIGHT + rows.saturating_sub(1) as u32 * TAG_GAP_Y
}

fn metrics_height(rows: usize) -> u32 {
    rows as u32 * ROW_HEIGHT + rows.saturating_sub(1) as u32 * ROW_GAP
}

fn draw_hairline(image: &mut RgbaImage, x: u32, y: u32, width: u32, color: Rgba<u8>) {
    if y >= image.height() {
        return;
    }
    let end = (x + width).min(image.width());
    for px in x..end {
        image.get_pixel_mut(px, y).blend(&color);
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    image: &mut RgbaImage,
    data: &ExtraSectionData,
    x: u32,
    y: u32,
    content_width: u32,
    font: &FontInfo,
    tag_font: &FontInfo,
    accent: Rgba<u8>,
    is_dark: bool,
    emojis: &EmojiImageMap,
) {
    let mut cursor_y = y;

    if !data.tag_rows.is_empty() {
        let mut pill_font = tag_font.clone();
        let (pill_bg, pill_ink) = if is_dark {
            (
                Rgba([accent[0], accent[1], accent[2], 32]),
                mix(accent, Rgba([255, 255, 255, 255]), 0.35),
            )
        } else {
            (
                Rgba([accent[0], accent[1], accent[2], 24]),
                mix(accent, Rgba([34, 30, 38, 255]), 0.42),
            )
        };
        pill_font.fill = pill_ink;
        for (index, row) in data.tag_rows.iter().enumerate() {
            let row_y = cursor_y + index as u32 * (TAG_HEIGHT + TAG_GAP_Y);
            let mut item_x = x;
            for tag in row {
                corners::fill(
                    image,
                    Rect::at(item_x as i32, row_y as i32).of_size(tag.width, TAG_HEIGHT),
                    TAG_RADIUS,
                    false,
                    |_| pill_bg,
                );
                draw_single_line(
                    image,
                    item_x + TAG_PAD_X,
                    row_y + TAG_HEIGHT.saturating_sub(pill_font.line_height) / 2,
                    &tag.text,
                    &pill_font,
                    emojis,
                );
                item_x += tag.width + TAG_GAP_X;
            }
        }
        cursor_y += tags_height(data.tag_rows.len());
    }

    let has_footer = !data.metric_rows.is_empty() || !data.lines.is_empty();
    if has_footer {
        let hairline_color = if is_dark {
            Rgba([255, 255, 255, 26])
        } else {
            Rgba([28, 25, 32, 28])
        };
        let hairline_y = if data.tag_rows.is_empty() {
            let line_y = cursor_y;
            cursor_y += HAIRLINE_TOP_GAP;
            line_y
        } else {
            let line_y = cursor_y + TAG_DIVIDER_GAP / 2;
            cursor_y += TAG_DIVIDER_GAP;
            line_y
        };
        draw_hairline(image, x, hairline_y, content_width, hairline_color);
    }

    let mut inline = font.clone();
    inline.line_height = font.size.ceil() as u32;
    let mut icon_color = font.fill;
    icon_color[3] = 210;
    for (index, row) in data.metric_rows.iter().enumerate() {
        let row_y = cursor_y + index as u32 * (ROW_HEIGHT + ROW_GAP);
        let mut item_x = x;
        for metric in row {
            metric.icon.draw(
                image,
                item_x,
                row_y + (ROW_HEIGHT - ICON_SIZE) / 2,
                icon_color,
            );
            draw_single_line(
                image,
                item_x + ICON_SIZE + ICON_GAP,
                row_y + ROW_HEIGHT.saturating_sub(inline.line_height) / 2,
                &metric.value,
                &inline,
                emojis,
            );
            item_x += metric.width + ITEM_GAP;
        }
    }
    let details_y = cursor_y
        + metrics_height(data.metric_rows.len())
        + if !data.metric_rows.is_empty() && !data.lines.is_empty() {
            DETAIL_GAP
        } else {
            0
        };
    draw_text_lines(image, x, details_y, &data.lines, font, emojis);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::image::font::Font;

    fn extra_font() -> FontInfo {
        let font = Font::new(
            include_bytes!("../../../../../resources/fonts/HYSongYunLangHeiW-1.ttf").as_slice(),
        )
        .expect("embedded font");
        FontInfo::new(font, Rgba([100, 100, 100, 255]), 18.0)
    }

    #[test]
    fn tags_parse_whitespace_and_commas_and_cap_overflow() {
        assert_eq!(parse_tags("#川西 #日落 #川西"), vec!["#川西", "#日落"]);
        assert_eq!(
            parse_tags("虚拟,直播，杂谈"),
            vec!["#虚拟", "#直播", "#杂谈"]
        );
        assert_eq!(parse_tags("#摄影,vlog #川西"), vec!["#摄影,vlog", "#川西"]);

        let many = (1..=15)
            .map(|i| format!("#标签{i}"))
            .collect::<Vec<_>>()
            .join(" ");
        let mut extra = HashMap::new();
        extra.insert("标签".to_string(), many);
        extra.insert("点赞".to_string(), "1.2万".to_string());
        let font = extra_font();
        let data = layout(&extra, 704, &font, &font).expect("extra layout");
        let flat: Vec<&str> = data
            .tag_rows
            .iter()
            .flat_map(|row| row.iter().map(|t| t.text.as_str()))
            .collect();
        assert_eq!(flat.len(), MAX_TAGS_DISPLAY + 1);
        assert_eq!(flat.last().copied(), Some("+3"));
        assert_eq!(data.metric_rows.len(), 1);
        assert!(data.lines.is_empty());
    }
}
