# 配置

Karta 从工作目录下的 `data/karta/config.json` 读取配置。首次启动时会自动生成默认文件；如果配置文件存在语法错误，Karta 会将其备份为 `config.json.bad` 并回退到默认配置，避免因一处笔误丢失已有凭据。

`config.json` **绝不会在运行时被程序改写**——管理员通过聊天命令修改的开关与封禁列表会独立保存在 [`state.json`](#runtime-state) 中。

## 常用配置生成器 {#config-generator}

<KartaConfigPlayground />

## 常用配置 {#general}

### 渲染与外观 {#rendering}

| 选项 | 默认值 | 说明 |
|---|---|---|
| `render_type` | `"text"` | 回复样式：`"text"`（纯文本）或 `"image"`（原生图片卡片） |
| `image_header_style` | `"blur_background"` | 原生卡片视觉样式：默认全卡磨砂背景；`"mesh_glow"`（弥散氛围光）、`"blur"`（仅顶部模糊）与 `"soft_prism"`（玻璃棱镜光带）提供纯代码生成的顶部装饰；`"auto"` 按内容稳定选用三者之一。所有样式均支持纯文字帖子 |
| `image_theme_mode` | `"auto"` | 原生卡片明暗极性：`"auto"`（暗调封面自动启用带 1px 玻璃高光边的黑曜石深色磨砂，亮调与纯文字保持浅色暖纸）、`"light"`（始终浅色）或 `"dark"`（始终深色） |
| `emoji_enabled` | `true` | 在图片卡片中将 Emoji 渲染为彩色图标 |
| `emoji_style` | `"twemoji"` | Emoji 图标集：`"twemoji"`、`"apple"`、`"google"`、`"facebook"` |
| `emoji_cdn_url` | `null` | 自定义 Emoji CDN 地址（设置后覆盖 `emoji_style`） |
| `custom_font` | `null` | **预留字段，当前暂未生效**。目前字体加载会优先检查数据目录或运行目录下的 `resources/fonts/HYSongYunLangHeiW-1.ttf`，不存在时直接使用二进制内置字体 |

原生图片卡片会自动按内容形态切换排版：竖屏视频封面（高宽比 `> 1.15`，如 9:16 抖音/TikTok）采用紧凑的左海报右文案分栏，2–9 张多图自动拼成零空洞 Bento 拼图并附右下角毛玻璃张数角标，标签以淡底胶囊呈现，嵌套转发按实际宽度原生清晰渲染。

### 媒体、网络与平台开关 {#media-and-network}

| 选项 | 默认值 | 说明 |
|---|---|---|
| `download_media` | `true` | 是否下载并附带媒体文件（视频、图片、音频） |
| `max_size` | `90` | 单个媒体下载体积上限（MiB）；超出时不发送视频，改为发送文字提示 |
| `duration_maximum` | `480` | 视频时长上限（秒） |
| `proxy` | `null` | 代理地址，如 `http://127.0.0.1:7890` 或 `socks5://127.0.0.1:1080`（同时透传给 `yt-dlp`）。仅填 `host:port` 时按 HTTP 代理处理 |
| `disabled_platforms` | `[]` | 全局禁用的平台列表，填写小写 slug（如 `"youtube"`、`"wechatchannel"`） |

::: warning 无效 `proxy` 的行为差异
如果 `proxy` 填写了无法解析的非法地址：
- 在 **`karta` 机器人**中，Karta 插件初始化会失败并输出错误日志（不会注册消息监听，但 Kovi 主进程不会崩溃退出）。
- 在 **`karta-cli`** 中，程序会直接报错退出。
:::

### 转发、发送方式与表情回应 {#output-behavior}

| 选项 | 默认值 | 说明 |
|---|---|---|
| `forward_contents` | `true` | 将图集（连同文字说明）打包发送——在 QQ 上表现为合并转发消息 |
| `forward_text_threshold` | `300` | 文本长度超过该字符数时自动转为合并转发（`0` 表示禁用） |
| `forward_media_threshold` | `4` | 媒体条数超过该值时自动转为合并转发（`0` 表示禁用） |
| `append_url` | `false` | 在解析结果末尾附带原始链接 |
| `use_base64` | `false` | 使用 base64 发送图片/音频/视频，而非本地文件路径（适用于 OneBot 与 Karta 不在同一台机器的情况） |
| `need_upload` | `false` | 将独立音频/视频直接以群文件/私聊文件形式上传，而非内嵌媒体消息 |
| `reaction_enabled` | `true` | 匹配到链接时添加表情回应：开始解析时先回应确认，完成后回应成功（QQ 表情 144）或失败（❌） |

媒体超出 `max_size` 或 `duration_maximum` 时，即便卡片已发送，机器人仍会单独发送一条提示并标记失败回应；这类提示不会混入合并转发消息中。

当 `need_upload: false` 时，只有在内嵌音频/视频消息尚未送达、在发送前就被服务端明确拒绝的情况下，Karta 才会自动改用文件上传重试；若遇到超时或运行时错误（此时 QQ 端可能已经送达），则只提示失败而不自动重发，避免重复刷屏。希望始终以文件形式发送时，请直接设为 `need_upload: true`。

## 平台凭据与 Cookie {#platform-credentials}

所有凭据默认均为 `null`（匿名访问）。各平台的详细能力与 Cookie 获取方式请见[平台支持](platforms.md)。

| 选项 | 适用平台与说明 |
|---|---|
| `bili_cookie` | Bilibili（更推荐在聊天中直接使用 `/login bilibili` 扫码登录） |
| `douyin_cookie` | 抖音——启用官方 Web API，提升解析稳定性与长文章恢复能力 |
| `douyin_mobile` | 抖音移动端回退：`{ "auto_register": true }` 可自动注册并缓存设备身份，也支持显式指定 `device_id` / `install_id` |
| `ytb_cookie` | YouTube——**仅在未启用/未检测到 `yt-dlp` 时**用于 oEmbed 回退请求；走 `yt-dlp` 主路径时请改配 `ytdlp_cookies_file` |
| `xhs_cookie` | 小红书——推荐配置，减少匿名触发验证码的概率 |
| `instagram_cookie` | Instagram——解锁 GraphQL API 与 Stories（快拍） |
| `xhh_cookie` | 小黑盒 |
| `hongguo_api_auth` | 红果漫剧视频授权对象（详见[红果短剧](platforms.md#hongguo)） |
| `wechat_channel_cookie` | 微信视频号——填写腾讯元宝网页版 Cookie |
| `twitter_cookie`、`weibo_cookie` | **预留字段，当前暂未生效**（会显示在 `/status` 中，但目前 Twitter/X 与微博均为匿名解析，不会实际发送） |

### Bilibili Cookie 自动刷新 {#bilibili-cookie-refresh}

| 选项 | 默认值 | 说明 |
|---|---|---|
| `bili_cookie_auto_refresh` | `true` | 按 B 站要求自动续期 Cookie：启动时检查一次，之后每 12 小时（00:00 / 12:00）检查一次，结果自动保存至 `bilibili_cookies.json` |

::: tip 优先使用 `/login bilibili` 扫码登录
扫码登录会创建独立会话并自动保存刷新口令，续期时不会顶掉你浏览器上的 B 站登录，也无需手动维护 `config.json`。
:::

::: details 手动刷新与 ac_time_value 说明
如果你是手动从浏览器复制 `bili_cookie`：

1. **为什么会互顶掉线**：B 站刷新 Cookie 时会使旧会话失效；浏览器自身也会刷新同一会话，因此复制出来的 Cookie 只应在一处使用。如果希望永远不自动刷新，可将 `bili_cookie_auto_refresh` 设为 `false`。
2. **如何启用手动 Cookie 自动刷新**：需在 `bili_cookie` 末尾追加 `; ac_time_value=<值>`。该值存放在浏览器 Local Storage 中——打开 `www.bilibili.com` → DevTools → Application → Local Storage → `https://www.bilibili.com`，复制 `ac_time_value`（或在控制台运行 `localStorage.getItem("ac_time_value")`）。未附带时，Cookie 会一直用到自然过期。
3. **优先级**：自动刷新后，`config.json` 中原来的旧 `bili_cookie` 会失效，重启时 Karta 会自动优先使用已保存的新凭据；如果你在 `config.json` 中重新粘贴了一份新的有效 `bili_cookie`，则会以新粘贴的为准。
:::

### Bilibili 画质与直播 {#bilibili-quality}

| 选项 | 默认值 | 说明 |
|---|---|---|
| `bili_video_quality` | `"1080p"` | 首选画质：`"240p"`、`"360p"`、`"480p"`、`"720p"`、`"720p60"`、`"1080p"`、`"1080p_plus"`、`"1080p60"`、`"4k"`、`"hdr"`、`"dolby"`、`"8k"`（1080p 以上通常需大会员 Cookie；`"hdr"` 与 `"dolby"` 仅在明确指定时启用） |
| `bili_video_codecs` | `["avc", "av1", "hevc"]` | 编码格式优先顺序，在已选定分辨率**内部**生效 |
| `bili_cdn_mode` | `"auto"` | CDN 策略：`"auto"` 优先高速节点并避开慢速 `mcdn`；`"original"` 保留 API 原地址；`"mirror"` 将已知慢速域名替换为上游镜像（备用地址始终保留用于重试） |
| `bili_smart_resolution` | `false` | 预估合并体积超过 `max_size` 时自动逐级降画质，而非直接放弃下载 |
| `bili_live_record_seconds` | `0` | 录制并发送直播片段的秒数；`0` 表示仅发送直播间信息与封面 |

### 外部工具 {#external-tools}

| 选项 | 默认值 | 说明 |
|---|---|---|
| `ffmpeg_path` | `null` | `ffmpeg` 可执行文件路径（不在系统 `PATH` 中时填写） |
| `ytdlp_path` | `null` | `yt-dlp` 可执行文件路径（不在系统 `PATH` 中时填写；需要启用 `ytdlp` 编译特性） |
| `ytdlp_cookies_file` | `null` | 传给 `yt-dlp` 的 Netscape 格式 Cookie 文件路径（用于 YouTube / TikTok 下载鉴权） |

## 进阶调优（并发与缓存） {#advanced}

| 选项 | 默认值 | 说明 |
|---|---|---|
| `download_concurrency` | `4` | 渲染期间单次任务的最大并发媒体下载数 |
| `cache_size` | `50` | 内存 LRU 缓存保留的解析结果条数 |
| `parse_timeout` | `60` | 超时时间（秒），分别独立作用于解析阶段与渲染阶段，各享一份完整预算（`0` 表示不限时） |
| `max_concurrent_parses` | `8` | 机器人全局同时处理的解析+渲染消息上限；进行中的相同链接会自动复用同一次解析（`0` 表示不限制） |

### 访问控制 {#access-control}

| 选项 | 默认值 | 说明 |
|---|---|---|
| `access_control` | 禁用 | 框架级黑白名单：`{"enabled": false, "mode": "blacklist"\|"whitelist", "users": [], "groups": []}` |
| `group_blacklist_enabled` | `true` | 控制 `/karta on\|off` 的工作模式：`true` 为黑名单模式（默认全群开启，按群关闭）；`false` 为白名单模式（默认全群关闭，按群开启）。两种模式共用同一份群组集合，切换后会反转已有记录的含义 |

## 运行时状态（`state.json`） {#runtime-state}

通过[管理命令](commands.md)修改的状态会持久化到 `data/karta/state.json`，重启后自动恢复：

| 字段 | 对应命令 | 说明 |
|---|---|---|
| `banned_users` | `/ban`、`/unban` | 全局封禁的 QQ 号列表 |
| `disabled_groups` | `/karta on\|off` | 运行时切换的群组集合（黑名单模式下表示已禁用群，白名单模式下表示已启用群） |
| `group_disabled_platforms` | `/enable`、`/disable` | 按群生效的平台禁用覆盖 |
