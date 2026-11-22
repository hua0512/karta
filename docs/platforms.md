# Platforms

Karta matches links anywhere inside a message, including URLs embedded in QQ JSON share cards and mini-program shares. Every platform can be disabled globally via `disabled_platforms` in [`config.json`](configuration.md) or per group with [`/disable <platform>`](commands.md).

<KartaPlatformGrid />

::: tip Obtaining a browser cookie
For platforms that accept a web cookie, log into the platform's web site in your browser, open **DevTools → Network**, click any authenticated request, and copy the `Cookie` value from **Request Headers** into [`data/karta/config.json`](configuration.md#platform-credentials).
:::

## Bilibili {#bilibili}

- **Works out of the box:** Videos (`bilibili.com/video/BV…`, bare `BV…`/`av…`), short links (`b23.tv/…`, `bili2233.cn/…`), dynamics/opus (`t.bilibili.com/…`, `bilibili.com/opus/…`), articles (`bilibili.com/read/cv…`), live rooms, favlists, and bangumi episodes.
- **Built-in triggers:**
  - **Audio-only:** `bm BV1xx411c7mD` or `bm BV1xx411c7mD 2` (optional page number)
  - **Multi-part videos:** `BV1xx411c7mD 2`, `BV1xx411c7mD?p=2`, `av170001 3`, or `av170001?p=3`
- **What auth unlocks:** Higher video/audio quality tiers (above 1080p requires a VIP account) and member-only content.

Two ways to log in:

1. **QR login (recommended):** an admin runs `/login bilibili` in chat, scans the QR code with the Bilibili app (120 s timeout), and lets Karta persist and refresh the session automatically.
2. **Manual cookie:** set `bili_cookie` in `config.json` (see [Bilibili cookie refresh](configuration.md#bilibili-cookie-refresh) and [Bilibili quality](configuration.md#bilibili-quality)).

Video and audio DASH streams are downloaded separately and merged with `ffmpeg`.

## Douyin {#douyin}

- **Works out of the box:** Short links (`v.douyin.com/…`), web video/note/slides links, live rooms, daily shares, and long-article links or share tokens (口令, e.g. `【标题】长按复制打开抖音，即可阅读文章 ˇˇ…➝➝`).
- **What auth unlocks:** Setting `douyin_cookie` switches to the authenticated web API (more reliable for tricky items and logged-in article recovery). Enabling `douyin_mobile` adds an automatic Android detail fallback for works that fail web retrieval.

Minimal config for the optional mobile fallback:

```json
{
  "douyin_cookie": "...",
  "douyin_mobile": { "auto_register": true }
}
```

::: details Technical notes & fallback behavior
- **Anonymous web path:** Registers an anonymous `ttwid` and probes CDN ratios. With `douyin_cookie`, requests switch to the official web API with `a_bogus` signing.
- **Share tokens & long articles:** Tokens wrapped in doubled decorations (including `##…##`, `%%…%%`, `$$…$$`, currency/Greek wrappers, and `activity_…` commands) resolve via the share-carrier endpoint. Long articles render with header poster, flattened text, inline images in document order, reading time, and stats. Expired or unknown tokens are reported explicitly, and SSR deletion/privacy reasons are preserved if all detail routes fail.
- **Signed Android fallback (`douyin_mobile`):** When `"auto_register": true` is set, registration runs lazily after web/SSR retrieval fails and caches the server-issued identity (`openudid`, `cdid`, `clientudid`) in `data/karta/douyin-device.json` with a 60-second failure cooldown. You can also supply an explicit numeric `device_id` / `install_id` pair (and optional `openudid`, `cdid`, `clientudid`, `device_token`). The default profile uses `version_code` / `update_version_code` `400300` and `version_name` `40.3.0` (override all three together). Signing runs natively in Rust (`X-Gorgon`, `X-Argus`, `X-Ladon`, `X-Helios`, `X-Khronos`) without Python or web cookies; it cannot unlock private or expired works.
:::

## Twitter/X {#twitter}

- **Works out of the box:** `x.com/<user>/status/…` and `twitter.com/<user>/status/…` posts, images, and videos.
- **Auth:** None needed — parses anonymously. (`twitter_cookie` is accepted and shown in `/status`, but currently unused.)

## YouTube {#youtube}

- **Works out of the box:** `youtube.com/watch?v=…`, `youtu.be/…`, and `youtube.com/shorts/…`.
- **Built-in triggers:**
  - **Audio-only:** `ym <url>` (e.g. `ym https://youtu.be/dQw4w9WgXcQ` or `ym https://youtube.com/shorts/…`) — requires `yt-dlp`.
- **Primary path (`yt-dlp`):** When built with the `ytdlp` feature and `yt-dlp` is available on `PATH` or `ytdlp_path`, Karta uses `yt-dlp` for full metadata and video/audio downloads, forwarding `ytdlp_cookies_file` and `proxy`.
- **Fallback path (oEmbed):** When `yt-dlp` is unavailable, Karta falls back to YouTube's oEmbed endpoint (title, channel, thumbnail only). `ytb_cookie` is **only** sent on this oEmbed fallback — for authenticated `yt-dlp` downloads, configure `ytdlp_cookies_file` instead.

## TikTok {#tiktok}

- **Works out of the box:** `tiktok.com/@user/video/…` and short links (`vm.tiktok.com/…`, `vt.tiktok.com/…`).
- **Downloads:** Parses metadata directly and uses `yt-dlp` as a download fallback when available (`ytdlp_cookies_file` and `proxy` are forwarded).
- **Auth:** None required.

## Weibo {#weibo}

- **Works out of the box:** `weibo.com/<uid>/<mid>`, `m.weibo.cn/status/…`, `video.weibo.com/show?fid=…`, `weibo.com/tv/show/…`, and Weibo articles (`card.weibo.com/article/…`, `weibo.com/ttarticle/…`).
- **Auth:** None needed — parses anonymously. (`weibo_cookie` is accepted and shown in `/status`, but currently unused.)

## Xiaohongshu {#xiaohongshu}

- **Works out of the box:** `xhslink.com/…`, `xhslink.cn/…`, `xiaohongshu.com/explore/…`, and `xiaohongshu.com/discovery/item/…`.
- **Auth (recommended):** Set `xhs_cookie` in `config.json`. Anonymous requests frequently hit Xiaohongshu's captcha wall; Karta recovers when the target is embedded in the redirect URL, but a valid cookie avoids most captchas.

## Kuaishou {#kuaishou}

- **Works out of the box:** `v.kuaishou.com/…`, `kuaishou.com/short-video/…`, and `v.m.chenzhongtech.com/fw/photo/…`.
- **Auth:** None required.

## Hongguo {#hongguo}

- **Works out of the box:** Drama player/detail links (`hongguoduanju.com/player/…`, `/detail?series_id=…`) and motion-comic links (`hongguoduanju.com/comic/…`), which render anonymously as a page-image gallery (up to 50 pages).
- **What auth unlocks:** Supplying `hongguo_api_auth` resolves motion-comic *videos* instead of falling back to the image gallery.

Minimal config snippet for motion-comic video resolution:

```json
{
  "hongguo_api_auth": {
    "params": { "device_id": "...", "iid": "..." },
    "headers": { "Cookie": "..." }
  }
}
```

::: details Technical notes & fallback behavior
Motion-comic videos come from ByteDance's reading app `multi_video_model` API (`https://reading.snssdk.com/novel/player/multi_video_model/default/v1/`), which risk-control gates anonymous callers. You can optionally override `url` and `body` alongside `params` and `headers` (which merge over built-in defaults). Returned video streams are typically CENC-AES-CTR encrypted; Karta unwraps the content key from the response's `spade_a` blob and decrypts the download transparently in the render layer. If video resolution fails for any reason, the parser silently falls back to the anonymous image gallery.
:::

## AcFun {#acfun}

- **Works out of the box:** `acfun.cn/v/ac…` and mobile links (`m.acfun.cn/v/?ac=…`).
- **Auth:** None required.

## NGA {#nga}

- **Works out of the box:** Thread links on `nga.178.com`, `ngabbs.com`, and `bbs.nga.cn` (`/read.php?tid=…`).
- **Auth:** None required.

## Xiaoheihe {#xiaoheihe}

- **Works out of the box:** BBS posts (`xiaoheihe.cn/bbs/link/…`, share URLs with `link_id=…`) and PC/console/mobile game pages (`xiaoheihe.cn/game/…`).
- **Auth (optional):** Set `xhh_cookie` in `config.json` if you want requests sent with your web session cookie.

## Instagram {#instagram}

- **Works out of the box:** Public posts and reels (`instagram.com/p/…`, `/reel/…`, `/reels/…`) parse anonymously.
- **What auth unlocks:** Set `instagram_cookie` in `config.json` to enable the GraphQL API and Instagram Stories.

## V2EX {#v2ex}

- **Works out of the box:** Topic links (`v2ex.com/t/<id>`).
- **Auth:** None required.

## WeChat Articles {#wechat}

- **Works out of the box:** `mp.weixin.qq.com/s/…` and `mp.weixin.qq.com/s?__biz=…` links parse anonymously. Cards include title, account name, author, cover, publish time, and interleaved body text/images (capped at 50 images / 1000 chars).
- **Auth:** None required. Deleted articles, violation notices, and anti-crawler verification pages surface clear error messages instead of blank cards.

## WeChat Channels {#wechatchannel}

- **Recognized links:** `weixin.qq.com/sph/<id>` share links.
- **Auth (required):** Uses a **Tencent Yuanbao web cookie** — no WeChat login is needed:
  1. Log into <https://yuanbao.tencent.com> in your browser.
  2. Open **DevTools → Network**, pick any request, and copy the `Cookie` request header.
  3. Save it as `wechat_channel_cookie` in `data/karta/config.json`.

Share links carry short-lived preview credentials; an expired link or cookie reports a targeted error message. Image-post feeds (图文动态) are not supported.
