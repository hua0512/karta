//! Decode directly to display size when possible, then use portable SIMD resizing.

use std::io::Read;
use std::path::Path;

use fast_image_resize::{FilterType, ResizeAlg, ResizeOptions, Resizer};
use image::RgbaImage;
use libwebp_sys::{VP8StatusCode::VP8_STATUS_OK, WEBP_CSP_MODE::MODE_RGBA};

use super::constants::MAX_COVER_HEIGHT;

#[derive(Clone, Copy)]
pub(super) enum Thumbnail {
    Grid {
        size: u32,
        square: bool,
    },
    Cover {
        width: u32,
    },
    /// Center-crop to match `width / height` and resize directly to exact pixel dimensions.
    Crop {
        width: u32,
        height: u32,
    },
}

/// Fast header-only dimension probe inspecting magic bytes (not file extensions)
/// without decoding pixels.
pub(super) fn probe_dimensions(path: &Path) -> Option<(u32, u32)> {
    image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
}

#[derive(Debug)]
struct ResizePlan {
    left: u32,
    top: u32,
    crop_width: u32,
    crop_height: u32,
    width: u32,
    height: u32,
}

impl ResizePlan {
    fn new(width: u32, height: u32, request: Thumbnail) -> Option<Self> {
        if width == 0 || height == 0 {
            return None;
        }
        let mut plan = Self {
            left: 0,
            top: 0,
            crop_width: width,
            crop_height: height,
            width,
            height,
        };
        match request {
            Thumbnail::Grid { size, square } => {
                if square {
                    let side = width.min(height);
                    plan.left = (width - side) / 2;
                    plan.top = (height - side) / 2;
                    plan.crop_width = side;
                    plan.crop_height = side;
                }
                let scale = (f64::from(size) / f64::from(plan.crop_width))
                    .min(f64::from(size) / f64::from(plan.crop_height))
                    .min(1.0);
                plan.width = (f64::from(plan.crop_width) * scale) as u32;
                plan.height = (f64::from(plan.crop_height) * scale) as u32;
            }
            Thumbnail::Cover { width: target } => {
                let scale = (f64::from(target) / f64::from(width))
                    .min(f64::from(MAX_COVER_HEIGHT) / f64::from(height));
                plan.width = (f64::from(width) * scale) as u32;
                plan.height = (f64::from(height) * scale) as u32;
            }
            Thumbnail::Crop {
                width: target_w,
                height: target_h,
            } => {
                let target_aspect = f64::from(target_w.max(1)) / f64::from(target_h.max(1));
                let src_aspect = f64::from(width) / f64::from(height);
                if src_aspect > target_aspect {
                    let crop_w = (f64::from(height) * target_aspect)
                        .round()
                        .clamp(1.0, f64::from(width)) as u32;
                    plan.left = (width - crop_w) / 2;
                    plan.crop_width = crop_w;
                } else {
                    let crop_h = (f64::from(width) / target_aspect)
                        .round()
                        .clamp(1.0, f64::from(height)) as u32;
                    plan.top = (height - crop_h) / 2;
                    plan.crop_height = crop_h;
                }
                plan.width = target_w.max(1);
                plan.height = target_h.max(1);
            }
        }
        plan.width = plan.width.max(1);
        plan.height = plan.height.max(1);
        Some(plan)
    }

    fn options(&self) -> ResizeOptions {
        ResizeOptions::new().crop(
            f64::from(self.left),
            f64::from(self.top),
            f64::from(self.crop_width),
            f64::from(self.crop_height),
        )
    }
}

/// Large lossy WebP galleries decode directly to thumbnails, so their worker
/// budget can follow available CPUs. Retain four workers for formats whose
/// fallback decoder can allocate a full-resolution image.
pub(super) fn grid_decode_workers(paths: &[&str]) -> usize {
    let available = std::thread::available_parallelism().map_or(1, |count| count.get());
    let limit = if available > 4
        && paths
            .iter()
            .all(|path| bounded_webp_thumbnail(Path::new(path)))
    {
        available
    } else {
        available.min(4)
    };
    paths.len().min(limit)
}

fn bounded_webp_thumbnail(path: &Path) -> bool {
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    // A VP8X file can place color-profile metadata before the VP8 header.
    // Keep probing bounded; larger metadata simply retains the smaller budget.
    let mut header = [0; 4096];
    let Ok(length) = file.read(&mut header) else {
        return false;
    };
    let Ok(mut config) = libwebp_sys::WebPDecoderConfig::new() else {
        return false;
    };
    // SAFETY: header is readable for length bytes and config.input is initialized
    // and exclusively borrowed. Feature inspection retains neither pointer.
    let status =
        unsafe { libwebp_sys::WebPGetFeatures(header.as_ptr(), length, &mut config.input) };
    // Lossless WebP can need a full-resolution intermediate even when scaled.
    status == VP8_STATUS_OK
        && config.input.format == 1
        && config.input.has_animation == 0
        && (config.input.width as u64 * config.input.height as u64 * 4)
            <= image::Limits::default().max_alloc.unwrap_or(u64::MAX)
}

