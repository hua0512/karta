//! Benchmark native card drawing and PNG encoding with cached media assets.
//!
//! Run a release build with `--example card_bench --
//! <data-path> <output-path> <share-text-url-or-local-json> [iterations] [--fresh-media]`.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, bail};
use image::{ImageFormat, RgbaImage};
use karta_core::data::{Content, ParseResult};
use karta_core::download::StreamDownloader;
use karta_core::render::RenderOptions;
use karta_core::render::image::ImageRenderer;
use karta_core::{Config, ParserEngine};
use serde_json::{Value, json};

#[path = "support/gallery.rs"]
mod gallery;

fn elapsed(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

fn stats(values: &[f64]) -> Value {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    let median = if sorted.len().is_multiple_of(2) {
        (sorted[middle - 1] + sorted[middle]) / 2.0
    } else {
        sorted[middle]
    };
    let p95 = sorted[((sorted.len() as f64 * 0.95).ceil() as usize).saturating_sub(1)];
    json!({"min_ms": sorted[0], "median_ms": median, "p95_ms": p95,
        "mean_ms": sorted.iter().sum::<f64>() / sorted.len() as f64})
}

fn encode(image: &RgbaImage) -> anyhow::Result<Vec<u8>> {
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, ImageFormat::Png)?;
    Ok(bytes.into_inner())
}

fn copy_asset(source: &Path, root: &Path, sequence: &mut usize) -> anyhow::Result<PathBuf> {
    if !source.is_file() {
        bail!("fresh-media mode requires local hydrated media");
    }
    let extension = source.extension().and_then(|s| s.to_str()).unwrap_or("img");
    let destination = root.join(format!("asset-{sequence:03}.{extension}"));
    *sequence += 1;
    std::fs::copy(source, &destination)?;
    Ok(destination)
}

fn copy_media(result: &mut ParseResult, root: &Path, sequence: &mut usize) -> anyhow::Result<()> {
    if let Some(author) = &mut result.author
        && let Some(avatar) = &mut author.avatar
    {
        *avatar = copy_asset(Path::new(avatar), root, sequence)?
            .to_string_lossy()
            .into_owned();
    }
    for content in &mut result.contents {
        match content {
            Content::Image(image) => {
                let path = image.path.as_mut().context("Image was not hydrated")?;
                *path = copy_asset(path, root, sequence)?;
            }
            Content::Video(video) => {
                if let Some(cover) = &mut video.cover {
                    *cover = copy_asset(Path::new(cover), root, sequence)?
                        .to_string_lossy()
                        .into_owned();
                }
            }
            Content::Graphics(graphics) => {
                for image in &mut graphics.images {
                    let path = image
                        .path
                        .as_mut()
                        .context("Graphics image was not hydrated")?;
                    *path = copy_asset(path, root, sequence)?;
                }
            }
            Content::Dynamic(dynamic) => {
                if let Some(cover) = &mut dynamic.cover_url {
                    *cover = copy_asset(Path::new(cover), root, sequence)?
                        .to_string_lossy()
                        .into_owned();
                }
            }
            _ => {}
        }
    }
    if let Some(repost) = &mut result.repost {
        copy_media(repost, root, sequence)?;
    }
    Ok(())
}

fn fresh_result(result: &ParseResult, root: &Path) -> anyhow::Result<ParseResult> {
    std::fs::create_dir_all(root)?;
    let mut result = result.clone();
    copy_media(&mut result, root, &mut 0)?;
    Ok(result)
}

