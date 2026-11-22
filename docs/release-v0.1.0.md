# Karta v0.1.0

Initial public release of **Karta** (`v0.1.0`), a pure-Rust social media link parser, rich card renderer, and OneBot v11 bot plugin.

## Highlights

- **16 Supported Platforms**
  - Video & Streaming: Bilibili, Douyin (including share 口令 & long articles), YouTube, TikTok, Kuaishou, AcFun, WeChat Channels
  - Short Drama: Hongguo (including motion comics)
  - Social & Community: Twitter / X, Weibo, Xiaohongshu, Instagram, NGA, Xiaoheihe, V2EX, WeChat articles
- **Pure-Rust Native Card Renderer**
  - Adaptive dark frosted-glass cards with portrait split hero layout, Bento media gallery, embedded platform logos, native repost cards, and emoji support — no headless browser required.
- **Binaries & Modular Workspace**
  - `karta` — Kovi OneBot v11 bot runner & plugin
  - `karta-cli` — standalone CLI for parsing, card rendering, and local debugging

## Installation

### Docker (Multi-Arch)

Multi-arch container images (`linux/amd64`, `linux/arm64`) are published to GHCR:

```bash
docker pull ghcr.io/hua0512/karta:0.1.0
```

### Prebuilt Binaries

Release archives include `karta` and `karta-cli` for:

- `x86_64-unknown-linux-gnu`
- `x86_64-unknown-linux-musl`
- `aarch64-unknown-linux-gnu`
- `aarch64-unknown-linux-musl`
- `x86_64-pc-windows-msvc`

## License

Licensed under [MPL-2.0](https://www.mozilla.org/en-US/MPL/2.0/).