pub(super) fn load_thumbnail(path: &Path, request: Thumbnail) -> Option<RgbaImage> {
    let bytes = std::fs::read(path).ok()?;
    // Inspect the bitstream, not the filename: CDN files often have no suffix.
    if image::guess_format(&bytes).ok() == Some(image::ImageFormat::WebP)
        && let Some(image) = decode_webp(&bytes, request)
    {
        return Some(image);
    }

    // Retain the existing codecs, including first-frame animated WebP fallback.
    let decoded = image::load_from_memory(&bytes).ok()?;
    let has_alpha = decoded.color().has_alpha();
    let plan = ResizePlan::new(decoded.width(), decoded.height(), request)?;
    let source = decoded.into_rgba8();
    if plan.left == 0
        && plan.top == 0
        && plan.width == source.width()
        && plan.height == source.height()
    {
        return Some(source);
    }
    let mut output = RgbaImage::new(plan.width, plan.height);
    // Crop through an image view, avoiding a full-resolution square copy.
    Resizer::new()
        .resize(&source, &mut output, &plan.options().use_alpha(has_alpha))
        .ok()?;
    Some(output)
}

fn decode_webp(bytes: &[u8], request: Thumbnail) -> Option<RgbaImage> {
    let mut config = libwebp_sys::WebPDecoderConfig::new().ok()?;
    // SAFETY: bytes is readable for its entire length; input is initialized and
    // exclusively borrowed. Feature inspection does not retain either pointer.
    let status =
        unsafe { libwebp_sys::WebPGetFeatures(bytes.as_ptr(), bytes.len(), &mut config.input) };
    if status != VP8_STATUS_OK || config.input.has_animation != 0 {
        return None;
    }
    let width = u32::try_from(config.input.width).ok()?;
    let height = u32::try_from(config.input.height).ok()?;
    // Keep the image crate's default decoded-byte limit for source images too.
    let decoded_bytes = u64::from(width) * u64::from(height) * 4;
    if decoded_bytes > image::Limits::default().max_alloc.unwrap_or(u64::MAX) {
        return None;
    }
    let plan = ResizePlan::new(width, height, request)?;
    let mut output = RgbaImage::new(plan.width, plan.height);
    config.options.use_cropping = 1;
    // libwebp snaps crop offsets to even coordinates for YUV subsampling.
    config.options.crop_left = (plan.left & !1) as i32;
    config.options.crop_top = (plan.top & !1) as i32;
    config.options.crop_width = plan.crop_width as i32;
    config.options.crop_height = plan.crop_height as i32;
    config.options.use_scaling = 1;
    config.options.scaled_width = plan.width as i32;
    config.options.scaled_height = plan.height as i32;
    // Galleries already parallelize independent photos. Single covers and
    // illustrations can overlap libwebp's decoding/reconstruction work instead.
    config.options.use_threads = i32::from(
        matches!(
            request,
            Thumbnail::Cover { .. } | Thumbnail::Grid { square: false, .. }
        ) && std::thread::available_parallelism().is_ok_and(|count| count.get() > 1),
    );
    config.output.colorspace = MODE_RGBA;
    config.output.is_external_memory = 1;
    config.output.u.RGBA = libwebp_sys::WebPRGBABuffer {
        rgba: output.as_mut_ptr(),
        stride: i32::try_from(plan.width.checked_mul(4)?).ok()?,
        size: output.as_raw().len(),
    };
    // SAFETY: the initialized config describes a writable RGBA buffer with a
    // checked stride and exact byte length. bytes and output remain alive and
    // unmoved until decoding and cleanup finish. No pointers escape this call.
    let status = unsafe { libwebp_sys::WebPDecode(bytes.as_ptr(), bytes.len(), &mut config) };
    // SAFETY: release decoder-owned scratch storage on success AND failure.
    // is_external_memory ensures libwebp never frees our output buffer.
    unsafe { libwebp_sys::WebPFreeDecBuffer(&mut config.output) };
    (status == VP8_STATUS_OK).then_some(output)
}

