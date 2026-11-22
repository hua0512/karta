use std::path::PathBuf;
use std::time::Instant;

use anyhow::anyhow;
use karta_core::config::Config;
use karta_core::download::StreamDownloader;
use karta_core::{ParserEngine, RenderType};
use tracing_subscriber::EnvFilter;

fn print_usage() {
    eprintln!(
        "Usage: karta-cli [options] <text-or-url>\n\nOptions:\n  --data-path <dir>        Data directory (config/cache/downloads). Default: $TMP/karta-cli\n  --parse-timeout <secs>   Override config.parse_timeout (0 disables).\n  --render                 Also render the parsed result (uses config.render_type).\n  --render-type <text|image>  Override config.render_type (default: from config).\n  --no-download             Set config.download_media=false (faster, avoids media downloads).\n  --append-url              Set config.append_url=true (include original URL in result).\n  --json                    Print machine-readable JSON output.\n  --trace                  Enable tracing logs (configure with RUST_LOG).\n  -h, --help                Show this help."
    );
}

fn parse_args() -> Result<Args, anyhow::Error> {
    let mut args = std::env::args().skip(1);

    let mut out = Args::default();
    while let Some(a) = args.next() {
        match a.as_str() {
            "-h" | "--help" => {
                out.help = true;
                return Ok(out);
            }
            "--data-path" => {
                out.data_path = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| anyhow!("--data-path requires a value"))?,
                ));
            }
            "--parse-timeout" => {
                out.parse_timeout = Some(
                    args.next()
                        .ok_or_else(|| anyhow!("--parse-timeout requires a value"))?
                        .parse::<u64>()
                        .map_err(|e| anyhow!("invalid --parse-timeout: {e}"))?,
                );
            }
            "--render" => out.render = true,
            "--render-type" => {
                let v = args
                    .next()
                    .ok_or_else(|| anyhow!("--render-type requires a value"))?;
                out.render_type = Some(match v.as_str() {
                    "text" => RenderType::Text,
                    "image" => RenderType::Image,
                    _ => {
                        return Err(anyhow!(
                            "invalid --render-type {v} (expected 'text' or 'image')"
                        ));
                    }
                });
            }
            "--no-download" => out.no_download = true,
            "--append-url" => out.append_url = true,
            "--json" => out.json = true,
            "--trace" => out.trace = true,
            _ if a.starts_with('-') => return Err(anyhow!("unknown flag: {a}")),
            _ => {
                let mut parts = vec![a];
                parts.extend(args);
                out.input = Some(parts.join(" "));
                break;
            }
        }
    }

    Ok(out)
}

#[derive(Default)]
struct Args {
    help: bool,
    data_path: Option<PathBuf>,
    parse_timeout: Option<u64>,
    render: bool,
    render_type: Option<RenderType>,
    no_download: bool,
    append_url: bool,
    json: bool,
    trace: bool,
    input: Option<String>,
}

