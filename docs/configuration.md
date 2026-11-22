# Configuration

Karta reads `config.json` from its plugin data directory (`<working dir>/data/karta/config.json`). A default file is generated on first start.

- If `config.json` has a syntax error, Karta copies it to `config.json.bad`, leaves your original file untouched, and starts with in-memory defaults so configured cookies are never overwritten.
- `config.json` is **never modified at runtime**. Changes made through [admin commands](commands.md) are saved to [`state.json`](#runtime-state) instead.

## Quick Config Builder

Pick common options below and copy the generated snippet into `data/karta/config.json`:

<KartaConfigPlayground />

## Common Settings

### Rendering & Cards

| Option | Default | Description |
|---|---|---|
| `render_type` | `"text"` | Output format: `"text"` (plain text + media) or `"image"` (native image card) |
| `image_header_style` | `"blur_background"` | Card backdrop style. `"blur_background"` renders a full-card frosted backdrop with no header art. `"mesh_glow"` (ambient multi-lobe glow), `"blur"` (header only), and `"soft_prism"` (layered glass arcs with specular rims) use procedural decorative top headers; `"auto"` picks a stable variant among those three. All styles support text-only posts. |
| `image_theme_mode` | `"auto"` | Card color polarity: `"auto"` (obsidian dark frosted glass with 1px rim for dark/moody media, warm light paper otherwise), `"light"`, or `"dark"`. |
| `emoji_enabled` | `true` | Render emoji as crisp images on cards |
| `emoji_style` | `"twemoji"` | Emoji artwork set: `"twemoji"`, `"apple"`, `"google"`, or `"facebook"` |
| `emoji_cdn_url` | `null` | Custom emoji CDN URL (overrides `emoji_style`) |
| `custom_font` | `null` | **Currently unused / reserved.** Card rendering loads `resources/fonts/HYSongYunLangHeiW-1.ttf` (from the data or working directory) when present, falling back directly to the embedded font. |

Native image cards automatically adapt their layout to the content: portrait video covers (aspect ratio `> 1.15`, e.g. 9:16 Douyin/TikTok) use a compact left-poster / right-info split layout, multi-image galleries (2–9 photos) use a hole-free Bento collage with a frosted count badge, tags render as tinted pills, and reposts render natively at full resolution.

### Media Limits & Proxy

| Option | Default | Description |
|---|---|---|
| `download_media` | `true` | Download and attach media files (video, images, audio) |
| `max_size` | `90` | Max download size in MiB; media over this limit sends a text notice instead |
| `duration_maximum` | `480` | Max video/audio duration in seconds (`0` disables the check on supported platforms) |
| `proxy` | `null` | Proxy URL, e.g. `http://127.0.0.1:7890` or `socks5://127.0.0.1:1080` (also forwarded to `yt-dlp`). A bare `host:port` defaults to HTTP. |
| `disabled_platforms` | `[]` | Lowercase platform slugs to disable globally, e.g. `["youtube", "wechatchannel"]` |

::: warning Invalid proxy URLs
In the `karta` bot, an invalid `proxy` value causes the Karta plugin to log an error and fail to register its listeners (while Kovi itself stays running). In `karta-cli`, an invalid proxy exits immediately with an error.
:::

### Forwarding, Delivery & Reactions

| Option | Default | Description |
|---|---|---|
| `forward_contents` | `true` | Bundle multi-image galleries (with captions) into a QQ merged-forward message |
| `forward_text_threshold` | `300` | Character count above which text replies auto-bundle into a merged-forward (`0` disables) |
| `forward_media_threshold` | `4` | Media item count above which replies auto-bundle into a merged-forward (`0` disables) |
| `append_url` | `false` | Append the original link to parsed replies |
| `use_base64` | `false` | Send images, audio, and video as base64 instead of local file paths (needed when OneBot runs on another machine or container without shared mounts) |
| `need_upload` | `false` | Upload standalone audio/video via the group/private file API instead of sending inline media |
| `reaction_enabled` | `true` | React to matched messages while parsing, then update to a success (QQ face 144) or failure (❌) reaction when finished |

Media skipped due to `max_size` or `duration_maximum` sends a text notice (outside merged-forward bundles) and marks the reaction as failed even when the card itself succeeds.

When `need_upload` is `false`, Karta only falls back to a file upload if the OneBot server rejects the inline media request before delivery. Runtime errors and timeouts produce a notice without automatically resending, to avoid duplicate messages. Set `"need_upload": true` if you always prefer file uploads.

### Access Control

| Option | Default | Description |
|---|---|---|
| `access_control` | disabled | Framework-level user/group filter: `{"enabled": false, "mode": "blacklist"\|"whitelist", "users": [], "groups": []}` |
| `group_blacklist_enabled` | `true` | Controls `/karta on\|off`: `true` = blacklist mode (every group parses unless turned off); `false` = whitelist mode (only groups turned on parse). Both modes share the same persisted group list, so flipping this inverts existing entries. |

## Platform Auth & Cookies {#platform-credentials}

Every credential option defaults to `null` (anonymous). See [Platforms](platforms.md) for what each credential unlocks and how to obtain it.

| Option | Platform | Notes |
|---|---|---|
| `bili_cookie` | Bilibili | Or use `/login bilibili` QR login in chat |
| `douyin_cookie` | Douyin | Enables the authenticated web API |
| `douyin_mobile` | Douyin | Optional Android detail fallback: `{"auto_register": true}` or explicit `device_id`/`install_id` ([details](platforms.md#douyin)) |
| `ytb_cookie` | YouTube | Used **only** for the oEmbed fallback when `yt-dlp` is unavailable; when `yt-dlp` is used (primary path), configure `ytdlp_cookies_file` instead |
| `xhs_cookie` | Xiaohongshu | Recommended to avoid captcha walls |
| `instagram_cookie` | Instagram | Enables the GraphQL API and stories |
| `xhh_cookie` | Xiaoheihe | Optional web cookie for BBS and game endpoints |
| `hongguo_api_auth` | Hongguo | Object with authorized ByteDance app session for motion-comic videos ([details](platforms.md#hongguo)) |
| `wechat_channel_cookie` | WeChat Channels | Required — Tencent Yuanbao web cookie ([setup](platforms.md#wechatchannel)) |
| `twitter_cookie`, `weibo_cookie` | Twitter/X, Weibo | **Currently unused / reserved.** Accepted and shown in `/status`, but both parsers run anonymously. |

### Bilibili cookie refresh {#bilibili-cookie-refresh}

| Option | Default | Description |
|---|---|---|
| `bili_cookie_auto_refresh` | `true` | Automatically refresh the Bilibili cookie when Bilibili requests rotation (checked at startup and every 12 hours at 00:00 / 12:00) |

::: tip Recommended: use `/login bilibili`
Running `/login bilibili` in chat creates a dedicated session with its own refresh token, stored in `bilibili_cookies.json`. Auto-refreshes keep it alive without logging out your browser.
:::

::: details Manual refresh with ac_time_value
When Bilibili rotates a cookie, the old cookie is retired immediately — so a cookie copied from your browser will log that browser out once refreshed (and vice versa). If you prefer pasting `bili_cookie` manually:

1. On `www.bilibili.com`, open DevTools → Application → Local Storage → `https://www.bilibili.com` (or run `localStorage.getItem("ac_time_value")` in the console).
2. Append `; ac_time_value=<value>` to `bili_cookie` in `config.json`. Without it, the cookie is used as-is until it expires.
3. After a refresh, Karta uses the newer saved cookie in `bilibili_cookies.json`; pasting a fresh working `bili_cookie` later still takes precedence.
4. Set `"bili_cookie_auto_refresh": false` to disable automatic rotation completely.
:::

### Bilibili quality {#bilibili-quality}

| Option | Default | Description |
|---|---|---|
| `bili_video_quality` | `"1080p"` | Preferred quality: `"240p"`, `"360p"`, `"480p"`, `"720p"`, `"720p60"`, `"1080p"`, `"1080p_plus"`, `"1080p60"`, `"4k"`, `"hdr"`, `"dolby"`, `"8k"` (above 1080p requires a VIP cookie; `"hdr"` and `"dolby"` are only chosen when requested explicitly) |
| `bili_video_codecs` | `["avc", "av1", "hevc"]` | Codec preference order within the chosen resolution |
| `bili_cdn_mode` | `"auto"` | CDN selection: `"auto"` avoids slow `mcdn` nodes, `"original"` keeps API order, `"mirror"` rewrites known-slow hosts to upstream mirrors |
| `bili_smart_resolution` | `false` | Step resolution down automatically until the estimated merged size fits `max_size` |
| `bili_live_record_seconds` | `0` | Seconds of a live stream to record and send (`0` sends room info and cover only) |

## Advanced: Concurrency, Caching & Tools

| Option | Default | Description |
|---|---|---|
| `cache_size` | `50` | Number of parsed results kept in the in-memory LRU cache |
| `parse_timeout` | `60` | Timeout in seconds, applied separately to parsing and again to rendering/downloads (`0` disables both) |
| `max_concurrent_parses` | `8` | Max messages parsed and rendered concurrently bot-wide; identical in-flight links share one parse (`0` = unlimited) |
| `download_concurrency` | `4` | Max concurrent media downloads per render |
| `ffmpeg_path` | `null` | Explicit path to `ffmpeg` (when not on `PATH`) |
| `ytdlp_path` | `null` | Explicit path to `yt-dlp` (when not on `PATH`; requires the `ytdlp` build feature) |
| `ytdlp_cookies_file` | `null` | Path to a Netscape-format cookies file passed to `yt-dlp` (used for YouTube/TikTok auth via `yt-dlp`) |

## Runtime State (`state.json`) {#runtime-state}

Changes made via [admin commands](commands.md) are persisted in `data/karta/state.json` and survive restarts:

| Field | Managed by | Description |
|---|---|---|
| `banned_users` | `/ban`, `/unban` | Globally banned QQ user IDs |
| `disabled_groups` | `/karta on\|off` | Group IDs toggled by `/karta` (disabled groups in blacklist mode; enabled groups in whitelist mode) |
| `group_disabled_platforms` | `/enable`, `/disable` | Per-group disabled platform slugs |
