# Deployment

Karta ships as multi-arch Docker images, prebuilt release binaries, or a Rust workspace you can compile from source.

## Docker (recommended)

Multi-arch images (`linux/amd64`, `linux/arm64`) are published to GHCR on every release:

```text
ghcr.io/hua0512/karta:latest    # newest release
ghcr.io/hua0512/karta:0.1.0     # pinned version
ghcr.io/hua0512/karta:0.1       # newest patch of a minor
```

The container image bundles `ffmpeg` and all native card rendering assets (fonts and platform logos). `yt-dlp` is supplied separately (see [yt-dlp in Docker](#yt-dlp-in-docker)).

Two paths must be mounted into the container:

| Container path | Purpose |
|---|---|
| `/app/kovi.conf.toml` | OneBot V11 connection and admin IDs ([format](getting-started.md)) |
| `/app/data` | Persistent Karta directory: `data/karta/{config.json,state.json,cache,downloads}` |

::: code-group

```yaml [docker-compose.yml]
services:
  karta:
    image: ghcr.io/hua0512/karta:latest
    restart: unless-stopped
    volumes:
      - ./kovi.conf.toml:/app/kovi.conf.toml
      - ./data:/app/data
```

```bash [docker compose]
# If kovi.conf.toml already exists:
docker compose up -d

# Or run interactively once to generate kovi.conf.toml:
docker compose run --rm karta
docker compose up -d
```

```bash [docker run]
docker run -d \
  --name karta \
  --restart unless-stopped \
  -v "$(pwd)/kovi.conf.toml:/app/kovi.conf.toml" \
  -v "$(pwd)/data:/app/data" \
  ghcr.io/hua0512/karta:latest
```

:::

On first start, Karta writes `./data/karta/config.json` with default values. Edit it and restart the container to apply changes ([Configuration reference](configuration.md)).

### yt-dlp in Docker

The Docker image is built with the `ytdlp` Cargo feature, but does not bundle the `yt-dlp` binary itself. YouTube video/audio downloads (and TikTok fallback downloads) use `yt-dlp` when present.

Mount a standalone `yt-dlp` binary into the container:

```yaml
services:
  karta:
    image: ghcr.io/hua0512/karta:latest
    restart: unless-stopped
    volumes:
      - ./kovi.conf.toml:/app/kovi.conf.toml
      - ./data:/app/data
      - ./yt-dlp:/usr/local/bin/yt-dlp:ro
```

If you mount it at another path, point `"ytdlp_path"` in `data/karta/config.json` to that path.

## Prebuilt binaries

Each [GitHub Release](https://github.com/hua0512/karta/releases) ships both `karta` and `karta-cli` (compiled with the `ytdlp` feature) for five targets:

- `x86_64-unknown-linux-gnu` / `x86_64-unknown-linux-musl`
- `aarch64-unknown-linux-gnu` / `aarch64-unknown-linux-musl`
- `x86_64-pc-windows-msvc`

Install these optional runtime tools on `PATH` (or configure their paths in `config.json`):

| Dependency | Needed for |
|---|---|
| `ffmpeg` | Merging separate video + audio streams (Bilibili, YouTube, etc.) |
| `yt-dlp` | YouTube video/audio downloads and TikTok fallback (optional) |

::: info Card rendering & platform logos with standalone binaries
The default font and 10 bundled platform logos are embedded directly in the binary (and all header styles are rendered procedurally), so `"render_type": "image"` works standalone out of the box with no extra files required. To override a bundled logo (or supply logos for platforms that otherwise render text wordmarks), place `<slug>.png` under `data/karta/resources/logos/` or `./resources/logos/`.
:::

## Building from source

Karta uses [wreq](https://github.com/0x676e67/wreq) (a `reqwest` fork with BoringSSL TLS-fingerprint impersonation). Compiling BoringSSL requires a C/C++ toolchain and CMake:

| OS | Prerequisites |
|---|---|
| Debian / Ubuntu | `sudo apt install -y cmake perl clang libclang-dev pkg-config git build-essential` |
| Arch Linux | `sudo pacman -S cmake perl clang pkgconf git` |
| Alpine | `apk add cmake make perl clang-dev musl-dev g++ pkgconf git` |
| macOS | `brew install cmake llvm` (Xcode Command Line Tools provide the rest) |
| Windows | Visual Studio Build Tools (MSVC C++ workload), CMake, Perl, LLVM/Clang, and **NASM** on `PATH` |

::: code-group

```bash [With yt-dlp (Recommended)]
cargo build --release -p karta --features ytdlp
```

```bash [Plain build (No yt-dlp)]
cargo build --release -p karta
```

:::

Run the compiled binary (`target/release/karta`) from the directory holding `kovi.conf.toml`. Karta stores its config, runtime state, cache, and downloads under `./data/karta/`.
