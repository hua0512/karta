# 管理命令

在 `kovi.conf.toml` 中通过 `main_admin` 与 `admins` 配置的管理员可以执行以下命令。变更会持久化到 `data/karta/state.json`，重启后依然生效（见[运行时状态](configuration.md#runtime-state)）。

## 任意聊天可用（群聊 / 私聊） {#global-commands}

以下命令在机器人可见的任意群聊或私聊中均可使用：

| 命令 | 说明 |
|---|---|
| `/status` | 查看机器人状态总览：渲染模式、各平台开关与凭据状态、工具及运行时统计 |
| `/login bilibili` | Bilibili 扫码登录（别名 `/login bili`），返回二维码图片并轮询最长 120 秒 |
| `/ban <uid>` | 按 QQ 号全局封禁指定用户，忽略其后续消息中的链接 |
| `/unban <uid>` | 解除对指定 QQ 号的全局封禁 |

::: info `/status` 显示范围说明
- **凭据状态**：`/status` 目前仅展示 7 个主要 Cookie 平台的凭据状态——`bilibili`、`youtube`、`xiaohongshu`、`twitter`、`weibo`、`xiaoheihe`、`douyin`（其中 `bilibili` 会区分扫码/刷新凭据 `credential: ok` 与手动 `cookie: configured`）。
- **`yt-dlp` 状态**：仅根据 `config.json` 中是否配置了 `ytdlp_path` 或 `ytdlp_cookies_file` 判断为 `configured` / `not configured`，不会额外检测系统 `PATH` 中的可执行文件。
:::

## 仅限群聊使用 {#group-commands}

以下命令作用于当前群聊，在私聊中发送会提示仅限群聊使用：

| 命令 | 说明 |
|---|---|
| `/enable <平台>` | 在当前群聊启用指定平台 |
| `/disable <平台>` | 在当前群聊禁用指定平台 |
| `/karta on` | 在当前群聊开启 Karta |
| `/karta off` | 在当前群聊关闭 Karta |

::: warning 全局禁用优先级更高
`/enable <平台>` **无法覆盖** `config.json` 中 `disabled_platforms` 的全局禁用设置。全局禁用的平台不会注册解析器，如需在特定群启用，请先将其从 `disabled_platforms` 中移除并重启机器人。
:::

## `/karta on|off` 语义 {#karta-onoff-semantics}

群开关的行为取决于 `config.json` 中的 `group_blacklist_enabled`：

- **黑名单模式（默认，`true`）**：所有群初始为开启状态，使用 `/karta off` 关闭特定群。
- **白名单模式（`false`）**：所有群初始为关闭状态，使用 `/karta on` 开启特定群。

两种模式共用 `state.json` 中同一份群组集合，因此切换 `group_blacklist_enabled` 会让之前已操作过的群组状态反转——修改模式后请重新执行 `/karta on|off`。私聊不受此开关影响，始终会进行解析。

## 平台名称与别名 {#platform-names-and-aliases}

`/enable` 与 `/disable` 接受全部 16 个平台的规范小写 slug（`bilibili`、`douyin`、`twitter`、`youtube`、`tiktok`、`weibo`、`xiaohongshu`、`kuaishou`、`acfun`、`nga`、`xiaoheihe`、`instagram`、`v2ex`、`hongguo`、`wechat`、`wechatchannel`），也支持以下不区分大小写的别名：

| 平台 | 别名 |
|---|---|
| Bilibili | `bili`、`b站` |
| Twitter/X | `x`、`推特` |
| YouTube | `yt` |
| 抖音 | `抖音` |
| 微博 | `微博` |
| 小红书 | `xhs`、`小红书` |
| 快手 | `ks`、`快手` |
| 小黑盒 | `xhh`、`heybox`、`小黑盒` |
| Instagram | `ig` |
| V2EX | `v站` |
| 红果短剧 | `hg`、`红果`、`红果短剧` |
| 微信公众号文章 | `wx`、`mp`、`微信`、`公众号`、`微信公众号` |
| 微信视频号 | `sph`、`wxchannel`、`视频号`、`微信视频号` |
