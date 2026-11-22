# 开发

## 工作原理 {#how-it-works}

当消息包含可识别的 URL（或 QQ JSON 卡片 / 小程序分享）时，插件会依次执行：

1. 从消息文本或 JSON 卡片负载中提取 URL（`karta-kovi/src/input.rs`）
2. 通过基于 `RegexSet` 的注册表匹配各平台正则模式
3. 调用平台 API 或抓取页面，构建统一的 `ParseResult`
4. 将结果渲染为文本消息段或原生图片卡片
5. 按需下载并附带媒体（视频、图片、音频），并将图集聚合成与聊天平台无关的打包（`Bundle`）
6. 适配器通过 OneBot 协议按顺序投递输出——卡片、打包（在 QQ 上即合并转发）、独立视频/音频

同一时刻在途的相同链接只会解析一次（single-flight），解析结果带 LRU 缓存，且解析与渲染两个阶段分别受超时预算约束。

## 项目结构 {#project-structure}

```text
karta/
├── crates/
│   ├── karta-core/        # 与聊天框架无关的核心引擎
│   │   ├── engine.rs      # ParserEngine、解析缓存与超时协调
│   │   ├── parser.rs      # Parser trait、基于 RegexSet 的注册表
│   │   ├── data/          # ParseResult、Content 类型、Author
│   │   ├── render/        # 纯文本与原生图片卡片渲染器
│   │   ├── download/      # 流式下载器、FFmpeg 封装、yt-dlp
│   │   ├── cache.rs       # LRU 内存缓存与 SHA-256 文件缓存
│   │   ├── config.rs      # 配置模型
│   │   ├── http.rs        # 共享 HTTP 工具（重试、重定向、请求头）
│   │   └── html.rs        # HTML/BBCode 转文本转换器
│   ├── karta-platforms/   # 每个平台一个模块 + create_all_parsers()
│   ├── karta-kovi/        # 将 karta-core 桥接到 Kovi 机器人框架
│   │   ├── access_control.rs # 框架层访问控制
│   │   ├── commands.rs    # 管理命令处理器（/status、/ban 等）
│   │   ├── forward.rs     # OneBot 合并转发发送与回退
│   │   ├── gate.rs        # ParseGate、并发限制与 single-flight 去重
│   │   ├── input.rs       # QQ 文本 / JSON 卡片的 URL 提取
│   │   ├── output.rs      # karta-core MessageOutput → Kovi Message
│   │   ├── pipeline.rs    # 消息处理流水线、Bilibili 刷新与缓存清理
│   │   ├── reaction.rs    # 表情回应 API 兼容性探测
│   │   └── state.rs       # 运行时状态持久化（state.json）
│   ├── karta/             # 二进制入口：机器人主程序 + karta-cli
│   └── test-harness/      # 共享端到端测试工具（伪上游服务器、OneBot 驱动与 fixture）
├── resources/
│   ├── fonts/             # 图片渲染内置字体
│   └── logos/             # 卡片右上角平台图标
├── docs/                  # 文档站点源码
├── config.example.json    # 插件配置示例
└── Cargo.toml             # Workspace 根配置
```

`karta` 可选 Cargo feature：`ytdlp`（启用 `yt-dlp` 下载器）。

### 新增平台 {#adding-a-platform}

1. 在 `karta-core/src/constants.rs` 的 `Platform` 枚举中新增变体（同步更新 `ALL`、`slug`、`Display`）。
2. 创建 `karta-platforms/src/<name>.rs`（解析器结构体）与 `karta-platforms/src/<name>/`（`patterns.rs`、API/结果模块），可参照现有相似平台。
3. 在 `karta-platforms/src/lib.rs::create_all_parsers` 中注册。
4. 补齐配套设施：配置键（如需认证）、`karta-kovi/src/commands.rs::resolve_platform` 中的别名、`karta-core/src/http.rs` 中的下载请求头、`resources/logos/<slug>.png` 平台图标（如需随独立二进制内置，同时加入 `karta-core/src/render/image.rs::load_logos` 的 `embedded` 列表）。
5. 在 `crates/karta-platforms/tests/test_urls.md` 中添加正则匹配测试用例并在 `tests/test_patterns.rs` 注册标题映射，同步更新文档。

### 新增聊天平台（适配器） {#adding-a-chat-provider-adapter}

`karta-core` 与聊天框架解耦：渲染器输出 `RenderOutput`——要么是单条 `Message`，要么是 `Bundle`（有序分组，由适配器映射为目标平台的原生展示方式，例如 QQ 合并转发、Telegram 媒体组或 Discord 批量 Embed）。