/// Resize RGBA pixels with runtime AVX2/SSE4.1/NEON detection and scalar fallback.
/// Alpha is premultiplied during filtering to avoid halos around transparency.
pub(super) fn resize(source: &RgbaImage, width: u32, height: u32, filter: FilterType) -> RgbaImage {
    if source.dimensions() == (width, height) {
        return source.clone();
    }
    let mut output = RgbaImage::new(width.max(1), height.max(1));
    if !source.is_empty() {
        Resizer::new()
            .resize(
                source,
                &mut output,
                &ResizeOptions::new().resize_alg(ResizeAlg::Convolution(filter)),
            )
            .expect("nonempty RGBA8 source and destination views are compatible");
    }
    output
}

#[cfg(test)]
mod tests {
    use image::{ImageFormat, Rgba};

    use super::*;

    #[test]
    fn expanded_worker_budget_only_accepts_bounded_lossy_webp() {
        let directory = tempfile::tempdir().unwrap();
        let source = vec![128; 64 * 64 * 3];
        let mut encoded = std::ptr::null_mut();
        // SAFETY: source holds 64 RGB rows at the stated stride, and encoded is
        // a writable out-pointer. libwebp owns the returned allocation.
        let length = unsafe {
            libwebp_sys::WebPEncodeRGB(source.as_ptr(), 64, 64, 64 * 3, 75.0, &mut encoded)
        };
        assert!(length > 0 && !encoded.is_null());
        // SAFETY: a successful encoder result owns length readable bytes. Copy
        // them before releasing the allocation exactly once with WebPFree.
        let bytes = unsafe {
            let bytes = std::slice::from_raw_parts(encoded, length).to_vec();
            libwebp_sys::WebPFree(encoded.cast());
            bytes
        };
        let lossy = directory.path().join("lossy.bin");
        std::fs::write(&lossy, &bytes).unwrap();
        assert!(bounded_webp_thumbnail(&lossy));

        let mut extended = b"RIFF\0\0\0\0WEBPVP8X".to_vec();
        extended.extend_from_slice(&10u32.to_le_bytes());
        extended.extend_from_slice(&[0x20, 0, 0, 0, 63, 0, 0, 63, 0, 0]);
        extended.extend_from_slice(b"ICCP");
        extended.extend_from_slice(&512u32.to_le_bytes());
        extended.resize(extended.len() + 512, 0);
        extended.extend_from_slice(&bytes[12..]);
        let riff_size = (extended.len() - 8) as u32;
        extended[4..8].copy_from_slice(&riff_size.to_le_bytes());
        let metadata = directory.path().join("profiled.bin");
        std::fs::write(&metadata, &extended).unwrap();
        assert!(bounded_webp_thumbnail(&metadata));
        assert!(
            decode_webp(
                &extended,
                Thumbnail::Grid {
                    size: 32,
                    square: true
                }
            )
            .is_some()
        );

        let source = RgbaImage::from_pixel(64, 64, Rgba([40, 80, 120, 160]));
        for format in [ImageFormat::Png, ImageFormat::WebP] {
            let path = directory.path().join(format!("{format:?}.bin"));
            source.save_with_format(&path, format).unwrap();
            assert!(!bounded_webp_thumbnail(&path), "{format:?}");
        }
        std::fs::write(&lossy, &bytes[..12]).unwrap();
        assert!(!bounded_webp_thumbnail(&lossy));
        assert!(!bounded_webp_thumbnail(&directory.path().join("missing")));
    }

    #[test]
    fn thumbnail_plans_bound_extreme_aspect_ratios() {
        let cases = [
            ((2160, 2880), Thumbnail::Cover { width: 704 }, (600, 800)),
            (
                (4000, 1),
                Thumbnail::Grid {
                    size: 228,
                    square: false,
                },
                (228, 1),
            ),
            ((1, 4000), Thumbnail::Cover { width: 704 }, (1, 800)),
            (
                (60, 40),
                Thumbnail::Grid {
                    size: 704,
                    square: false,
                },
                (60, 40),
            ),
        ];
        for ((width, height), request, expected) in cases {
            let plan = ResizePlan::new(width, height, request).unwrap();
            assert_eq!((plan.width, plan.height), expected);
        }
    }