async fn native_sample(
    renderer: &ImageRenderer,
    result: &karta_core::ParseResult,
) -> anyhow::Result<(RgbaImage, Vec<u8>, f64, f64, f64)> {
    let start = Instant::now();
    let image = renderer.render_card_image(result).await?;
    let draw_ms = elapsed(start);
    let encoding = Instant::now();
    let png = encode(&image)?;
    let encode_ms = elapsed(encoding);
    Ok((image, png, draw_ms, encode_ms, elapsed(start)))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Opt-in diagnostics (e.g. backdrop_us) without logging signed media URLs.
    if let Ok(filter) = tracing_subscriber::EnvFilter::try_from_default_env() {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_ansi(false)
            .without_time()
            .with_writer(std::io::stderr)
            .try_init();
    }
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        bail!(
            "Usage: card_bench <data-path> <output-path> <share-text-url-or-local-json> [iterations] [--fresh-media]"
        );
    }
    let data_path = PathBuf::from(&args[0]);
    let output_path = Path::new(&args[1]);
    let iterations: usize = args.get(3).map(|s| s.parse()).transpose()?.unwrap_or(20);
    if iterations == 0 {
        bail!("iterations must be positive");
    }
    let fresh_media = args.get(4).is_some_and(|s| s == "--fresh-media");
    if args.len() > 4 && !fresh_media {
        bail!("Unknown benchmark mode");
    }
    std::fs::create_dir_all(output_path)?;
    let mut config = Config::load(&data_path);
    config.download_media = true;
    config.append_url = false;
    config.emoji_enabled = false;
    let local_input = Path::new(&args[2]).is_file();
    let downloader: Option<karta_core::download::SharedDownloader> = if local_input {
        None
    } else {
        Some(Arc::new(StreamDownloader::new(&config, &data_path)?))
    };

    let parse_start = Instant::now();
    let result = if let Some(downloader) = &downloader {
        let parsers = karta_platforms::create_all_parsers(&config, downloader.clone(), &data_path);
        let engine = ParserEngine::with_parsers(
            config.clone(),
            data_path.clone(),
            downloader.clone(),
            parsers,
        );
        engine
            .parse(&args[2])
            .await?
            .context("No parser matched the input")?
            .as_ref()
            .clone()
    } else {
        gallery::local_post(Path::new(&args[2]))?
    };
    let parse_ms = elapsed(parse_start);
    println!(
        "Parsed {}: author={}, images={}, parse={parse_ms:.2}ms",
        result.platform,
        result
            .author
            .as_ref()
            .map(|a| a.name.as_str())
            .unwrap_or(""),
        result.images().len()
    );

    let resources_start = Instant::now();
    let options = RenderOptions::from(&config);
    let native = ImageRenderer::new(data_path.clone(), options);
    let resource_init_ms = elapsed(resources_start);
    let prepare_start = Instant::now();
    let hydrated = if let Some(downloader) = &downloader {
        native.prepare_card(&result, downloader).await?
    } else {
        result.clone()
    };
    let assets_ms = elapsed(prepare_start);
    println!("Media prepared in {assets_ms:.2}ms (existing downloader cache may be warm)");

    let (first_image, first_png, first_draw, first_encode, first_total) =
        native_sample(&native, &hydrated).await?;
    std::fs::write(output_path.join("native-d.png"), &first_png)?;
    let unique_media = tempfile::tempdir_in(&data_path)?;
    let mut draw = Vec::new();
    let mut encoding = Vec::new();
    let mut native_total = Vec::new();
    let mut native_bytes = first_png.len();
    for i in 0..iterations {
        // Unique paths invalidate processed-image caches.
        // Copying is outside the timer, like files just written by a downloader.
        let fresh;
        let sample = if fresh_media {
            fresh = fresh_result(
                &hydrated,
                &unique_media.path().join(format!("request-{i:03}")),
            )?;
            &fresh
        } else {
            &hydrated
        };
        let (_, png, draw_ms, encode_ms, total_ms) = native_sample(&native, sample).await?;
        draw.push(draw_ms);
        encoding.push(encode_ms);
        native_total.push(total_ms);
        native_bytes = png.len();
        println!(
            "Sample {}/{}: native={total_ms:.2}ms (draw={draw_ms:.2}ms, PNG={encode_ms:.2}ms)",
            i + 1,
            iterations
        );
    }
    let native_stats = stats(&native_total);
    let input_kind = match (local_input, result.video().is_some()) {
        (false, _) => "share_text_or_url",
        (true, true) => "local_post_json",
        (true, false) => "local_gallery_json",
    };
    let report = json!({
        "input": args[2], "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
        "input_kind": input_kind,
        "author": result.author.as_ref().map(|a| &a.name), "caption": result.text,
        "media_images": result.images().len(), "gallery_previews": result.images().len().min(9),
        "iterations": iterations, "output_pixels": [first_image.width(), first_image.height()],
        "mode": if fresh_media { "warm_process_fresh_media" } else { "repeated_media" },
        "asset_cache": if fresh_media {
            "Each sample uses new local file paths; file copying excluded; OS page cache warm; renderer process and fonts stay warm"
        } else { "Repeated local files; processed thumbnails can be cached" },
        "measurement": "Native card drawing and PNG encoding. Fresh-media mode simulates new assets using copies of this one post, not distinct real posts. File copies and output writes are excluded.",
        "cold": {"parse_ms": parse_ms, "asset_prepare_ms": assets_ms, "resource_init_ms": resource_init_ms,
            "native_draw_ms": first_draw, "native_png_ms": first_encode, "native_total_ms": first_total},
        "sample_statistics": {"native_draw": stats(&draw), "native_png": stats(&encoding),
            "native_total": native_stats},
        "samples_ms": {"native_draw": draw, "native_png": encoding, "native_total": native_total},
        "png_bytes": {"native": native_bytes}
    });
    std::fs::write(
        output_path.join("timings.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("Saved images and timings.json to {}", output_path.display());
    Ok(())
}