新增聊天平台适配器可参照 `karta-kovi`，实现四项职责：

1. **输入**：从消息事件中提取待解析的文本或 URL。
2. **输出**：将 `RenderOutput` / `MessageSegment` 转换为平台消息并按序投递。
3. **命令与状态**：实现管理命令及持久化状态（引擎本身不涉及管控）。
4. **生命周期**：根据 `Config` 构建 `ParserEngine`，并在事件入口外围接入解析闸门（single-flight + 并发上限）。

## 测试 {#testing}

```bash
# 运行全部常规测试（CI 使用 cargo-nextest 运行同一测试集）
cargo test

# 正则匹配模式集成测试（校验 test_urls.md 清单）
cargo test -p karta-platforms --test test_patterns

# 需要真实平台 Cookie 的测试（默认忽略）
cargo test -p karta-platforms --test bilibili_need_ck -- --ignored
cargo test -p karta-platforms --test douyin_need_ck -- --ignored
cargo test -p karta-platforms --test instagram_need_ck -- --ignored

# 原生图片卡片渲染器端到端测试
cargo test -p karta-core --test render_image_e2e

# 调用真实 ffmpeg/ffprobe 的测试（默认忽略；CI 任务 test-tools）。
# 设置 KARTA_REQUIRE_TOOLS=1 后，缺少工具会直接报错而非跳过。
KARTA_REQUIRE_TOOLS=1 cargo test -p karta-core --test ffmpeg_e2e -- --ignored
```

### 卡片渲染与基准测试 (card_bench) {#card-bench}

`card_bench` 示例用于独立评估原生卡片渲染各阶段（解析、媒体准备、绘制与 PNG 编码）的耗时：

```bash
cargo run -p karta --release --example card_bench -- \
  /path/to/karta-data /path/to/results '<分享文本或链接>' 20 --fresh-media
```

- **`--fresh-media`**：为每次采样准备独立的本地媒体路径，避免缩略图缓存复用，同时保持进程与字体处于热态（文件复制与输出写入不计入耗时）。结果目录会生成 `native-d.png` 以及包含逐次样本与统计数据的 `timings.json`。省略该参数仅会重复测量缓存媒体，不能代表真实新链接开销。
- **本地 JSON Fixture 模式**：除在线链接外，也可传入本地 JSON 帖子 fixture（指定平台 slug、作者、本地头像与图片/视频封面路径），跳过网络解析与下载，保证多轮压测输入完全一致。
- **细分阶段诊断日志**：排查布局、背景模糊或阴影开销时，可附加：

```bash
RUST_LOG=karta_core::render::image=debug cargo run -p karta --release --example card_bench -- \
  /path/to/karta-data /path/to/results '<分享文本或链接>' 5 --fresh-media
```

开启后日志会输出微秒级细分指标（`layout_us`、`palette_us`、`frame_us`、`draw_us`、`header_us`、`shadow_us`、`background_us`）。正式基准采样时请保持关闭，以免日志输出干扰计时。

## 独立命令行工具 {#standalone-cli}

`karta-cli` 无需配置机器人账号即可完整运行解析与渲染流水线：

```bash
cargo run -p karta --bin karta-cli -- <text-or-url>
```

| 选项 | 作用 |
|---|---|
| `--render` | 同时渲染解析结果（遵循配置中的 `render_type`） |
| `--render-type <text\|image>` | 临时覆盖渲染类型 |
| `--no-download` | 跳过媒体下载（仅验证元数据解析，更快） |
| `--json` | 输出机器可读的 JSON |
| `--append-url` | 在输出结果中附带原始 URL |
| `--data-path <dir>` | 指定数据目录（默认 `$TMP/karta-cli`） |
| `--parse-timeout <secs>` | 覆盖 `parse_timeout`（`0` 表示不限时） |
| `--trace` | 启用 tracing 日志（配合 `RUST_LOG` 使用） |

## 发布 {#releasing}

推送 `v*.*.*` 标签即可触发 `.github/workflows/release.yml` 自动发版：CI 会构建五个二进制目标与多架构 Docker 镜像（推送至 GHCR，标签为 `<version>`、`<major>.<minor>` 与 `latest`），并创建 GitHub Release。

1. 更新根 `Cargo.toml` 中的 `version`，运行 `cargo update --workspace`。
2. 创建 `docs/release-vX.Y.Z.md` 编写本次发布说明（`.github/workflows/release.yml` 会读取该文件作为 GitHub Release 正文，并自动附带生成变更列表）。
3. 以 `chore(release): vX.Y.Z` 提交，打附注标签 `vX.Y.Z`，并将提交与标签一并推送。
