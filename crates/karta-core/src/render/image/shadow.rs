//! A blurred nine-slice mask gives every card a soft shadow at bounded cost.

use std::sync::OnceLock;

use image::{GrayImage, Luma, Rgba, RgbaImage, imageops};
use imageproc::rect::Rect;

use super::constants::PAPER_RADIUS;
use super::corners::CornerMask;
use super::palette::mix;
use super::utils::draw_rounded_rect;

const MARGIN: u32 = 40;
const BLOCK: u32 = 128;
const OFFSET_Y: i32 = 8;

fn mask() -> &'static GrayImage {
    static MASK: OnceLock<GrayImage> = OnceLock::new();
    MASK.get_or_init(|| {
        let size = BLOCK + MARGIN * 2;
        let mut shape = RgbaImage::new(size, size);
        draw_rounded_rect(
            &mut shape,
            MARGIN,
            MARGIN,
            BLOCK,
            BLOCK,
            PAPER_RADIUS,
            Rgba([255; 4]),
        );
        let shape = GrayImage::from_fn(size, size, |x, y| Luma([shape.get_pixel(x, y)[3]]));
        let wide = imageops::fast_blur(&shape, 11.5);
        let contact = imageops::fast_blur(&shape, 3.5);
        GrayImage::from_fn(size, size, |x, y| {
            Luma([(f32::from(wide.get_pixel(x, y)[0]) * 0.18
                + f32::from(contact.get_pixel(x, y)[0]) * 0.055)
                .round() as u8])
        })
    })
}

fn slice_coordinate(position: i32, start: i32, length: u32) -> u32 {
    let relative = position - start;
    let coordinate = if length < BLOCK {
        MARGIN as i32 + relative * BLOCK as i32 / length.max(1) as i32
    } else if relative < (BLOCK / 2) as i32 {
        MARGIN as i32 + relative
    } else if relative >= length as i32 - (BLOCK / 2) as i32 {
        (MARGIN + BLOCK) as i32 - (length as i32 - relative)
    } else {
        (MARGIN + BLOCK / 2) as i32
    };
    coordinate.clamp(0, (MARGIN * 2 + BLOCK - 1) as i32) as u32
}

pub(super) fn paint(image: &mut RgbaImage, bounds: Rect, ink: Rgba<u8>) {
    let mask = mask();
    let corner_extent =
        CornerMask::for_size(bounds.width(), bounds.height(), PAPER_RADIUS).extent();
    let xs: Vec<_> = (0..image.width())
        .map(|x| slice_coordinate(x as i32, bounds.left(), bounds.width()))
        .collect();
    for y in 0..image.height() {
        let sy = slice_coordinate(y as i32, bounds.top() + OFFSET_Y, bounds.height());
        // The paper will cover the interior. Paint only the perimeter so the
        // work grows with edge length, not the area of a long article card.
        let ranges = if (bounds.top()..=bounds.bottom()).contains(&(y as i32)) {
            let corner = if y < bounds.top() as u32 + corner_extent
                || y > bounds.bottom() as u32 - corner_extent
            {
                corner_extent
            } else {
                0
            };
            [
                0..bounds.left() as u32 + corner,
                bounds.right() as u32 + 1 - corner..image.width(),
            ]
        } else {
            [0..image.width(), 0..0]
        };
        for x in ranges.into_iter().flatten() {
            let alpha = mask.get_pixel(xs[x as usize], sy)[0];
            if alpha != 0 {
                let pixel = image.get_pixel_mut(x, y);
                *pixel = mix(*pixel, ink, f32::from(alpha) / 255.0);
            }
        }
    }
}
