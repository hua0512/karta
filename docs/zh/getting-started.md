# 快速开始

Karta 通过 **OneBot V11 正向 WebSocket** 与 QQ 通信：你运行一个 OneBot V11 实现——例如 [NapCat](https://github.com/NapNeko/NapCatQQ)、[Lagrange](https://github.com/LagrangeDev/Lagrange.Core) 或 [LLOneBot](https://github.com/LLOneBot/LLOneBot)——Karta 作为客户端主动连接到它。

运行目录中只有两个核心配置文件：

| 文件 | 用途 |
|---|---|
| `kovi.conf.toml` | Kovi 框架配置：OneBot 连接信息与管理员 QQ 号 |
| `data/karta/config.json` | Karta 配置：渲染样式、媒体上限、代理与各平台 Cookie（[完整参考](configuration.md)） |

按下面三步即可完成首次启动。

## 1. 连接 OneBot V11 {#kovi-conf-toml}

先在 OneBot 端（如 NapCat / Lagrange / LLOneBot）开启**正向 WebSocket** 服务（默认端口如 `8081`），然后在 Karta 工作目录创建 `kovi.conf.toml`：

```toml
[server]                 # OneBot V11 连接（由 kovi-onebot 读取）
host = "127.0.0.1"       # OneBot 服务器地址（IP 或域名）
port = 8081              # OneBot 正向 WebSocket 端口
access_token = ""        # 访问令牌；未设置则留空
secure = false           # false → ws://，true → wss://
path = "/"               # WebSocket 路由路径
all_in_one = false       # 服务端仅暴露单个合并端点时设为 true

[config]                 # Kovi 框架设置（由 kovi 读取）
main_admin = 123456789   # 主管理员 QQ 号（可执行管理命令）
admins = []              # 其他管理员 QQ 号列表
debug = false            # 设为 true 且未设置 RUST_LOG 时自动开启 debug 日志
```

这里配置的 `main_admin` 与 `admins` 即为有权执行[管理命令](commands.md)的账号。

::: details 也可以交互式生成配置
如果不提前创建 `kovi.conf.toml`，首次直接启动机器人时会进入交互问答，依次输入主机地址、端口、Token 与管理员 QQ 号后自动写入该文件。
:::

## 2. 启动 Karta {#start-karta}

选择最适合你的方式直接启动：

::: code-group

```yaml [Docker Compose（推荐）]
# 保存为 docker-compose.yml，然后在同目录执行：docker compose up -d
services:
  karta:
    image: ghcr.io/hua0512/karta:latest
    restart: unless-stopped
    volumes:
      - ./kovi.conf.toml:/app/kovi.conf.toml
      - ./data:/app/data
```

```bash [预编译二进制]
# 从 GitHub Releases 下载对应平台的压缩包并解压后，
# 在存放 kovi.conf.toml 的目录中直接运行：
./karta
```

:::

::: info 需要更多部署选项？
Docker 镜像已内置 `ffmpeg`；预编译二进制已内置卡片字体与内置平台图标，仅需确保系统已安装 `ffmpeg`（以及可选的 `yt-dlp`）。自定义图标覆盖、版本固定或从源码编译请参考[部署文档](deployment.md)。
:::

## 3. 发送测试链接 {#verify-it-works}

在机器人所在的群聊或私聊中发送一条测试链接，例如：

```text
https://b23.tv/YTg9oSw
```

正常情况下，机器人会先回复一个表情回应表示开始解析（默认开启 `reaction_enabled`），随后返回解析结果，并在完成后给出成功回应。

## Karta 配置 {#karta-configuration}

首次启动时，Karta 会在 `data/karta/config.json` 生成默认配置（默认纯文本回复、自动下载媒体）。

最常见的第一步调整是开启**原生图片卡片**，并按需限制媒体体积或配置 Bilibili 凭据（该文件为严格 JSON，不支持注释）：

```json
{
  "render_type": "image",
  "max_size": 90
}
```

保存后重启 Karta 即可生效。需要登录 Bilibili 时，推荐直接在聊天中发送 `/login bilibili` 扫码登录，无需手动复制 Cookie。

- 所有配置项与可视化生成器：见[配置参考](configuration.md)
- 各平台支持范围与 Cookie 获取：见[平台支持](platforms.md)

## 解析流程一览 {#pipeline}

<KartaPipelineFlow />

::: tip 只想在命令行使用？
不需要部署 QQ 机器人也能直接解析并渲染卡片——使用随包附带的 `karta-cli` 即可：

```bash
karta-cli --render --render-type image "https://b23.tv/YTg9oSw"
```

更多参数与本地调试用法见[独立命令行工具](development.md#standalone-cli)。
:::
