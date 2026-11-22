# Getting Started

Karta connects to QQ over an **OneBot V11 forward-WebSocket** connection — you run a OneBot V11 server such as [NapCat](https://github.com/NapNeko/NapCatQQ), [Lagrange](https://github.com/LagrangeDev/Lagrange.Core), or [LLOneBot](https://github.com/LLOneBot/LLOneBot), and Karta connects to it as a client.

## How Karta works

<KartaPipelineFlow />

Two files in your working directory control everything:

| File | Purpose |
|---|---|
| `kovi.conf.toml` | Kovi framework config: OneBot V11 connection and admin QQ IDs |
| `data/karta/config.json` | Karta config: rendering style, media limits, proxy, and platform cookies ([reference](configuration.md)) |

::: tip Just want the CLI?
You can parse links and render image cards locally without a QQ account or OneBot server using `karta-cli`:

```bash
karta-cli --render --render-type image "https://b23.tv/YTg9oSw"
```

See [Standalone CLI](development.md#standalone-cli) for all flags.
:::

## Quickstart

### 1. Connect OneBot V11 (`kovi.conf.toml`)

Create `kovi.conf.toml` in your deployment directory pointing at your NapCat, Lagrange, or LLOneBot forward-WebSocket server:

```toml
[server]                 # OneBot V11 forward-WebSocket endpoint
host = "127.0.0.1"       # Server IP or hostname
port = 8081              # Server port
access_token = ""        # Leave empty if your OneBot server has no token
secure = false           # false = ws://, true = wss://
path = "/"               # WebSocket route path
all_in_one = false       # true if your server exposes one combined endpoint

[config]                 # Kovi framework settings
main_admin = 123456789   # Primary admin QQ ID (allowed to run /status, /karta, etc.)
admins = []              # Additional admin QQ IDs
debug = false            # true enables RUST_LOG=debug when RUST_LOG is unset
```

The accounts in `main_admin` and `admins` can run Karta's [admin commands](commands.md). If you omit `kovi.conf.toml`, Kovi prompts interactively for these fields on first start and writes the file for you.

### 2. Start Karta

::: code-group

```yaml [Docker Compose (Recommended)]
# docker-compose.yml
services:
  karta:
    image: ghcr.io/hua0512/karta:latest
    restart: unless-stopped
    volumes:
      - ./kovi.conf.toml:/app/kovi.conf.toml
      - ./data:/app/data
```

```bash [Docker Compose Run]
docker compose up -d
```

```bash [Prebuilt Binary]
# Download karta + karta-cli from GitHub Releases and place next to kovi.conf.toml
# Ensure ffmpeg is on PATH (optional: yt-dlp for YouTube downloads)
./karta
```

:::

For pinned image tags, mounting `yt-dlp` in Docker, custom logo overrides, or building from source, see [Deployment](deployment.md).

### 3. Send a test link

Send a supported link in any group or private chat the bot can see — for example:

```text
https://b23.tv/YTg9oSw
```

Karta reacts with an emoji while parsing (when `reaction_enabled` is `true`) and replies with the parsed post and media.

## Karta configuration {#karta-configuration}

On first start, Karta generates `data/karta/config.json` with working defaults (plain-text replies and media downloads enabled). To switch to native image cards, edit `data/karta/config.json` and restart:

```json
{
  "render_type": "image",
  "max_size": 90
}
```

::: warning Strict JSON
`data/karta/config.json` does not allow comments or trailing commas. If a syntax error occurs, Karta saves a copy to `config.json.bad`, leaves your file untouched, and boots with defaults so configured cookies are never lost.
:::

## Next steps

- **[Configuration](configuration.md)** — visually build a config, tune card styles, media limits, proxy, and forwarding rules.
- **[Platforms](platforms.md)** — see all 16 supported platforms, special triggers (`bm`, `ym`), and optional cookie setup.
- **[Admin Commands](commands.md)** — check `/status`, log in to Bilibili via `/login bilibili`, and toggle groups or platforms at runtime.