    #[test]
    fn webp_and_png_thumbnails_keep_center_crop_and_alpha() {
        let directory = tempfile::tempdir().unwrap();
        let source = RgbaImage::from_fn(320, 160, |x, _| match x {
            0..80 => Rgba([255, 0, 0, 255]),
            80..240 => Rgba([0, 90, 200, 160]),
            _ => Rgba([0, 255, 0, 255]),
        });
        for format in [ImageFormat::Png, ImageFormat::WebP] {
            let path = directory.path().join(format!("{format:?}.bin"));
            source.save_with_format(&path, format).unwrap();
            if format == ImageFormat::WebP {
                assert!(
                    decode_webp(
                        &std::fs::read(&path).unwrap(),
                        Thumbnail::Grid {
                            size: 80,
                            square: true
                        }
                    )
                    .is_some(),
                    "the native thumbnail decoder must be exercised"
                );
            }
            let thumb = load_thumbnail(
                &path,
                Thumbnail::Grid {
                    size: 80,
                    square: true,
                },
            )
            .unwrap();
            assert_eq!(thumb.dimensions(), (80, 80));
            let pixel = thumb.get_pixel(40, 40);
            assert!(
                (i16::from(pixel[2]) - 200).abs() <= 2,
                "{format:?}: {pixel:?}"
            );
            assert!(
                (i16::from(pixel[3]) - 160).abs() <= 1,
                "{format:?}: {pixel:?}"
            );
        }
        let invalid = directory.path().join("broken.webp");
        std::fs::write(&invalid, b"RIFF\x20\0\0\0WEBPVP8 ").unwrap();
        assert!(load_thumbnail(&invalid, Thumbnail::Cover { width: 704 }).is_none());
    }

    #[test]
    fn animated_webp_uses_the_existing_first_frame_decoder() {
        fn chunk(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
            let mut bytes = Vec::from(tag.as_slice());
            bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            bytes.extend_from_slice(payload);
            if !payload.len().is_multiple_of(2) {
                bytes.push(0);
            }
            bytes
        }
        // Wrap one lossless frame in an animated WebP container. The fast path
        // must decline animation, while the established codec draws frame one.
        let source = RgbaImage::from_pixel(16, 16, Rgba([30, 120, 200, 255]));
        let mut frame = std::io::Cursor::new(Vec::new());
        source.write_to(&mut frame, ImageFormat::WebP).unwrap();
        let mut payload = Vec::from(b"WEBP".as_slice());
        payload.extend(chunk(b"VP8X", &[2, 0, 0, 0, 15, 0, 0, 15, 0, 0]));
        payload.extend(chunk(b"ANIM", &[0; 6]));
        let mut frame_header = vec![0; 16];
        frame_header[6] = 15;
        frame_header[9] = 15;
        frame_header[12] = 100;
        frame_header[15] = 2;
        frame_header.extend_from_slice(&frame.into_inner()[12..]);
        payload.extend(chunk(b"ANMF", &frame_header));
        let mut bytes = Vec::from(b"RIFF".as_slice());
        bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes.extend(payload);
        let request = Thumbnail::Grid {
            size: 8,
            square: true,
        };
        assert!(decode_webp(&bytes, request).is_none());
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("animation.webp");
        std::fs::write(&path, bytes).unwrap();
        let thumb = load_thumbnail(&path, request).expect("animated WebP first frame");
        assert_eq!(thumb.dimensions(), (8, 8));
        assert_eq!(thumb.get_pixel(4, 4).0, [30, 120, 200, 255]);
    }

    #[test]
    fn simd_resize_does_not_bleed_transparent_rgb_into_visible_edges() {
        let source = RgbaImage::from_fn(32, 32, |x, _| {
            if x < 16 {
                Rgba([255, 0, 0, 0])
            } else {
                Rgba([0, 0, 255, 255])
            }
        });
        let output = resize(&source, 13, 13, FilterType::Lanczos3);
        for pixel in output.pixels().filter(|pixel| pixel[3] > 16) {
            assert_eq!(pixel[0], 0, "transparent red leaked: {pixel:?}");
            assert!(pixel[2] >= 250);
        }
    }

    #[test]
    fn probe_dimensions_guesses_magic_bytes_and_crop_webp_scales() {
        let directory = tempfile::tempdir().unwrap();
        let source = RgbaImage::from_pixel(180, 320, Rgba([40, 80, 120, 255]));
        // Save a WebP payload under a misleading `.jpg` suffix and an extensionless file.
        let misnamed = directory.path().join("cover.jpg");
        source
            .save_with_format(&misnamed, ImageFormat::WebP)
            .unwrap();
        let extensionless = directory.path().join("cover");
        std::fs::copy(&misnamed, &extensionless).unwrap();

        assert_eq!(probe_dimensions(&misnamed), Some((180, 320)));
        assert_eq!(probe_dimensions(&extensionless), Some((180, 320)));

        let cropped = load_thumbnail(
            &misnamed,
            Thumbnail::Crop {
                width: 90,
                height: 120,
            },
        )
        .expect("WebP Thumbnail::Crop");
        assert_eq!(cropped.dimensions(), (90, 120));
    }
}
