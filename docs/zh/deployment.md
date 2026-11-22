# 部署

Karta 提供三种部署方式：**Docker 镜像（推荐）**、**预编译二进制**与**从源码构建**。

::: code-group

```bash [Docker Compose]
docker compose pull
docker compose up -d
```

```bash [预编译二进制]
# 解压 Release 压缩包后，在 kovi.conf.toml 所在目录运行
./karta
```

```bash [从源码构建]
cargo build --release -p karta --features ytdlp
./target/release/karta
```

:::

## Docker（推荐） {#docker-recommended}

每次发版都会向 GHCR 推送多架构镜像（`linux/amd64`、`linux/arm64`）：

```text
ghcr.io/hua0512/karta:latest    # 最新发布版本
ghcr.io/hua0512/karta:0.1.0     # 固定到具体版本
ghcr.io/hua0512/karta:0.1       # 某个次版本的最新补丁版
```

镜像内置 `ffmpeg` 与原生卡片渲染所需的全部资源（包括字体与 `resources/logos/` 平台图标）。`yt-dlp` 需按需自行挂载（见[下文说明](#yt-dlp-in-docker)）。

必须挂载两个路径：

| 容器路径 | 用途 |
|---|---|
| `/app/kovi.conf.toml` | OneBot 连接配置与管理员账号（格式见[快速开始](getting-started.md#kovi-conf-toml)） |
| `/app/data` | Karta 数据目录：`data/karta/{config.json,state.json,cache,downloads}` |

### docker compose {#docker-compose}

```yaml
services:
  karta:
    image: ghcr.io/hua0512/karta:latest
    restart: unless-stopped
    volumes:
      - ./kovi.conf.toml:/app/kovi.conf.toml
      - ./data:/app/data
```

::: tip 跳过交互式初始化
提前在宿主机写好 `./kovi.conf.toml` 即可直接 `docker compose up -d`。如果想通过交互问答生成该文件，先执行一次 `docker compose run --rm karta`，完成后再 `docker compose up -d`。
:::

首次启动时会自动创建 `./data/karta/config.json`；修改配置后执行 `docker compose restart` 即可生效（[完整配置参考](configuration.md)）。

### Docker 中的 yt-dlp {#yt-dlp-in-docker}

镜像在编译时已启用 `ytdlp` 特性，但未内置 `yt-dlp` 可执行文件。当检测到 `yt-dlp` 时，YouTube 下载（以及 TikTok 回退）会自动调用它。挂载一个静态构建的 `yt-dlp` 即可启用：

```yaml
    volumes:
      - ./yt-dlp:/usr/local/bin/yt-dlp:ro
```

如果你挂载到了其他路径，请在 `config.json` 中设置 `"ytdlp_path"` 指向该路径。

## 预编译二进制 {#prebuilt-binaries}

每个 [GitHub Release](https://github.com/hua0512/karta/releases) 都提供以下五个目标平台的 `karta` 与 `karta-cli` 压缩包（默认已开启 `ytdlp` 特性）：

- `x86_64-unknown-linux-gnu` / `x86_64-unknown-linux-musl`
- `aarch64-unknown-linux-gnu` / `aarch64-unknown-linux-musl`
- `x86_64-pc-windows-msvc`

需要自行准备以下运行时工具（加入 `PATH` 或在 `config.json` 中指定路径）：

| 依赖 | 用途 |
|---|---|
| `ffmpeg` | 合并音视频流（Bilibili、YouTube 等） |
| `yt-dlp` | YouTube 下载与音频提取、TikTok 回退（可选） |

::: info 原生图片卡片与平台图标资源
默认字体以及 10 个内置平台图标均已直接嵌入二进制文件（所有顶部装饰样式均由代码实时绘制），开启 `"render_type": "image"` 即可开箱即用，无需额外复制资源目录。如需覆盖内置图标，或为默认以文字标识展示的平台补充图标，可在 `data/karta/resources/logos/` 或工作目录的 `resources/logos/` 下放置 `<slug>.png`。
:::

## 从源码构建 {#building-from-source}

本项目使用 [wreq](https://github.com/0x676e67/wreq)（带 BoringSSL TLS 指纹伪装的 `reqwest` 分支），编译 BoringSSL 需要先安装相应的构建工具链：

| 操作系统 | 依赖安装说明 |
|---|---|
| Debian / Ubuntu | `sudo apt install -y cmake perl clang libclang-dev build-essential pkg-config git` |
| Arch Linux | `sudo pacman -S cmake perl clang` |
| Alpine | `apk add cmake make perl clang-dev musl-dev g++` |
| macOS | `brew install cmake llvm`（其余工具由 Xcode Command Line Tools 提供） |
| Windows (`msvc`) | 除 Visual Studio C++ 构建工具、CMake、Perl 与 LLVM 外，还需安装 **NASM**（BoringSSL / `btls-sys` 在 `windows-msvc` 下使用 NASM 进行汇编） |

::: code-group

```bash [启用 yt-dlp 支持（推荐）]
# 启用 yt-dlp 下载器支持（含纯文本与原生图片卡片渲染）
cargo build --release -p karta --features ytdlp
```

```bash [基础构建]
# 基础构建（包含纯文本与原生图片卡片渲染，不含 yt-dlp）
cargo build --release -p karta
```

:::

请在存放（或计划存放）`kovi.conf.toml` 的目录中运行生成的二进制文件；Karta 会自动在该目录下创建 `./data/karta/`。
