# 平台支持

消息中任意位置出现的链接（以及 QQ JSON 分享卡片、小程序卡片中携带的链接）都会被自动识别。每个平台均可通过 `config.json` 的 `disabled_platforms` 全局关闭，或在单个群聊中使用 [`/disable <平台>`](commands.md) 关闭。

<KartaPlatformGrid />

::: tip 如何获取网页 Cookie
绝大多数平台的 Cookie 获取方式一致：在浏览器登录对应平台网页版 → 打开 DevTools → Network → 任意请求 → Request Headers → 复制 `Cookie` 字段值，并填入 `data/karta/config.json` 对应键名（见[平台凭据参考](configuration.md#platform-credentials)）。
:::

## Bilibili {#bilibili}

匿名即可解析视频、动态（opus）、专栏、直播间、收藏夹与番剧；登录后可解锁 1080p 以上画质与大会员内容。视频与音频流会分别下载并由 `ffmpeg` 合并。

**常用触发方式与隐藏技巧：**

- **视频与短链**：支持 `bilibili.com/video/BV…`、`b23.tv/…`、`bili2233.cn/…` 以及裸 `BV…` / `av…` 号。
- **指定分 P**：发送 `BV… <页码>`（例如 `BV1584y167sD 40`）或链接中携带 `?p=<页码>`（同样支持 `av…`）。
- **纯音频提取**：发送 `bm BV… [分P]`（例如 `bm BV1xx411c7mD` 或 `bm BV1584y167sD 40`），仅提取并发送音频。
- **专栏与其他**：支持专栏 `bilibili.com/read/cv…`、动态（`t.bilibili.com/…`、`bilibili.com/opus/…`）、直播间 `live.bilibili.com/…`、收藏夹与番剧。

**认证方式（二选一）：**

- **扫码登录（推荐）**：管理员发送 `/login bilibili`（或 `/login bili`），扫描二维码后自动持久化并续期。
- **手动 Cookie**：配置 `bili_cookie`；自动续期说明见 [Bilibili Cookie 自动刷新](configuration.md#bilibili-cookie-refresh)。画质与编码策略见 [Bilibili 画质与直播](configuration.md#bilibili-quality)。

## 抖音 {#douyin}

**开箱即用**：匿名即可解析视频、图文笔记、长文章（含无链接的抖音分享口令）、日常分享与直播间，支持 `v.douyin.com/…`、`douyin.com/video/…`、`douyin.com/note/…` 等常见链接。

**进阶增强**：配置 `douyin_cookie` 可提高部分作品的解析稳定性，并在长文章接口失败时兜底读取网页内容；若遇到提示需要移动端鉴权的作品，开启 `douyin_mobile` 自动注册即可：

```json
{
  "douyin_cookie": "可选：浏览器复制的抖音 Cookie",
  "douyin_mobile": { "auto_register": true }
}
```

::: details 技术细节与回退机制
- **匿名与官方 API**：匿名路径会自动注册 `ttwid` 并探测 CDN 分辨率比例；配置 `douyin_cookie` 后切换到带 `a_bogus` 签名的官方 Web API，并在长文章详情 API 失败时尝试读取已登录 PC 页面。
- **口令与日常分享**：支持长文章 `/article/<id>`、`/share/article/<id>` 与无链接分享口令（包括 `ˇˇ…➝➝`、`##…##`、`%%…%%`、`$$…$$`、货币符号及希腊字母包裹格式，完整保留 token 中的 `_`、`+`、`/`、`=`、`-`，`activity_…` 也可直接识别）。口令经分享载体接口换取作品/文章/直播间 ID，过期或未知口令会明确提示。日常分享文案（如 `打开抖音，看看Ta的日常 …`）走同一作品解析流程；若 SSR 明确返回删除、私密或过滤原因，全部回退失败后会保留该提示。
- **移动端回退与设备缓存**：`douyin_mobile: { "auto_register": true }` 仅在 Web/SSR 解析失败后延迟触发 AppLog 注册，并将设备 ID 及 `openudid`、`cdid`、`clientudid` 缓存至 `data/karta/douyin-device.json` 复用（并发共享注册请求，失败后冷却 60 秒；删除缓存文件重启或修改版本配置会重新注册）。也可显式提供同一安装实例的正整数 `device_id`、`install_id`（优先于自动注册）及可选 `openudid`、`cdid`、`clientudid`、`device_token`；设为 `null` 则关闭回退。
- **版本与签名**：默认 `version_code` / `update_version_code` 为 `400300`、`version_name` 为 `40.3.0`，覆盖时需同时设置三项。移动端请求由 Rust 原生计算 `X-Gorgon`、`X-Argus`、`X-Ladon`、`X-Helios` 与 `X-Khronos`，不依赖 Python，也不转发 Web Cookie；签名无法绕过私密或已失效内容。
:::

## Twitter/X {#twitter}

支持 `x.com/<user>/status/…`、`twitter.com/<user>/status/…` 等推文链接，**无需配置 Cookie**，开箱即用。

::: info 关于 `twitter_cookie`
`config.json` 中的 `twitter_cookie` 仅作为预留配置项接受并在 `/status` 中显示，当前解析器为纯匿名实现，不会实际发送该 Cookie。
:::

## YouTube {#youtube}

支持 `youtu.be/…`、`youtube.com/watch?v=…` 以及短视频 `youtube.com/shorts/…`；同时支持**纯音频触发词**：

- 发送 `ym <YouTube 链接>`（支持 `watch?v=`、`shorts/` 与 `youtu.be/`），通过 `yt-dlp` 仅提取并发送音频。

**下载与凭据说明：**

- **主路径（推荐）**：当系统存在 `yt-dlp`（位于 `PATH` 或通过 `ytdlp_path` 指定，且使用启用了 `ytdlp` 特性的构建）时，视频下载与 `ym` 音频提取均通过 `yt-dlp` 完成，并自动转发 `proxy` 与 `ytdlp_cookies_file`（Netscape 格式 Cookie 文件）。
- **无 `yt-dlp` 回退**：若未安装 `yt-dlp`，解析器会回退到 YouTube oEmbed 接口仅获取标题与封面元数据；`config.json` 中的 `ytb_cookie` **仅用于这条 oEmbed 回退请求**。使用 `yt-dlp` 主路径需要登录态时，请配置 `ytdlp_cookies_file`。

## TikTok {#tiktok}

支持 `tiktok.com/@user/video/…` 以及 `vm.tiktok.com/…`、`vt.tiktok.com/…` 短链，无需 Cookie。元数据由内置解析器获取；在 `yt-dlp` 可用时，视频下载可调用 `yt-dlp` 作为回退并自动继承 `proxy` 与 `ytdlp_cookies_file`。

## 微博 {#weibo}

支持 `weibo.com/<uid>/<mid>`、`m.weibo.cn/status/…`、`m.weibo.cn/detail/…`、`video.weibo.com/show?fid=…`、`weibo.com/tv/show/…` 以及头条文章链接，**匿名即可解析**。

::: info 关于 `weibo_cookie`
与 `twitter_cookie` 相同，`weibo_cookie` 目前为预留字段，当前解析器不会发送它。
:::

## 小红书 {#xiaohongshu}

支持 `xhslink.com/…`、`xhslink.cn/…`、`xiaohongshu.com/explore/…` 与 `xiaohongshu.com/discovery/item/…`。

**强烈推荐配置 `xhs_cookie`**：匿名访问极易触发验证码拦截（虽然在目标链接仍嵌在重定向 URL 中时解析器会尝试恢复，但提供有效 Cookie 可以避免绝大多数验证码问题）。

## 快手 {#kuaishou}

支持 `v.kuaishou.com/…`、`kuaishou.com/short-video/…` 以及 `v.m.chenzhongtech.com/fw/photo/…`，无需登录，开箱即用。

## 红果短剧 {#hongguo}

**开箱即用**：支持 `hongguoduanju.com/player/…`、`/detail?series_id=…` 以及漫剧 `/comic/<id>`。其中漫剧链接默认可匿名解析为可阅读章节的图文图集（上限 50 页），无需任何配置。

**解锁漫剧视频**：若希望将 `/comic/<id>` 直接解析为视频流，可在 `config.json` 中补充抓取到的已授权会话 `hongguo_api_auth`：

```json
{
  "hongguo_api_auth": {
    "url": "https://reading.snssdk.com/novel/player/multi_video_model/default/v1/",
    "params": { "device_id": "...", "iid": "..." },
    "headers": { "Cookie": "..." },
    "body": {}
  }
}
```

::: details 技术细节与回退机制
- 漫剧底层视频位于字节跳动阅读类 App 的 `multi_video_model` 接口之后，风控会拦截匿名请求。
- `hongguo_api_auth` 中 `url` 与 `body` 为可选覆盖项，`params` 与 `headers` 会合并到内置默认请求参数之上。
- 返回的视频流通常使用 `CENC-AES-CTR` 加密，解析器会从响应的 `spade_a` 数据块中解包内容密钥，并在下载阶段透明解密。
- 若视频解析因任何原因失败，解析器会自动静默回退到匿名图文图集，不会中断体验。
:::

## AcFun {#acfun}

支持 `acfun.cn/v/ac…`（含多分 P 如 `ac4626695_2`）及移动端 `m.acfun.cn/v/?ac=…` 链接，匿名即可解析。

## NGA {#nga}

支持 `nga.178.com/read.php?tid=…`、`ngabbs.com/read.php?tid=…` 与 `bbs.nga.cn/read.php?tid=…`，匿名即可解析。

## 小黑盒 {#xiaoheihe}

支持社区分享链接（`xiaoheihe.cn/bbs/link/…`、`/bbs/app/api/web/share?link_id=…`）以及 PC / 主机 / 手机游戏详情页（`xiaoheihe.cn/game/…`）。匿名可用，也可按需在 `config.json` 中配置 `xhh_cookie`。

## Instagram {#instagram}

支持帖子（`instagram.com/p/…`）、Reels（`/reel/…`、`/reels/…`）与快拍（Stories）。

- **匿名可用**：公开帖子与 Reels 可通过 REST / embed 端点匿名解析。
- **配置 `instagram_cookie`**：启用 GraphQL API 并解锁 Stories 解析。

## V2EX {#v2ex}

支持 `v2ex.com/t/<id>` 主题帖链接，匿名即可解析。

## 微信公众号文章 {#wechat}

支持 `mp.weixin.qq.com/s/…` 短链及 `mp.weixin.qq.com/s?__biz=…` 参数链接，**匿名开箱即用**。

渲染结果包含标题、公众号名称、署名作者、封面、发布时间，以及按原文顺序穿插的图文正文（上限 50 张图片 / 1000 字符）。遇到已删除、违规或反爬验证页面时，会直接返回清晰的错误说明而非空白卡片。

## 微信视频号 {#wechatchannel}

支持 `weixin.qq.com/sph/<id>` 分享链接，**必须配置 `wechat_channel_cookie`**。

只需使用**腾讯元宝网页版 Cookie**，无需登录微信：

1. 在浏览器中登录 <https://yuanbao.tencent.com>。
2. 打开 DevTools → Network → 任意请求 → Request Headers → 复制 `Cookie`。
3. 填入 `data/karta/config.json` 的 `wechat_channel_cookie`。

::: warning 分享链接时效与限制
视频号分享链接内含一次性预览凭据，存在有效期；过期链接或失效 Cookie 会给出针对性的错误提示。目前暂不支持图文动态。
:::
