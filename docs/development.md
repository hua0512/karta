# Development

## How it works

When a message contains a recognized URL (or a QQ JSON card / mini-program share), the plugin:

1. Extracts candidate URLs from message text or JSON card payloads (`karta-kovi/src/input.rs`)
2. Matches them against platform regex patterns (`RegexSet`-based registry)
3. Calls the platform's API or scrapes the page to build a `ParseResult`
4. Renders the result as text segments or a native PNG card image
5. Optionally downloads and attaches media (video, images, audio), grouping galleries into platform-agnostic bundles
6. Delivers outputs in order — card, bundle (merged-forward on QQ), standalone video/audio — via OneBot V11

Identical in-flight links share one parse (single-flight), results are LRU-cached, and separate per-stage timeouts bound parsing and rendering.

## Project structure

```text
karta/
├── crates/
│   ├── karta-core/           # Framework-agnostic engine
│   │   ├── engine.rs         # ParserEngine, parse cache, timeout coordination
│   │   ├── parser.rs         # Parser trait, RegexSet-based registry
│   │   ├── data/             # ParseResult, Content types, Author
│   │   ├── render/           # Text and native image card renderers
│   │   ├── download/         # Stream downloader, FFmpeg wrapper, yt-dlp
│   │   ├── cache.rs          # LRU memory cache & SHA-256-keyed file cache
│   │   ├── config.rs         # Configuration model
│   │   ├── http.rs           # Shared HTTP helpers (retry, redirect, headers)
│   │   └── html.rs           # HTML/BBCode to text converters
│   ├── karta-platforms/      # One module per platform + create_all_parsers()
│   ├── karta-kovi/           # Bridges karta-core to the Kovi bot framework
│   │   ├── access_control.rs # Framework-level access-control setup
│   │   ├── commands.rs       # Admin command handlers (/status, /ban, ...)
│   │   ├── forward.rs        # OneBot forward-message sending and fallback
│   │   ├── gate.rs           # ParseGate, concurrency cap & single-flight
│   │   ├── input.rs          # QQ text / JSON-card URL extraction
│   │   ├── output.rs         # karta-core MessageOutput -> Kovi Message
│   │   ├── pipeline.rs       # Message handling, Bilibili refresh & cache cleanup
│   │   ├── reaction.rs       # Emoji reaction API compatibility probing
│   │   └── state.rs          # Runtime state persistence (state.json)
│   ├── karta/                # Binary crate: bot entry point + karta-cli
│   └── test-harness/         # Shared integration-test harness (fake upstream, tools, OneBot)
├── resources/
│   ├── fonts/                # Bundled font for image rendering
│   └── logos/                # Platform logos for card images
├── docs/                     # VitePress documentation
├── config.example.json       # Example plugin configuration
└── Cargo.toml                # Workspace root
```

Cargo feature on `karta`: `ytdlp` (enables the `yt-dlp` downloader).

### Adding a platform

1. Add the variant to `Platform` in `karta-core/src/constants.rs` (enum, `ALL`, `slug`, `Display`).
2. Create `karta-platforms/src/<name>.rs` (parser struct) and `karta-platforms/src/<name>/` (`patterns.rs`, API/result modules); mirror a similar existing platform.
3. Register it in `karta-platforms/src/lib.rs::create_all_parsers`.
4. Wire auxiliaries: config key (if auth is needed), alias in `karta-kovi/src/commands.rs::resolve_platform`, download headers in `karta-core/src/http.rs`, and a transparent logo in `resources/logos/<slug>.png` (add it to `embedded` in `karta-core/src/render/image.rs::load_logos` to bundle it into standalone binaries).
5. Add pattern cases to `crates/karta-platforms/tests/test_urls.md` and the heading mapping in `tests/test_patterns.rs`; document the platform in `docs/platforms.md` and `docs/zh/platforms.md`.

### Adding a chat provider (adapter)

`karta-core` is chat-platform-agnostic: renderers emit `RenderOutput` values — either a single `Message` or a `Bundle` (an ordered group the adapter should present natively). The QQ adapter maps bundles to OneBot merged-forwards; another adapter could map them to Telegram media groups, Discord embed batches, or sequential messages.

Implement a new adapter crate following `karta-kovi` across four concerns:

1. **Input:** extract candidate text/URLs from incoming messages.
2. **Output:** convert `RenderOutput` / `MessageSegment` values into native messages and deliver them in order.
3. **Commands & state:** admin commands and persisted runtime state (owned by the adapter; the engine is moderation-agnostic).
4. **Lifecycle:** construct `ParserEngine` from `Config` and wrap message events in a parse gate (single-flight + concurrency limits).

## Testing

```bash
# Run all workspace tests (CI uses cargo-nextest)
cargo test

# Pattern-matching integration tests (test_urls.md catalog)
cargo test -p karta-platforms --test test_patterns

# Live tests requiring real platform cookies (ignored by default)
cargo test -p karta-platforms --test bilibili_need_ck -- --ignored
cargo test -p karta-platforms --test douyin_need_ck -- --ignored
cargo test -p karta-platforms --test instagram_need_ck -- --ignored

# Native image renderer E2E tests
cargo test -p karta-core --test render_image_e2e

# Real ffmpeg/ffprobe E2E tests (ignored by default; CI job test-tools).
# KARTA_REQUIRE_TOOLS=1 fails instead of skipping when a tool is missing.
KARTA_REQUIRE_TOOLS=1 cargo test -p karta-core --test ffmpeg_e2e -- --ignored
```

### Renderer architecture & benchmarking (`card_bench`)

The native card renderer draws directly in Rust using `image`, `imageproc`, `swash`, `fast_image_resize` (runtime SIMD detection), and bundled `libwebp` for fast static-WebP thumbnail decoding. Do not enable libwebp's `avx2`/`sse41` Cargo features — they inject global compiler flags into its C build.

To benchmark parsing, media preparation, native drawing, and PNG encoding separately:

```bash
cargo run -p karta --release --example card_bench -- \
  /path/to/karta-data /path/to/results '<share-text-or-url>' 20 --fresh-media
```

- **`--fresh-media`** copies media to distinct local paths per iteration so thumbnail caches aren't reused while keeping the process and fonts warm. Omitting it measures repeated media paths.
- **Local JSON fixtures:** `card_bench` also accepts a local JSON fixture (`platform`, author/avatar, optional title/text/timestamp/extra, and `image`/`video` contents with local file paths) to benchmark drawing and PNG encoding without network requests.
- **Stage diagnostics:** Run with `RUST_LOG=karta_core::render::image=debug` to inspect per-stage timings (`layout_us`, `palette_us`, `frame_us`, `draw_us`, `header_us`, `shadow_us`, `background_us`). Leave debug logging off during normal benchmark runs.
- Outputs are written to the results directory as `native-d.png` and `timings.json`. When tuning for small VPS deployments (1–2 vCPU), evaluate under matching CPU affinity (e.g. `taskset` on Linux).

## Standalone CLI

The `karta-cli` binary runs the full parse and render pipeline without a QQ account or OneBot server:

```bash
cargo run -p karta --bin karta-cli -- <text-or-url>
```

| Option | Effect |
|---|---|
| `--render` | Render the parsed result (uses `render_type` from config) |
| `--render-type <text\|image>` | Override `render_type` |
| `--no-download` | Skip media downloads (faster metadata check) |
| `--json` | Emit machine-readable JSON output |
| `--append-url` | Include the original URL in the output |
| `--data-path <dir>` | Data directory (default `$TMP/karta-cli`) |
| `--parse-timeout <secs>` | Override `parse_timeout` (`0` disables) |
| `--trace` | Enable tracing logs (filter via `RUST_LOG`) |

## Releasing

Releases are automated by [`.github/workflows/release.yml`](https://github.com/hua0512/karta/blob/main/.github/workflows/release.yml) when a `v*.*.*` tag is pushed: CI builds all five binary targets, publishes multi-arch Docker images to GHCR (`<version>`, `<major>.<minor>`, and `latest`), and creates a GitHub Release using `docs/release-<tag>.md` as `body_path` (plus auto-generated notes).

1. Bump `version` in the root `Cargo.toml` and run `cargo update --workspace`.
2. Write the release notes at `docs/release-vX.Y.Z.md` (read by the release workflow as `body_path` for the GitHub Release).
3. Commit as `chore(release): vX.Y.Z`, create an annotated tag `vX.Y.Z`, and push both the commit and the tag.