fn json_content_summary(result: &karta_core::ParseResult) -> serde_json::Value {
    use karta_core::data::Content;
    let contents = result
        .contents
        .iter()
        .map(|c| match c {
            Content::Text(t) => serde_json::json!({
                "type": "text",
                "text": t,
            }),
            Content::Video(v) => serde_json::json!({
                "type": "video",
                "url": v.url,
                "video_url": v.video_url,
                "audio_url": v.audio_url,
                "cover": v.cover,
                "duration": v.duration,
                "path": v.path.as_ref().map(|p| p.display().to_string()),
            }),
            Content::Image(i) => serde_json::json!({
                "type": "image",
                "url": i.url,
                "path": i.path.as_ref().map(|p| p.display().to_string()),
            }),
            Content::Audio(a) => serde_json::json!({
                "type": "audio",
                "url": a.url,
                "title": a.title,
                "artist": a.artist,
                "duration": a.duration,
                "path": a.path.as_ref().map(|p| p.display().to_string()),
            }),
            Content::Dynamic(_) => serde_json::json!({"type": "dynamic"}),
            Content::Graphics(_) => serde_json::json!({"type": "graphics"}),
        })
        .collect::<Vec<_>>();

    serde_json::Value::Array(contents)
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let args = match parse_args() {
        Ok(v) => v,
        Err(e) => {
            print_usage();
            return Err(e);
        }
    };
    if args.help {
        print_usage();
        return Ok(());
    }

    let input = match args.input {
        Some(v) => v,
        None => {
            print_usage();
            return Err(anyhow!("missing <text-or-url>"));
        }
    };

    let data_path = args
        .data_path
        .unwrap_or_else(|| std::env::temp_dir().join("karta-cli"));
    let mut config = Config::load(&data_path);

    if args.trace {
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new("info,karta_platforms=debug,karta_core=info,karta=info")
        });
        let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
        tracing::info!(
            data_path = %data_path.display(),
            douyin_cookie_present = config.douyin_cookie.is_some(),
            "Loaded config"
        );
    }

    if let Some(t) = args.parse_timeout {
        config.parse_timeout = t;
    }
    if args.no_download {
        config.download_media = false;
    }
    if args.append_url {
        config.append_url = true;
    }
    if let Some(rt) = args.render_type {
        config.render_type = rt;
    }

    let downloader = std::sync::Arc::new(StreamDownloader::new(&config, &data_path)?);
    let parsers = karta_platforms::create_all_parsers(&config, downloader.clone(), &data_path);
    let engine = ParserEngine::with_parsers(config, data_path, downloader, parsers);

    let start = Instant::now();
    let parsed = engine.parse(&input).await;
    let elapsed_ms = start.elapsed().as_millis();

    let parsed = match parsed {
        Ok(v) => v,
        Err(e) => {
            if args.json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": false,
                        "stage": "parse",
                        "error": e.to_string(),
                        "elapsed_ms": elapsed_ms,
                    })
                );
                return Ok(());
            }
            return Err(e.into());
        }
    };

    let Some(result) = parsed else {
        if args.json {
            println!(
                "{}",
                serde_json::json!({"ok": true, "matched": false, "elapsed_ms": elapsed_ms})
            );
        } else {
            println!("No parser matched. elapsed={}ms", elapsed_ms);
        }
        return Ok(());
    };

    if args.json {
        let mut v = serde_json::json!({
            "ok": true,
            "matched": true,
            "elapsed_ms": elapsed_ms,
            "platform": result.platform.to_string(),
            "title": result.title.clone(),
            "text": result.text.clone(),
            "timestamp": result.timestamp,
            "url": result.url.clone(),
            "author": result.author.as_ref().map(|a| serde_json::json!({
                "name": a.name,
                "uid": a.uid,
                "avatar": a.avatar,
                "url": a.url,
                "description": a.description,
            })),
            "extra": result.extra.clone(),
            "contents": json_content_summary(&result),
        });

        if args.render {
            let start = Instant::now();
            match engine.render(&result).await {
                Ok(messages) => {
                    let render_ms = start.elapsed().as_millis();
                    let msgs = messages
                        .iter()
                        .map(|output| {
                            use karta_core::message::RenderOutput;
                            let kind = match output {
                                RenderOutput::Message(_) => "message",
                                RenderOutput::Bundle(_) => "bundle",
                            };
                            serde_json::json!({
                                "kind": kind,
                                "messages": output
                                    .messages()
                                    .iter()
                                    .map(|m| serde_json::json!({
                                        "segments": m
                                            .segments
                                            .iter()
                                            .map(|s| serde_json::json!({
                                                "type": format!("{:?}", s.segment_type),
                                                "text": s.text.clone(),
                                                "extra": s.extra.clone(),
                                            }))
                                            .collect::<Vec<_>>(),
                                    }))
                                    .collect::<Vec<_>>(),
                            })
                        })
                        .collect::<Vec<_>>();
                    if let serde_json::Value::Object(map) = &mut v {
                        map.insert("render_elapsed_ms".into(), serde_json::json!(render_ms));
                        map.insert("render_messages".into(), serde_json::Value::Array(msgs));
                    }
                }
                Err(e) => {
                    if let serde_json::Value::Object(map) = &mut v {
                        map.insert("render_error".into(), serde_json::json!(e.to_string()));
                    }
                }
            }
        }

        println!("{}", v);
        return Ok(());
    }

    println!(
        "Matched: platform={} elapsed={}ms",
        result.platform, elapsed_ms
    );
    if let Some(title) = &result.title {
        println!("Title: {title}");
    }
    if let Some(author) = &result.author {
        println!("Author: {}", author.name);
    }
    if let Some(ts) = result.formatted_time() {
        println!("Time: {ts}");
    }
    if !result.extra.is_empty() {
        println!("Extra:");
        for (k, v) in &result.extra {
            println!("  - {k}: {v}");
        }
    }
    println!("Contents: {}", result.contents.len());
    for (i, c) in result.contents.iter().enumerate() {
        use karta_core::data::Content;
        match c {
            Content::Text(t) => println!("  [{i}] text len={}", t.len()),
            Content::Video(v) => println!(
                "  [{i}] video url={:?} cover={:?} duration={:?}",
                v.url.as_deref(),
                v.cover.as_deref(),
                v.duration
            ),
            Content::Image(img) => println!("  [{i}] image url={:?}", img.url.as_deref()),
            Content::Audio(a) => println!(
                "  [{i}] audio url={:?} title={:?} artist={:?}",
                a.url.as_deref(),
                a.title.as_deref(),
                a.artist.as_deref()
            ),
            Content::Dynamic(_) => println!("  [{i}] dynamic"),
            Content::Graphics(_) => println!("  [{i}] graphics"),
        }
    }

    if args.render {
        let start = Instant::now();
        match engine.render(&result).await {
            Ok(messages) => {
                let render_ms = start.elapsed().as_millis();
                println!("Rendered {} message(s) in {}ms", messages.len(), render_ms);

                // Save image segments to disk for visual inspection
                let out_dir = std::env::temp_dir().join("karta-cli-output");
                let _ = std::fs::create_dir_all(&out_dir);
                let mut saved = 0u32;

                for (mi, output) in messages.iter().enumerate() {
                    let items = output.messages();
                    if items.len() > 1 {
                        println!("  msg[{mi}] bundle of {} message(s)", items.len());
                    }
                    for (bi, msg) in items.iter().enumerate() {
                        println!("  msg[{mi}.{bi}] segments={}", msg.segments.len());
                        for (si, seg) in msg.segments.iter().enumerate() {
                            use karta_core::message::SegmentType;
                            if seg.segment_type == SegmentType::Image
                                && let Some(src) = &seg.file_path
                            {
                                let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("png");
                                let filename = format!("msg{mi}_{bi}_seg{si}.{ext}");
                                let dest = out_dir.join(&filename);
                                match std::fs::copy(src, &dest) {
                                    Ok(_) => {
                                        println!("    Saved: {}", dest.display());
                                        saved += 1;
                                    }
                                    Err(e) => eprintln!("    Copy failed {filename}: {e}"),
                                }
                            }
                        }
                    }
                }

                if saved > 0 {
                    println!("=== {saved} image(s) saved to {} ===", out_dir.display());
                }
            }
            Err(e) => {
                println!("Render error: {e}");
            }
        }
    }

    Ok(())
}
